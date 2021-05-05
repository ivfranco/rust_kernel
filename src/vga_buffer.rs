use core::fmt;

use lazy_static::lazy_static;
use spin::Mutex;
use volatile::Volatile;

lazy_static! {
    /// A global interface to the VGA text buffer. Unlike in the blog posts text starts from the top
    /// left of the screen.
    pub static ref WRITER: Mutex<Writer> = {
        let writer = Writer {
            row_position: 0,
            column_position: 0,
            color_code: ColorCode::new(Color::Yellow, Color::Black),
            /// # Safety
            /// 0xb8000 is the address to the memory mapped VGA text buffer, memory layout is
            /// ensured by repr(C) or repr(transparent) on corresponding types, the buffer is
            /// bounded by the [Buffer] type, by lazy_static and Mutex the buffer is never
            /// concurrently accessed.
            buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
        };

        Mutex::new(writer)
    };
}

/// A 4-bit VGA color.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[allow(missing_docs)]
pub enum Color {
    // dark variants
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    // light variants
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

/// A VGA color code representing foreground and background colors of a single code point.  Meaning
/// of bits From the least significant bit:
/// 0 - 2:  foreground color
/// 3:      if set, foreground color is their lighter variant
/// 4 - 6:  background color
/// 7:      if set, the code point blinks
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
#[doc(hidden)]
pub struct ColorCode(u8);

impl ColorCode {
    const BLINK_BIT: u8 = 0b1000_0000;

    /// Construct a VGA color code with foreground and background color. The background color may
    /// only be the first 8 dark variants of colors. When the background color is set to one of
    /// those light variants (0x8 ~ 0xf) it is automatically converted to its dark variant. Without
    /// further modification the code point is not blinking.
    pub fn new(foreground: Color, background: Color) -> Self {
        let code = (background as u8) << 4 | (foreground as u8);
        Self(code & !Self::BLINK_BIT)
    }

    /// Let the VGA code point blink.
    #[allow(dead_code)]
    pub fn blink(self) -> Self {
        Self(self.0 | Self::BLINK_BIT)
    }
}

/// A code page 437 character with color code. repr(C) ensures the order of fields is not messed by
/// the Rust compiler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
    cp437_code: u8,
    color_code: ColorCode,
}

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

#[repr(transparent)]
struct Buffer {
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

#[doc(hidden)]
pub struct Writer {
    row_position: usize,
    column_position: usize,
    color_code: ColorCode,
    buffer: &'static mut Buffer,
}

impl Writer {
    /// If `byte` is '\n' or current row is full, switch to a next line by possibly moving all
    /// previous rows upwards; otherwise write a byte as a code page 437 character to the VGA text
    /// buffer with the stored color code.
    fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            _ => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = self.row_position;
                let col = self.column_position;

                self.buffer.chars[row][col].write(ScreenChar {
                    cp437_code: byte,
                    color_code: self.color_code,
                });

                self.column_position += 1;
            }
        }
    }

    fn new_line(&mut self) {
        if self.row_position < BUFFER_HEIGHT - 1 {
            self.row_position += 1;
        } else {
            for row in 1..BUFFER_HEIGHT {
                for col in 0..BUFFER_WIDTH {
                    let char = self.buffer.chars[row][col].read();
                    self.buffer.chars[row - 1][col].write(char);
                }
            }

            self.clear_row(BUFFER_HEIGHT - 1);
        }

        self.column_position = 0;
    }

    fn clear_row(&mut self, row: usize) {
        let blank: ScreenChar = ScreenChar {
            cp437_code: b' ',
            color_code: self.color_code,
        };

        for col in 0..BUFFER_WIDTH {
            self.buffer.chars[row][col].write(blank);
        }
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        const UNPRINTABLE: u8 = 0xfe;

        for byte in s.bytes() {
            let code = match byte {
                0x20..=0x7e | b'\n' => byte,
                _ => UNPRINTABLE,
            };

            self.write_byte(code);
        }

        Ok(())
    }
}

#[macro_export]
/// Prints to the VGA text buffer. When the current line is full switch to a next line by possibly
/// moving all previous rows upwards.
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga_buffer::_print(format_args!($($arg)*)));
}

#[macro_export]
/// Prints to the VGA text buffer, with a newline.
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    // An interrupt when the WRITER is locked may trigger a handler that itself invokes `print!`,
    // hence try to acquire the mutex again and deadlock.
    interrupts::without_interrupts(|| {
        WRITER.lock().write_fmt(args).unwrap();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn test_println_simple() {
        println!("test_println_simple output");
    }

    #[test_case]
    fn test_println_many() {
        for _ in 0..200 {
            println!("test_println_many output");
        }
    }

    #[test_case]
    fn test_println_output() {
        use core::fmt::Write;
        use x86_64::instructions::interrupts;

        let s = "Some test string that fits on a single line";

        interrupts::without_interrupts(|| {
            let mut writer = WRITER.lock();
            writeln!(writer, "\n{}", s).expect("writeln failed");
            let row = writer.row_position - 1;
            for (i, c) in s.chars().enumerate() {
                let screen_char = writer.buffer.chars[row][i].read();
                assert_eq!(char::from(screen_char.cp437_code), c);
            }
        })
    }
}
