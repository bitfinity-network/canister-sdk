use std::fmt;

use humantime::format_rfc3339_nanos;

use crate::platform;

/// An [RFC3339] formatted timestamp.
///
/// The timestamp implements [`Display`] and can be written to a [`Formatter`].
///
/// [RFC3339]: https://www.ietf.org/rfc/rfc3339.txt
/// [`Display`]: https://doc.rust-lang.org/stable/std/fmt/trait.Display.html
/// [`Formatter`]: struct.Formatter.html
pub struct Rfc3339Timestamp {
    time: std::time::SystemTime,
}

impl Rfc3339Timestamp {
    pub fn now() -> Self {
        Rfc3339Timestamp {
            time: platform::current_system_time(),
        }
    }
}

impl fmt::Debug for Rfc3339Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        /// A `Debug` wrapper for `Timestamp` that uses the `Display` implementation.
        struct TimestampValue<'a>(&'a Rfc3339Timestamp);

        impl<'a> fmt::Debug for TimestampValue<'a> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        f.debug_tuple("Timestamp")
            .field(&TimestampValue(self))
            .finish()
    }
}

impl fmt::Display for Rfc3339Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        format_rfc3339_nanos(self.time).fmt(f)
    }
}
