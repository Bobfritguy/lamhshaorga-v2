use esp_idf_hal::ledc::LedcDriver;
use log::{error, info};

pub struct Servo {
    name: String,
    driver: LedcDriver<'static>,
    angle: u16,
    min_duty: u32,
    max_duty: u32,
    duty_interval: u32,
    max_angle_degrees: u16,
}

impl Servo {

    pub fn new(name: String, mut driver: LedcDriver<'static>, min_percent: f32, max_percent: f32, max_angle_degrees: u16) -> Servo {
        match driver.set_duty(0) {
            Ok(_) => info!("{} initialised", name),
            Err(e) => error!("{} not initialised: {}", name, e),
        }
        let max_duty = driver.get_max_duty() as f32;
        let min_duty = (max_duty * min_percent).round() as u32;
        let max_duty = (max_duty * max_percent).round() as u32;
        Servo {
            name,
            driver,
            angle: 0,
            min_duty,
            max_duty,
            duty_interval: max_duty - min_duty,
            max_angle_degrees,
        }
    }

    pub fn set_angle(&mut self, angle: u16){
        let duty = self.get_servo_duty(angle);
        match self.driver.set_duty(duty) {
            Ok(_) => info!("{} set to {} degrees, duty {} of {}", self.name, angle, duty, self.driver.get_max_duty()),
            Err(e) => error!("Failed to change angle of {}: {}", self.name, e),
        }
    }

    fn get_servo_duty(&self, angle: u16) -> u32 {
        let percentage = (angle as f32 / self.max_angle_degrees as f32);

        (self.duty_interval as f32 * percentage).round() as u32 + self.min_duty
    }

    pub fn get_angle(&self) -> u16 {
        self.angle
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

}
