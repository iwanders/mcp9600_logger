#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]
use cortex_m_rt::entry;
use firmware;

#[cfg_attr(not(test), entry)]
fn main() -> ! {
    firmware::main();
}
