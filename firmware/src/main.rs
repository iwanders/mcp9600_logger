//! Blinks an LED
//!
//! This assumes that a LED is connected to pc13 as is the case on the blue pill board.
//!
//! Note: Without additional hardware, PC13 should not be used to drive an LED, see page 5.1.2 of
//! the reference manual for an explanation. This is not an issue on the blue pill.

// MCP9600; Default I2C address is 0x67.
// Following the Start condition, the host must transmit an
// 8-bit address byte to the MCP960X/L0X/RL0X
#![deny(unsafe_code)]
#![no_std]
#![no_main]

use panic_halt as _;

use cortex_m_semihosting::hprintln;
use nb::block;

use cortex_m::asm::delay;
use cortex_m_rt::entry;
use stm32f1xx_hal::{pac, prelude::*, rcc, timer::Timer};

use stm32f1xx_hal::usb::{Peripheral, UsbBus};

use usb_device::prelude::*;
use usbd_serial::{SerialPort, USB_CLASS_CDC};

use stm32f1xx_hal::i2c::{BlockingI2c, DutyCycle, Mode};
mod mcp9600 {
    use embedded_hal::i2c::I2c;

    pub const ADAFRUIT_MCP9600_ADDR: u8 = 0x67;
    //pub const ADAFRUIT_MCP9600_ADDR: u8 = 0x60;
    pub struct TemperatureSensorDriver<I2C> {
        i2c: I2C,
        address: u8,
    }

    impl<I2C: I2c> TemperatureSensorDriver<I2C> {
        pub fn new(i2c: I2C, address: u8) -> Self {
            Self { i2c, address }
        }

        pub fn read_device_id(&mut self) -> Result<u8, I2C::Error> {
            let mut tmp = [0u8, 0u8];
            self.i2c
                .write_read(self.address as u8, &[0b00100000], &mut tmp)?;
            Ok(tmp[1])
        }
    }
}

#[entry]
fn main() -> ! {
    // Get access to the core peripherals from the cortex-m crate
    let mut cp = cortex_m::Peripherals::take().unwrap();
    // Get access to the device specific peripherals from the peripheral access crate
    let dp = pac::Peripherals::take().unwrap();

    //let mut cortex_peripherals = cortex_m::Peripherals::take().unwrap();

    cp.DWT.enable_cycle_counter();

    // Take ownership over the raw flash and rcc devices and convert them into the corresponding
    // HAL structs
    let mut flash = dp.FLASH.constrain();
    let rcc = dp.RCC.constrain();

    // Freeze the configuration of all the clocks in the system and store the frozen frequencies in
    // `clocks`
    // Set a real clock that allows usb.

    let mut rcc = rcc.freeze(
        rcc::Config::hse(8.MHz()).sysclk(48.MHz()).pclk1(6.MHz()),
        &mut flash.acr,
    );

    assert!(rcc.clocks.usbclk_valid());

    // Acquire the GPIOC peripheral
    let mut gpioc = dp.GPIOC.split(&mut rcc);
    // Configure gpio C pin 13 as a push-pull output. The `crh` register is passed to the function
    // in order to configure the port. For pins 0-7, crl should be passed instead.
    let mut led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);
    // Configure the syst timer to trigger an update every second
    /**/
    let mut timer = Timer::syst(cp.SYST, &rcc.clocks).counter_hz();
    timer.start(10.Hz()).unwrap();

    // Wait for the timer to trigger an update and change the state of the LED

    //---
    //
    let mut gpioa = dp.GPIOA.split(&mut rcc);

    // BluePill board has a pull-up resistor on the D+ line.
    // Pull the D+ pin down to send a RESET condition to the USB bus.
    // This forced reset is needed only for development, without it host
    // will not reset your device when you upload new firmware.
    let mut usb_dp = gpioa.pa12.into_push_pull_output(&mut gpioa.crh);
    usb_dp.set_low();
    delay(rcc.clocks.sysclk().raw() / 100);

    let usb = Peripheral {
        usb: dp.USB,
        pin_dm: gpioa.pa11,
        pin_dp: usb_dp.into_floating_input(&mut gpioa.crh),
    };
    let usb_bus = UsbBus::new(usb);

    let mut serial = SerialPort::new(&usb_bus);
    // ---

    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .device_class(USB_CLASS_CDC)
        .strings(&[StringDescriptors::default()
            .manufacturer("Fake Company")
            .product("Serial port")
            .serial_number("TEST")])
        .unwrap()
        .build();

    led.set_high();

    let mut afio = dp.AFIO.constrain(&mut rcc);
    // Acquire the GPIOB peripheral
    let gpiob = dp.GPIOB.split(&mut rcc);

    let scl = gpiob.pb8;
    let sda = gpiob.pb9;

    let i2c = dp
        .I2C1
        .remap(&mut afio.mapr) // add this if want to use PB8, PB9 instead
        .blocking_i2c(
            (scl, sda),
            Mode::Standard {
                frequency: 100.kHz(),
                //duty_cycle: DutyCycle::Ratio16to9,
            },
            &mut rcc,
            1000,
            10,
            1000,
            1000,
        );
    delay(rcc.clocks.sysclk().raw() / 100);

    let mut mcp = mcp9600::TemperatureSensorDriver::new(i2c, mcp9600::ADAFRUIT_MCP9600_ADDR);

    let devid = mcp.read_device_id();
    hprintln!("device id: {:?}", devid);

    loop {
        if !usb_dev.poll(&mut [&mut serial]) {
            continue;
        }
        led.set_low(); // Turn on

        let mut buf = [0u8; 64];

        match serial.read(&mut buf) {
            Ok(count) if count > 0 => {
                led.set_low(); // Turn on
                hprintln!("got data");
                // Echo back in upper case
                for c in buf[0..count].iter_mut() {
                    if 0x61 <= *c && *c <= 0x7a {
                        *c &= !0x20;
                    }
                }

                let mut write_offset = 0;
                while write_offset < count {
                    match serial.write(&buf[write_offset..count]) {
                        Ok(len) if len > 0 => {
                            write_offset += len;
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }

        led.set_high(); // Turn off
    }
}
