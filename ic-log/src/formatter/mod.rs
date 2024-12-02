//! Formatting for log records.
//!
//! This module contains a [`Formatter`] that can be used to format log records
//! into without needing temporary allocations. Usually you won't need to worry
//! about the contents of this module and can use the `Formatter` like an ordinary
//! [`Write`].
//!
//! # Formatting log records
//!
//! The format used to print log records can be customised using the [`Builder::format`]
//! method.
//! Custom formats can apply different color and weight to printed values using
//! [`Style`] builders.
//!
//! ```
//! use std::io::Write;
//!
//! let mut builder = ic_log::Builder::new()
//!     .parse_filters("debug,crate1::mod1=error,crate1::mod2,crate2=debug");
//!
//! builder.build();
//! ```
//!
//! [`Formatter`]: struct.Formatter.html
//! [`Style`]: struct.Style.html
//! [`Builder::format`]: ../struct.Builder.html#method.format
//! [`Write`]: https://doc.rust-lang.org/stable/std/io/trait.Write.html

use std::cell::RefCell;
use std::fmt::Display;
use std::io::prelude::*;
use std::rc::Rc;
use std::{fmt, io};

pub mod buffer;
mod humantime;
use log::Record;

use self::buffer::Buffer;
use self::humantime::Rfc3339Timestamp;
use crate::writer::Writer;

/// A formatter to write logs into.
///
/// `Formatter` implements the standard [`Write`] trait for writing log records.
/// It also supports terminal colors, through the [`style`] method.
///
/// # Examples
///
/// Use the [`writeln`] macro to format a log record.
/// An instance of a `Formatter` is passed to an `env_logger` format as `buf`:
///
#[derive(Default)]
pub struct Formatter {
    buf: Rc<RefCell<Buffer>>,
}

impl Formatter {
    pub(crate) fn print(&self, writer: &dyn Writer) -> io::Result<()> {
        writer.print(&self.buf.borrow())
    }

    pub(crate) fn clear(&mut self) {
        self.buf.borrow_mut().clear()
    }
}

impl Write for Formatter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buf.borrow_mut().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.buf.borrow_mut().flush()
    }
}

impl fmt::Debug for Formatter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Formatter").finish()
    }
}

pub(crate) type FormatFn = Box<dyn Fn(&mut Formatter, &Record) -> io::Result<()> + Sync + Send>;

pub(crate) struct Builder {
    pub timestamp: bool,
    pub format_module_path: bool,
    pub format_target: bool,
    pub format_level: bool,
    pub format_indent: Option<usize>,
    pub custom_format: Option<FormatFn>,
    pub format_suffix: &'static str,
}

impl Default for Builder {
    fn default() -> Self {
        Builder {
            timestamp: true,
            format_module_path: false,
            format_target: true,
            format_level: true,
            format_indent: Some(4),
            custom_format: None,
            format_suffix: "\n",
        }
    }
}

impl Builder {
    /// Convert the format into a callable function.
    ///
    /// If the `custom_format` is `Some`, then any `default_format` switches are ignored.
    /// If the `custom_format` is `None`, then a default format is returned.
    /// Any `default_format` switches set to `false` won't be written by the format.
    pub fn build(self) -> FormatFn {
        if let Some(fmt) = self.custom_format {
            fmt
        } else {
            Box::new(move |buf, record| {
                let fmt = DefaultFormat {
                    timestamp: self.timestamp,
                    module_path: self.format_module_path,
                    target: self.format_target,
                    level: self.format_level,
                    written_header_value: false,
                    indent: self.format_indent,
                    suffix: self.format_suffix,
                    formatter: buf,
                };

                fmt.write(record)
            })
        }
    }
}

type SubtleStyle = &'static str;

/// The default format.
///
/// This format needs to work with any combination of crate features.
struct DefaultFormat<'a> {
    timestamp: bool,
    module_path: bool,
    target: bool,
    level: bool,
    written_header_value: bool,
    indent: Option<usize>,
    formatter: &'a mut Formatter,
    suffix: &'a str,
}

