use std::cell::RefCell;

use candid::CandidType;
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

#[derive(Debug, Default, PartialEq, Eq, CandidType)]
pub struct Logs {
    /// the list of logs
    pub logs: Vec<Log>,
    /// the count of available logs
    pub all_logs_count: usize,
}

#[derive(Debug, Default, PartialEq, Eq, CandidType)]
pub struct Log {
    /// the log text
    pub log: String,
    /// the offset of the log
    pub offset: usize,
}

impl InMemoryWriter {
    pub fn init_buffer(capacity: usize) {
        LOG_RECORDS.with(|records| {
            *records.borrow_mut() = (0, LogRecordsBuffer::new(capacity));
        });
    }

    pub fn take_records(max_count: usize, from_offset: usize) -> Logs {
        LOG_RECORDS.with(|records| {
            let records = records.borrow_mut();
            let all_logs_count = records.0 ;

            if (from_offset >= all_logs_count) || all_logs_count == 0 {
                Logs {
                    all_logs_count,
                    logs: vec![],
                }
            } else {
                let first_index = from_offset % records.1.capacity();

                let mut result = Vec::with_capacity(max_count);
                
                let mut count = 0;
                for log in records.1.iter().skip(first_index).take(max_count) {
                    result.push(Log { log: log.clone(), offset: from_offset + count });
                    count += 1;
                }
                
                Logs {
                    all_logs_count,
                    logs: result,
                }
            }
        })
    }
}

impl Writer for InMemoryWriter {
    fn print(&self, buf: &Buffer) -> std::io::Result<()> {
        LOG_RECORDS.with(|records| {
            let mut borrow = records.borrow_mut();
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
            assert!(records.borrow().1.iter().eq(["some data".to_string()].iter()));
            assert_eq!(records.borrow().0, 1);
        });

        writer.print(&"some more data".into()).unwrap();
        LOG_RECORDS.with(|records| {
            assert!(records.borrow().1.iter().eq([
                "some data".to_string(),
                "some more data".to_string()
            ]
            .iter()));
        assert_eq!(records.borrow().0, 2);
        });
    }

    #[test]
    fn test_memory_writer_take_data_with_empty_buffer() {
        clear_memory_records();

        let _writer = InMemoryWriter {};

        // Empty buffer
            let res = InMemoryWriter::take_records(0, 0);
            assert_eq!(res, Logs{
                logs: vec![],
                all_logs_count: 0,
            });
            
            let res = InMemoryWriter::take_records(1, 0);
            assert_eq!(res, Logs{
                logs: vec![],
                all_logs_count: 0,
            });
            
            let res = InMemoryWriter::take_records(0, 3);
            assert_eq!(res, Logs{
                logs: vec![],
                all_logs_count: 0,
            });

            let res = InMemoryWriter::take_records(2, LOG_RECORDS_MAX_COUNT);
            assert_eq!(res, Logs{
                logs: vec![],
                all_logs_count: 0,
            });

            let res = InMemoryWriter::take_records(3, LOG_RECORDS_MAX_COUNT + 1);
            assert_eq!(res, Logs{
                logs: vec![],
                all_logs_count: 0,
            });

    }

    #[test]
    fn test_memory_writer_take_data_with_one_entry_in_buffer() {
        clear_memory_records();

        let writer = InMemoryWriter {};
        writer.print(&"some data 1".into()).unwrap();

            let res = InMemoryWriter::take_records(0, 0);
            assert_eq!(res, Logs{
                logs: vec![],
                all_logs_count: 1,
            });
            
            let res = InMemoryWriter::take_records(1, 0);
            assert_eq!(res, Logs{
                logs: vec![
                    Log{
                        log: "some data 1".to_string(),
                        offset: 0,
                    }
                ],
                all_logs_count: 1,
            });

            let res = InMemoryWriter::take_records(0, 3);
            assert_eq!(res, Logs{
                logs: vec![],
                all_logs_count: 1,
            });

            let res = InMemoryWriter::take_records(2, LOG_RECORDS_MAX_COUNT);
            assert_eq!(res, Logs{
                logs: vec![],
                all_logs_count: 1,
            });

            let res = InMemoryWriter::take_records(3, LOG_RECORDS_MAX_COUNT + 1);
            assert_eq!(res, Logs{
                logs: vec![],
                all_logs_count: 1,
            });

        }

        #[test]
        fn test_memory_writer_take_data_with_two_entries_in_buffer() {
            clear_memory_records();
    
            let writer = InMemoryWriter {};
            writer.print(&"some data 1".into()).unwrap();
        writer.print(&"some data 2".into()).unwrap();

            let res = InMemoryWriter::take_records(0, 0);
            assert_eq!(res, Logs{
                logs: vec![],
                all_logs_count: 2,
            });
            
            let res = InMemoryWriter::take_records(1, 0);
            assert_eq!(res, Logs{
                logs: vec![
                    Log{
                        log: "some data 1".to_string(),
                        offset: 0,
                    },
                ],
                all_logs_count: 2,
            });

            let res = InMemoryWriter::take_records(1, 1);
            assert_eq!(res, Logs{
                logs: vec![
                    Log{
                        log: "some data 2".to_string(),
                        offset: 1,
                    },
                ],
                all_logs_count: 2,
            });

            let res = InMemoryWriter::take_records(2, 0);
            assert_eq!(res, Logs{
                logs: vec![
                    Log{
                        log: "some data 1".to_string(),
                        offset: 0,
                    },
                    Log{
                        log: "some data 2".to_string(),
                        offset: 1,
                    },
                ],
                all_logs_count: 2,
            });

            let res = InMemoryWriter::take_records(0, 3);
            assert_eq!(res, Logs{
                logs: vec![],
                all_logs_count: 2,
            });

            let res = InMemoryWriter::take_records(2, LOG_RECORDS_MAX_COUNT);
            assert_eq!(res, Logs{
                logs: vec![],
                all_logs_count: 2,
            });

            let res = InMemoryWriter::take_records(3, LOG_RECORDS_MAX_COUNT + 1);
            assert_eq!(res, Logs{
                logs: vec![],
                all_logs_count: 2,
            });

        }

        #[test]
        fn test_memory_writer_take_data_with_full_buffer() {
            clear_memory_records();
            let size = LOG_RECORDS_MAX_COUNT;
            InMemoryWriter::init_buffer(size);
            let writer = InMemoryWriter{};

            for i in 0..size {
                writer.print(&format!("{i}").into()).unwrap();
            }

        let res = InMemoryWriter::take_records(1, 0);
        assert_eq!(res, Logs{
            logs: vec![
                Log{
                log: "0".to_string(),
                offset: 0,
            }],
            all_logs_count: size,
        });

        let res = InMemoryWriter::take_records(2, 0);
        assert_eq!(res, Logs{
            logs: vec![
                Log{
                log: "0".to_string(),
                offset: 0,
            }, 
            Log{
                log: "1".to_string(),
                offset: 1,
            }],
            all_logs_count: size,
        });

        let res = InMemoryWriter::take_records(2, 1);
        assert_eq!(res, Logs {
            logs: vec![
                Log{
                log: "1".to_string(),
                offset: 1,
            }, 
            Log{
                log: "2".to_string(),
                offset: 2,
            }],
            all_logs_count: size,
        });

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
                .1
                .iter()
                .cloned()
                .eq((2..(LOG_RECORDS_MAX_COUNT + 2)).map(|i| format!("{i}"))));
        });
    }
}
