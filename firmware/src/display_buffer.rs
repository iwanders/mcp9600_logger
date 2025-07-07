use core::ops::DerefMut;

use ssd1306::{
    Ssd1306,
    command::AddrMode,
    mode::BasicMode,
    rotation::DisplayRotation,
    size::{DisplaySize, NewZeroed},
};

use display_interface::{DisplayError, WriteOnlyDataCommand};

pub trait DeltaSize {
    const DELTASIZE: usize;
    type Buffer: AsMut<[u8]> + NewZeroed;
}
impl DeltaSize for ssd1306::size::DisplaySize128x32 {
    const DELTASIZE: usize = (
        ((ssd1306::size::DisplaySize128x32::WIDTH as usize
            * ssd1306::size::DisplaySize128x32::HEIGHT as usize)
            / 8)
        // monochrome
    ); // track delta per page.
    type Buffer = [u8; Self::DELTASIZE];
}

pub struct DeltaBuffer<SIZE>
where
    SIZE: DisplaySize + DeltaSize,
{
    dimensions: (u8, u8),
    buffer: <SIZE as DisplaySize>::Buffer,
    /// Buffer to track which pixels are dirty.
    delta: <SIZE as DeltaSize>::Buffer,
    /// The blocks (1 by 8 pixels) to send for each flush_partial.
    blocks_per_update: u8,
    /// The current update position index, advanced with flush_partial.
    update_position: usize,
}

impl<SIZE> DeltaBuffer<SIZE>
where
    SIZE: DisplaySize + DeltaSize,
{
    /// Create a new buffered graphics mode instance.
    pub fn new() -> Self {
        Self {
            dimensions: (SIZE::WIDTH, SIZE::HEIGHT),
            buffer: NewZeroed::new_zeroed(),
            delta: NewZeroed::new_zeroed(),
            blocks_per_update: 10,
            update_position: 0,
        }
    }

    pub fn set_blocks_per_update(&mut self, v: u8) {
        self.blocks_per_update = v;
    }

    fn clear_impl(&mut self, value: bool) {
        self.buffer.as_mut().fill(if value { 0xff } else { 0 });
    }

    /// Clear the underlying framebuffer. You need to call `disp.flush()` for any effect on the screen.
    pub fn clear_buffer(&mut self) {
        self.clear_impl(false);
        self.delta.as_mut().fill(1);
    }

    /// Turn a pixel on or off. A non-zero `value` is treated as on, `0` as off. If the X and Y
    /// coordinates are out of the bounds of the display, this method call is a noop.
    pub fn set_pixel(&mut self, x: u32, y: u32, value: bool) {
        let value = value as u8;

        let (idx, bit) = {
            let idx = ((y as usize) / 8 * SIZE::WIDTH as usize) + (x as usize);
            let bit = y % 8;

            (idx, bit)
        };

        if let Some(byte) = self.buffer.as_mut().get_mut(idx) {
            let previous = *byte;

            // Set pixel value in byte
            // Ref this comment https://stackoverflow.com/questions/47981/how-do-you-set-clear-and-toggle-a-single-bit#comment46654671_47990
            *byte = *byte & !(1 << bit) | (value << bit);

            // If there was a change, mark this index dirty.
            if previous != *byte {
                self.delta.as_mut()[idx] = 1;
            }
        }
    }

    pub fn set_area<DI>(
        &mut self,
        display: &mut Ssd1306<DI, SIZE, BasicMode>,
    ) -> Result<(), DisplayError>
    where
        DI: WriteOnlyDataCommand,
    {
        display.set_draw_area((0, 0), self.dimensions)
    }

    /// Write out data to a display.
    ///
    /// This only updates the parts of the display that have changed since the last flush.
    pub fn flush<DI>(
        &mut self,
        display: &mut Ssd1306<DI, SIZE, BasicMode>,
    ) -> Result<(), DisplayError>
    where
        DI: WriteOnlyDataCommand,
    {
        let (width, height) = self.dimensions;

        for y in (0..(height - 8)).step_by(8) {
            //let mut dirty: bool = false;
            for x in (0..width) {
                let (idx, bit) = {
                    let idx = ((y as usize) / 8 * SIZE::WIDTH as usize) + (x as usize);
                    let bit = y % 8;

                    (idx, bit)
                };
                let dirty = self.delta.as_mut()[idx] != 0;
                self.delta.as_mut()[idx] = 0;

                if dirty {
                    //let idx = ((y as usize) / 8 * SIZE::WIDTH as usize) + (x as usize);
                    let start = (x, y);
                    let end = (x + 1, y | 7);
                    display.set_draw_area(start, end)?;
                    display.bounded_draw(&self.buffer.as_mut(), width as usize, start, end)?;
                }
            }
        }

        Ok(())
    }

    /// Flush a partial number of blocks.
    pub fn flush_partial<DI>(
        &mut self,
        display: &mut Ssd1306<DI, SIZE, BasicMode>,
    ) -> Result<(), DisplayError>
    where
        DI: WriteOnlyDataCommand,
    {
        let (width, height) = self.dimensions;

        // blocks_per_update
        let mut send_blocks = 0;
        for y in (0..(height)).step_by(8) {
            //let mut dirty: bool = false;
            for x in (0..width) {
                let (idx, bit) = {
                    let idx = ((y as usize) / 8 * SIZE::WIDTH as usize) + (x as usize);
                    let bit = y % 8;

                    (idx, bit)
                };

                if idx < self.update_position {
                    continue;
                }

                let dirty = self.delta.as_mut()[idx] != 0;
                self.delta.as_mut()[idx] = 0;

                if dirty {
                    let start = (x, y);
                    let end = (x + 1, y | 7);
                    display.set_draw_area(start, end)?;
                    display.bounded_draw(&self.buffer.as_mut(), width as usize, start, end)?;
                    send_blocks += 1;
                    self.update_position = idx;
                    if send_blocks >= self.blocks_per_update {
                        return Ok(());
                    }
                }
            }
        }
        self.update_position = 0;
        Ok(())
    }
}

