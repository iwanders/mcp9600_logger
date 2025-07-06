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

use cortex_m::asm::delay;

use stm32f1xx_hal::{pac, prelude::*, rcc, timer::Timer};

use stm32f1xx_hal::usb::{Peripheral, UsbBus};

use usb_device::prelude::*;
use usbd_serial::{SerialPort, USB_CLASS_CDC};

use stm32f1xx_hal::i2c::{BlockingI2c, DutyCycle, Mode};

use ssd1306::{I2CDisplayInterface, Ssd1306, prelude::*};

pub mod clock;
pub mod display;
pub mod mcp9600;
pub mod util;
use clock::ElapsedMillis;

pub fn main() -> ! {
    // ------------------------------------------------------
    // Oscillators & peripheral setup.
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

    // ------------------------------------------------------
    // Setup on board led, and clock for measuring time.
    // Acquire the GPIOC peripheral
    let mut gpioc = dp.GPIOC.split(&mut rcc);
    // Configure gpio C pin 13 as a push-pull output. The `crh` register is passed to the function
    // in order to configure the port. For pins 0-7, crl should be passed instead.
    let mut led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);
    led.set_high();
    //loop {}
    // Configure the syst timer to trigger an update every second
    /**/
    //let mut timer = Timer::syst(cp.SYST, &rcc.clocks).counter_us();
    //timer.start(stm32f1xx_hal::time::us(100)).unwrap();

    // Setup our milliseconds clock & measurement interval timer.
    clock::setup_ms_clock(dp.TIM2, &mut rcc);
    let mut elapsed = ElapsedMillis::new();

    // ------------------------------------------------------
    //  Setup USB & CDC
    let mut gpioa = dp.GPIOA.split(&mut rcc);

    // BluePill board has a pull-up resistor on the D+ line.
    // Pull the D+ pin down to send a RESET condition to the USB bus.
    // This forced reset is needed only for development, without it host
    // will not reset your device when you upload new firmware.
    let mut usb_dp = gpioa.pa12.into_push_pull_output(&mut gpioa.crh);
    usb_dp.set_low();
    delay(rcc.clocks.sysclk().raw() / 100);

    // Setup USB and CDC serial port.
    let usb = Peripheral {
        usb: dp.USB,
        pin_dm: gpioa.pa11,
        pin_dp: usb_dp.into_floating_input(&mut gpioa.crh),
    };
    let usb_bus = UsbBus::new(usb);

    let mut serial = SerialPort::new(&usb_bus);

    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .device_class(USB_CLASS_CDC)
        .strings(&[StringDescriptors::default()
            .manufacturer("Fake Company")
            .product("Serial port")
            .serial_number("TEST")])
        .unwrap()
        .build();

    // ------------------------------------------------------
    // Setup i2c for temperature sensor
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

    // ------------------------------------------------------
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
    let mut disp = display::Display::new(interface);
    if !disp.init() {
        sprintln!(serial, "# disp init failed.");
    }

    {
        *disp.contents_mut() = display::Contents::test_contents();
    }

    loop {
        if elapsed >= stm32f1xx_hal::time::ms(10) {
            //sprintln!(serial, "{:?}, {}", elapsed, clock::millis());
            let s = mcp.read_status();
            if let Ok(v) = s {
                //sprintln!(serial, "{}, {:?}", clock::millis(), v.conversion_complete);
                if v.conversion_complete {
                    if let Ok(hot) = mcp.read_hot_junction() {
                        let v = hot.as_f32();
                        sprintln!(serial, "{}, {:.4}", clock::millis(), v);
                    }
                    let _ = mcp.clear_status();
                }
            }
            elapsed.reset();
            led.toggle();
        }

        // This returns true if the serial port has data available for reading.
        if !usb_dev.poll(&mut [&mut serial]) {
            continue;
        }

        let mut buf = [0u8; 64];
        match serial.read(&mut buf) {
            Ok(count) => {
                {
                    *disp.contents_mut() = display::Contents::test_contents();
                }
                elapsed.reset();

                disp.update();
                sprintln!(serial, "{:?}", elapsed);
                {
                    *disp.contents_mut() = Default::default();
                }
                elapsed.reset();

                disp.update();
                sprintln!(serial, "{:?}", elapsed);
            }
            _ => {}
        }
        /*

        match serial.read(&mut buf) {
            Ok(count) if count > 0 => {
                use crate::sprintln;
                sprintln!(serial, "{}", clock::millis());
                //led.toggle();
                //led.set_low(); // Turn on
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
        */
        //led.set_high(); // Turn off
    }
}
