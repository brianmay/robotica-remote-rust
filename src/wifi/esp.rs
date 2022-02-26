use std::{env, sync::Arc};

use embedded_svc::wifi::*;

use esp_idf_svc::netif::*;
use esp_idf_svc::nvs::*;
use esp_idf_svc::sntp::EspSntp;
use esp_idf_svc::sysloop::*;
use esp_idf_svc::wifi::*;

use anyhow::bail;
use anyhow::Result;

use log::*;

const SSID: &str = env!("WIFI_SSID");
const PASS: &str = env!("WIFI_PASS");

#[allow(dead_code)]
pub struct MyWifi {
    wifi: Box<EspWifi>,
    sntp: EspSntp,
}

impl crate::wifi::Wifi for MyWifi {}

pub fn connect() -> Result<MyWifi> {
    let netif_stack = Arc::new(EspNetifStack::new()?);
    let sys_loop_stack = Arc::new(EspSysLoopStack::new()?);
    let default_nvs = Arc::new(EspDefaultNvs::new()?);

    let wifi = wifi(netif_stack, sys_loop_stack, default_nvs)?;
    let sntp = EspSntp::new_default()?;

    Ok(MyWifi { wifi, sntp })
}

fn wifi(
    netif_stack: Arc<EspNetifStack>,
    sys_loop_stack: Arc<EspSysLoopStack>,
    default_nvs: Arc<EspDefaultNvs>,
) -> Result<Box<EspWifi>> {
    let mut wifi = Box::new(EspWifi::new(netif_stack, sys_loop_stack, default_nvs)?);

    info!("Wifi created, about to scan");

    let ap_infos = wifi.scan()?;

    let ours = ap_infos.into_iter().find(|a| a.ssid == SSID);

    let channel = if let Some(ours) = ours {
        info!(
            "Found configured access point {} on channel {}",
            SSID, ours.channel
        );
        Some(ours.channel)
    } else {
        info!(
            "Configured access point {} not found during scanning, will go with unknown channel",
            SSID
        );
        None
    };

    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: SSID.into(),
        password: PASS.into(),
        channel,
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

    info!("Wifi configuration set, about to get status");

    let status = wifi.get_status();

    if let Started(Connected(Done(ip_settings))) = status.0 {
        info!("Wifi connected: {:?}", ip_settings);
    } else {
        bail!("Unexpected Wifi status: {:?}", status);
    }

    Ok(wifi)
}
