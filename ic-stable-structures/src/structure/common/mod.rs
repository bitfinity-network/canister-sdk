pub mod ring_buffer;

pub use ring_buffer::{StableRingBuffer, StableRingBufferIndices};

pub struct Bounds {
    pub max_size: usize,
    pub is_fixed_size: bool,
    pub size_prefix_len: usize,
}

impl Bounds {
    pub const fn new(max_size: usize, is_fixed_size: bool) -> Self {
        Self {
            max_size,
            is_fixed_size,
            size_prefix_len: Bounds::size_prefix_len(max_size, is_fixed_size),
        }
    }

    pub const fn size_prefix_len(max_size: usize, is_fixed_size: bool) -> usize {
        if is_fixed_size {
            0
        } else if max_size <= u8::MAX as usize {
            1
        } else if max_size <= u16::MAX as usize {
            2
        } else {
            4
        }
    }
}
