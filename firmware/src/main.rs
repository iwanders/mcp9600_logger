//! Blinks an LED
//!
//! This assumes that a LED is connected to pc13 as is the case on the blue pill board.
//!
//! Note: Without additional hardware, PC13 should not be used to drive an LED, see page 5.1.2 of
//! the reference manual for an explanation. This is not an issue on the blue pill.

// MCP9600; Default I2C address is 0x67.
// Following the Start condition, the host must transmit an
// 8-bit address byte to the MCP960X/L0X/RL0X

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

use panic_halt as _;

use nb::block;

use cortex_m::asm::{delay, wfi};
use cortex_m_rt::entry;
use stm32f1xx_hal::{
    pac,
    pac::{Interrupt, NVIC, interrupt},
    prelude::*,
    rcc,
    timer::Timer,
};

//use stm32f1xx_hal::usb::{Peripheral, UsbBus};

use usb_device::prelude::*;
use usbd_serial::{SerialPort, USB_CLASS_CDC};
/*
mod mcp9600 {
    use embedded_hal::blocking::i2c::I2c;

    const ADAFRUIT_MCP9600_ADDR: u8 = 0x67;
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
                .write_read(self.address as u8, &[0b0010_0000], &mut tmp)?;
            Ok(tmp[1])
        }
    }
}
*/

use stm32f1xx_hal::usb::{Peripheral, UsbBus, UsbBusType};
use usb_device::bus::UsbBusAllocator;
static mut USB_BUS: Option<UsbBusAllocator<UsbBusType>> = None;
static mut USB_SERIAL: Option<usbd_serial::SerialPort<UsbBusType>> = None;
static mut USB_DEVICE: Option<UsbDevice<UsbBusType>> = None;

#[entry]
fn main() -> ! {
    // Get access to the core peripherals from the cortex-m crate
    let cp = cortex_m::Peripherals::take().unwrap();
    // Get access to the device specific peripherals from the peripheral access crate
    let dp = pac::Peripherals::take().unwrap();

    // Take ownership over the raw flash and rcc devices and convert them into the corresponding
    // HAL structs
    let mut flash = dp.FLASH.constrain();
    let rcc = dp.RCC.constrain();

    // Freeze the configuration of all the clocks in the system and store the frozen frequencies in
    // `clocks`
    // Set a real clock that allows usb.
    let clocks = rcc
        .cfgr
        .use_hse(8.MHz())
        .sysclk(48.MHz())
        .pclk1(24.MHz())
        .freeze(&mut flash.acr);

    // Acquire the GPIOC peripheral
    let mut gpioc = dp.GPIOC.split();
    // Configure gpio C pin 13 as a push-pull output. The `crh` register is passed to the function
    // in order to configure the port. For pins 0-7, crl should be passed instead.
    let mut led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);
    // Configure the syst timer to trigger an update every second
    /**/
    let mut timer = Timer::syst(cp.SYST, &clocks).counter_hz();
    timer.start(10.Hz()).unwrap();

    // Wait for the timer to trigger an update and change the state of the LED

    assert!(clocks.usbclk_valid());
    let mut gpioa = dp.GPIOA.split();

    // BluePill board has a pull-up resistor on the D+ line.
    // Pull the D+ pin down to send a RESET condition to the USB bus.
    // This forced reset is needed only for development, without it host
    // will not reset your device when you upload new firmware.
    let mut usb_dp = gpioa.pa12.into_push_pull_output(&mut gpioa.crh);
    usb_dp.set_low();
    delay(clocks.sysclk().raw() / 100);

    let usb_dm = gpioa.pa11;
    let usb_dp = usb_dp.into_floating_input(&mut gpioa.crh);

    let usb = Peripheral {
        usb: dp.USB,
        pin_dm: usb_dm,
        pin_dp: usb_dp,
    };

    // Unsafe to allow access to static variables
    unsafe {
        let bus = UsbBus::new(usb);

        USB_BUS = Some(bus);

        USB_SERIAL = Some(SerialPort::new(USB_BUS.as_ref().unwrap()));

        let usb_dev = UsbDeviceBuilder::new(USB_BUS.as_ref().unwrap(), UsbVidPid(0x16c0, 0x27dd))
            .manufacturer("Fake company")
            .product("Serial port")
            .serial_number("TEST")
            .device_class(USB_CLASS_CDC)
            .build();

        USB_DEVICE = Some(usb_dev);
    }

    unsafe {
        NVIC::unmask(Interrupt::USB_HP_CAN_TX);
        NVIC::unmask(Interrupt::USB_LP_CAN_RX0);
    }

    loop {
        block!(timer.wait()).unwrap();
        led.set_high();
        block!(timer.wait()).unwrap();
        led.set_low();
    }
}

#[interrupt]
fn USB_HP_CAN_TX() {
    usb_interrupt();
}

#[interrupt]
fn USB_LP_CAN_RX0() {
    usb_interrupt();
}

fn usb_interrupt() {
    let usb_dev = unsafe { USB_DEVICE.as_mut().unwrap() };
    let serial = unsafe { USB_SERIAL.as_mut().unwrap() };

    if !usb_dev.poll(&mut [serial]) {
        return;
    }

    let mut buf = [0u8; 8];

    match serial.read(&mut buf) {
        Ok(count) if count > 0 => {
            // Echo back in upper case
            for c in buf[0..count].iter_mut() {
                if 0x61 <= *c && *c <= 0x7a {
                    *c &= !0x20;
                }
            }

            serial.write(&buf[0..count]).ok();
        }
        _ => {}
    }
}
