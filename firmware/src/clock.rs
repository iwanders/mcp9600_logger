#![allow(unsafe_code)]
use core::cell::RefCell;
use core::ops::DerefMut;
use core::sync::atomic::AtomicU32;
use stm32f1xx_hal::pac::Interrupt;
use stm32f1xx_hal::pac::interrupt;

use cortex_m::interrupt::Mutex;
use stm32f1xx_hal::pac::RCC;
use stm32f1xx_hal::pac::TIM2;
use stm32f1xx_hal::rcc::Rcc;
use stm32f1xx_hal::timer::TimerExt;
use stm32f1xx_hal::timer::{CounterUs, Event};

static GLOBAL_TIM2: Mutex<RefCell<Option<CounterUs<TIM2>>>> = Mutex::new(RefCell::new(None));
static GLOBAL_MS: AtomicU32 = AtomicU32::new(0);

pub fn millis() -> u32 {
    GLOBAL_MS.load(core::sync::atomic::Ordering::Acquire)
}

#[interrupt]
fn TIM2() {
    GLOBAL_MS.fetch_add(1, core::sync::atomic::Ordering::Release);

    cortex_m::interrupt::free(|cs| {
        if let Some(t2) = GLOBAL_TIM2.borrow(cs).borrow_mut().deref_mut() {
            t2.clear_interrupt(Event::Update);
        }
    });
}

pub fn setup_ms_clock(t: TIM2, rcc: &mut Rcc) {
    let mut timer = t.counter_us(rcc);
    timer.start(stm32f1xx_hal::time::us(1000)).unwrap();
    timer.listen(Event::Update);

    cortex_m::interrupt::free(|cs| {
        GLOBAL_TIM2.borrow(cs).borrow_mut().replace(timer);
    });

    unsafe {
        cortex_m::peripheral::NVIC::unmask(Interrupt::TIM2);
    }
}