impl DefaultFormat<'_> {
    fn write(mut self, record: &Record) -> io::Result<()> {
        self.write_timestamp()?;
        self.write_level(record)?;
        self.write_module_path(record)?;
        self.write_target(record)?;
        self.finish_header()?;

        self.write_args(record)
    }

    fn subtle_style(&self, text: &'static str) -> SubtleStyle {
        {
            text
        }
    }

    fn write_header_value<T>(&mut self, value: T) -> io::Result<()>
    where
        T: Display,
    {
        if !self.written_header_value {
            self.written_header_value = true;

            let open_brace = self.subtle_style("[");
            write!(self.formatter, "{}{}", open_brace, value)
        } else {
            write!(self.formatter, " {}", value)
        }
    }

    fn write_level(&mut self, record: &Record) -> io::Result<()> {
        if !self.level {
            return Ok(());
        }

        let level = {
            {
                record.level()
            }
        };

        self.write_header_value(format_args!("{:<5}", level))
    }

    fn write_timestamp(&mut self) -> io::Result<()> {
        if !self.timestamp {
            return Ok(());
        }

        let timestamp = Rfc3339Timestamp::now();
        self.write_header_value(timestamp)
    }

    fn write_module_path(&mut self, record: &Record) -> io::Result<()> {
        if !self.module_path {
            return Ok(());
        }

        if let Some(module_path) = record.module_path() {
            self.write_header_value(module_path)
        } else {
            Ok(())
        }
    }

    fn write_target(&mut self, record: &Record) -> io::Result<()> {
        if !self.target {
            return Ok(());
        }

        match record.target() {
            "" => Ok(()),
            target => self.write_header_value(target),
        }
    }

    fn finish_header(&mut self) -> io::Result<()> {
        if self.written_header_value {
            let close_brace = self.subtle_style("]");
            write!(self.formatter, "{} ", close_brace)
        } else {
            Ok(())
        }
    }

    fn write_args(&mut self, record: &Record) -> io::Result<()> {
        match self.indent {
            // Fast path for no indentation
            None => write!(self.formatter, "{}{}", record.args(), self.suffix),

            Some(indent_count) => {
                // Create a wrapper around the buffer only if we have to actually indent the message

                struct IndentWrapper<'a, 'b: 'a> {
                    fmt: &'a mut DefaultFormat<'b>,
                    indent_count: usize,
                }

                impl Write for IndentWrapper<'_, '_> {
                    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                        let mut first = true;
                        for chunk in buf.split(|&x| x == b'\n') {
                            if !first {
                                write!(
                                    self.fmt.formatter,
                                    "{}{:width$}",
                                    self.fmt.suffix,
                                    "",
                                    width = self.indent_count
                                )?;
                            }
                            self.fmt.formatter.write_all(chunk)?;
                            first = false;
                        }

                        Ok(buf.len())
                    }

                    fn flush(&mut self) -> io::Result<()> {
                        self.fmt.formatter.flush()
                    }
                }

                // The explicit scope here is just to make older versions of Rust happy
                {
                    let mut wrapper = IndentWrapper {
                        fmt: self,
                        indent_count,
                    };
                    write!(wrapper, "{}", record.args())?;
                }

                write!(self.formatter, "{}", self.suffix)?;

                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use log::{Level, Record};

    use super::*;

    fn write_record(record: Record, fmt: DefaultFormat) -> String {
        let buf = fmt.formatter.buf.clone();

        fmt.write(&record).expect("failed to write record");

        let buf = buf.borrow();
        String::from_utf8(buf.bytes().to_vec()).expect("failed to read record")
    }

    fn write_target(target: &str, fmt: DefaultFormat) -> String {
        write_record(
            Record::builder()
                .args(format_args!("log\nmessage"))
                .level(Level::Info)
                .file(Some("test.rs"))
                .line(Some(144))
                .module_path(Some("test::path"))
                .target(target)
                .build(),
            fmt,
        )
    }

    fn write(fmt: DefaultFormat) -> String {
        write_target("", fmt)
    }

    #[test]
    fn format_with_header() {
        let mut f = Formatter::default();

        let written = write(DefaultFormat {
            timestamp: false,
            module_path: true,
            target: false,
            level: true,
            written_header_value: false,
            indent: None,
            suffix: "\n",
            formatter: &mut f,
        });

        assert_eq!("[INFO  test::path] log\nmessage\n", written);
    }

    #[test]
    fn format_no_header() {
        let mut f = Formatter::default();

        let written = write(DefaultFormat {
            timestamp: false,
            module_path: false,
            target: false,
            level: false,
            written_header_value: false,
            indent: None,
            suffix: "\n",
            formatter: &mut f,
        });

        assert_eq!("log\nmessage\n", written);
    }

    #[test]
    fn format_indent_spaces() {
        let mut f = Formatter::default();

        let written = write(DefaultFormat {
            timestamp: false,
            module_path: true,
            target: false,
            level: true,
            written_header_value: false,
            indent: Some(4),
            suffix: "\n",
            formatter: &mut f,
        });

        assert_eq!("[INFO  test::path] log\n    message\n", written);
    }

    #[test]
    fn format_indent_zero_spaces() {
        let mut f = Formatter::default();

        let written = write(DefaultFormat {
            timestamp: false,
            module_path: true,
            target: false,
            level: true,
            written_header_value: false,
            indent: Some(0),
            suffix: "\n",
            formatter: &mut f,
        });

        assert_eq!("[INFO  test::path] log\nmessage\n", written);
    }

    #[test]
    fn format_indent_spaces_no_header() {
        let mut f = Formatter::default();

        let written = write(DefaultFormat {
            timestamp: false,
            module_path: false,
            target: false,
            level: false,
            written_header_value: false,
            indent: Some(4),
            suffix: "\n",
            formatter: &mut f,
        });

        assert_eq!("log\n    message\n", written);
    }

    #[test]
    fn format_suffix() {
        let mut f = Formatter::default();

        let written = write(DefaultFormat {
            timestamp: false,
            module_path: false,
            target: false,
            level: false,
            written_header_value: false,
            indent: None,
            suffix: "\n\n",
            formatter: &mut f,
        });

        assert_eq!("log\nmessage\n\n", written);
    }

    #[test]
    fn format_suffix_with_indent() {
        let mut f = Formatter::default();

        let written = write(DefaultFormat {
            timestamp: false,
            module_path: false,
            target: false,
            level: false,
            written_header_value: false,
            indent: Some(4),
            suffix: "\n\n",
            formatter: &mut f,
        });

        assert_eq!("log\n\n    message\n\n", written);
    }

    #[test]
    fn format_target() {
        let mut f = Formatter::default();

        let written = write_target(
            "target",
            DefaultFormat {
                timestamp: false,
                module_path: true,
                target: true,
                level: true,
                written_header_value: false,
                indent: None,
                suffix: "\n",
                formatter: &mut f,
            },
        );

        assert_eq!("[INFO  test::path target] log\nmessage\n", written);
    }

    #[test]
    fn format_empty_target() {
        let mut f = Formatter::default();

        let written = write(DefaultFormat {
            timestamp: false,
            module_path: true,
            target: true,
            level: true,
            written_header_value: false,
            indent: None,
            suffix: "\n",
            formatter: &mut f,
        });

        assert_eq!("[INFO  test::path] log\nmessage\n", written);
    }

    #[test]
    fn format_no_target() {
        let mut f = Formatter::default();

        let written = write_target(
            "target",
            DefaultFormat {
                timestamp: false,
                module_path: true,
                target: false,
                level: true,
                written_header_value: false,
                indent: None,
                suffix: "\n",
                formatter: &mut f,
            },
        );

        assert_eq!("[INFO  test::path] log\nmessage\n", written);
    }
}
