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
#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(not(target_os = "linux"))]
use panic_halt as _;

use cortex_m_semihosting::hprintln;
use nb::block;

use cortex_m::asm::delay;
use stm32f1xx_hal::{pac, prelude::*, rcc, timer::Timer};

use stm32f1xx_hal::usb::{Peripheral, UsbBus};

use usb_device::prelude::*;
use usbd_serial::{SerialPort, USB_CLASS_CDC};

use stm32f1xx_hal::i2c::{BlockingI2c, DutyCycle, Mode};

use embedded_graphics::{
    mono_font::{MonoTextStyleBuilder, ascii::FONT_6X10, ascii::FONT_9X15_BOLD, ascii::FONT_10X20},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text},
};

use ssd1306::{I2CDisplayInterface, Ssd1306, prelude::*};

pub mod mcp9600;

pub fn main() -> ! {
    // Get access to the core peripherals from the cortex-m crate
    let mut cp = cortex_m::Peripherals::take().unwrap();
    // Get access to the device specific peripherals from the peripheral access crate
    let dp = pac::Peripherals::take().unwrap();

    // i2c requires this cycle counter to be set.
    cp.DWT.enable_cycle_counter();

    // Take ownership over the raw flash and rcc devices and convert them into the corresponding
    // HAL structs
    let mut flash = dp.FLASH.constrain();
    let rcc = dp.RCC.constrain();

    // Freeze the configuration of all the clocks in the system and store the frozen frequencies in
    // `clocks`
    // Set a real clock that allows usb.
    let mut rcc = rcc.freeze(
        rcc::Config::hse(8.MHz()).sysclk(48.MHz()).pclk1(24.MHz()),
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
    /*
    hprintln!("device id: {:?}", devid);
    hprintln!(
        "read_sensor_configuration: {:?}",
        mcp.read_sensor_configuration()
    );
    hprintln!("device id: {:?}", devid);
    hprintln!("read_hot_junction: {:?}", mcp.read_hot_junction());*/

    // And the lcd;
    //

    let scl = gpiob.pb10;
    let sda = gpiob.pb11;

    let i2c2 = dp
        .I2C2
        //.remap(&mut afio.mapr) // add this if want to use PB8, PB9 instead
        .blocking_i2c(
            (scl, sda),
            Mode::Fast {
                frequency: 400.kHz(),
                duty_cycle: DutyCycle::Ratio16to9,
            },
            &mut rcc,
            1000,
            10,
            1000,
            1000,
        );
    let interface = I2CDisplayInterface::new(i2c2);
    let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();
    display.init().unwrap();

    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(BinaryColor::On)
        .build();

    let text_style_big = MonoTextStyleBuilder::new()
        .font(&FONT_10X20)
        .text_color(BinaryColor::On)
        .build();

    Text::with_baseline("Hello world!", Point::zero(), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();

    Text::with_baseline(
        "Hello Krispin!",
        Point::new(0, 48),
        text_style_big,
        Baseline::Top,
    )
    .draw(&mut display)
    .unwrap();

    display.flush().unwrap();

    loop {
        //hprintln!("hj: {:?}", mcp.read_hot_junction());
        if !usb_dev.poll(&mut [&mut serial]) {
            continue;
        }
        led.set_low(); // Turn on

        let mut buf = [0u8; 64];

        match serial.read(&mut buf) {
            Ok(count) if count > 0 => {
                led.set_low(); // Turn on
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
