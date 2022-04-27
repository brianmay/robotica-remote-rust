use std::{env, sync::Arc};

use embedded_svc::ipv4;
use embedded_svc::ipv4::DHCPClientSettings;
use embedded_svc::wifi::*;

use esp_idf_svc::netif::*;
use esp_idf_svc::nvs::*;
use esp_idf_svc::sntp::EspSntp;
use esp_idf_svc::sysloop::*;
use esp_idf_svc::wifi::*;

use anyhow::bail;
use anyhow::Result;

use log::*;

use crate::hardware::esp32::get_unique_id;

const SSID: &str = env!("WIFI_SSID");
const PASS: &str = env!("WIFI_PASS");

pub fn connect() -> Result<(EspWifi, EspSntp)> {
    let netif_stack = Arc::new(EspNetifStack::new()?);
    let sys_loop_stack = Arc::new(EspSysLoopStack::new()?);
    let default_nvs = Arc::new(EspDefaultNvs::new()?);

    let wifi = wifi(netif_stack, sys_loop_stack, default_nvs)?;
    let sntp = EspSntp::new_default()?;

    Ok((wifi, sntp))
}

fn wifi(
    netif_stack: Arc<EspNetifStack>,
    sys_loop_stack: Arc<EspSysLoopStack>,
    default_nvs: Arc<EspDefaultNvs>,
) -> Result<EspWifi> {
    let mut wifi = EspWifi::new(netif_stack, sys_loop_stack, default_nvs)?;

    info!("Connecting to wifi");
    let hostname = format!("robotica-remote_{}", get_unique_id());
    let dhcp_conf = DHCPClientSettings {
        hostname: Some(hostname),
    };

    let ip_conf = ipv4::ClientConfiguration::DHCP(dhcp_conf);
    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: SSID.into(),
        password: PASS.into(),
        ip_conf: Some(ip_conf),
        ..Default::default()
    }))?;

    info!("Waiting for wifi");
    use ClientConnectionStatus::Connected;
    use ClientIpStatus::Done;
    use ClientStatus::Started;

    fn check_status(status: &Status) -> bool {
        matches!(&status.0, Started(Connected(Done(_ip_settings))))
    }
    wifi.wait_status(check_status);

    let status = wifi.get_status();

    if let Started(Connected(Done(ip_settings))) = status.0 {
        info!("Wifi connected: {:?}", ip_settings);
    } else {
        bail!("Unexpected Wifi status: {:?}", status);
    }

    Ok(wifi)
}
