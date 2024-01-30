use esp_idf_hal::ledc::LedcDriver;
use log::{error, info};

pub struct Servo {
    name: String,
    driver: LedcDriver<'static>,
    angle: u16,
    goal: u16,
    deg_s: u16,
    min_angle_duty: u32,
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
        let min_angle_duty = (max_duty * min_percent).round() as u32;
        let max_angle_duty = (max_duty * max_percent).round() as u32;
        Servo {
            name,
            driver,
            angle: 0,
            goal: 0,
            deg_s: 2,
            min_angle_duty,
            duty_interval: max_angle_duty - min_angle_duty,
            max_angle_degrees,
        }
    }

    pub fn set_angle(&mut self, goal: u16){
        self.goal = goal;
        self.angle = goal;
        let duty = self.get_servo_duty(goal);
        match self.driver.set_duty(duty) {
            Ok(_) => {},
            Err(e) => error!("Failed to change duty of {}: {}", self.name, e),
        }
    }

    fn get_servo_duty(&self, angle: u16) -> u32 {
        let percentage = angle as f32 / self.max_angle_degrees as f32;

        (self.duty_interval as f32 * percentage).round() as u32 + self.min_angle_duty
    }

    pub fn set_duty(&mut self, duty: u16) {
        match self.driver.set_duty(duty as u32) {
            Ok(_) => {},
            Err(e) => error!("Failed to change duty of {}: {}", self.name, e),
        }
    }

    pub fn stop(&mut self) {
        match self.driver.disable() {
            Ok(_) => {},
            Err(e) => error!("Failed to stop {}: {}", self.name, e),
        }
    }

    pub fn poll(&mut self) {
        // if self.angle != self.goal {
        //     let mut new_angle = self.angle;
        //     if self.angle < self.goal {
        //         new_angle += self.deg_s;
        //         if new_angle > self.goal {
        //             new_angle = self.goal;
        //         }
        //     } else {
        //         new_angle -= self.deg_s;
        //         if new_angle < self.goal {
        //             new_angle = self.goal;
        //         }
        //     }
        //     self.angle = new_angle;
        //     let duty = self.get_servo_duty(self.angle);
        //     match self.driver.set_duty(duty) {
        //         Ok(_) => {},
        //         Err(e) => error!("Failed to change duty of {}: {}", self.name, e),
        //     }
        // }
        // TODO: Make it not a stub
    }

    pub fn get_angle(&self) -> u16 {
        self.angle
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn to_string(&self) -> String {
        format!("{}: {}°", self.name, self.angle)
    }
}
