#![feature(let_chains)]

// Modules
mod wifi_setup;
mod servo;
mod display;

// Standard library imports
use std::{io, vec};
use std::borrow::Borrow;
use std::net::{UdpSocket};


// Third-party imports
use anyhow::Result;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::Drawable;
use embedded_graphics::geometry::Point;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::iso_8859_16::FONT_5X8;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::text::Text;
use log::{error, info};

// ESP IDF related imports
use esp_idf_hal::gpio::{OutputPin, PinDriver};
use esp_idf_hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_hal::ledc::{config, LedcChannel, LedcDriver, LedcTimerDriver};
use esp_idf_hal::peripheral::Peripheral;
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::units::FromValueType;
use esp_idf_hal::timer::{TimerDriver, config as HalTimerConfig};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_sys::{esp, EspError, nvs_flash_init};

// Custom Imports
use servo::Servo;
use crate::display::Display;

#[allow(unused_imports)]
use esp_idf_sys as _;
use ssd1306::{I2CDisplayInterface, Ssd1306};
use ssd1306::mode::{BufferedGraphicsMode, DisplayConfig};
use ssd1306::prelude::{DisplayRotation, DisplaySize128x64, I2CInterface};


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
            error_code => error!("NVS Flash initialization failed with error code: {}", error_code),
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

    let mut driver = match I2cDriver::new(i2c, sda, scl, &config) {
        Ok(driver) => driver,
        Err(e) => {
            panic!("Failed to initialize I2C driver: {:?}", e);
        }
    };
    let mut interface = I2CDisplayInterface::new(driver);

    let mut display = Display::new(
        Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0).into_buffered_graphics_mode()
    );

    let mut to_oled: String = "Starting...".parse()?;

    display.init();
    display.set_text(to_oled);
    display.set_text_style(MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(BinaryColor::On)
        .build());
    display.draw(0, 7);


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

    to_oled = format!("Robotic Limb V{}.{}\nIP Address: \n{}", VERSION_MAJ, VERSION_MIN, ip_string).parse()?;

    display.clear();
    display.flush();
    display.set_text(to_oled);
    display.draw(0, 7);




    // Set up the servo drivers
    let ledc_driver = match LedcTimerDriver::new(
                                    peripherals.ledc.timer0,
                                    &config::TimerConfig::new().resolution(esp_idf_hal::ledc::Resolution::Bits12).frequency(50.Hz().into()),
                                   ) {
                                    Ok(driver) => driver,
                                    Err(e) => panic!("LEDc Timer driver failed to initialise: {}", e), // Serious issue if ledc driver cannot be initialised
                                   };


    let mut servos: Vec<Servo> = Vec::new();

    create_and_add_servo("Top", peripherals.ledc.channel0, &ledc_driver, peripherals.pins.gpio15, &mut servos, MIUZEI_MINI_MIN_DUTY, MIUZEI_MINI_MAX_DUTY, 180);
    create_and_add_servo("Shoulder", peripherals.ledc.channel1, &ledc_driver, peripherals.pins.gpio16, &mut servos, MIUZEI_MINI_MIN_DUTY, MIUZEI_MINI_MAX_DUTY, 180);
    create_and_add_servo("Upper Arm", peripherals.ledc.channel2, &ledc_driver, peripherals.pins.gpio17, &mut servos, MIUZEI_MINI_MIN_DUTY, MIUZEI_MINI_MAX_DUTY, 180);
    create_and_add_servo("Elbow", peripherals.ledc.channel3, &ledc_driver, peripherals.pins.gpio18, &mut servos, MIUZEI_MINI_MIN_DUTY, MIUZEI_MINI_MAX_DUTY, 180);
    create_and_add_servo("Lower Arm", peripherals.ledc.channel4, &ledc_driver, peripherals.pins.gpio19, &mut servos, MIUZEI_MINI_MIN_DUTY, MIUZEI_MINI_MAX_DUTY, 180);



    let mut led = PinDriver::output(peripherals.pins.gpio4)?;

    // Timer setup
    let mut timer =  TimerDriver::new(peripherals.timer00, &HalTimerConfig::Config::new().auto_reload(true))?;
    timer.set_alarm(1_000_000)?; // Set for 1 second (in microseconds)
    unsafe {
        timer.subscribe(move || {
            let _ = led.toggle();
        })?;
    }
    timer.enable_interrupt()?;
    timer.enable_alarm(true)?;
    timer.enable(true)?;

    let mut data: Vec<u8>;
    let mut from_addr: std::net::SocketAddr;
    let mut ack: Vec<u8> = vec![0u8];

    let mut servo_string = "Servo Positions:\n".to_string();
    display.set_text_style(MonoTextStyleBuilder::new()
        .font(&FONT_5X8)
        .text_color(BinaryColor::On)
        .build());

    info!("Entering Loop");
    loop {
        match recv_data(&socket, MAX_CONTROL_SIGNAL_SIZE) {
            Ok(Some((received_data, src_addr))) => {
                data = received_data;
                from_addr = src_addr;
            },
            Ok(None) => {
                info!("Received None");
                continue;
            },
            Err(_) => {
                error!("Failed to receive data");
                continue;
            }
        }


        match data[0] {
            0 => {
                info!("Received Control Signal");
                &servos[0].set_angle(u16::from_be_bytes([data[1], data[2]]));
                &servos[1].set_angle(u16::from_be_bytes([data[3], data[4]]));
                &servos[2].set_angle(u16::from_be_bytes([data[5], data[6]]));
                &servos[3].set_angle(u16::from_be_bytes([data[7], data[8]]));
                &servos[4].set_angle(u16::from_be_bytes([data[9], data[10]]));

                let servo_string = format!(
                    "Servo Positions:\n{}\n{}\n{}\n{}\n{}",
                    servos[0].to_string(),
                    servos[1].to_string(),
                    servos[2].to_string(),
                    servos[3].to_string(),
                    servos[4].to_string()
                );

                display.clear();
                display.draw_text(0, 7, servo_string);
                display.flush();

                ack.clear();
                for servo in &servos{
                    ack.push(servo.get_angle() as u8);
                    ack.push((servo.get_angle() >> 8) as u8);
                }
                socket.send_to(&ack, from_addr)?;

                // TIMER TEST
                //timer.counter()?;
                //timer.enable(true)?;
            },
            1 => {
                info!("Received Poll Signal");
                info!("Sending back to {}", from_addr);
                // let mut poll: Vec<u8> = Vec::new();
                //
                // socket.send_to(&poll, from_addr)?;
                // TODO: Send back servo positions & other info
            },
            _ => {
                error!("Not a valid command");
            },
        }
    }
}

// Function to receive data from UDP packet and return it along with the source address
fn recv_data(socket: &UdpSocket, max_size: usize) -> Result<Option<(Vec<u8>, std::net::SocketAddr)>> {
    let mut buf = vec![0; max_size];
    match socket.recv_from(&mut buf) {
        Ok((size, src_addr)) => {
            buf.resize(size, 0);
            info!("Received data from: {}", src_addr);
            info!("Data: {:?}", buf);
            Ok(Some((buf, src_addr)))
        },
        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
            // WouldBlock is the error kind for a read timeout
            Ok(None)
        },
        Err(_) => {
            // Handle other errors by setting all byte values to 0, effectively halting the system.
            buf.iter_mut().for_each(|byte| *byte = 0);
            Ok(Some((buf, "0.0.0.0:8080".parse().unwrap())))
        },
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
    max_angle_degrees: u16
) {
    match LedcDriver::new(channel, ledc_driver, pin) {
        Ok(driver) => {
            let servo = Servo::new(name.to_string(), driver, min_duty, max_duty, max_angle_degrees);
            servos.push(servo);
        },
        Err(e) => error!("Failed to create servo {}: {}", name, e),
    }
}

