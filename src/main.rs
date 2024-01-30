#![feature(let_chains)]

// Modules
mod display;
mod servo;
mod wifi_setup;

// Standard library imports
use std::borrow::Borrow;
use std::io;
use std::net::UdpSocket;

// Third-party imports
use anyhow::Result;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::iso_8859_16::FONT_5X8;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use log::{error, info};

// ESP IDF related imports
use esp_idf_hal::gpio::{OutputPin, PinDriver};
use esp_idf_hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_hal::ledc::{config, LedcChannel, LedcDriver, LedcTimerDriver};
use esp_idf_hal::peripheral::Peripheral;
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::timer::{config as HalTimerConfig, TimerDriver};
use esp_idf_hal::units::FromValueType;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_sys::nvs_flash_init;

// Custom Imports
use crate::display::Display;
use servo::Servo;

#[allow(unused_imports)]
use esp_idf_sys as _;
use ssd1306::prelude::{DisplayRotation, DisplaySize128x64};
use ssd1306::{I2CDisplayInterface, Ssd1306};

#[toml_cfg::toml_config]
pub struct Config {
    #[default("")]
    wifi_ssid: &'static str,
    #[default("")]
    wifi_psk: &'static str,
}

// Set a constant CONTROL_SIGNAL_SIZE
const VERSION_MIN: u32 = 6;
const VERSION_MAJ: u32 = 0;
const MAX_CONTROL_SIGNAL_SIZE: usize = 11;

// VALUES FOR SERVOS
const HOBBY_FANS_MIN_DUTY: f32 = 0.0275;
const HOBBY_FANS_MAX_DUTY: f32 = 0.125;

const MIUZEI_MIN_DUTY: f32 = 0.018;
const MIUZEI_MAX_DUTY: f32 = 0.11;

const MIUZEI_MINI_MIN_DUTY: f32 = 0.024;
const MIUZEI_MINI_MAX_DUTY: f32 = 0.11;

// Control bytes

