//use cortex_m_semihosting::hprintln;
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

/// Status register.
#[derive(Debug, Copy, Clone)]
pub struct StatusRegister {
    /// Set if first burst is complete.
    pub burst_complete: bool,
    /// Set if temperature conversion is complete.
    pub conversion_complete: bool,
    /// True if shorted to Vss or Vdd, requires VSense and MCP9601.
    pub short_circuit: bool,
    /// Set if outside of range.
    pub out_of_range: bool,
    /// Tx > Talert. Index 0 maps to Alert1, Index 1 maps to Alert2,...
    pub alerts: [bool; 4],
}
impl StatusRegister {
    pub fn from_u8(v: u8) -> Self {
        Self {
            burst_complete: (v & 0b1000_0000) != 0,
            conversion_complete: (v & 0b0100_0000) != 0,
            short_circuit: (v & 0b0010_0000) != 0,
            out_of_range: (v & 0b0001_0000) != 0,
            alerts: [
                (v & 0b0000_0001) != 0,
                (v & 0b0000_0010) != 0,
                (v & 0b0000_0100) != 0,
                (v & 0b0000_1000) != 0,
            ],
        }
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
    pub fn read_hot_junction_raw(&mut self) -> Result<(u8, u8), I2C::Error> {
        let mut tmp = [0u8, 0u8];
        self.i2c
            .write_read(self.address as u8, &[0b0000_0000], &mut tmp)?;
        Ok((tmp[1], tmp[0]))
    }

    pub fn read_hot_junction(&mut self) -> Result<f32, I2C::Error> {
        let mut tmp = [0u8, 0u8];
        self.i2c
            .write_read(self.address as u8, &[0b0000_0000], &mut tmp)?;
        Ok(hot_junction_to_temp(tmp[1], tmp[0]))
    }

    // Status bit;
    // TH Update: Temperature Update Flag bit
    // 1 = Temperature conversion complete
    // 0 = Writing ‘0’ has no effect
    // This bit is normally set. User can clear it and poll the bit until the next temperature conversion is complete.
    pub fn read_status(&mut self) -> Result<StatusRegister, I2C::Error> {
        let mut tmp = [0u8];
        self.i2c
            .write_read(self.address as u8, &[0b0000_0100], &mut tmp)?;
        Ok(StatusRegister::from_u8(tmp[0]))
    }

    pub fn clear_status(&mut self) -> Result<(), I2C::Error> {
        self.i2c.write(self.address as u8, &[0b0000_0100, 0])?;
        Ok(())
    }
}
