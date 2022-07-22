# stable memory

## background
* The entire stable memory is divided into pages of the same size.
    ```
      page_0       page_1       page_2             page_131071
    | ---------- | ---------- | ---------- | ... | ---------- | 

    Every page size = 64 KiB = 65536 bytes;

    The max page amount is 131072(8Gib);
    ```
* The stable memory is divided into usable and potential part. In the beginning, all pages are potential, and data cannot be read or written from them. We need to use a system api `stable64_grow` to convert potential pages into usable pages. If the conversion is successful, the operation is irreversible.
    ```
    After stable_memory.stable64_grow(2):

      page_0       page_1       page_2             page_131071
    | ---------- | ---------- | ---------- | ... | ---------- | 
                              ￪
           [usable pages]           [potential pages] 
    ```  
* The memory size of canister status will only calculate usable part, and the cycle billing only calculates the usable part too, and the price is the same as the wasm heap memory's.
* Why canister should use it directly?
   * More capacity for state, 4G vs 8G, and possibly more in the future.
   * Reduce the risk when canister upgrade, such as serializing the state of the entire state in wasm heap memory, which will also cause the wasm heap memory to have only 2G of effective space, and another 2G needs to be reserved for the serialized bytes.
* What are the disadvantages?
  * Stable memory currently has only a few low-level interfaces, and it is troublesome to build on them. Fortunately, there is currently a StableBTreeMap that can be used.
  * Compared with operating directly in wasm heap memory, stable memory is a system call, which reduces performance. However, I think it has something to do with the specific scenario, if it is a large block of data writing and reading, the speed of stable memory and wasm heap is almost [the same](https://github.com/aewc/balance/tree/bench#readme).
  * Will take up [additional storage space](https://github.com/aewc/balance/tree/size#readme) to manage the stable memory.
* Why not develop a generic dynamic allocator for stable memory？
  >> Q: One direction worth investigating for the future might be to generalize the underlying allocators. Both StableBTreeMap and senior.joinu’s data structures use an allocator if I understand correctly. On one hand, it’s what needs to be generalized to remove the fixed size contstraint of StableBTreeMap. Secondly multiple data structures using the same allocator is also a path to having them side by side.

  > A: Definitely. That was the original direction that we first explored when building StableBTreeMap. I agree that having a generic dynamic allocator would open up more options for us. It is a more ambitious undertaking though, which is why we opted for the simpler fixed-size allocator on the grounds that it’s easier to implement initially and easier to verify its correctness.
  from [src](https://forum.dfinity.org/t/stablebtreemap-in-canisters/14210/12)

## The system api description
[Stable memory](https://internetcomputer.org/docs/current/references/ic-interface-spec/#system-api-stable-memory)
> 1. `ic0.stable64_size : () → (page_count : i64)`  
    returns the current size of the [usable] stable memory in WebAssembly pages. (One WebAssembly page is 64KiB)
    This system call is experimental. It may be changed or removed in the future. Canisters using it may stop working.
> 2. `ic0.stable64_grow : (new_pages : i64) → (old_page_count : i64)`
    tries to grow the memory by new_pages many pages containing zeroes.
    If successful, returns the previous size of the memory (in pages). Otherwise, returns -1.
    This system call is experimental. It may be changed or removed in the future. Canisters using it may stop working.
> 3. `ic0.stable64_write : (offset : i64, src : i64, size : i64) → ()`
    Copies the data from location [src, src+size) of the canister [wasm heap] memory to location [offset, offset+size) in the stable memory.
    This system call traps if src+size exceeds the size of the WebAssembly [heap] memory or offset+size exceeds the size of the stable memory.
    This system call is experimental. It may be changed or removed in the future. Canisters using it may stop working.
> 4. `ic0.stable64_read : (dst : i64, offset : i64, size : i64) → ()`
    Copies the data from location [offset, offset+size) of the stable memory to the location [dst, dst+size) in the canister [wasm heap] memory.
    This system call traps if dst+size exceeds the size of the WebAssembly memory or offset+size exceeds the size of the stable memory.
     This system call is experimental. It may be changed or removed in the future. Canisters using it may stop working.  

Basically, all operations on stable memory can only be done through these four methods.

## Memory trait
Memory trait from [stable-structures](https://github.com/dfinity/ic/blob/8d7d9b44ee/rs/stable-structures/src/lib.rs#L25) in ic:
```rs
pub trait Memory {
    /// Returns the current size of the stable memory in WebAssembly
    /// pages. (One WebAssembly page is 64Ki bytes.)
    fn size(&self) -> u64;

    /// Tries to grow the memory by new_pages many pages containing
    /// zeroes.  If successful, returns the previous size of the
    /// memory (in pages).  Otherwise, returns -1.
    fn grow(&self, pages: u64) -> i64;

    /// Copies the data referred to by offset out of the stable memory
    /// and replaces the corresponding bytes in dst.
    fn read(&self, offset: u64, dst: &mut [u8]);

    /// Copies the data referred to by src and replaces the
    /// corresponding segment starting at offset in the stable memory.
    fn write(&self, offset: u64, src: &[u8]);
}
```
It is similar to the system api, but with some changes:
* When the value of `i64` is definitely greater than or equal to 0, use `u64`.
* Use slice instead of pointers and size in the wasm heap, the size is the slice's length. So we need to carefully control the length of the slice.

 

## StableBTreeMap

```rs
pub struct StableBTreeMap<M: Memory, K: Storable, V: Storable>
```
* StableBTreeMap is a data structure built on stable memory that implements most of the methods of BTreeMap. And it comes with an allocator, so StableBTreeMap can complete the addressing very well by itself.
* `M` is required to implement the `Memory` trait, so it has four methods for manipulating memory, and StableBTreeMap interacts with memory through `M`'s four methods.
* Therefore, when `M` is `Rc<RefCell<Vec<u8>>>`, we can easily perform unit testing locally, when `M` is `Ic0StableMemory`, the StableBTreeMap can run in the canister wasm environment.
* When `M` is `Ic0StableMemory`, this StableBTreeMap will occupy the entire stable memory, because the methods of `Ic0StableMemory` are to directly call the system API. If we create another structure, both of them will overwrite each other's data and destroy the whole state.

```rs
#[derive(Clone, Copy, Default)]
pub struct Ic0StableMemory;

// Call system API directly
impl Memory for Ic0StableMemory {
    fn size(&self) -> u64 {
        // SAFETY: This is safe because of the ic0 api guarantees.
        unsafe { stable64_size() }
    }

    fn grow(&self, pages: u64) -> i64 {
        // SAFETY: This is safe because of the ic0 api guarantees.
        unsafe { stable64_grow(pages) }
    }

    fn read(&self, offset: u64, dst: &mut [u8]) {
        // SAFETY: This is safe because of the ic0 api guarantees.
        unsafe { stable64_read(dst.as_ptr() as u64, offset, dst.len() as u64) }
    }

    fn write(&self, offset: u64, src: &[u8]) {
        // SAFETY: This is safe because of the ic0 api guarantees.
        unsafe { stable64_write(offset, src.as_ptr() as u64, src.len() as u64) }
    }
}
```

## Memory Page Manger
We'd better have a page manager so that we can store different types of structures.

### RestrictedMemory
One Option is `RestrictedMemory`:
```rs
/// RestrictedMemory creates a limited view of another memory.  This
/// allows one to divide the main memory into non-intersecting ranges
/// and use different layouts in each region.
#[derive(Clone)]
pub struct RestrictedMemory<M: Memory> {
    page_range: core::ops::Range<u64>,
    memory: M,
}
```

```
                    |<-          RestrictedMemory_0      ->|<-      RestrictedMemory_1  -> 
RestrictedMemory:   | ---------- | ---------- | ---------- | ---------- | ---------- | ...

                      page_0       page_1       page_2       page_3       page_4
Memory:             | ---------- | ---------- | ---------- | ---------- | ---------- | ...
```

* It is simple. By specifying `page_range` in advance, the program knows which RestrictedMemory a page belongs to, and it also implements `Memory` trait.
* The problem is that the size of RestrictedMemorys need to be determined in advance and only the last RestrictedMemory can grow dynamically.

### VirtualMemory
Another option is `VirtualMemory`.

Its basic idea is to allocate memory pages to different `VirtualMemory` as needed:
```
                                page_0                    page_1
VirtualMemory_1: |            | ---------- |            | ---------- |            |            | ...

                   page_0                    page_1                    page_2       page_3
VirtualMemory_0: | ---------- |            | ---------- |            | ---------- | ---------- | ...

                   page_0       page_1       page_2       page_3       page_4       page_5
Memory:          | ---------- | ---------- | ---------- | ---------- | ---------- | ---------- | ...

```

So we need a Manager to record the page of Memory belongs to which VirtualMemory:
```rs
/// Manger is used to manage VirtualMemory. The specific function is to mark which wasm page in
/// memory belongs to which data, for example, the 0th page belongs to Balance, the 1st page belongs to History, etc.
pub struct Manager<M: Memory>(StableBTreeMap<M, Vec<u8>, Vec<u8>>);

/// Pack fragmented memory composed of different pages into contiguous memory.
///
/// index stand for different data structures.
/// In the same canister, different data structures should use different indexes.
#[derive(Clone)]
pub struct VirtualMemory<M1: Memory, M2: Memory + Clone> {
    memory: M1,
    pub page_range: Rc<RefCell<Manager<M2>>>,
    index: u8,
}
```

* The `memory` is used to store data, the `page_range` is used to store the page index in `memory` belongs to which VirtualMemory `index`, `index` is used to distinguish different `VirtualMemory`.
