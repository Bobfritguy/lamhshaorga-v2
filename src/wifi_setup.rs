use anyhow::{bail, Error};

use embedded_svc::wifi::{AuthMethod, Configuration, ClientConfiguration, AccessPointConfiguration};
use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::peripheral;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::wifi::{BlockingWifi, EspWifi};
use log::{info, error};
use core::time::Duration;




pub fn wifi(
    ssid: &str,
    pass: &str,
    modem: impl peripheral::Peripheral<P = esp_idf_hal::modem::Modem> + 'static,
    sysloop: EspSystemEventLoop,
    max_retries: u8,
) -> Result<Box<EspWifi<'static>>, Error> {
    let mut auth_method = AuthMethod::WPA2Personal;
    if ssid.is_empty() {
        bail!("Missing WiFi name")
    }
    if pass.is_empty() {
        auth_method = AuthMethod::None;
        info!("Wifi password is empty");
    }
    let mut esp_wifi = EspWifi::new(modem, sysloop.clone(), None)?;

    let mut wifi = BlockingWifi::wrap(&mut esp_wifi, sysloop)?;

    wifi.set_configuration(&Configuration::Client(ClientConfiguration::default()))?;

    info!("Starting wifi...");

    wifi.start()?;

    info!("Scanning...");

    let ap_infos = wifi.scan()?;

    let ours = ap_infos.into_iter().find(|a| a.ssid == ssid);

    let channel = if let Some(ours) = ours {
        info!(
            "Found configured access point {} on channel {}",
            ssid, ours.channel
        );
        Some(ours.channel)
    } else {
        info!(
            "Configured access point {} not found during scanning, will go with unknown channel",
            ssid
        );
        None
    };

    wifi.set_configuration(&Configuration::Mixed(
        ClientConfiguration {
            ssid: ssid.into(),
            password: pass.into(),
            channel,
            auth_method,
            ..Default::default()
        },
        AccessPointConfiguration {
            ssid: "aptest".into(),
            channel: channel.unwrap_or(1),
            ..Default::default()
        },
    ))?;

    // Due to EspError(263) we need to retry connecting to wifi. ESP_ERR_TIMEOUT (0x107): Operation timed out
    let mut retry_count = 0;
    loop {
        info!(
            "Attempting to connect to wifi... Attempt: {}",
            retry_count + 1
        );

        match wifi.connect() {
            Ok(_) => {
                info!("Successfully connected to wifi!");
                break;
            }
            Err(e) => {
                error!("Failed to connect to wifi: {:?}", e);
                retry_count += 1;
                if retry_count >= max_retries {
                    bail!("Failed to connect to wifi after {} attempts", max_retries);
                }
                FreeRtos::delay_ms(2000 * (retry_count as u32)); // Delay between attempts
            }
        }
    }

    info!("Waiting for DHCP lease...");

    wifi.wait_netif_up()?;

    let ip_info = wifi.wifi().sta_netif().get_ip_info()?;

    info!("Wifi DHCP info: {:?}", ip_info);

    Ok(Box::new(esp_wifi))
}


pub fn init_mdns() -> Result<esp_idf_svc::mdns::EspMdns, esp_idf_sys::EspError> {
    let mut mdns = esp_idf_svc::mdns::EspMdns::take()?;
    mdns.set_hostname("limbcontroller")?;
    // add a custom udp service
    mdns.add_service(
        Some("Limb Controller ESP32"),
        "_controller",
        "_udp",
        8080,
        &[
            ("controls", "8") // 8 controls
        ]
    )?;
    Ok(mdns)
}

pub fn init_socket(read_timeout: Option<Duration>) -> std::net::UdpSocket {
    let socket = match std::net::UdpSocket::bind("0.0.0.0:8080") {
        Ok(socket) => socket,
        Err(e) => panic!("Unable to bind socket on 0.0.0.0:8080 with error: {}", e), // Serious error, robot is effectively unusable
    };

    match socket.set_read_timeout(read_timeout) {
        Ok(_) => info!("Set socket read timeout to {}", read_timeout.unwrap().as_millis()),
        Err(e) => error!("Failed to set socket timeout to {}: {}", read_timeout.unwrap().as_millis(), e)
    };


    match socket.set_nonblocking(true) {
        Ok(_) => info!("Socket set to non-blocking."),
        Err(e) => error!("Failed to set the socket to non-blocking: {}", e),
    };

    socket
}