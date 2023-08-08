use std::cell::RefCell;
use std::cmp::min;
use std::mem::size_of;
use std::thread::LocalKey;

use super::{StableCell, StableVec};
use crate::{BoundedStorable, Storable};

/// Ring buffer indices state
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Indices {
    /// Index after the latest element in the buffer
    pub(crate) latest: u64,
    /// Capacity of the buffer
    pub(crate) capacity: u64,
}

impl Indices {
    /// Get next index within ring buffer
    fn next_index(&self, mut index: u64) -> u64 {
        assert!(self.capacity > 0);

        index += 1;
        if index == self.capacity {
            index = 0;
        }

        index
    }

    /// Get the element by index from the end of buffer
    fn index_from_end(&self, index: u64) -> Option<u64> {
        if index < self.capacity {
            let result = if index <= self.latest {
                self.latest - index
            } else {
                self.capacity - (index - self.latest)
            };

            Some(result)
        } else {
            None
        }
    }
}

impl Storable for Indices {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        let mut buf = Vec::with_capacity(Self::MAX_SIZE as _);
        buf.extend_from_slice(&self.latest.to_le_bytes());
        buf.extend_from_slice(&self.capacity.to_le_bytes());
        buf.into()
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Self {
            latest: u64::from_le_bytes(bytes[..8].try_into().expect("latest: expected 8 bytes")),
            capacity: u64::from_le_bytes(
                bytes[8..][..8]
                    .try_into()
                    .expect("capacity: expected 8 bytes"),
            ),
        }
    }
}

impl BoundedStorable for Indices {
    const MAX_SIZE: u32 = 2 * (size_of::<u64>() as u32);

    const IS_FIXED_SIZE: bool = true;
}

/// Stable ring buffer implementation
#[derive(Debug)]
pub struct StableRingBuffer<T: BoundedStorable + 'static> {
    /// Vector with elements
    data: &'static LocalKey<RefCell<StableVec<T>>>,
    /// Indices that specify where are the first and last elements in the buffer
    indices: &'static LocalKey<RefCell<StableCell<Indices>>>,
}

