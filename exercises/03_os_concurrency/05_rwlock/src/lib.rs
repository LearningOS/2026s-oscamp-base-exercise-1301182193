//! # Read-Write Lock (Writer-Priority)
//!
//! In this exercise, you will implement a **writer-priority** read-write lock from scratch using atomics.
//! Multiple readers may hold the lock concurrently; a writer holds it exclusively.
//!
//! **Note:** Rust's standard library already provides [`std::sync::RwLock`]. This exercise implements
//! a minimal version for learning the protocol and policy without using the standard one.
//!
//! ## Common policies for read-write locks
//! Different implementations can give different **priority** when both readers and writers are waiting:
//!
//! - **Reader-priority (读者优先)**: New readers are allowed to enter while a writer is waiting, so writers
//!   may be starved if readers keep arriving.
//! - **Writer-priority (写者优先)**: Once a writer is waiting, no new readers are admitted until that writer
//!   has run; this exercise implements this policy.
//! - **Read-write fair (读写公平)**: Requests are served in a fair order (e.g. FIFO or round-robin), so
//!   neither readers nor writers are systematically starved.
//!
//! ## Key Concepts
//! - **Readers**: share access; many threads can hold a read lock at once.
//! - **Writer**: exclusive access; only one writer, and no readers while the writer holds the lock.
//! - **Writer-priority (this implementation)**: when at least one writer is waiting, new readers block
//!   until the writer runs.
//!
//! ## State (single atomic)
//! We use one `AtomicU32`: low bits = reader count, two flags = writer holding / writer waiting.
//! All logic is implemented with compare_exchange and load/store; no use of `std::sync::RwLock`.

use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU32, Ordering};

/// Maximum number of concurrent readers (fits in state bits).
const READER_MASK: u32 = (1 << 30) - 1;
/// Bit set when a writer holds the lock.
const WRITER_HOLDING: u32 = 1 << 30;
/// Bit set when at least one writer is waiting (writer-priority: block new readers).
const WRITER_WAITING: u32 = 1 << 31;

/// Writer-priority read-write lock. Implemented from scratch; does not use `std::sync::RwLock`.
pub struct RwLock<T> {
    state: AtomicU32,
    data: UnsafeCell<T>,
    //UnsafeCell 是唯一允许“通过共享引用 &T 修改数据”的类型，
    //  即不再需要取得这个数据的可变引用，通过 data.get()即可得到其可变裸指针(*mut T)
    //  self.data.get() -> *mut T
    //  绕过了 rust 的核心保证：“&T 不可修改”，允许&self 修改内部数据
    //  它是所有并发原语的基础
}

unsafe impl<T: Send> Send for RwLock<T> {}
//<T: Send>: 这个是要求使用 RwLock 的类型，一定要实现了 Send
// 对于Send: 自己实现的数据类型，需要实现 Send trait，才能够被移动进新的线程，即在新线程中能够使用这个数据类型
unsafe impl<T: Send + Sync> Sync for RwLock<T> {}
// 对于Sync: 自己实现的数据类型，需要实现 Sync trait， 新的数据类型才能够在多个线程间共享
//<T: Send + Sync>: 这个是要求使用 RwLock 的类型，一定要实现了 Send + Sync


impl<T> RwLock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            state: AtomicU32::new(0),
            data: UnsafeCell::new(data),
        }
    }

    /// Acquire a read lock. Blocks (spins) until no writer holds and no writer is waiting (writer-priority).
    ///
    /// TODO: Implement read lock acquisition
    /// 1. In a loop, load state (Acquire).
    /// 2. If WRITER_HOLDING or WRITER_WAITING is set, spin_loop and continue (writer-priority: no new readers while writer waits).
    /// 3. If reader count (state & READER_MASK) is already READER_MASK, spin and continue.
    /// 4. Try compare_exchange(s, s + 1, AcqRel, Acquire); on success return RwLockReadGuard { lock: self }.
    pub fn read(&self) -> RwLockReadGuard<'_, T> {
        // TODO
        loop {
            let t: u32 = self.state.load(Ordering::Acquire) as u32;
            if t & (WRITER_HOLDING | WRITER_WAITING) == 0 && (t & READER_MASK != READER_MASK) {
                match self.state.compare_exchange(t, t + 1, Ordering::AcqRel, Ordering::Acquire) {
                    //          对于 compare_exchange:成功一定要使用 Order::AcqRel,因为要写这个变量，首先需要保证读的正确性，然后保证写的正确性
                    Ok(_) => {
                        return RwLockReadGuard {
                            lock: self
                        }
                    },
                    Err(_) => continue
                }
            } else {
                core::hint::spin_loop();
                //hint 给 CPU 的提示，
                //spin_loop 提示 CPU: 当前线程在忙等待（spin），可以优化（例如 pause 指令）
            }
        }
    }

    /// Acquire the write lock. Blocks until no readers and no other writer.
    ///
    /// TODO: Implement write lock acquisition (writer-priority)
    /// 1. Set WRITER_WAITING first: fetch_or(WRITER_WAITING, Release) so new readers will block.
    /// 2. In a loop: load state; if any readers (READER_MASK) or WRITER_HOLDING, spin_loop and continue.
    /// 3. Try compare_exchange(WRITER_WAITING, WRITER_HOLDING, ...) to take the lock; or compare_exchange(0, WRITER_HOLDING, ...) if a writer just released.
    /// 4. On success return RwLockWriteGuard { lock: self }.
    pub fn write(&self) -> RwLockWriteGuard<'_, T> {
        // TODO
        self.state.fetch_or(WRITER_WAITING, Ordering::AcqRel);
        loop {
            let s = self.state.load(Ordering::Acquire);
            if (s & READER_MASK) != 0 || (s & WRITER_HOLDING) != 0{
                core::hint::spin_loop();
            } else {
                match self.state.compare_exchange(s, WRITER_HOLDING, Ordering::AcqRel, Ordering::Acquire) {
                    Ok(_) => {
                        return RwLockWriteGuard {
                            lock: self
                        }
                    },
                    Err(_) => {
                        continue
                    }
                }
            }
        }
    }
}

