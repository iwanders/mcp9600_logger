#![allow(unsafe_code)]
use core::cell::RefCell;
use core::ops::DerefMut;
use core::sync::atomic::AtomicU32;
use stm32f1xx_hal::pac::Interrupt;
use stm32f1xx_hal::pac::interrupt;

use cortex_m::interrupt::Mutex;
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

pub struct ElapsedMillis {
    previous_ms: u32,
}
impl ElapsedMillis {
    pub fn new() -> Self {
        Self {
            previous_ms: millis(),
        }
    }
    pub fn elapsed(&self) -> u32 {
        millis().wrapping_sub(self.previous_ms)
    }
    pub fn reset(&mut self) {
        self.previous_ms = millis();
    }
}
impl core::fmt::Debug for ElapsedMillis {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.elapsed())
    }
}
impl PartialEq<stm32f1xx_hal::time::MilliSeconds> for ElapsedMillis {
    fn eq(&self, other: &stm32f1xx_hal::time::MilliSeconds) -> bool {
        let ours = self.elapsed();
        ours == other.to_millis()
    }
}

impl PartialOrd<stm32f1xx_hal::time::MilliSeconds> for ElapsedMillis {
    fn partial_cmp(
        &self,
        other: &stm32f1xx_hal::time::MilliSeconds,
    ) -> Option<core::cmp::Ordering> {
        let ours = self.elapsed();
        Some(ours.cmp(&other.to_millis()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn set_clock(v: u32) {
        GLOBAL_MS.store(v, core::sync::atomic::Ordering::Release);
    }
    #[test]
    fn test_elapsed_millis() {
        let mut l = ElapsedMillis::new();
        set_clock(100);
        assert!(l.elapsed() == 100);
        assert!(l > stm32f1xx_hal::time::ms(50));
        assert!(l >= stm32f1xx_hal::time::ms(100));
        assert!(l == stm32f1xx_hal::time::ms(100));
        assert!(l <= stm32f1xx_hal::time::ms(150));
        set_clock(200);
        assert!(l.elapsed() == 200);
        l.reset();
        assert!(l.elapsed() == 0);
        set_clock(300);
        assert!(l.elapsed() == 100);
        // Check wrapping.
        set_clock(u32::MAX - 5);
        let k = ElapsedMillis::new();
        set_clock(5);
        println!("{}", k.elapsed());
        assert!(k.elapsed() == 11);
    }
}
