#![feature(let_chains)]

// Modules
mod wifi_setup;
mod servo;

// Standard library imports
use std::{io, vec};
use std::borrow::Borrow;
use std::net::{UdpSocket};
use std::time::Duration;

// Third-party imports
use anyhow::Result;
use log::{error, info};
use embedded_graphics::{mono_font::{ascii::FONT_6X10, MonoTextStyleBuilder}, pixelcolor::BinaryColor, prelude::*, text::{Baseline, Text}};
use ssd1306::{I2CDisplayInterface, Ssd1306};
use ssd1306::rotation::DisplayRotation;
use ssd1306::size::DisplaySize128x64;
use ssd1306::mode::{DisplayConfig};

// ESP IDF related imports
use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::gpio::{AnyOutputPin, OutputPin, PinDriver};
use esp_idf_hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_hal::ledc::{config, LEDC, LedcChannel, LedcDriver, LedcTimerDriver};
use esp_idf_hal::peripheral::Peripheral;
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::units::FromValueType;
use esp_idf_hal::timer::{TimerDriver, config as HalTimerConfig};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_sys::{esp, EspError, ledc_channel_t, nvs_flash_init};

// Custom Imports
use servo::Servo;
#[allow(unused_imports)]
use esp_idf_sys as _;


#[toml_cfg::toml_config]
pub struct Config {
    #[default("")]
    wifi_ssid: &'static str,
    #[default("")]
    wifi_psk: &'static str,
}

// Set a constant CONTROL_SIGNAL_SIZE
const VERSION_MIN: u32 = 5;
const VERSION_MAJ: u32 = 0;
const READ_TIMEOUT: Duration = Duration::from_millis(1000); // 1 second
const MAX_CONTROL_SIGNAL_SIZE: usize = 11;

// VALUES FOR SERVOS
const HOBBY_FANS_MIN_DUTY: f32 = 0.0275;
const HOBBY_FANS_MAX_DUTY: f32 = 0.125;

const MIUZEI_MIN_DUTY: f32 = 0.018;
const MIUZEI_MAX_DUTY: f32 = 0.11;


const MIUZEI_MINI_MIN_DUTY: f32 = 0.018;
const MIUZEI_MINI_MAX_DUTY: f32 = 0.11;

// Control bytes

