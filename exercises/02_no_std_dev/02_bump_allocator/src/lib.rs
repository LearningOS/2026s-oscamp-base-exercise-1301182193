//! # Bump Allocator (no_std)
//!
//! Implement the simplest heap memory allocator: a Bump Allocator (bump pointer allocator).
//!
//! ## How It Works
//!
//! A Bump Allocator maintains a pointer `next` to the "next available address".
//! On each allocation, it aligns `next` to the requested alignment, then advances by `size` bytes.
//! It does not support freeing individual objects (`dealloc` is a no-op).
//!
//! ```text
//! heap_start                              heap_end
//! |----[allocated]----[allocated]----| next |---[free]---|
//!                                        ^
//!                                    next allocation starts here
//! ```
//!
//! ## Task
//!
//! Implement `BumpAllocator`'s `GlobalAlloc::alloc` method:
//! 1. Align the current `next` up to `layout.align()`
//!    Hint: `align_up(addr, align) = (addr + align - 1) & !(align - 1)`
//! 2. Check if the aligned address plus `layout.size()` exceeds `heap_end`
//! 3. If it exceeds, return `null_mut()`; otherwise atomically update `next` with `compare_exchange`
//!
//! ## Key Concepts
//!
//! - `core::alloc::{GlobalAlloc, Layout}`
//! - Memory alignment calculation
//! - `AtomicUsize` and `compare_exchange` (CAS loop)

#![cfg_attr(not(test), no_std)]

use core::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;
use core::sync::atomic::{AtomicUsize, Ordering};

//rust 采用统一导入机制，可以导入模块 ptr， 函数null_mut,类型Layout, trait:Debug,常量：use core::usize::MAX；
//  宏：use std::println;

//说明原子操作和原子类型都存在于 core 库中

pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: AtomicUsize,
    //原子类型的 Usize 变量类型
}

impl BumpAllocator {
    //impl是给结构体添加行为的，可以添加 关联函数，和方法

    /// Create a new BumpAllocator.
    ///
    /// # Safety
    /// `heap_start..heap_end` must be a valid, readable and writable memory region,
    /// and must not be used by other code during this allocator's lifetime.
    /// 
    
    //简单来说，不存在 self 的就是关联函数，即不存在作用的实体
    //而存在 self 参数的，即为方法，因为这个函数可以作用在 self 这个实例之上
    pub const unsafe fn new(heap_start: usize, heap_end: usize) -> Self {
        Self {
            heap_start,
            heap_end,
            next: AtomicUsize::new(heap_start),
        }
    }

    /// Reset the allocator (free all allocated memory).
    pub fn reset(&self) {
        self.next.store(self.heap_start, Ordering::SeqCst);
        //self.next.store(...) 来自 Rust 的 原子类型（Atomic）
        //表示以原子方式，把 heap_start 写入 next
        //SquCst 表示最强顺序性的内存序，即所有线程看到一直顺序
        //Rust 里的“原子操作只能作用在原子类型上” 如：AtomicUsize  AtomicU32  AtomicPtr
        

    }
}

unsafe impl GlobalAlloc for BumpAllocator {
    // 实现了 GlobalAlloc 这个 trait 要求一定要实现两个函数
    // 1.unsafe fn alloc() -> *mut u8
    // 对于 GlobalAlloc 这个 trait，只管理指针，不具体管理内存，所以其 dealloc 一般都是留空，
    //      因为其无法真正释放内存

    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align();
        let size = layout.size();
        //Layout 类型取得 layout.align， layout.size 的方法，类似于面向对象的方法，不可直接访问这两个变量


        loop {
            let current = self.next.load(Ordering::SeqCst);
            let aligned = (current + align - 1) &!(align - 1); 
            //对于这种对齐方式，只适用于 align 为 2 的整数次幂

            let end = aligned.saturating_add(size);
            //saturating:饱和地
            //saturating_add 是 usize 提供的一种饱和式加法，
            //  超过可表示的最大值，不会溢出，不会 panic，直接会返回最大值

            if end > self.heap_end {
                return null_mut();
                //null_mut是一个函数，通过core::ptr::null_mut引入
                //函数定义：pub const fn null_mut<T>() -> *mut T
                //作用：返回一个空的可变指针，表示分配失败，这个指针是有效的，对应 C++中的 NULL
                //其返回的空指针不能解引用

            }

            if self
                .next
                .compare_exchange(current, end, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return aligned as *mut u8;
            }
        }
    }

    // unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    //     let align = layout.align();
    //     let size = layout.size();
    //     loop {
    //         let current = self.next.load(Ordering::SeqCst);
    //         let aligned = (current + align - 1) &! (align - 1);
    //         let end = aligned.saturating_add(size);

    //         if end > self.heap_end {
    //             return null_mut();
    //         }

    //         if self
    //             .next
    //             .compare_exchange(current, aligned, Ordering::SeqCst, Ordering::SeqCst)
    //             .is_ok() {
    //                 return aligned as *mut u8;
    //             }

    //     }
    // }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator does not reclaim individual objects — leave empty
    }
}

