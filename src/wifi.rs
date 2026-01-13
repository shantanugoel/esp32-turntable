use alloc::string::String;
use embassy_net::{Runner, Stack, StackResources};
use embassy_time::{Duration, Timer};
use esp_radio::wifi::{ClientConfig, ModeConfig, WifiController, WifiDevice, WifiEvent};

pub const WIFI_SSID: &str = env!("WIFI_SSID");
pub const WIFI_PASSWORD: &str = env!("WIFI_PASSWORD");

#[embassy_executor::task]
pub async fn wifi_connection_task(mut controller: WifiController<'static>) {
    defmt::info!("WiFi task started");

    let client_config = ClientConfig::default()
        .with_ssid(String::from(WIFI_SSID))
        .with_password(String::from(WIFI_PASSWORD));
    let mode_config = ModeConfig::Client(client_config);

    controller
        .set_config(&mode_config)
        .expect("Failed to set WiFi config");

    controller
        .start_async()
        .await
        .expect("Failed to start WiFi");
    defmt::info!("WiFi started, attempting connection...");

    loop {
        match controller.is_connected() {
            Ok(true) => {
                controller
                    .wait_for_all_events(WifiEvent::StaDisconnected.into(), true)
                    .await;
                defmt::warn!("WiFi disconnected, reconnecting...");
                Timer::after(Duration::from_millis(1000)).await;
            }
            _ => match controller.connect() {
                Ok(_) => {
                    controller
                        .wait_for_all_events(WifiEvent::StaConnected.into(), true)
                        .await;
                    defmt::info!("WiFi connected");
                }
                Err(e) => {
                    defmt::error!("WiFi connect error: {:?}", e);
                    Timer::after(Duration::from_millis(5000)).await;
                }
            },
        }
    }
}

#[embassy_executor::task]
pub async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await;
}

pub async fn wait_for_ip(stack: &Stack<'static>) {
    loop {
        if let Some(config) = stack.config_v4() {
            defmt::info!("Got IP: {}", config.address);
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }
}

pub fn create_stack_resources() -> &'static mut StackResources<3> {
    static RESOURCES: static_cell::StaticCell<StackResources<3>> = static_cell::StaticCell::new();
    RESOURCES.init(StackResources::new())
}
