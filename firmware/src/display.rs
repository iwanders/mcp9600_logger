use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::mono_font::ascii::FONT_9X15;
use embedded_graphics::mono_font::iso_8859_9::{FONT_5X7, FONT_8X13_BOLD};
use embedded_graphics::mono_font::iso_8859_16::FONT_8X13;

use ssd1306::mode::{BasicMode, BufferedGraphicsMode};
use ssd1306::size::DisplaySize128x32;
use ssd1306::{I2CDisplayInterface, Ssd1306, prelude::*};

use embedded_graphics::{
    mono_font::{MonoTextStyleBuilder, ascii::FONT_6X10, ascii::FONT_9X15_BOLD, ascii::FONT_10X20},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text},
};

#[derive(Default, Copy, Clone)]
pub struct Contents {
    /// The current temperature
    pub temperature: f32,
    /// The temperature change in dC/s
    pub temp_change: f32,
    /// The current time.
    pub time: u32,
}
impl Contents {
    pub fn test_contents() -> Self {
        Self {
            temperature: -1337.0000,
            temp_change: 123.123,
            time: 3600 * 1000 * 10,
        }
    }
}

type Size = ssd1306::size::DisplaySize128x32;

use crate::display_buffer::DeltaBuffer;

pub struct Display<DI: WriteOnlyDataCommand> {
    display: Ssd1306<DI, Size, BasicMode>,
    buffer: DeltaBuffer<Size>,
    contents: Contents,
    old_contents: Contents,
}
impl<DI: WriteOnlyDataCommand> Display<DI> {
    pub fn new(disp_int: DI) -> Self
    where
        DI: WriteOnlyDataCommand + Sized,
    {
        Self {
            display: Ssd1306::new(disp_int, Size {}, DisplayRotation::Rotate0),
            buffer: DeltaBuffer::<Size>::new(),
            old_contents: Default::default(),
            contents: Default::default(),
        }
    }
    pub fn init(&mut self) -> bool {
        if let Ok(()) = self.display.init() {
            self.buffer.clear_buffer();
            if let Ok(()) = self.buffer.flush(&mut self.display) {
                return true;
            }
        }
        false
    }

    pub fn contents_mut(&mut self) -> &mut Contents {
        &mut self.contents
    }

    pub fn update(&mut self, contents: &Contents) -> Result<(), display_interface::DisplayError> {
        //self.buffer.clear_buffer();
        let text_style = MonoTextStyleBuilder::new()
            .font(&FONT_5X7)
            .text_color(BinaryColor::On)
            .build();

        let text_style_off = MonoTextStyleBuilder::new()
            .font(&FONT_5X7)
            .text_color(BinaryColor::Off)
            .build();

        let text_style_big = MonoTextStyleBuilder::new()
            .font(&FONT_8X13_BOLD)
            .text_color(BinaryColor::On)
            .build();

        let text_style_big_off = MonoTextStyleBuilder::new()
            .font(&FONT_8X13_BOLD)
            .text_color(BinaryColor::Off)
            .build();

        struct RenderSpec<'a> {
            position: Point,
            style: &'a MonoTextStyle<'a, BinaryColor>,
            style_off: &'a MonoTextStyle<'a, BinaryColor>,
            content: fn(&Contents) -> Result<crate::util::StackString, core::fmt::Error>,
        }
        let render_temp = RenderSpec {
            position: Point::zero(),
            style: &text_style_big,
            style_off: &text_style_big_off,
            content: |c: &Contents| {
                crate::util::StackString::from_format(format_args!("{: >10.4} C", c.temperature))
            },
        };

        let render_change = RenderSpec {
            position: Point::new(0, text_style_big.font.character_size.height as i32 + 2),
            style: &text_style,
            style_off: &text_style_off,
            content: |c: &Contents| {
                crate::util::StackString::from_format(format_args!("dC/dt {: >9.2}", c.temp_change))
            },
        };

        let render_time = RenderSpec {
            position: Point::new(
                0,
                render_change.position.y + text_style.font.character_size.height as i32 + 2,
            ),
            style: &text_style,
            style_off: &text_style_off,
            content: |c: &Contents| {
                crate::util::StackString::from_format(format_args!(
                    "t:  {: >12.3} s",
                    c.time as f32 / 1000.0
                ))
            },
        };

        for r in [render_temp, render_change, render_time] {
            let old_res = (r.content)(&self.old_contents);
            let new_res = (r.content)(&contents);
            if old_res == new_res {
                continue;
            }

            if let Ok(v) = old_res {
                if let Ok(s) = v.as_str() {
                    Text::with_baseline(s, r.position, *r.style_off, Baseline::Top)
                        .draw(&mut self.buffer)
                        .unwrap();
                }
            }
            if let Ok(v) = new_res {
                if let Ok(s) = v.as_str() {
                    Text::with_baseline(s, r.position, *r.style, Baseline::Top)
                        .draw(&mut self.buffer)
                        .unwrap();
                }
            }
        }
        self.old_contents = *contents;
        Ok(())
    }

    pub fn update_target_fill(
        &mut self,
        state: bool,
    ) -> Result<(), display_interface::DisplayError> {
        self.buffer.clear(if state {
            BinaryColor::On
        } else {
            BinaryColor::Off
        })
    }

    pub fn update_partial(&mut self) -> Result<(), display_interface::DisplayError> {
        self.buffer.flush_partial(&mut self.display)
    }
}
