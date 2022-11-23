use std::env;
use std::net::Ipv4Addr;
use std::time::Duration;

use embedded_svc::ipv4;
use embedded_svc::ipv4::DHCPClientSettings;
use embedded_svc::wifi::*;

use esp_idf_hal::peripheral;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::netif::*;
use esp_idf_svc::sntp::EspSntp;
use esp_idf_svc::wifi::*;

use anyhow::bail;
use anyhow::Result;

use heapless::String;
use log::*;

use crate::hardware::esp32::get_unique_id;

const SSID: &str = env!("WIFI_SSID");
const PASS: &str = env!("WIFI_PASS");

pub fn connect(
    modem: impl peripheral::Peripheral<P = esp_idf_hal::modem::Modem> + 'static,
) -> Result<(EspWifi<'static>, EspSntp)> {
    let sysloop = EspSystemEventLoop::take()?;
    // let netif_stack = Arc::new(EspNetifStack::new()?);
    // let sys_loop_stack = Arc::new(EspSysLoopStack::new()?);
    // let default_nvs = Arc::new(EspDefaultNvs::new()?);

    let wifi = wifi(modem, sysloop)?;
    let sntp = EspSntp::new_default()?;

    Ok((wifi, sntp))
}

fn wifi(
    modem: impl peripheral::Peripheral<P = esp_idf_hal::modem::Modem> + 'static,
    sysloop: EspSystemEventLoop,
    // netif_stack: Arc<EspNetifStack>,
    // sys_loop_stack: Arc<EspSysLoopStack>,
    // default_nvs: Arc<EspDefaultNvs>,
) -> Result<EspWifi<'static>> {
    let config = {
        let hostname = format!("robotica-remote_{}", get_unique_id());
        let dhcp_conf = DHCPClientSettings {
            hostname: Some(String::from(hostname.as_str())),
        };

        let client_conf = ipv4::ClientConfiguration::DHCP(dhcp_conf);
        let mut config = NetifConfiguration::wifi_default_client();
        config.ip_configuration = ipv4::Configuration::Client(client_conf);
        config
    };

    let driver = WifiDriver::new(modem, sysloop.clone(), None)?;
    let sta_netif = EspNetif::new_with_conf(&config)?;
    let ap_netif = EspNetif::new(NetifStack::Ap)?;
    let mut wifi = EspWifi::wrap_all(driver, sta_netif, ap_netif)?;

    info!("Connecting to wifi...");

    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: SSID.into(),
        password: PASS.into(),
        ..Default::default()
    }))?;

    wifi.start()?;

    info!("Starting wifi...");

    if !WifiWait::new(&sysloop)?
        .wait_with_timeout(Duration::from_secs(20), || wifi.is_started().unwrap())
    {
        bail!("Wifi did not start");
    }

    info!("Connecting wifi...");

    wifi.connect()?;
    info!("Connecting wifi...(2)");

    if !EspNetifWait::new::<EspNetif>(wifi.sta_netif(), &sysloop)?.wait_with_timeout(
        Duration::from_secs(20),
        || {
            wifi.is_connected().unwrap()
                && wifi.sta_netif().get_ip_info().unwrap().ip != Ipv4Addr::new(0, 0, 0, 0)
        },
    ) {
        bail!("Wifi did not connect or did not receive a DHCP lease");
    }

    let ip_info = wifi.sta_netif().get_ip_info()?;

    info!("Wifi DHCP info: {:?}", ip_info);

    // ping(ip_info.subnet.gateway)?;
    Ok(wifi)
}
