use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::Point;
use embedded_graphics::mono_font::{MonoTextStyle, MonoTextStyleBuilder};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::text::Text;
use esp_idf_hal::i2c::{I2cDriver};
use log::{error, info};
use ssd1306::mode::{BufferedGraphicsMode, DisplayConfig};
use ssd1306::prelude::{DisplaySize128x64, I2CInterface};
use ssd1306::rotation::DisplayRotation;
use ssd1306::{I2CDisplayInterface, Ssd1306};
use embedded_graphics::Drawable;


pub struct Display<'a>{
    text: String,
    display: Ssd1306<I2CInterface<I2cDriver<'static>>, DisplaySize128x64, BufferedGraphicsMode<DisplaySize128x64>>,
    text_style: MonoTextStyle<'a, BinaryColor>,
}

impl<'a> Display<'a>{
    pub fn new(driver: I2cDriver<'static>) -> Display{
        Display{
            text: "".to_string(),
            display: Ssd1306::new(I2CDisplayInterface::new(driver), DisplaySize128x64, DisplayRotation::Rotate0).into_buffered_graphics_mode(),
            text_style: MonoTextStyleBuilder::new()
                .font(&embedded_graphics::mono_font::ascii::FONT_6X10)
                .text_color(BinaryColor::On).build(),
        }
    }
    pub fn set_text(&mut self, text: String){
        self.text = text;
    }

    pub fn get_text(&self) -> &str{
        &self.text
    }
    pub fn set_text_style(&mut self, text_style: MonoTextStyle<'a, BinaryColor>) {
        self.text_style = text_style;
    }
    pub fn draw(&mut self){
        self.clear();
        match Text::new(&self.text, Point::new(0, 7), self.text_style)
            .draw(&mut self.display){
                Ok(_) => {
                    info!("Drew {}", self.text);
                },
                Err(e) => {
                    error!("Error drawing text: {:?}", e);
                }
        }

    }
    pub fn flush(&mut self){
        match self.display.flush(){
            Ok(_) => {
                info!("Flushed display");
            },
            Err(e) => {
                error!("Error flushing display: {:?}", e);
            }
        }
    }
    pub fn init(&mut self){
        match self.display.init() {
            Ok(_) => {
                info!("Display initialised");
            },
            Err(e) => {
                error!("Error initializing display: {:?}", e);
            }
        }
        self.clear();
    }
    pub fn clear(&mut self){
        match self.display.clear(BinaryColor::Off) {
            Ok(_) => {
                info!("Display cleared");
            },
            Err(e) => {
                error!("Error clearing display: {:?}", e);
            }
        };
    }
}

