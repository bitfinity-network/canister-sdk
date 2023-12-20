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
            let all_logs_count = records.0;

            if (from_offset >= all_logs_count) || all_logs_count == 0 {
                Logs {
                    all_logs_count,
                    logs: vec![],
                }
            } else {
                let mut result = Vec::with_capacity(max_count);

                let from_offset = if (from_offset + records.1.len()) < all_logs_count {
                    0
                } else {
                    if all_logs_count > records.1.len() {
                        from_offset - (all_logs_count % records.1.capacity())
                    } else {
                        from_offset
                    }
                };

                let first_index = from_offset % records.1.capacity();
                let mut offset = all_logs_count + first_index - records.1.len();

                for log in records.1.iter().skip(first_index).take(max_count) {
                    result.push(Log {
                        log: log.clone(),
                        offset,
                    });
                    offset += 1;
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
            borrow
                .1
                .push(String::from_utf8_lossy(buf.bytes()).to_string());
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
            assert!(records
                .borrow()
                .1
                .iter()
                .eq(["some data".to_string()].iter()));
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
        assert_eq!(
            res,
            Logs {
                logs: vec![],
                all_logs_count: 0,
            }
        );

        let res = InMemoryWriter::take_records(1, 0);
        assert_eq!(
            res,
            Logs {
                logs: vec![],
                all_logs_count: 0,
            }
        );

        let res = InMemoryWriter::take_records(0, 3);
        assert_eq!(
            res,
            Logs {
                logs: vec![],
                all_logs_count: 0,
            }
        );

        let res = InMemoryWriter::take_records(2, LOG_RECORDS_MAX_COUNT);
        assert_eq!(
            res,
            Logs {
                logs: vec![],
                all_logs_count: 0,
            }
        );

        let res = InMemoryWriter::take_records(3, LOG_RECORDS_MAX_COUNT + 1);
        assert_eq!(
            res,
            Logs {
                logs: vec![],
                all_logs_count: 0,
            }
        );
    }

    #[test]
    fn test_memory_writer_take_data_with_one_entry_in_buffer() {
        clear_memory_records();

        let writer = InMemoryWriter {};
        writer.print(&"some data 1".into()).unwrap();

        let res = InMemoryWriter::take_records(0, 0);
        assert_eq!(
            res,
            Logs {
                logs: vec![],
                all_logs_count: 1,
            }
        );

        let res = InMemoryWriter::take_records(1, 0);
        assert_eq!(
            res,
            Logs {
                logs: vec![Log {
                    log: "some data 1".to_string(),
                    offset: 0,
                }],
                all_logs_count: 1,
            }
        );

        let res = InMemoryWriter::take_records(0, 3);
        assert_eq!(
            res,
            Logs {
                logs: vec![],
                all_logs_count: 1,
            }
        );

        let res = InMemoryWriter::take_records(2, LOG_RECORDS_MAX_COUNT);
        assert_eq!(
            res,
            Logs {
                logs: vec![],
                all_logs_count: 1,
            }
        );

        let res = InMemoryWriter::take_records(3, LOG_RECORDS_MAX_COUNT + 1);
        assert_eq!(
            res,
            Logs {
                logs: vec![],
                all_logs_count: 1,
            }
        );
    }

    #[test]
    fn test_memory_writer_take_data_with_two_entries_in_buffer() {
        clear_memory_records();

        let writer = InMemoryWriter {};
        writer.print(&"0".into()).unwrap();
        writer.print(&"1".into()).unwrap();

        let res = InMemoryWriter::take_records(0, 0);
        assert_eq!(
            res,
            Logs {
                logs: vec![],
                all_logs_count: 2,
            }
        );

        let res = InMemoryWriter::take_records(1, 0);
        assert_eq!(
            res,
            Logs {
                logs: vec![Log {
                    log: "0".to_string(),
                    offset: 0,
                },],
                all_logs_count: 2,
            }
        );

        let res = InMemoryWriter::take_records(1, 1);
        assert_eq!(
            res,
            Logs {
                logs: vec![Log {
                    log: "1".to_string(),
                    offset: 1,
                },],
                all_logs_count: 2,
            }
        );

        let res = InMemoryWriter::take_records(2, 0);
        assert_eq!(
            res,
            Logs {
                logs: vec![
                    Log {
                        log: "0".to_string(),
                        offset: 0,
                    },
                    Log {
                        log: "1".to_string(),
                        offset: 1,
                    },
                ],
                all_logs_count: 2,
            }
        );

        let res = InMemoryWriter::take_records(0, 3);
        assert_eq!(
            res,
            Logs {
                logs: vec![],
                all_logs_count: 2,
            }
        );

        let res = InMemoryWriter::take_records(2, LOG_RECORDS_MAX_COUNT);
        assert_eq!(
            res,
            Logs {
                logs: vec![],
                all_logs_count: 2,
            }
        );

        let res = InMemoryWriter::take_records(3, LOG_RECORDS_MAX_COUNT + 1);
        assert_eq!(
            res,
            Logs {
                logs: vec![],
                all_logs_count: 2,
            }
        );
    }

    #[test]
    fn test_memory_writer_take_data_with_full_buffer() {
        clear_memory_records();
        let size = 6;
        InMemoryWriter::init_buffer(size);
        let writer = InMemoryWriter {};

        for i in 0..size {
            writer.print(&format!("{i}").into()).unwrap();
        }

        let res = InMemoryWriter::take_records(1, 0);
        assert_eq!(
            res,
            Logs {
                logs: vec![Log {
                    log: "0".to_string(),
                    offset: 0,
                }],
                all_logs_count: size,
            }
        );

        let res = InMemoryWriter::take_records(2, 0);
        assert_eq!(
            res,
            Logs {
                logs: vec![
                    Log {
                        log: "0".to_string(),
                        offset: 0,
                    },
                    Log {
                        log: "1".to_string(),
                        offset: 1,
                    }
                ],
                all_logs_count: size,
            }
        );

        let res = InMemoryWriter::take_records(2, 1);
        assert_eq!(
            res,
            Logs {
                logs: vec![
                    Log {
                        log: "1".to_string(),
                        offset: 1,
                    },
                    Log {
                        log: "2".to_string(),
                        offset: 2,
                    }
                ],
                all_logs_count: size,
            }
        );

        let res = InMemoryWriter::take_records(size, 3);
        assert_eq!(
            res,
            Logs {
                logs: vec![
                    Log {
                        log: "3".to_string(),
                        offset: 3,
                    },
                    Log {
                        log: "4".to_string(),
                        offset: 4,
                    },
                    Log {
                        log: "5".to_string(),
                        offset: 5,
                    },
                ],
                all_logs_count: size,
            }
        );

        let res = InMemoryWriter::take_records(size, size);
        assert_eq!(
            res,
            Logs {
                logs: vec![],
                all_logs_count: size,
            }
        );

        let res = InMemoryWriter::take_records(size, size + 5);
        assert_eq!(
            res,
            Logs {
                logs: vec![],
                all_logs_count: size,
            }
        );
    }

    #[test]
    fn test_memory_writer_take_data_with_overridden_buffer() {
        clear_memory_records();

        let size = 6;
        InMemoryWriter::init_buffer(size);
        let writer = InMemoryWriter {};

        let all_logs_count = size * 2;

        for i in 0..all_logs_count {
            writer.print(&format!("{i}").into()).unwrap();
        }

        let res = InMemoryWriter::take_records(1, 0);
        assert_eq!(
            res,
            Logs {
                logs: vec![Log {
                    log: "6".to_string(),
                    offset: 6,
                }],
                all_logs_count,
            }
        );

        let res = InMemoryWriter::take_records(2, 0);
        assert_eq!(
            res,
            Logs {
                logs: vec![
                    Log {
                        log: "6".to_string(),
                        offset: 6,
                    },
                    Log {
                        log: "7".to_string(),
                        offset: 7,
                    }
                ],
                all_logs_count,
            }
        );

        let res = InMemoryWriter::take_records(2, 1);
        assert_eq!(
            res,
            Logs {
                logs: vec![
                    Log {
                        log: "6".to_string(),
                        offset: 6,
                    },
                    Log {
                        log: "7".to_string(),
                        offset: 7,
                    }
                ],
                all_logs_count,
            }
        );

        let res = InMemoryWriter::take_records(size, 9);
        assert_eq!(
            res,
            Logs {
                logs: vec![
                    Log {
                        log: "9".to_string(),
                        offset: 9,
                    },
                    Log {
                        log: "10".to_string(),
                        offset: 10,
                    },
                    Log {
                        log: "11".to_string(),
                        offset: 11,
                    },
                ],
                all_logs_count,
            }
        );

        let res = InMemoryWriter::take_records(size, all_logs_count);
        assert_eq!(
            res,
            Logs {
                logs: vec![],
                all_logs_count,
            }
        );

        let res = InMemoryWriter::take_records(size, all_logs_count + 5);
        assert_eq!(
            res,
            Logs {
                logs: vec![],
                all_logs_count,
            }
        );
    }

    #[test]
    fn test_memory_writer_take_data_with_overridden_buffer_not_multiple_of_size() {
        clear_memory_records();

        let size = 6;
        InMemoryWriter::init_buffer(size);
        let writer = InMemoryWriter {};

        let mut all_logs_count = (size * 3) + 1;

        for i in 0..all_logs_count {
            writer.print(&format!("{i}").into()).unwrap();
        }

        let res = InMemoryWriter::take_records(1, 0);
        assert_eq!(
            res,
            Logs {
                logs: vec![Log {
                    log: "13".to_string(),
                    offset: 13,
                }],
                all_logs_count,
            }
        );

        let res = InMemoryWriter::take_records(2, 0);
        assert_eq!(
            res,
            Logs {
                logs: vec![
                    Log {
                        log: "13".to_string(),
                        offset: 13,
                    },
                    Log {
                        log: "14".to_string(),
                        offset: 14,
                    }
                ],
                all_logs_count,
            }
        );

        let res = InMemoryWriter::take_records(2, 13);
        assert_eq!(
            res,
            Logs {
                logs: vec![
                    Log {
                        log: "13".to_string(),
                        offset: 13,
                    },
                    Log {
                        log: "14".to_string(),
                        offset: 14,
                    }
                ],
                all_logs_count,
            }
        );

        let res = InMemoryWriter::take_records(5, 15);
        assert_eq!(
            res,
            Logs {
                logs: vec![
                    Log {
                        log: "15".to_string(),
                        offset: 15,
                    },
                    Log {
                        log: "16".to_string(),
                        offset: 16,
                    },
                    Log {
                        log: "17".to_string(),
                        offset: 17,
                    },
                    Log {
                        log: "18".to_string(),
                        offset: 18,
                    },
                ],
                all_logs_count,
            }
        );

        let res = InMemoryWriter::take_records(size, all_logs_count);
        assert_eq!(
            res,
            Logs {
                logs: vec![],
                all_logs_count,
            }
        );

        let res = InMemoryWriter::take_records(size, all_logs_count + 1);
        assert_eq!(
            res,
            Logs {
                logs: vec![],
                all_logs_count,
            }
        );

        writer.print(&format!("{all_logs_count}").into()).unwrap();
        all_logs_count += 1;

        let res = InMemoryWriter::take_records(2, 13);
        assert_eq!(
            res,
            Logs {
                logs: vec![
                    Log {
                        log: "14".to_string(),
                        offset: 14,
                    },
                    Log {
                        log: "15".to_string(),
                        offset: 15,
                    }
                ],
                all_logs_count,
            }
        );

        let res = InMemoryWriter::take_records(5, 15);
        assert_eq!(
            res,
            Logs {
                logs: vec![
                    Log {
                        log: "15".to_string(),
                        offset: 15,
                    },
                    Log {
                        log: "16".to_string(),
                        offset: 16,
                    },
                    Log {
                        log: "17".to_string(),
                        offset: 17,
                    },
                    Log {
                        log: "18".to_string(),
                        offset: 18,
                    },
                    Log {
                        log: "19".to_string(),
                        offset: 19,
                    },
                ],
                all_logs_count,
            }
        );
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