/// Guard for a read lock; releases the read lock on drop.
pub struct RwLockReadGuard<'a, T> {
    lock: &'a RwLock<T>,
    //这里的 lock 是 RwLock<T> 的引用类型，所以这里是借用
    //因为这是一个引用，引用的生命周期一定要 >= guard 的，不然就会造成悬垂引用
    //‘an 表示这个引用最多就活到 ‘a，如果超过这个生命周期，编译器报错
    //     let guard;
    // {
    //     let lock = RwLock::new(5);
    //     guard = lock.read(); // guard 借用了 lock
    // }  lock 在这里被释放
    // ❌ 这里还在用 guard
    // println!("{}", *guard);
    //编译器报错：borrowed value does not live long enough
}

// TODO: Implement Deref for RwLockReadGuard
// Return shared reference to data: unsafe { &*self.lock.data.get() }
impl<T> Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe {
            &*self.lock.data.get()
        }
    }
}

// TODO: Implement Drop for RwLockReadGuard
// Decrement reader count: self.lock.state.fetch_sub(1, Ordering::Release)
impl<T> Drop for RwLockReadGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.state.fetch_sub(1, Ordering::Release);
    }
}

/// Guard for a write lock; releases the write lock on drop.
pub struct RwLockWriteGuard<'a, T> {
    lock: &'a RwLock<T>,
}

// TODO: Implement Deref for RwLockWriteGuard
// Return shared reference: unsafe { &*self.lock.data.get() }
impl<T> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe {
            &*self.lock.data.get()
        }
        //使用 unsafe 的原因：因为self.lock.data.get()返回的是裸指针类型 *mut T
        //  将裸指针转换成引用在 rust 中是不安全的操作，因此需要使用 unsafe 来避免 rust 的检查

        //  使用 &* 的原因：先理解 *, 再理解 & 
        //  使用 * 的原因：将裸指针ptr: *mut T 转换为 *ptr: T,注意：这里并没有转移其所有权，只是指向了这个位置
        //  使用 & 的原因： 对 （*ptr）中的值取引用
        //  最终的结果是返回了 UnsafeCell<T> 里面的 T 的引用，因为这个 data 是 UnsafeCell 类型的，可以直接通过 &T（引用) 修改
    }
}

//实现 Deref 和 DerefMut 这两个 trait 之后，如果需要访问这个引用的值，直接使用 *lock 即可，
//  如果没有实现 Deref，查看 lock 的数据的时候，需要 unsafe{ &*lock.data.get() },很麻烦

//对于 Deref 和 DerefMut 来说，需要修改的时候，自动会调用 DerefMut 以供修改，只读的时候调用 Deref

// TODO: Implement DerefMut for RwLockWriteGuard
// Return mutable reference: unsafe { &mut *self.lock.data.get() }
impl<T> DerefMut for RwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe {
            &mut *self.lock.data.get()
        }
    }
}

// TODO: Implement Drop for RwLockWriteGuard
// Clear writer bits so lock is free: self.lock.state.fetch_and(!(WRITER_HOLDING | WRITER_WAITING), Ordering::Release)
impl<T> Drop for RwLockWriteGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.state.fetch_and(!(WRITER_HOLDING | WRITER_WAITING), Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_multiple_readers() {
        let lock = Arc::new(RwLock::new(0u32));
        let mut handles = vec![];
        for _ in 0..10 {
            let l = Arc::clone(&lock);
            handles.push(thread::spawn(move || {
                let g = l.read();
                assert_eq!(*g, 0);
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
    }

    #[test]
    fn test_writer_excludes_readers() {
        let lock = Arc::new(RwLock::new(0u32));
        let lock_w = Arc::clone(&lock);
        let writer = thread::spawn(move || {
            let mut g = lock_w.write();
            *g = 42;
        });
        writer.join().unwrap();
        let g = lock.read();
        assert_eq!(*g, 42);
    }

    #[test]
    fn test_concurrent_reads_after_write() {
        let lock = Arc::new(RwLock::new(Vec::<i32>::new()));
        {
            let mut g = lock.write();
            g.push(1);
            g.push(2);
        }
        let mut handles = vec![];
        for _ in 0..5 {
            let l = Arc::clone(&lock);
            handles.push(thread::spawn(move || {
                let g = l.read();
                assert_eq!(g.len(), 2);
                assert_eq!(&*g, &[1, 2]);
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
    }

    #[test]
    fn test_concurrent_writes_serialized() {
        let lock = Arc::new(RwLock::new(0u64));
        let mut handles = vec![];
        for _ in 0..10 {
            let l = Arc::clone(&lock);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    let mut g = l.write();
                    *g += 1;
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(*lock.read(), 1000);
    }
}