use embedded_graphics::{
    Pixel,
    draw_target::DrawTarget,
    geometry::{AnchorX, AnchorY, Dimensions, OriginDimensions, Size},
    pixelcolor::BinaryColor,
    primitives::Rectangle,
};

impl<SIZE> DrawTarget for DeltaBuffer<SIZE>
where
    SIZE: DisplaySize + DeltaSize,
{
    type Color = BinaryColor;
    type Error = DisplayError;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let bb = self.bounding_box();

        pixels
            .into_iter()
            .filter(|Pixel(pos, _color)| bb.contains(*pos))
            .for_each(|Pixel(pos, color)| {
                self.set_pixel(pos.x as u32, pos.y as u32, color.is_on());
            });
        Ok(())
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        let area = area.intersection(&self.bounding_box());
        let mut pixels = pixels.into_iter();

        let buffer = self.buffer.as_mut();

        for y in area.rows() {
            for x in area.columns() {
                let Some(color) = pixels.next() else {
                    return Ok(());
                };
                let value = color.is_on() as u8;
                let idx = ((y as usize) / 8 * SIZE::WIDTH as usize) + (x as usize);
                let bit = y % 8;
                let byte = &mut buffer[idx];
                let previous = *byte;
                *byte = *byte & !(1 << bit) | (value << bit);
                if previous != *byte {
                    self.delta.as_mut()[idx] = 1;
                }
            }
        }

        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.clear_impl(color.is_on());
        Ok(())
    }
}

impl<SIZE> OriginDimensions for DeltaBuffer<SIZE>
where
    SIZE: DisplaySize + DeltaSize,
{
    fn size(&self) -> Size {
        let (w, h) = self.dimensions;

        Size::new(w.into(), h.into())
    }
}