fn main() -> Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_hal::sys::link_patches();

    esp_idf_svc::log::EspLogger::initialize_default();
    // Initialize NVS, unsure if we will need this in future
    unsafe {
        match nvs_flash_init() {
            0 => info!("NVS Flash initialized"),
            error_code => error!(
                "NVS Flash initialization failed with error code: {}",
                error_code
            ),
        }
    }

    // get peripherals
    let peripherals: Peripherals = match Peripherals::take() {
        Ok(peripherals) => peripherals,
        Err(e) => {
            panic!("Failed to take peripherals: {:?}", e);
        }
    };

    // get system event loop
    let system_loop = match EspSystemEventLoop::take() {
        Ok(sloop) => sloop,
        Err(e) => {
            panic!("Failed to take system event loop: {:?}", e);
        }
    };

    // Set up pins for i2c, and i2c port
    let i2c = peripherals.i2c0;
    let sda = peripherals.pins.gpio21;
    let scl = peripherals.pins.gpio22;

    // Set up the i2c driver
    let config = I2cConfig::new().baudrate(1.MHz().into());

    let driver = match I2cDriver::new(i2c, sda, scl, &config) {
        Ok(driver) => driver,
        Err(e) => {
            panic!("Failed to initialize I2C driver: {:?}", e);
        }
    };

    let interface = I2CDisplayInterface::new(driver);

    let mut display = Display::new(
        Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
            .into_buffered_graphics_mode(),
    );

    let mut to_oled: String = "Starting...".to_string();

    display.init();
    display.set_text_style(
        MonoTextStyleBuilder::new()
            .font(&FONT_6X10)
            .text_color(BinaryColor::On)
            .build(),
    );
    display.draw_new_text(0, 7, &to_oled);

    // Connect to WiFi
    info!("Socket initialize");
    let _wifi = wifi_setup::wifi(
        CONFIG.wifi_ssid,
        CONFIG.wifi_psk,
        peripherals.modem,
        system_loop,
        6,
    )?;

    let socket = wifi_setup::init_socket(None);
    info!("Socket initialized");

    let _mdns = wifi_setup::init_mdns();
    info!("mDNS initialized");

    let ip_string = _wifi.sta_netif().get_ip_info()?.ip;

    to_oled = format!(
        "Robotic Limb V{}.{}\nIP Address: \n{}",
        VERSION_MAJ, VERSION_MIN, ip_string
    )
    .parse()?;

    display.draw_new_text(0, 7, &to_oled);
    drop(to_oled);

    // Set up the servo drivers
    let ledc_driver = match LedcTimerDriver::new(
        peripherals.ledc.timer0,
        &config::TimerConfig::new()
            .resolution(esp_idf_hal::ledc::Resolution::Bits12)
            .frequency(50.Hz().into()),
    ) {
        Ok(driver) => driver,
        Err(e) => panic!("LEDc Timer driver failed to initialise: {}", e), // Serious issue if ledc driver cannot be initialised
    };

    let mut servos: Vec<Servo> = Vec::with_capacity(5);

    create_and_add_servo(
        "Top",
        peripherals.ledc.channel0,
        &ledc_driver,
        peripherals.pins.gpio15,
        &mut servos,
        MIUZEI_MINI_MIN_DUTY,
        MIUZEI_MINI_MAX_DUTY,
        180,
    );
    create_and_add_servo(
        "Shoulder",
        peripherals.ledc.channel1,
        &ledc_driver,
        peripherals.pins.gpio16,
        &mut servos,
        MIUZEI_MINI_MIN_DUTY,
        MIUZEI_MINI_MAX_DUTY,
        180,
    );
    create_and_add_servo(
        "Upper Arm",
        peripherals.ledc.channel2,
        &ledc_driver,
        peripherals.pins.gpio17,
        &mut servos,
        MIUZEI_MINI_MIN_DUTY,
        MIUZEI_MINI_MAX_DUTY,
        180,
    );
    create_and_add_servo(
        "Elbow",
        peripherals.ledc.channel3,
        &ledc_driver,
        peripherals.pins.gpio18,
        &mut servos,
        MIUZEI_MINI_MIN_DUTY,
        MIUZEI_MINI_MAX_DUTY,
        180,
    );
    create_and_add_servo(
        "Lower Arm",
        peripherals.ledc.channel4,
        &ledc_driver,
        peripherals.pins.gpio19,
        &mut servos,
        MIUZEI_MINI_MIN_DUTY,
        MIUZEI_MINI_MAX_DUTY,
        180,
    );

    let mut led = PinDriver::output(peripherals.pins.gpio4)?;

    // Timer setup
    let mut timer = match TimerDriver::new(
        peripherals.timer00,
        &HalTimerConfig::Config::new().auto_reload(true),
    ){
        Ok(timer) => timer,
        Err(e) => panic!("Failed to initialize timer: {}", e),
    };

    let mut alarm_time_us: u64 = 1_000_000; // Set for 1 second (in microseconds)

    match timer.set_alarm(alarm_time_us){
        Ok(_) => {},
        Err(e) => error!("Failed to set alarm: {}", e),
    };

    unsafe {
        match timer.subscribe(move || {
            led.toggle().unwrap();
        }){
            Ok(_) => {},
            Err(e) => error!("Failed to subscribe to timer: {}", e),
        };
    }

    timer.enable_interrupt()?;
    timer.enable_alarm(true)?;
    timer.enable(false)?;

    let mut from_addr: std::net::SocketAddr;
    let mut ctrl_vec: Vec<u8> = Vec::with_capacity(MAX_CONTROL_SIGNAL_SIZE);
    ctrl_vec = vec![0; MAX_CONTROL_SIGNAL_SIZE];

    display.set_text_style(
        MonoTextStyleBuilder::new()
            .font(&FONT_5X8)
            .text_color(BinaryColor::On)
            .build(),
    );

    let calc_string = format!(
        // Create a stub string to calculate the size of the servo string
        "Servo Positions:\n{}0\n{}0\n{}0\n{}0\n{}0",
        servos[0].to_string(),
        servos[1].to_string(),
        servos[2].to_string(),
        servos[3].to_string(),
        servos[4].to_string()
    );

    let mut servo_string = String::with_capacity(calc_string.len()); // Allocate the space for the loop string, small performance boost
    drop(calc_string); // Drop the stub string




    info!("Entering Loop");
    loop {
        match recv_data(&socket, &mut ctrl_vec) {
            Ok(Some((received_data, src_addr))) => {
                if received_data.is_empty() {
                    continue;
                }
                from_addr = src_addr;
            }
            Ok(None) => {
                info!("Received None");
                continue;
            }
            Err(e) => {
                error!("Failed to receive data: {}", e);
                continue;
            }
        }
            match ctrl_vec[0] {
                0 => {
                    servos[0].set_angle(u16::from_be_bytes([ctrl_vec[1], ctrl_vec[2]]));
                    servos[1].set_angle(u16::from_be_bytes([ctrl_vec[3], ctrl_vec[4]]));
                    servos[2].set_angle(u16::from_be_bytes([ctrl_vec[5], ctrl_vec[6]]));
                    servos[3].set_angle(u16::from_be_bytes([ctrl_vec[7], ctrl_vec[8]]));
                    servos[4].set_angle(u16::from_be_bytes([ctrl_vec[9], ctrl_vec[10]]));

                    servo_string.clear();
                    // Append the static part of the display string
                    servo_string.push_str("Servo Positions:\n");
                    servo_string.push_str(&*format!(
                        "{}\n{}\n{}\n{}\n{}",
                        servos[0].to_string(),
                        servos[1].to_string(),
                        servos[2].to_string(),
                        servos[3].to_string(),
                        servos[4].to_string()
                    ));

                    display.draw_new_text(0, 7, &servo_string);

                    ctrl_vec.clear();
                    for servo in &servos {
                        ctrl_vec.push(servo.get_angle() as u8);
                        ctrl_vec.push((servo.get_angle() >> 8) as u8);
                    }
                    match socket.send_to(&ctrl_vec, from_addr){
                        Ok(_) => {},
                        Err(e) => error!("Failed to send servo positions: {}", e),
                    }
                    ctrl_vec.push(0); // Required to make ctrl vec = 11
                    // TIMER TEST
                    //timer.counter()?;
                    //timer.enable(true)?;
                }
                1 => {
                    info!("Received Ping Signal");
                    info!("Sending back to {}", from_addr);
                    let mut ping_vec: Vec<u8> = Vec::new();

                    for servo in &servos {
                        ping_vec.push(servo.get_angle() as u8);
                        ping_vec.push((servo.get_angle() >> 8) as u8);
                    }

                    match socket.send_to(&ping_vec, from_addr){
                        Ok(_) => {},
                        Err(e) => error!("Failed to send servo positions: {}", e),
                    }
                    drop(ping_vec);
                }
                2 => {
                    info!("Received Config Signal");

                }
                _ => {
                    error!("Not a valid command");
                }
            }
        }
}

