use ic_stable_structures::*;
use std::{cell::RefCell, mem::size_of};

use did::Transaction;

const TX_MAP_MEMORY_ID: MemoryId = MemoryId::new(1);
const TX_VEC_MEMORY_ID: MemoryId = MemoryId::new(2);
const TX_LOG_INDEX_MEMORY_ID: MemoryId = MemoryId::new(3);
const TX_LOG_MEMORY_ID: MemoryId = MemoryId::new(4);
const TX_CELL_MEMORY_ID: MemoryId = MemoryId::new(5);
const TX_BTREEMAP_MEMORY_ID: MemoryId = MemoryId::new(6);
const TX_MULTIMAP_MEMORY_ID: MemoryId = MemoryId::new(7);
const TX_RING_BUFFER_INDICES_MEMORY_ID: MemoryId = MemoryId::new(8);
const TX_RING_BUFFER_VEC_MEMORY_ID: MemoryId = MemoryId::new(9);

const U64_SIZE: usize = size_of::<u64>();

thread_local! {
    static MEMORY_MANAGER: DefaultMemoryManager = DefaultMemoryManager::init(DefaultMemoryResourceType::default());

    static TX_BTREEMAP: RefCell<StableBTreeMap<u64, Transaction, DefaultMemoryType>> = {
        RefCell::new(StableBTreeMap::new(get_memory_by_id(&MEMORY_MANAGER, TX_BTREEMAP_MEMORY_ID)))
    };

    static TX_CELL: RefCell<StableCell<Transaction, DefaultMemoryType>> = {
        RefCell::new(StableCell::new(get_memory_by_id(&MEMORY_MANAGER, TX_CELL_MEMORY_ID), Transaction::default()).expect("failed to create stable cell"))
    };

    static TX_LOG: RefCell<StableLog<Transaction, DefaultMemoryType>> = {
        RefCell::new(StableLog::new(get_memory_by_id(&MEMORY_MANAGER, TX_LOG_INDEX_MEMORY_ID), get_memory_by_id(&MEMORY_MANAGER, TX_LOG_MEMORY_ID)).expect("failed to create stable log"))
    };

    static TX_MAP: RefCell<StableUnboundedMap<u64, Transaction, DefaultMemoryType>> = {
        RefCell::new(StableUnboundedMap::new(get_memory_by_id(&MEMORY_MANAGER, TX_MAP_MEMORY_ID)))
    };

    static TX_MULTIMAP: RefCell<StableMultimap<u64, u64, Transaction, DefaultMemoryType>> = {
        RefCell::new(StableMultimap::new(get_memory_by_id(&MEMORY_MANAGER, TX_MULTIMAP_MEMORY_ID)))
    };

    static TX_VEC: RefCell<StableVec<Transaction, DefaultMemoryType>> = {
        RefCell::new(StableVec::new(get_memory_by_id(&MEMORY_MANAGER, TX_VEC_MEMORY_ID)).expect("failed to create stable vec"))
    };

    static TX_RING_BUFFER_DATA: RefCell<StableVec<Transaction, DefaultMemoryType>> = {
        RefCell::new(StableVec::new(get_memory_by_id(&MEMORY_MANAGER, TX_RING_BUFFER_VEC_MEMORY_ID)).expect("failed to create stable vec"))
    };

    static TX_RING_BUFFER_INDICES: RefCell<StableCell<StableRingBufferIndices, DefaultMemoryType>> = {
        RefCell::new(StableCell::new(get_memory_by_id(&MEMORY_MANAGER, TX_RING_BUFFER_INDICES_MEMORY_ID), StableRingBufferIndices::new(4)).expect("failed to create stable cell"))
    };

    static TX_RING_BUFFER: RefCell<StableRingBuffer<Transaction, DefaultMemoryType>> = {
        RefCell::new(StableRingBuffer::new(&TX_RING_BUFFER_DATA, &TX_RING_BUFFER_INDICES))
    };


}

#[derive(Default)]
pub struct Service;

