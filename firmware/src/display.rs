use core::u32;

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
pub enum InternalStatus {
    Error,
    #[default]
    Good,
}

#[derive(Default, Copy, Clone)]
pub struct Contents {
    /// The current temperature
    pub temperature: f32,
    /// The temperature change in dC/s
    pub avg_short: Change,
    /// The temperature change in dC/s
    pub avg_long: Change,
    /// The current time.
    pub time: u32,
    /// The internal status (reading success etc)
    pub status: InternalStatus,
}
impl Contents {
    pub fn test_contents() -> Self {
        Self {
            temperature: -1337.0000,
            avg_short: Change {
                duration: 2000,
                temperature_delta: 21.347,
            },
            avg_long: Change {
                duration: 9000,
                temperature_delta: -41.347,
            },
            time: 3600 * 1000 * 10,
            status: InternalStatus::Error,
        }
    }
}

#[derive(Copy, Clone, Default, PartialEq)]
pub struct Measurement {
    pub time: u32,
    pub temperature: f32,
}
impl core::fmt::Debug for Measurement {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{:?}", (self.time, self.temperature)))
    }
}
#[derive(Copy, Clone, Default, Debug, PartialEq)]
pub struct Average {
    buffer: [Measurement; 32],
    index: usize,
}
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct AverageIter<'a> {
    average: &'a Average,
    our_index: usize,
}
impl<'a> Iterator for AverageIter<'a> {
    type Item = Measurement;

    fn next(&mut self) -> Option<Self::Item> {
        let next_index = self
            .our_index
            .wrapping_sub(1)
            .rem_euclid(self.average.buffer.len());
        if next_index == self.average.index {
            None
        } else {
            self.our_index = next_index;
            self.average.buffer.get(next_index).copied()
        }
    }
}
#[derive(Copy, Clone, Default, Debug, PartialEq)]
pub struct Change {
    duration: u32,
    temperature_delta: f32,
}
impl Change {
    pub fn to_rate(&self) -> f32 {
        if self.duration != 0 {
            self.temperature_delta / (self.duration as f32 / 1000.0)
        } else {
            0.0
        }
    }
    pub fn duration_ms(&self) -> u32 {
        self.duration
    }
    pub fn duration_s(&self) -> u32 {
        self.duration / 1000
    }
    pub fn from_measurement(now: Measurement, old: Measurement) -> Self {
        let duration = now.time - old.time;
        let temperature_delta = now.temperature - old.temperature;
        Change {
            duration,
            temperature_delta,
        }
    }
}

impl Average {
    pub fn add_measurement(&mut self, time: u32, temperature: f32) {
        self.buffer[self.index].time = time;
        self.buffer[self.index].temperature = temperature;
        self.index = (self.index + 1) % self.buffer.len();
    }
    pub fn get_average(&mut self, dt: u32) -> Change {
        let mut iter = self.iter();
        let now = if let Some(first) = iter.next() {
            first
        } else {
            return Default::default();
        };
        let mut longest: Change = Default::default();
        for m in iter {
            longest = Change::from_measurement(now, m);

            if longest.duration_ms() < dt {
                continue; // keep advancing
            } else {
                return longest;
            }
        }
        longest
    }
    /// Iterate over measurements, with least old first.
    pub fn iter(&self) -> AverageIter {
        AverageIter {
            average: self,
            our_index: self.index,
        }
    }

    pub fn buffer(&self) -> &[Measurement] {
        &self.buffer
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_average_ring() {
        let mut avg = Average::default();
        for i in 0..avg.buffer.len() {
            avg.add_measurement(i as u32, i as f32);
        }
        println!("avg: {avg:?}");
        let mut iter = avg.iter();
        for i in 1..avg.buffer.len() + 3 {
            if i < avg.buffer.len() {
                assert_eq!(
                    iter.next(),
                    Some(Measurement {
                        time: (avg.buffer.len() - i) as u32,
                        temperature: (avg.buffer.len() - i) as f32
                    })
                );
            } else {
                assert_eq!(iter.next(), None,);
            }
        }

        let change = avg.get_average(3);
        assert_eq!(change.duration_ms(), 3);
        assert_eq!(change.to_rate(), ((3.0) / (3.0 / 1000.0)));

        let mut avg = Average::default();
        avg.add_measurement(0, 0.0);
        avg.add_measurement(0, 0.0);
        avg.add_measurement(0, 0.0);
        assert_eq!(avg.index, 3);
        for i in 0..avg.buffer.len() {
            avg.add_measurement(i as u32, i as f32);
        }
        let change = avg.get_average(3);
        assert_eq!(change.duration_ms(), 3);
        assert_eq!(change.to_rate(), ((3.0) / (3.0 / 1000.0)));
        let mut iter = avg.iter();
        for i in 1..avg.buffer.len() + 3 {
            if i < avg.buffer.len() {
                assert_eq!(
                    iter.next(),
                    Some(Measurement {
                        time: (avg.buffer.len() - i) as u32,
                        temperature: (avg.buffer.len() - i) as f32
                    })
                );
            } else {
                assert_eq!(iter.next(), None,);
            }
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
                crate::util::StackString::from_format(format_args!("T: {: >11.4} C", c.temperature))
            },
        };

        let render_change = RenderSpec {
            position: Point::new(0, text_style_big.font.character_size.height as i32 + 2),
            style: &text_style,
            style_off: &text_style_off,
            content: |c: &Contents| {
                crate::util::StackString::from_format(format_args!(
                    "dT/{}s {: >5.2} dT/{}s {: >5.2}",
                    c.avg_long.duration_s(),
                    c.avg_long.to_rate(),
                    c.avg_short.duration_s(),
                    c.avg_short.to_rate(),
                ))
            },
        };

        let render_time = RenderSpec {
            position: Point::new(
                30,
                render_change.position.y + text_style.font.character_size.height as i32 + 2,
            ),
            style: &text_style,
            style_off: &text_style_off,
            content: |c: &Contents| {
                crate::util::StackString::from_format(format_args!(
                    "t:  {: >10.3} s",
                    c.time as f32 / 1000.0
                ))
            },
        };

        let render_status = RenderSpec {
            position: Point::new(
                0,
                render_change.position.y + text_style.font.character_size.height as i32 + 2,
            ),
            style: &text_style,
            style_off: &text_style_off,
            content: |c: &Contents| {
                Ok(crate::util::StackString::from_str(match c.status {
                    InternalStatus::Good => "ok",
                    InternalStatus::Error => "fail",
                }))
            },
        };

        for r in [render_temp, render_change, render_time, render_status] {
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
