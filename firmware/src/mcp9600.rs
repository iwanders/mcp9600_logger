use cortex_m_semihosting::hprintln;
use embedded_hal::i2c::I2c;

pub const ADAFRUIT_MCP9600_ADDR: u8 = 0x67;
pub struct TemperatureSensorDriver<I2C> {
    i2c: I2C,
    address: u8,
}

fn hot_junction_to_temp(upper: u8, lower: u8) -> f32 {
    let v = i16::from_le_bytes([upper, lower]);
    v as f32 * 0.0625
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_temp_conversion() {
        assert_eq!(
            hot_junction_to_temp(0b0000_0001, 0b0100_0001),
            1024.0 + 16.0 + 0.0625
        );
        assert_eq!(
            hot_junction_to_temp(0b1000_0001, 0b0100_0001),
            1024.0 + 16.0 + 8.0 + 0.0625
        );

        assert_eq!(hot_junction_to_temp(80, 2), 37.0);
    }
}

impl<I2C: I2c> TemperatureSensorDriver<I2C> {
    pub fn new(i2c: I2C, address: u8) -> Self {
        Self { i2c, address }
    }

    pub fn read_device_id(&mut self) -> Result<u8, I2C::Error> {
        let mut tmp = [0u8, 0u8];
        self.i2c
            .write_read(self.address as u8, &[0b0010_0000], &mut tmp)?;
        Ok(tmp[0])
    }

    /// Seems to initialise with 0x00 (K type, no filtering).
    pub fn read_sensor_configuration(&mut self) -> Result<u8, I2C::Error> {
        let mut tmp = [0u8, 0u8];
        self.i2c
            .write_read(self.address as u8, &[0b0000_0101], &mut tmp)?;
        Ok(tmp[0])
    }
    pub fn read_hot_junction(&mut self) -> Result<f32, I2C::Error> {
        let mut tmp = [0u8, 0u8];
        self.i2c
            .write_read(self.address as u8, &[0b0000_0000], &mut tmp)?;
        Ok(hot_junction_to_temp(tmp[1], tmp[0]))
    }
}