fn main() -> Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_hal::sys::link_patches();

    esp_idf_svc::log::EspLogger::initialize_default();
    // Initialize NVS, unsure if we will need this in future
    init_nvs()?;


    // get peripherals
    let peripherals: Peripherals = Peripherals::take().unwrap();

    // get system event loop
    let sysloop = EspSystemEventLoop::take()?;


    // Connect to WiFi
    info!("Socket initialize");
    let _wifi = wifi_setup::wifi(
        CONFIG.wifi_ssid,
        CONFIG.wifi_psk,
        peripherals.modem,
        sysloop,
        6,
    )?;

    let socket = wifi_setup::init_socket(None);
    info!("Socket initialized");

    let _mdns = wifi_setup::init_mdns();
    info!("mDNS initialized");


    // Set up pins for i2c, and i2c port
    let i2c = peripherals.i2c0;
    let sda = peripherals.pins.gpio21;
    let scl = peripherals.pins.gpio22;

    // Set up the i2c driver
    let mut display_connected = false;
    let config = I2cConfig::new().baudrate(100.kHz().into());
    let i2c_driver_result = I2cDriver::new(i2c, sda, scl, &config);
    let i2c_driver_option: Option<I2cDriver> = match i2c_driver_result {
        Ok(driver) => {
            display_connected = true;
            Some(driver)
        },
        Err(e) => {
            error!("Failed to initialize I2C driver: {:?}", e);
            None
        }
    };

    if let Some(i2c_driver) = i2c_driver_option {
        info!("I2C driver initialized successfully");
        let interface = I2CDisplayInterface::new(i2c_driver);
        let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0).into_buffered_graphics_mode();

        match display.init() {
            Ok(_) => {
                // Initialization succeeded
            },
            Err(e) => {
                error!("Error initializing display: {:?}", e);
            }
        }

        //Clear the display
        match display.clear(BinaryColor::Off) {
            Ok(_) => {
                // Clear succeeded
            },
            Err(e) => {
                error!("Error clearing display: {:?}", e);
            }
        };

        let text_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X10)
            .text_color(BinaryColor::On)
            .build();

        let to_oled: String = format!("Robotic Limb V{}.{}\nIP Address: \n{}", VERSION_MAJ, VERSION_MIN,_wifi.sta_netif().get_ip_info()?.ip);


        Text::new(&to_oled, Point::new(0, 7), text_style)
            .draw(&mut display)
            .unwrap();

        match display.flush() {
            Ok(_) => info!("Display flushed successfully"),
            Err(e) => error!("Failed to flush display: {:?}", e),

        }
        // End of display related code
    }

    let mut data: Vec<u8> = Vec::new();
    let mut from_addr: std::net::SocketAddr;


    let ledc_driver = match LedcTimerDriver::new(
                                    peripherals.ledc.timer0,
                                    &config::TimerConfig::new().resolution(esp_idf_hal::ledc::Resolution::Bits12).frequency(50.Hz().into()),
                                   ) {
                                    Ok(driver) => driver,
                                    Err(e) => panic!("LEDc Timer driver failed to initialise: {}", e), // Serious issue if ledc driver cannot be initialised
                                   };


    let mut servos: Vec<Servo> = Vec::new();

    create_and_add_servo("Top", peripherals.ledc.channel0, &ledc_driver, peripherals.pins.gpio15, &mut servos, HOBBY_FANS_MIN_DUTY, HOBBY_FANS_MAX_DUTY, 180);
    create_and_add_servo("Shoulder", peripherals.ledc.channel1, &ledc_driver, peripherals.pins.gpio16, &mut servos, HOBBY_FANS_MIN_DUTY, HOBBY_FANS_MAX_DUTY, 180);
    create_and_add_servo("Upper Arm", peripherals.ledc.channel2, &ledc_driver, peripherals.pins.gpio17, &mut servos, HOBBY_FANS_MIN_DUTY, HOBBY_FANS_MAX_DUTY, 180);
    create_and_add_servo("Elbow", peripherals.ledc.channel3, &ledc_driver, peripherals.pins.gpio18, &mut servos, HOBBY_FANS_MIN_DUTY, HOBBY_FANS_MAX_DUTY, 180);
    create_and_add_servo("Lower Arm", peripherals.ledc.channel4, &ledc_driver, peripherals.pins.gpio19, &mut servos, HOBBY_FANS_MIN_DUTY, HOBBY_FANS_MAX_DUTY, 180);



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
                servos[0].set_angle(u16::from_be_bytes([data[1], data[2]]));
                servos[1].set_angle(u16::from_be_bytes([data[3], data[4]]));
                servos[2].set_angle(u16::from_be_bytes([data[5], data[6]]));
                servos[3].set_angle(u16::from_be_bytes([data[7], data[8]]));
                servos[4].set_angle(u16::from_be_bytes([data[9], data[10]]));

                info!("Servo Positions:");
                for servo in &servos {
                    info!("{}: {}", servo.get_name(), servo.get_angle());
                }

                // Send an ack
                let ack: Vec<u8> = vec![0u8];
                socket.send_to(&ack, from_addr)?;

                // Code to send acknowledgement (TEST CODE)
                timer.counter()?;
                timer.enable(true)?;
            },
            1 => {
                info!("Received Poll Signal");
                info!("Sending back to {}", from_addr);
                let mut poll: Vec<u8> = Vec::new();

                socket.send_to(&poll, from_addr)?;
            },
            _ => {
                error!("Not a valid command");
            },
        }
    }
}




// Safe wrapper for nvs_flash_init()
fn init_nvs() -> Result<(), EspError> {
    let err = unsafe { nvs_flash_init() };
    esp!(err)?;
    Ok(())
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

