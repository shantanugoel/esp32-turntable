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
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::timer::timg::TimerGroup;
use turntable::stepper::{Direction, StepMode, Stepper};
use {esp_backtrace as _, esp_println as _};

extern crate alloc;

esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[esp_rtos::main]
async fn main(_spawner: Spawner) -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 66320);
    esp_alloc::heap_allocator!(size: 64 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    info!("Stepper motor test starting...");

    let in1 = Output::new(peripherals.GPIO1, Level::Low, OutputConfig::default());
    let in2 = Output::new(peripherals.GPIO2, Level::Low, OutputConfig::default());
    let in3 = Output::new(peripherals.GPIO3, Level::Low, OutputConfig::default());
    let in4 = Output::new(peripherals.GPIO4, Level::Low, OutputConfig::default());

    let mut motor = Stepper::new(in1, in2, in3, in4);

    loop {
        info!("=== Half-step mode tests ===");
        motor.set_mode(StepMode::Half);

        info!("Rotating 360° CW at 5 RPM (half-step)");
        motor.set_rpm(5.0);
        motor.rotate(360.0, Direction::Clockwise).await;
        motor.stop();
        Timer::after(Duration::from_secs(1)).await;

        info!("Rotating 360° CCW at 10 RPM (half-step)");
        motor.set_rpm(10.0);
        motor.rotate(360.0, Direction::CounterClockwise).await;
        motor.stop();
        Timer::after(Duration::from_secs(1)).await;

        info!("Rotating 360° CW at 15 RPM (half-step, max speed)");
        motor.set_rpm(15.0);
        motor.rotate(360.0, Direction::Clockwise).await;
        motor.stop();
        Timer::after(Duration::from_secs(1)).await;

        info!("=== Full-step mode tests ===");
        motor.set_mode(StepMode::Full);

        info!("Rotating 360° CCW at 5 RPM (full-step)");
        motor.set_rpm(5.0);
        motor.rotate(360.0, Direction::CounterClockwise).await;
        motor.stop();
        Timer::after(Duration::from_secs(1)).await;

        info!("Rotating 360° CW at 10 RPM (full-step)");
        motor.set_rpm(10.0);
        motor.rotate(360.0, Direction::Clockwise).await;
        motor.stop();
        Timer::after(Duration::from_secs(1)).await;

        info!("Rotating 360° CCW at 15 RPM (full-step, max speed)");
        motor.set_rpm(15.0);
        motor.rotate(360.0, Direction::CounterClockwise).await;
        motor.stop();
        Timer::after(Duration::from_secs(1)).await;

        info!("=== Test cycle complete, restarting in 3 seconds ===");
        Timer::after(Duration::from_secs(3)).await;
    }
}
