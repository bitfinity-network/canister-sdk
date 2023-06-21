use std::cell::RefCell;

use ringbuffer::{AllocRingBuffer, RingBufferRead, RingBufferWrite};

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

type LogRecordsBuffer = AllocRingBuffer<String>;
thread_local! {
    static LOG_RECORDS: RefCell<LogRecordsBuffer> = RefCell::new(LogRecordsBuffer::new());
}

/// Writer that stores strings in a thread_local memory circular buffer.
/// Note: it can be optimized to reduce the number of memory allocations.
pub struct InMemoryWriter {}

impl InMemoryWriter {
    pub fn init_buffer(capacity: usize) {
        LOG_RECORDS.with(|records| {
            *records.borrow_mut() = LogRecordsBuffer::with_capacity(capacity);
        });
    }

    pub fn take_records(max_count: usize) -> Vec<String> {
        LOG_RECORDS.with(|records| {
            let mut records = records.borrow_mut();
            let mut result = Vec::with_capacity(max_count);
            for _ in 0..max_count {
                if let Some(s) = records.dequeue() {
                    result.push(s);
                } else {
                    break;
                }
            }

            result
        })
    }
}

impl Writer for InMemoryWriter {
    fn print(&self, buf: &Buffer) -> std::io::Result<()> {
        LOG_RECORDS.with(|records| {
            records
                .borrow_mut()
                .push(String::from_utf8_lossy(buf.bytes()).to_string());
        });
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use ringbuffer::RingBufferExt;

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
            assert!(records
                .borrow()
                .iter()
                .eq(vec!["some data".to_string()].iter()));
        });

        writer.print(&"some more data".into()).unwrap();
        LOG_RECORDS.with(|records| {
            assert!(records.borrow().iter().eq(vec![
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