impl<T: BoundedStorable + 'static> StableRingBuffer<T> {
    /// Creates new ring buffer
    pub fn new(
        data: &'static LocalKey<RefCell<StableVec<T>>>,
        indices: &'static LocalKey<RefCell<StableCell<Indices>>>,
    ) -> Self {
        Self { data, indices }
    }

    /// Removes all elements in the buffer
    pub fn clear(&mut self) {
        self.with_indices_data_mut(|indices, data| {
            indices.latest = 0;
            data.clear().expect("failed to clear the vector");
        });
    }

    /// Number of elements in the buffer
    pub fn len(&self) -> u64 {
        self.data.with(|d| d.borrow().len())
    }

    /// Returns whether is empty
    pub fn is_empty(&self) -> bool {
        self.data.with(|d| d.borrow().is_empty())
    }

    /// Max capacity of the buffer
    pub fn capacity(&self) -> u64 {
        self.with_indices(|i| i.capacity)
    }

    /// Update the ring buffer capacity to the given value.
    /// The elements that do not fit into new capacity will be deleted.
    pub fn resize(&mut self, new_capacity: u64) {
        self.with_indices_data_mut(|indices, data| {
            if new_capacity == indices.capacity {
                return;
            }

            let elements_to_copy = min(data.len(), new_capacity);
            // Copy to memory all the elements that need to be preserved
            let mut elements = Vec::with_capacity(elements_to_copy as usize);
            for index in (0..elements_to_copy).rev() {
                elements.push(
                    indices
                        .index_from_end(index)
                        .and_then(|i| data.get(i))
                        .expect("element should be present"),
                );
            }

            // clear the stable vector and fill with the elements
            data.clear().expect("failed to clear the stable vector");
            for element in elements {
                data.push(&element).expect("failed to push element");
            }

            indices.latest = if elements_to_copy > 0 {
                elements_to_copy - 1
            } else {
                0
            };
            indices.capacity = new_capacity;
        });
    }

    /// Push new element to the buffer.
    ///
    /// Returns the index of the added item
    pub fn push(&mut self, val: &T) -> u64 {
        self.with_indices_data_mut(|indices, data| {
            let len = data.len();
            if len < indices.capacity {
                data.push(val).expect("failed to add new element");
                indices.latest = len;

                len
            } else {
                let new_newest = indices.next_index(indices.latest);
                data.set(new_newest, val).expect("failed to set value");
                indices.latest = indices.next_index(indices.latest);

                new_newest
            }
        })
    }

    /// Get the element by index from the buffer end
    pub fn get_value_from_end(&self, index: u64) -> Option<T> {
        self.with_indices(|indices| {
            indices
                .index_from_end(index)
                .and_then(|index| self.data.with(|d| d.borrow().get(index)))
        })
    }

    fn with_indices<R>(&self, f: impl Fn(&Indices) -> R) -> R {
        self.indices.with(|i| {
            let indices = i.borrow();
            f(indices.get())
        })
    }

    fn with_indices_data_mut<R>(&mut self, f: impl Fn(&mut Indices, &mut StableVec<T>) -> R) -> R {
        self.indices.with(|i| {
            let mut indices = i.borrow().get().clone();
            let result = self.data.with(|d| {
                let mut data = d.borrow_mut();
                f(&mut indices, &mut data)
            });
            i.borrow_mut()
                .set(indices)
                .expect("failed to update the indices");

            result
        })
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::fmt::Debug;

    use crate::{BoundedStorable, MemoryId};
    use candid::Principal;
    use ic_exports::ic_kit::MockContext;

    use super::*;

    /// Check the roundtrip value -> bytes -> value for `Storable` object
    fn test_storable_roundtrip<Val: Storable + Eq + std::fmt::Debug>(value: &Val) {
        let bytes = value.to_bytes();
        let decoded = Val::from_bytes(bytes);

        assert_eq!(&decoded, value);
    }

    #[test]
    #[should_panic]
    fn next_should_panic_on_zero_capacity() {
        let indices = Indices {
            capacity: 0,
            latest: 0,
        };
        _ = indices.next_index(0);
    }

    #[test]
    fn next_should_work() {
        let indices = Indices {
            capacity: 5,
            latest: 0,
        };

        assert_eq!(1, indices.next_index(0));
        assert_eq!(2, indices.next_index(1));
        assert_eq!(3, indices.next_index(2));
        assert_eq!(4, indices.next_index(3));
        assert_eq!(0, indices.next_index(4));
    }

    #[test]
    fn get_index_from_end_should_end() {
        let indices = Indices {
            capacity: 0,
            latest: 0,
        };
        assert_eq!(None, indices.index_from_end(0));

        let indices = Indices {
            latest: 0,
            capacity: 1,
        };
        assert_eq!(Some(0), indices.index_from_end(0));
        assert_eq!(None, indices.index_from_end(1));

        let indices = Indices {
            latest: 0,
            capacity: 2,
        };
        assert_eq!(Some(0), indices.index_from_end(0));
        assert_eq!(Some(1), indices.index_from_end(1));
        assert_eq!(None, indices.index_from_end(2));

        let indices = Indices {
            latest: 1,
            capacity: 2,
        };
        assert_eq!(Some(1), indices.index_from_end(0));
        assert_eq!(Some(0), indices.index_from_end(1));
        assert_eq!(None, indices.index_from_end(2));

        let indices = Indices {
            latest: 0,
            capacity: 3,
        };
        assert_eq!(Some(0), indices.index_from_end(0));
        assert_eq!(Some(2), indices.index_from_end(1));
        assert_eq!(Some(1), indices.index_from_end(2));
        assert_eq!(None, indices.index_from_end(3));
    }

    #[test]
    fn indices_should_be_storable() {
        test_storable_roundtrip(&Indices {
            capacity: 1,
            latest: 0,
        });
        test_storable_roundtrip(&Indices {
            capacity: 3,
            latest: 2,
        });
    }

    fn check_buffer<T: BoundedStorable + Eq + Debug>(
        buffer: &StableRingBuffer<T>,
        expected: &Vec<T>,
    ) {
        assert_eq!(buffer.len(), expected.len() as u64);

        for i in 0..expected.len() {
            assert_eq!(
                Some(&expected[expected.len() - i - 1]),
                buffer.get_value_from_end(i as u64).as_ref()
            );
        }

        assert_eq!(None, buffer.get_value_from_end(buffer.len()));
    }

    const TEST_DATA_MEMORY: MemoryId = MemoryId::new(203);
    const TEST_INDICES_MEMORY: MemoryId = MemoryId::new(204);

    thread_local! {
        static TEST_DATA: RefCell<StableVec<u64>> = RefCell::new(StableVec::new(TEST_DATA_MEMORY).unwrap());
        static TEST_INDICES: RefCell<StableCell<Indices>> = RefCell::new(StableCell::new(TEST_INDICES_MEMORY, Indices { capacity: 2, latest: 0}).unwrap());
    }

    fn with_buffer(capacity: u64, f: impl Fn(&mut StableRingBuffer<u64>)) {
        let mock_canister_id = Principal::from_slice(&[42; 29]);
        MockContext::new()
            .with_id(mock_canister_id)
            .with_caller(mock_canister_id)
            .inject();

        let mut buffer = StableRingBuffer::new(&TEST_DATA, &TEST_INDICES);
        buffer.clear();
        buffer.resize(capacity);

        f(&mut buffer);
    }

    #[test]
    fn should_push() {
        with_buffer(3, |buffer| {
            check_buffer(buffer, &vec![]);
            assert_eq!(buffer.is_empty(), true);

            assert_eq!(buffer.push(&1), 0);
            check_buffer(buffer, &vec![1]);

            assert_eq!(buffer.push(&2), 1);
            check_buffer(buffer, &vec![1, 2]);

            assert_eq!(buffer.push(&3), 2);
            check_buffer(buffer, &vec![1, 2, 3]);

            assert_eq!(buffer.push(&4), 0);
            check_buffer(buffer, &vec![2, 3, 4])
        });
    }

    #[test]
    fn should_resize_decrease() {
        with_buffer(3, |buffer| {
            // resize empty buffer
            buffer.resize(2);
            check_buffer(buffer, &vec![]);
            assert_eq!(2, buffer.capacity());

            // resize smaller buffer
            buffer.resize(3);
            buffer.push(&1);
            buffer.resize(2);
            check_buffer(buffer, &vec![1]);
            assert_eq!(2, buffer.capacity());

            // resize same size buffer
            buffer.clear();
            buffer.resize(3);
            buffer.push(&1);
            buffer.push(&2);
            buffer.resize(2);
            check_buffer(buffer, &vec![1, 2]);
            assert_eq!(2, buffer.capacity());

            // resize bigger buffer
            buffer.clear();
            buffer.resize(3);
            buffer.push(&1);
            buffer.push(&2);
            buffer.push(&3);
            buffer.resize(2);
            check_buffer(buffer, &vec![2, 3]);
            assert_eq!(2, buffer.capacity());

            // resize bigger buffer, rolled
            buffer.clear();
            buffer.resize(3);
            buffer.push(&1);
            buffer.push(&2);
            buffer.push(&3);
            buffer.push(&4);
            buffer.resize(2);
            check_buffer(buffer, &vec![3, 4]);
            assert_eq!(2, buffer.capacity());
        });
    }

    #[test]
    fn test_increase() {
        with_buffer(3, |buffer| {
            // resize empty buffer
            buffer.resize(4);
            check_buffer(buffer, &vec![]);
            assert_eq!(4, buffer.capacity());

            // resize non-full buffer
            buffer.resize(3);
            buffer.push(&1);
            buffer.push(&2);
            buffer.resize(4);
            check_buffer(buffer, &vec![1, 2]);
            assert_eq!(4, buffer.capacity());

            // resize full buffer
            buffer.clear();
            buffer.resize(3);
            buffer.push(&1);
            buffer.push(&2);
            buffer.push(&3);
            buffer.resize(4);
            check_buffer(buffer, &vec![1, 2, 3]);
            assert_eq!(4, buffer.capacity());

            // resize full buffer rolled
            buffer.clear();
            buffer.resize(3);
            buffer.push(&1);
            buffer.push(&2);
            buffer.push(&3);
            buffer.push(&4);
            buffer.resize(4);
            check_buffer(buffer, &vec![2, 3, 4]);
            assert_eq!(4, buffer.capacity());
        });
    }

    #[test]
    fn should_clear() {
        with_buffer(2, |buffer| {
            check_buffer(buffer, &vec![]);

            buffer.clear();
            check_buffer(buffer, &vec![]);
            assert_eq!(2, buffer.capacity());

            buffer.push(&1);
            buffer.push(&2);
            buffer.push(&3);
            check_buffer(buffer, &vec![2, 3]);
            assert_eq!(2, buffer.capacity());

            buffer.clear();
            check_buffer(buffer, &vec![]);
            assert_eq!(2, buffer.capacity());

            buffer.push(&1);
            check_buffer(buffer, &vec![1]);
            assert_eq!(2, buffer.capacity());

            buffer.clear();
            check_buffer(buffer, &vec![]);
            assert_eq!(2, buffer.capacity());
        })
    }
}
