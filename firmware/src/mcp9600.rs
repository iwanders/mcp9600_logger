//use cortex_m_semihosting::hprintln;
use embedded_hal::i2c::I2c;

pub const ADAFRUIT_MCP9600_ADDR: u8 = 0x67;
pub struct TemperatureSensorDriver<I2C> {
    i2c: I2C,
    address: u8,
}

fn hot_junction_to_temp(upper: u8, lower: u8) -> f32 {
    let v = i16::from_be_bytes([upper, lower]);
    v as f32 * 0.0625
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_temp_conversion() {
        // Some manual temperatures.
        assert_eq!(
            hot_junction_to_temp(0b0100_0001, 0b0000_0001),
            1024.0 + 16.0 + 0.0625
        );
        assert_eq!(
            hot_junction_to_temp(0b0100_0001, 0b1000_0001),
            1024.0 + 16.0 + 8.0 + 0.0625
        );

        assert_eq!(hot_junction_to_temp(2, 80), 37.0);
        assert_eq!(hot_junction_to_temp(2, 0x07), 32.4375);
        // Tested with a cool pack, temperature change is gradual and as expected.
        assert_eq!(hot_junction_to_temp(0xff, 0xfc), -0.25);
        assert_eq!(hot_junction_to_temp(0xff, 0xd0), -3.0);
        assert_eq!(hot_junction_to_temp(0xff, 0xc0), -4.0);
        assert_eq!(hot_junction_to_temp(0x00, 0x50), 5.0);
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
#[derive(Copy, Clone)]
pub struct HotJunctionRegister {
    /// Upper value for the temperature.
    pub upper: u8,
    /// Lower value for the temperature.
    pub lower: u8,
}
impl HotJunctionRegister {
    pub fn from_u8(upper: u8, lower: u8) -> Self {
        Self { upper, lower }
    }
    pub fn as_f32(&self) -> f32 {
        hot_junction_to_temp(self.upper, self.lower)
    }
}
impl core::fmt::Debug for HotJunctionRegister {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "0x{:0>2x} 0x{:0>2x}", self.upper, self.lower)
    }
}

pub const REG_DEVICE_ID: u8 = 0b0010_0000;
pub const REG_SENSOR_CONFIG: u8 = 0b0000_0101;
pub const REG_HOT_JUNCTION: u8 = 0b0000_0000;
pub const REG_STATUS: u8 = 0b0000_0100;

impl<I2C: I2c> TemperatureSensorDriver<I2C> {
    pub fn new(i2c: I2C, address: u8) -> Self {
        Self { i2c, address }
    }
    fn write_read(&mut self, w: &[u8], r: &mut [u8]) -> Result<(), I2C::Error> {
        self.i2c.write_read(self.address as u8, w, r)
    }
    fn write(&mut self, w: &[u8]) -> Result<(), I2C::Error> {
        self.i2c.write(self.address as u8, w)
    }

    /// Read the device ID, depends on chip version, but 64 for MCP9600.
    pub fn read_device_id(&mut self) -> Result<u8, I2C::Error> {
        let mut tmp = [0u8, 0u8];
        self.write_read(&[REG_DEVICE_ID], &mut tmp)?;
        Ok(tmp[0])
    }

    /// Seems to initialise with 0x00 (K type, no filtering), no need to change this.
    pub fn read_sensor_configuration(&mut self) -> Result<u8, I2C::Error> {
        let mut tmp = [0u8, 0u8];
        self.write_read(&[REG_SENSOR_CONFIG], &mut tmp)?;
        Ok(tmp[0])
    }

    /// Read the hot junction value for the most recent conversion.
    pub fn read_hot_junction(&mut self) -> Result<HotJunctionRegister, I2C::Error> {
        let mut tmp = [0u8, 0u8];
        self.write_read(&[REG_HOT_JUNCTION], &mut tmp)?;
        Ok(HotJunctionRegister::from_u8(tmp[0], tmp[1]))
    }

    /// Read the status register.
    pub fn read_status(&mut self) -> Result<StatusRegister, I2C::Error> {
        let mut tmp = [0u8];
        self.write_read(&[REG_STATUS], &mut tmp)?;
        Ok(StatusRegister::from_u8(tmp[0]))
    }

    /// Clear the status register bits.
    pub fn clear_status(&mut self) -> Result<(), I2C::Error> {
        self.write(&[REG_STATUS, 0])
    }
}
