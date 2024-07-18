use env_filter::Filter;
use formatter::FormatFn;
use writer::{ConsoleWriter, InMemoryWriter, Logs, MultiWriter, Writer};

#[cfg(feature = "canister")]
pub mod canister;

pub mod did;
mod formatter;
mod platform;
pub mod writer;

use std::cell::RefCell;
use std::sync::Arc;

use arc_swap::{ArcSwap, ArcSwapAny};
use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};

pub use self::did::LogSettings;
use crate::formatter::Formatter;

/// The logger.
///
/// This struct implements the `Log` trait from the [`log` crate][log-crate-url],
/// which allows it to act as a logger.
///
/// The [`init()`], [`try_init()`], [`Builder::init()`] and [`Builder::try_init()`]
/// methods will each construct a `Logger` and immediately initialize it as the
/// default global logger.
///
/// If you'd instead need access to the constructed `Logger`, you can use
/// the associated [`Builder`] and install it with the
/// [`log` crate][log-crate-url] directly.
///
/// [log-crate-url]: https://docs.rs/log/
/// [`init()`]: fn.init.html
/// [`try_init()`]: fn.try_init.html
/// [`Builder::init()`]: struct.Builder.html#method.init
/// [`Builder::try_init()`]: struct.Builder.html#method.try_init
/// [`Builder`]: struct.Builder.html
pub struct Logger {
    writer: Box<dyn Writer>,
    filter: Arc<ArcSwapAny<Arc<Filter>>>,
    format: FormatFn,
}

/// `Builder` acts as builder for initializing a `Logger`.
///
/// It can be used to customize the log format, change the environment variable used
/// to provide the logging directives and also set the default log level filter.
///
/// # Examples
///
/// ```
/// # #[macro_use] extern crate log;
/// # use std::io::Write;
/// use ic_log::Builder;
/// use log::LevelFilter;
///
/// let mut builder = Builder::new();
///
/// builder
///     .parse_filters("debug,crate1::mod1=error,crate1::mod2,crate2=debug")
///     .try_init();
///
/// error!("error message");
/// info!("info message");
/// ```
#[derive(Default)]
pub struct Builder {
    filter: env_filter::Builder,
    writer: MultiWriter,
    format: formatter::Builder,
}

impl Builder {
    /// Initializes the log builder with defaults.
    pub fn new() -> Builder {
        Default::default()
    }

    /// Whether or not to write the level in the default format.
    pub fn format_level(mut self, write: bool) -> Self {
        self.format.format_level = write;
        self
    }

    /// Whether or not to write the module path in the default format.
    pub fn format_module_path(mut self, write: bool) -> Self {
        self.format.format_module_path = write;
        self
    }

    /// Whether or not to write the target in the default format.
    pub fn format_target(mut self, write: bool) -> Self {
        self.format.format_target = write;
        self
    }

    /// Configures the amount of spaces to use to indent multiline log records.
    /// A value of `None` disables any kind of indentation.
    pub fn format_indent(mut self, indent: Option<usize>) -> Self {
        self.format.format_indent = indent;
        self
    }

    /// Configures the end of line suffix.
    pub fn format_suffix(mut self, suffix: &'static str) -> Self {
        self.format.format_suffix = suffix;
        self
    }

    /// Adds a directive to the filter for a specific module.
    ///
    /// # Examples
    ///
    /// Only include messages for info and above for logs in `path::to::module`:
    ///
    /// ```
    /// use ic_log::Builder;
    /// use log::LevelFilter;
    ///
    /// let mut builder = Builder::new();
    ///
    /// builder.filter_module("path::to::module", LevelFilter::Info);
    /// ```
    pub fn filter_module(mut self, module: &str, level: LevelFilter) -> Self {
        self.filter.filter_module(module, level);
        self
    }

    /// Adds a directive to the filter for all modules.
    ///
    /// # Examples
    ///
    /// Only include messages for info and above for logs globally:
    ///
    /// ```
    /// use ic_log::Builder;
    /// use log::LevelFilter;
    ///
    /// let mut builder = Builder::new();
    ///
    /// builder.filter_level(LevelFilter::Info);
    /// ```
    pub fn filter_level(mut self, level: LevelFilter) -> Self {
        self.filter.filter_level(level);
        self
    }

    /// Adds filters to the logger.
    ///
    /// The given module (if any) will log at most the specified level provided.
    /// If no module is provided then the filter will apply to all log messages.
    ///
    /// # Examples
    ///
    /// Only include messages for info and above for logs in `path::to::module`:
    ///
    /// ```
    /// use ic_log::Builder;
    /// use log::LevelFilter;
    ///
    /// let mut builder = Builder::new();
    ///
    /// builder.filter(Some("path::to::module"), LevelFilter::Info);
    /// ```
    pub fn filter(mut self, module: Option<&str>, level: LevelFilter) -> Self {
        self.filter.filter(module, level);
        self
    }

    /// Parses the directives string in the same form as the `RUST_LOG`
    /// environment variable.
    /// Example of valid filters:
    /// - info
    /// - debug,crate1::mod1=error,crate1::mod2,crate2=debug
    pub fn parse_filters(mut self, filters: &str) -> Self {
        self.filter.parse(filters);
        self
    }

    /// Append a new writer.
    pub fn add_writer(mut self, writer: Box<dyn Writer>) -> Self {
        self.writer.add(writer);
        self
    }

