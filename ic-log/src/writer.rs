use std::cell::RefCell;

use ringbuffer::{AllocRingBuffer, RingBuffer};

use crate::formatter::buffer::Buffer;
use crate::platform;

/// A trait for the object that consumes already formatted log line.
pub trait Writer: Send + Sync {
    fn print(&self, buf: &Buffer) -> std::io::Result<()>;
}

/// Writer implementation that prints the given data to the console
#[derive(Default)]
pub struct MultiWriter {
    pub(crate) writers: Vec<Box<dyn Writer>>,
}

impl MultiWriter {
    /// Add a new writer
    pub fn add(&mut self, writer: Box<dyn Writer>) {
        self.writers.push(writer)
    }
}

impl Writer for MultiWriter {
    fn print(&self, buf: &Buffer) -> std::io::Result<()> {
        for writer in &self.writers {
            writer.print(buf)?;
        }
        Ok(())
    }
}

/// Writer implementation that prints the given data to the console
pub struct ConsoleWriter {}

impl Writer for ConsoleWriter {
    fn print(&self, buf: &Buffer) -> std::io::Result<()> {
        platform::print(buf.bytes());
        Ok(())
    }
}

const INIT_LOG_CAPACITY: usize = 128;

type LogRecordsBuffer = AllocRingBuffer<String>;
thread_local! {
    static LOG_RECORDS: RefCell<(usize, LogRecordsBuffer)> =
        RefCell::new((0, LogRecordsBuffer::new(INIT_LOG_CAPACITY)));
}

/// Writer that stores strings in a thread_local memory circular buffer.
/// Note: it can be optimized to reduce the number of memory allocations.
pub struct InMemoryWriter {}

impl InMemoryWriter {
    pub fn init_buffer(capacity: usize) {
        LOG_RECORDS.with(|records| {
            *records.borrow_mut() = (0, LogRecordsBuffer::new(capacity));
        });
    }

    pub fn take_records(max_count: usize, from_offset: usize) -> Vec<String> {
        LOG_RECORDS.with(|records| {
            let records = records.borrow_mut();
            let current_offset = records.0;

            let mut result = Vec::with_capacity(max_count);

            let buffer_capacity = records.1.capacity();
            let offset = if from_offset > current_offset  {
                current_offset - from_offset
            } else {
                buffer_capacity
            };
            for log in records.1.iter().skip(buffer_capacity.saturating_sub(offset)).take(max_count) {
                result.push(log.clone());
            }

            result
        })
    }
}

impl Writer for InMemoryWriter {
    fn print(&self, buf: &Buffer) -> std::io::Result<()> {
        LOG_RECORDS.with(|records| {
            let mut borrow = records                .borrow_mut();
            borrow.0 += 1;
            borrow.1.push(String::from_utf8_lossy(buf.bytes()).to_string());
        });
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use ringbuffer::RingBuffer;

    use super::*;

    const LOG_RECORDS_MAX_COUNT: usize = 8;

    fn clear_memory_records() {
        InMemoryWriter::init_buffer(LOG_RECORDS_MAX_COUNT);
    }

    #[test]
    fn test_memory_writer_append() {
        clear_memory_records();

        let writer = InMemoryWriter {};
        writer.print(&"some data".into()).unwrap();

        LOG_RECORDS.with(|records| {
            assert!(records.borrow().iter().eq(["some data".to_string()].iter()));
        });

        writer.print(&"some more data".into()).unwrap();
        LOG_RECORDS.with(|records| {
            assert!(records.borrow().iter().eq([
                "some data".to_string(),
                "some more data".to_string()
            ]
            .iter()));
        });
    }

    #[test]
    fn test_memory_writer_take_data() {
        clear_memory_records();

        let writer = InMemoryWriter {};
        writer.print(&"some data 1".into()).unwrap();
        writer.print(&"some data 2".into()).unwrap();
        writer.print(&"some data 3".into()).unwrap();
        writer.print(&"some data 4".into()).unwrap();

        let res = InMemoryWriter::take_records(1);
        assert_eq!(res, vec!["some data 1".to_string()]);

        let res = InMemoryWriter::take_records(2);
        assert_eq!(
            res,
            vec!["some data 2".to_string(), "some data 3".to_string()]
        );

        let res = InMemoryWriter::take_records(2);
        assert_eq!(res, vec!["some data 4".to_string()]);

        let res = InMemoryWriter::take_records(2);
        assert_eq!(res, Vec::<String>::new());
    }

    #[test]
    fn test_circular_overwrite() {
        clear_memory_records();

        let writer = InMemoryWriter {};
        for i in 0..(LOG_RECORDS_MAX_COUNT + 2) {
            writer.print(&format!("{i}").into()).unwrap();
        }

        LOG_RECORDS.with(|records| {
            assert!(records
                .borrow()
                .iter()
                .cloned()
                .eq((2..(LOG_RECORDS_MAX_COUNT + 2)).map(|i| format!("{i}"))));
        });
    }
}
