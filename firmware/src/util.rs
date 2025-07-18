pub use core::fmt::Write;

// This is 100% copied from my syscall project, it's probably a bit over engineered for this.

/// Max length of our stack-string.
const STACK_STRING_SIZE: usize = 64;

/// Object to be able to write a string that's stored onto the stack.
pub struct StackString {
    pub buffer: [u8; STACK_STRING_SIZE],
    pub size: usize,
}
impl StackString {
    pub const STACK_STRING_SIZE: usize = STACK_STRING_SIZE;
    pub const fn capacity() -> usize {
        STACK_STRING_SIZE
    }
    pub fn as_ptr(&self) -> *const u8 {
        self.buffer.as_ptr() as *const u8
    }
    pub fn len(&self) -> usize {
        self.size
    }
    pub fn as_slice(&self) -> &[u8] {
        &self.buffer[0..self.size]
    }

    pub fn as_slice_mut(&mut self) -> &mut [u8] {
        &mut self.buffer[0..self.size]
    }
    pub fn as_str(&self) -> Result<&str, core::str::Utf8Error> {
        core::str::from_utf8(self.as_slice())
    }

    pub fn from_format(fm: core::fmt::Arguments<'_>) -> Result<Self, core::fmt::Error> {
        let mut v: crate::util::StackString = Default::default();
        core::fmt::write(&mut v, fm)?;
        Ok(v)
    }
    pub fn from_str(input: &str) -> Self {
        let mut v: crate::util::StackString = Default::default();
        let input_slice = input.as_bytes();
        let length = input_slice.len().min(Self::capacity());
        v.buffer[0..length].copy_from_slice(&input_slice[0..length]);
        v.size = length;
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_string_from_str() {
        let foo_str = StackString::from_str("foo");
        assert_eq!(foo_str.as_slice(), "foo".as_bytes());
    }
}

impl PartialEq for StackString {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl Default for StackString {
    fn default() -> Self {
        StackString {
            buffer: [0; Self::STACK_STRING_SIZE],
            size: 0,
        }
    }
}

use core::cmp::min;
use core::fmt::Error;
// Implement the Write trait for the stackstring.
impl core::fmt::Write for StackString {
    fn write_str(&mut self, s: &str) -> Result<(), Error> {
        for i in 0..min(s.len(), self.buffer.len() - self.size) {
            self.buffer[self.size] = s.as_bytes()[i];
            self.size += 1;
        }
        if self.size == self.buffer.len() {
            return Err(Error {});
        }
        Ok(())
    }
    // fn write_char(&mut self, c: char) -> Result { ... }
    // fn write_fmt(&mut self, args: Arguments<'_>) -> Result { ... }
}

pub use core::fmt;
// Adopted from https://doc.rust-lang.org/src/std/macros.rs.html#94-99
/// Provide a println! macro similar to Rust does.
#[macro_export]
macro_rules! println {
    () => ($crate::io::print("\n"));
    ($($arg:tt)*) => ({
        let mut v: $crate::io::StackString = Default::default();
        // let mut v: StackString = Default::default();
        core::fmt::write(&mut v, format_args!($($arg)*)).expect("Error occurred while trying to write in String");
        v.write_str("\n").expect("Shouldn't fail");
        //$crate::io::print_sstr(&v);
    })
}

#[macro_export]
macro_rules! sprintln {
  ($serial:expr, $($arg:tt)*) => ({
    pub use core::fmt::Write;
      let mut v: crate::util::StackString = Default::default();
      core::fmt::write(&mut v, format_args!($($arg)*)).expect("Error occurred while trying to write in String");
      v.write_str("\n").expect("Shouldn't fail");
      // Write, and then drop the result, such that we don't panic if nothing is consuming data from the port.
      match $serial.write(v.as_slice()){
        Ok(_count) => {
            // count bytes were written
        },
        Err(UsbError::WouldBlock) => {},// No data could be written (buffers full)
        Err(_err) => {},// An error occurred
      }
  })

}