    /// Initializes the global logger with the built logger.
    ///
    /// This should be called early in the execution of a Rust program. Any log
    /// events that occur before initialization will be ignored.
    ///
    /// # Errors
    ///
    /// This function will fail if it is called more than once, or if another
    /// library has already initialized a global logger.
    pub fn try_init(self) -> Result<LoggerConfig, SetLoggerError> {
        let (logger, filter) = self.build();

        let max_level = logger.filter();
        log::set_boxed_logger(Box::new(logger))?;
        log::set_max_level(max_level);
        Ok(filter)
    }

    /// Build a logger.
    ///
    /// The returned logger implements the `Log` trait and can be installed manually
    /// or nested within another logger.
    pub fn build(mut self) -> (Logger, LoggerConfig) {
        let filter = Arc::new(ArcSwap::from_pointee(self.filter.build()));

        let writer: Box<dyn Writer> = if self.writer.writers.len() == 1 {
            self.writer.writers.remove(0)
        } else {
            Box::new(self.writer)
        };

        (
            Logger {
                writer,
                filter: filter.clone(),
                format: self.format.build(),
            },
            LoggerConfig { filter },
        )
    }
}

pub struct LoggerConfig {
    filter: Arc<ArcSwapAny<Arc<Filter>>>,
}

impl LoggerConfig {
    /// Updates the runtime configuration of the logger with a new filter in the same form as the `RUST_LOG`
    /// environment variable.
    /// Example of valid filters:
    /// - info
    /// - debug,crate1::mod1=error,crate1::mod2,crate2=debug
    pub fn update_filters(&self, filters: &str) {
        let new_filter = env_filter::Builder::default().parse(filters).build();
        let max_level = new_filter.filter();
        self.filter.swap(Arc::new(new_filter));
        log::set_max_level(max_level);
    }
}

impl Logger {
    /// Returns the maximum `LevelFilter` that this logger instance is
    /// configured to output.
    pub fn filter(&self) -> LevelFilter {
        self.filter.load().filter()
    }

    /// Checks if this record matches the configured filter.
    pub fn matches(&self, record: &Record) -> bool {
        self.filter.load().matches(record)
    }
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.filter.load().enabled(metadata)
    }

    fn log(&self, record: &Record) {
        if self.matches(record) {
            // Log records are written to a thread-local buffer before being printed
            // to the terminal. We clear these buffers afterwards, but they aren't shrunk
            // so will always at least have capacity for the largest log record formatted
            // on that thread.
            //
            // If multiple `Logger`s are used by the same threads then the thread-local
            // formatter might have different color support. If this is the case the
            // formatter and its buffer are discarded and recreated.

            thread_local! {
                static FORMATTER: RefCell<Formatter> = RefCell::new(Formatter::default());
            }

            let print = |formatter: &mut Formatter, record: &Record| {
                let _ = (self.format)(formatter, record)
                    .and_then(|_| formatter.print(self.writer.as_ref()));

                // Always clear the buffer afterwards
                formatter.clear();
            };

            let printed = FORMATTER
                .try_with(|tl_buf| {
                    match tl_buf.try_borrow_mut() {
                        // There are no active borrows of the buffer
                        Ok(ref mut formatter) => print(formatter, record),
                        // There's already an active borrow of the buffer (due to re-entrancy)
                        Err(_) => {
                            print(&mut Formatter::default(), record);
                        }
                    }
                })
                .is_ok();

            if !printed {
                // The thread-local storage was not available (because its
                // destructor has already run). Create a new single-use
                // Formatter on the stack for this call.
                print(&mut Formatter::default(), record);
            }
        }
    }

    fn flush(&self) {}
}

mod std_fmt_impls {
    use std::fmt;

    use super::*;

    impl fmt::Debug for Logger {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.debug_struct("Logger")
                .field("filter", &self.filter)
                .finish()
        }
    }

    impl fmt::Debug for Builder {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.debug_struct("Logger")
                .field("filter", &self.filter)
                .finish()
        }
    }
}

/// Builds and initialize a logger based on the settings
pub fn init_log(settings: &LogSettings) -> Result<LoggerConfig, SetLoggerError> {
    let mut builder = Builder::default().parse_filters(&settings.log_filter);

    if settings.enable_console {
        builder = builder.add_writer(Box::new(ConsoleWriter {}));
    }

    writer::InMemoryWriter::init_buffer(settings.in_memory_records);
    builder = builder.add_writer(Box::new(InMemoryWriter {}));

    builder.try_init()
}

/// Take the log memory records for the circular buffer.
pub fn take_memory_records(max_count: usize, from_offset: usize) -> Logs {
    writer::InMemoryWriter::take_records(max_count, from_offset)
}

#[cfg(test)]
mod tests {

    use log::*;

    use super::*;

    #[test]
    fn update_filter_at_runtime() {
        let config = init_log(&LogSettings {
            enable_console: true,
            in_memory_records: 0,
            log_filter: "debug".to_string(),
            acl: Default::default(),
        })
        .unwrap();

        debug!("This one should be printed");
        info!("This one should be printed");

        config.update_filters("error");

        debug!("This one should NOT be printed");
        info!("This one should NOT be printed");

        config.update_filters("info");

        debug!("This one should NOT be printed");
        info!("This one should be printed");
    }
}