// Function to receive data from UDP packet and return it along with the source address
fn recv_data(
    socket: &UdpSocket,
    buf: &mut Vec<u8>,
) -> Result<Option<(Vec<u8>, std::net::SocketAddr)>> {
    match socket.recv_from(buf) {
        Ok((size, src_addr)) => {
            Ok(Some((buf.to_vec(), src_addr)))
        }
        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
            // WouldBlock is the error kind for a read timeout
            Ok(None)
        }
        Err(_) => {
            // Handle other errors by setting all byte values to 0, effectively halting the system.
            buf.iter_mut().for_each(|byte| *byte = 0);
            Ok(Some((buf.to_vec(), "0.0.0.0:8080".parse().unwrap())))
        }
    }
}

fn create_and_add_servo<'d, C: LedcChannel, B: Borrow<LedcTimerDriver<'static>>>(
    name: &str,
    channel: impl Peripheral<P = C> + 'static,
    ledc_driver: B,
    pin: impl Peripheral<P = impl OutputPin> + 'static,
    servos: &mut Vec<Servo>,
    min_duty: f32,
    max_duty: f32,
    max_angle_degrees: u16,
) {
    match LedcDriver::new(channel, ledc_driver, pin) {
        Ok(driver) => {
            let servo = Servo::new(
                name.to_string(),
                driver,
                min_duty,
                max_duty,
                max_angle_degrees,
            );
            servos.push(servo);
        }
        Err(e) => error!("Failed to create servo {}: {}", name, e),
    }
}