impl Service {
    pub fn init() {
        let should_init_btreemap = TX_BTREEMAP.with(|txs| txs.borrow().len()) == 0;
        if should_init_btreemap {
            Self::insert_tx_to_btreemap(Transaction {
                from: 0,
                to: 0,
                value: 0,
            });
        }
        let should_init_map = TX_MAP.with(|txs| txs.borrow().len()) == 0;
        if should_init_map {
            Self::insert_tx_to_map(Transaction {
                from: 0,
                to: 0,
                value: 0,
            });
        }
        let should_init_multimap = TX_MULTIMAP.with(|txs| txs.borrow().len()) == 0;
        if should_init_multimap {
            Self::insert_tx_to_multimap(Transaction {
                from: 0,
                to: 0,
                value: 0,
            });
        }
        let should_init_vec = TX_VEC.with(|txs| txs.borrow().len()) == 0;
        if should_init_vec {
            Self::push_tx_to_vec(Transaction {
                from: 0,
                to: 0,
                value: 0,
            });
        }
        let should_init_log = TX_LOG.with(|txs| txs.borrow().len()) == 0;
        if should_init_log {
            Self::push_tx_to_log(Transaction {
                from: 0,
                to: 0,
                value: 0,
            });
        }
        let should_init_ring_buf = TX_RING_BUFFER.with(|txs| txs.borrow().len()) == 0;
        if should_init_ring_buf {
            Self::push_tx_to_ring_buffer(Transaction {
                from: 0,
                to: 0,
                value: 0,
            });
        }
    }

    pub fn get_tx_from_btreemap(key: u64) -> Option<Transaction> {
        TX_BTREEMAP.with(|tx| tx.borrow().get(&key))
    }

    pub fn insert_tx_to_btreemap(transaction: Transaction) -> u64 {
        TX_BTREEMAP.with(|storage| {
            let new_key = storage.borrow().len();
            storage.borrow_mut().insert(new_key, transaction);

            new_key
        })
    }

    pub fn get_tx_from_cell() -> Transaction {
        TX_CELL.with(|tx| *tx.borrow().get())
    }

    pub fn insert_tx_to_cell(transaction: Transaction) {
        TX_CELL.with(|storage| {
            storage
                .borrow_mut()
                .set(transaction)
                .expect("failed to push to cell");
        })
    }

    pub fn get_tx_from_log(idx: u64) -> Option<Transaction> {
        TX_LOG.with(|tx| tx.borrow().get(idx))
    }

    pub fn push_tx_to_log(transaction: Transaction) -> u64 {
        TX_LOG.with(|storage| {
            storage
                .borrow_mut()
                .append(transaction)
                .expect("failed to push to log");

            storage.borrow().len()
        })
    }

    pub fn get_tx_from_ring_buffer(idx: u64) -> Option<Transaction> {
        TX_RING_BUFFER.with(|tx| tx.borrow().get_value_from_end(idx))
    }

    pub fn push_tx_to_ring_buffer(transaction: Transaction) -> u64 {
        TX_RING_BUFFER.with(|storage| storage.borrow_mut().push(&transaction).0)
    }

    pub fn get_tx_from_map(key: u64) -> Option<Transaction> {
        TX_MAP.with(|tx| tx.borrow().get(&key))
    }

    pub fn insert_tx_to_map(transaction: Transaction) -> u64 {
        TX_MAP.with(|storage| {
            let new_key = storage.borrow().len();
            storage.borrow_mut().insert(&new_key, &transaction);

            new_key
        })
    }

    pub fn get_tx_from_multimap(key: u64) -> Option<Transaction> {
        TX_MULTIMAP.with(|tx| tx.borrow().get(&key, &(key + 1)))
    }

    pub fn insert_tx_to_multimap(transaction: Transaction) -> u64 {
        TX_MULTIMAP.with(|storage| {
            let new_key = storage.borrow().len() as u64;
            storage
                .borrow_mut()
                .insert(&new_key, &(new_key + 1), &transaction);

            new_key
        })
    }

    pub fn get_tx_from_vec(idx: u64) -> Option<Transaction> {
        TX_VEC.with(|tx| tx.borrow().get(idx))
    }

    pub fn push_tx_to_vec(transaction: Transaction) -> u64 {
        TX_VEC.with(|storage| {
            storage
                .borrow_mut()
                .push(&transaction)
                .expect("failed to push to vec");

            storage.borrow().len()
        })
    }
}