// ============================================================
// Tests
// ============================================================
#[cfg(test)]
mod tests {
    use super::*;

    const HEAP_SIZE: usize = 4096;

    fn make_allocator() -> (BumpAllocator, Vec<u8>) {
        let mut heap = vec![0u8; HEAP_SIZE];
        //新建 heap 模拟内存，初始值为 0u8，一个 u8 类型的数据，就是一个字节，这里是 4096K 字节，即 4KB
        let start = heap.as_mut_ptr() as usize;
        let alloc = unsafe { BumpAllocator::new(start, start + HEAP_SIZE) };
        (alloc, heap)
    }

    #[test]
    fn test_alloc_basic() {
        let (alloc, _heap) = make_allocator();
        let layout = Layout::from_size_align(16, 8).unwrap();
        //rust 中的类型，就是相当于 c++里面的类，其中可以带有数据，关联函数， 方法
        //在 rust 中，所有的结构体都是类型，但是类型不一定是结构体，存在 i32, 元组等内建类型不是结构体的特例
        //  自己实现的类型，一定是结构体

        //Layout 在 allocator / 内存分配中是标准参数类型。
        // 表示一块内存的布局信息
        // 包含两个核心属性：
        // size（大小）
        // align（对齐）

        //最常用：from_size_align：
        //  调用方法Layout::from_size_align(size, align)
        //  返回：Result<Layout, LayoutError>
        //  因为返回的是 Result，所以调用完 Layout::from_size_align 方法之后，
        //      后面接上 unwrap 来提取其中的返回值
        let ptr = unsafe { alloc.alloc(layout) };
        assert!(!ptr.is_null(), "allocation should succeed");
    }

    #[test]
    fn test_alloc_alignment() {
        let (alloc, _heap) = make_allocator();
        for align in [1, 2, 4, 8, 16, 64] {
            let layout = Layout::from_size_align(1, align).unwrap();
            let ptr = unsafe { alloc.alloc(layout) };
            assert!(!ptr.is_null());
            assert_eq!(
                ptr as usize % align,
                0,
                "returned address must satisfy align={align}"
            );
        }
    }

    #[test]
    fn test_alloc_no_overlap() {
        let (alloc, _heap) = make_allocator();
        let layout = Layout::from_size_align(64, 8).unwrap();
        let p1 = unsafe { alloc.alloc(layout) } as usize;
        let p2 = unsafe { alloc.alloc(layout) } as usize;
        assert!(
            p1 + 64 <= p2 || p2 + 64 <= p1,
            "two allocations must not overlap"
        );
    }

    #[test]
    fn test_alloc_oom() {
        let (alloc, _heap) = make_allocator();
        let layout = Layout::from_size_align(HEAP_SIZE + 1, 1).unwrap();
        let ptr = unsafe { alloc.alloc(layout) };
        assert!(ptr.is_null(), "should return null when exceeding heap");
    }

    #[test]
    fn test_alloc_fill_heap() {
        let (alloc, _heap) = make_allocator();
        let layout = Layout::from_size_align(256, 1).unwrap();
        for i in 0..16 {
            let ptr = unsafe { alloc.alloc(layout) };
            assert!(!ptr.is_null(), "allocation #{i} should succeed");
        }
        let ptr = unsafe { alloc.alloc(layout) };
        assert!(ptr.is_null(), "should return null when heap is full");
    }

    #[test]
    fn test_reset() {
        let (alloc, _heap) = make_allocator();
        let layout = Layout::from_size_align(HEAP_SIZE, 1).unwrap();
        let p1 = unsafe { alloc.alloc(layout) };
        assert!(!p1.is_null());
        alloc.reset();
        let p2 = unsafe { alloc.alloc(layout) };
        assert!(!p2.is_null(), "should be able to allocate after reset");
        assert_eq!(
            p1, p2,
            "address after reset should match the first allocation"
        );
    }
}
