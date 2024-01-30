use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::Point;
use embedded_graphics::mono_font::{MonoTextStyle, MonoTextStyleBuilder};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::text::Text;
use esp_idf_hal::i2c::{I2cDriver};
use log::{error};
use ssd1306::mode::{BufferedGraphicsMode, DisplayConfig};
use ssd1306::prelude::{DisplaySize128x64, I2CInterface};
use ssd1306::{Ssd1306};
use embedded_graphics::Drawable;
use embedded_graphics::mono_font::ascii::FONT_6X10;

pub struct Display<'a>{
    display: Ssd1306<I2CInterface<I2cDriver<'static>>, DisplaySize128x64, BufferedGraphicsMode<DisplaySize128x64>>,
    text_style: MonoTextStyle<'a, BinaryColor>,
}

impl<'a> Display<'a>{
    pub fn new(display: Ssd1306<I2CInterface<I2cDriver<'static>>, DisplaySize128x64, BufferedGraphicsMode<DisplaySize128x64>>) -> Display<'a> {
    Display{
            display,
            text_style: MonoTextStyleBuilder::new()
                .font(&FONT_6X10)
                .text_color(BinaryColor::On)
                .build(),
        }
    }

    pub fn set_text_style(&mut self, text_style: MonoTextStyle<'a, BinaryColor>) {
        self.text_style = text_style;
    }

    pub fn draw_new_text(&mut self, x: i32, y: i32, text: &String){
        match self.display.clear(BinaryColor::Off) {
            Ok(_) => {},
            Err(e) => {
                error!("Error clearing display: {:?}", e);
            }
        };
        match Text::new(text.as_str(), Point::new(x, y), self.text_style)
            .draw(&mut self.display) {
            Ok(_) => {},
            Err(e) => error!("Error drawing text: {:?}", e),
        };
        match self.display.flush(){
            Ok(_) => {},
            Err(e) => error!("Error flushing display: {:?}", e),
        };
    }

    pub fn init(&mut self){
        match self.display.init() {
            Ok(_) => {},
            Err(e) => {
                error!("Error initializing display: {:?}", e);
            }
        }
    }
}

