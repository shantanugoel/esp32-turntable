#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use defmt::info;
use embassy_executor::Spawner;
use embassy_net::StackResources;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::timer::timg::TimerGroup;
use static_cell::StaticCell;
use turntable::http::{MOTOR_COMMAND_CHANNEL, MotorCommand, http_server_task};
use turntable::stepper::{Direction, Stepper};
use turntable::wifi::{net_task, wait_for_ip, wifi_connection_task};
use {esp_backtrace as _, esp_println as _};

extern crate alloc;

esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 66320);
    esp_alloc::heap_allocator!(size: 72 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    info!("Turntable WiFi controller starting...");

    let in1 = Output::new(peripherals.GPIO1, Level::Low, OutputConfig::default());
    let in2 = Output::new(peripherals.GPIO2, Level::Low, OutputConfig::default());
    let in3 = Output::new(peripherals.GPIO3, Level::Low, OutputConfig::default());
    let in4 = Output::new(peripherals.GPIO4, Level::Low, OutputConfig::default());

    let mut motor = Stepper::new(in1, in2, in3, in4);
    motor.set_gear_reduction(3);
    motor.set_output_rpm(1.0);

    static RADIO_INIT: StaticCell<esp_radio::Controller<'static>> = StaticCell::new();
    let radio_init = RADIO_INIT.init(esp_radio::init().expect("Failed to initialize radio"));
    let (wifi_controller, interfaces) =
        esp_radio::wifi::new(radio_init, peripherals.WIFI, Default::default())
            .expect("Failed to initialize WiFi");

    static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
    let resources = RESOURCES.init(StackResources::new());

    let dhcp_config = embassy_net::Config::dhcpv4(Default::default());
    let (stack, runner) = embassy_net::new(interfaces.sta, dhcp_config, resources, 1234);

    spawner
        .spawn(wifi_connection_task(wifi_controller))
        .unwrap();
    spawner.spawn(net_task(runner)).unwrap();

    wait_for_ip(&stack).await;

    spawner.spawn(http_server_task(stack)).unwrap();

    info!("HTTP server started on port 80");

    let receiver = MOTOR_COMMAND_CHANNEL.receiver();
    let mut continuous_mode = false;
    let mut continuous_direction = Direction::Clockwise;

    loop {
        if continuous_mode {
            match receiver.try_receive() {
                Ok(MotorCommand::Stop) => {
                    continuous_mode = false;
                    motor.stop();
                    info!("Motor stopped");
                }
                Ok(MotorCommand::SetSpeed { rpm }) => {
                    motor.set_output_rpm(rpm);
                    info!("Speed set to {} RPM", rpm);
                }
                Ok(cmd) => {
                    continuous_mode = false;
                    motor.stop();
                    handle_command(
                        &mut motor,
                        cmd,
                        &mut continuous_mode,
                        &mut continuous_direction,
                    )
                    .await;
                }
                Err(_) => {
                    motor.step(continuous_direction).await;
                }
            }
        } else {
            let cmd = receiver.receive().await;
            handle_command(
                &mut motor,
                cmd,
                &mut continuous_mode,
                &mut continuous_direction,
            )
            .await;
        }
    }
}

async fn handle_command(
    motor: &mut Stepper<'_>,
    cmd: MotorCommand,
    continuous_mode: &mut bool,
    continuous_direction: &mut Direction,
) {
    match cmd {
        MotorCommand::Rotate { degrees, direction } => {
            info!("Rotating {}° {:?}", degrees, direction);
            motor.rotate(degrees, direction).await;
            motor.stop();
            info!("Rotation complete");
        }
        MotorCommand::SetSpeed { rpm } => {
            motor.set_output_rpm(rpm);
            info!("Speed set to {} RPM", rpm);
        }
        MotorCommand::Continuous { direction } => {
            *continuous_mode = true;
            *continuous_direction = direction;
            info!("Continuous rotation {:?}", direction);
        }
        MotorCommand::Stop => {
            *continuous_mode = false;
            motor.stop();
            info!("Motor stopped");
        }
    }
}
