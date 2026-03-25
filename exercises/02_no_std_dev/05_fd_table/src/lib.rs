//! # File Descriptor Table
//!
//! Implement a simple file descriptor (fd) table — the core data structure
//! for managing open files in an OS kernel.
//!
//! ## Background
//!
//! In the Linux kernel, each process has an fd table that maps integer fds to kernel file objects.
//! User programs perform read/write/close via fds, and the kernel looks up the corresponding
//! file object through the fd table.
//!
//! ```text
//! fd table:
//!   0 -> Stdin
//!   1 -> Stdout
//!   2 -> Stderr
//!   3 -> File("/etc/passwd")
//!   4 -> (empty)
//!   5 -> Socket(...)
//! ```
//!
//! ## Task
//!
//! Implement the following methods on `FdTable`:
//!
//! - `new()` — create an empty fd table
//! - `alloc(file)` -> `usize` — allocate a new fd, return the fd number
//!   - Prefer reusing the smallest closed fd number
//!   - If no free slot, extend the table
//! - `get(fd)` -> `Option<Arc<dyn File>>` — get the file object for an fd
//! - `close(fd)` -> `bool` — close an fd, return whether it succeeded (false if fd doesn't exist)
//! - `count()` -> `usize` — return the number of currently allocated fds (excluding closed ones)
//!
//! ## Key Concepts
//!
//! - Trait objects: `Arc<dyn File>`
//! - `Vec<Option<T>>` as a sparse table
//! - fd number reuse strategy (find smallest free slot)
//! - `Arc` reference counting and resource release

use std::sync::Arc;

/// File abstraction trait — all "files" in the kernel (regular files, pipes, sockets) implement this
pub trait File: Send + Sync {
    fn read(&self, buf: &mut [u8]) -> isize;
    fn write(&self, buf: &[u8]) -> isize;
}

/// File descriptor table
pub struct FdTable {
    fdtable: Vec<Option<Arc<dyn File>>>,
    //Option: 可以用 Some 来标记 fd 已分配，None 表示未分配
    //Arc： 使得可以有多个引用，使得多个线程可以拥有对于这个文件的引用
    //dyn: 全称 dynamic 即多态，如果不用 dyn，rust 需要在编译时确定具体累型的大小，但是加入节点前无法知道，
    //      因此Arc<dyn File> 表示某个实现了 File 这个 trait 的具体类型，但是 rust 并不知道是谁
    // TODO: Design the internal structure
    // Hint: use Vec<Option<Arc<dyn File>>>
    //       the index is the fd number, None means the fd is closed or unallocated
}

impl FdTable {
    /// Create an empty fd table
    pub fn new() -> Self {
        // TODO
        FdTable {
            fdtable: Vec::new(),
        }
    }

    /// Allocate a new fd, return the fd number.
    ///
    /// Prefers reusing the smallest closed fd number; if no free slot, appends to the end.
    pub fn alloc(&mut self, file: Arc<dyn File>) -> usize {
        for (i, slot) in self.fdtable.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(file);
                return i;
            }
        }

        self.fdtable.push(Some(file));
        self.fdtable.len() - 1
    }

    /// Get the file object for an fd. Returns None if the fd doesn't exist or is closed.
    pub fn get(&self, fd: usize) -> Option<Arc<dyn File>> {
        // TODO
        self.fdtable
            .get(fd)
            .and_then(|opt| opt.as_ref().cloned())
        
        //get 返回 Option<&Option<Arc<dyn File>>>
        //  fd            返回
        // 越界            None
        // 有值            Some(&Option<…>)

        //and_then的作用：如果是 Some(x) → 执行函数 f(x)
        //               如果是 None → 直接返回 None

        //as_ref 是把Option<T> 转换为 Option<&T>是借用，而不转移其所有权，
        //如果直接操作，会使得｜T｜中的所有权发生转移，而 as_def 不拿走所有权，只借用里面的值

        //cloned 是复制一个值（由类型决定怎么复制）
        // 简单来说是一个语法糖 等价于 x.map(|v| v.clone())
        // cloned() = 把引用里的值 clone 出来，变成拥有所有权的值

    }

    /// Close an fd. Returns true on success, false if the fd doesn't exist or is already closed.
    pub fn close(&mut self, fd: usize) -> bool {
        match self.fdtable.get_mut(fd) {
            Some(slot) if slot.is_some() => {
                *slot = None;
                true
            }
            _ => false,
        }
    }

    /// Return the number of currently allocated fds (excluding closed ones)
    pub fn count(&self) -> usize {
        // TODO
        self.fdtable.iter().filter(|f| f.is_some()).count()
    }
}

impl Default for FdTable {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// Test File implementation
// ============================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct MockFile {
        id: usize,
        write_log: Mutex<Vec<Vec<u8>>>,
    }

    impl MockFile {
        fn new(id: usize) -> Arc<Self> {
            Arc::new(Self {
                id,
                write_log: Mutex::new(vec![]),
            })
        }
    }

    impl File for MockFile {
        fn read(&self, buf: &mut [u8]) -> isize {
            buf[0] = self.id as u8;
            1
        }
        fn write(&self, buf: &[u8]) -> isize {
            self.write_log.lock().unwrap().push(buf.to_vec());
            buf.len() as isize
        }
    }

    #[test]
    fn test_alloc_basic() {
        let mut table = FdTable::new();
        let fd = table.alloc(MockFile::new(0));
        assert_eq!(fd, 0, "first fd should be 0");
        let fd2 = table.alloc(MockFile::new(1));
        assert_eq!(fd2, 1, "second fd should be 1");
    }

    #[test]
    fn test_get() {
        let mut table = FdTable::new();
        let file = MockFile::new(42);
        let fd = table.alloc(file);
        let got = table.get(fd);
        assert!(got.is_some(), "get should return Some");
        let mut buf = [0u8; 1];
        got.unwrap().read(&mut buf);
        assert_eq!(buf[0], 42);
    }

    #[test]
    fn test_get_invalid() {
        let table = FdTable::new();
        assert!(table.get(0).is_none());
        assert!(table.get(999).is_none());
    }

    #[test]
    fn test_close_and_reuse() {
        let mut table = FdTable::new();
        let fd0 = table.alloc(MockFile::new(0)); // fd=0
        let fd1 = table.alloc(MockFile::new(1)); // fd=1
        let fd2 = table.alloc(MockFile::new(2)); // fd=2

        assert!(table.close(fd1), "closing fd=1 should succeed");
        assert!(
            table.get(fd1).is_none(),
            "get should return None after close"
        );

        // Next allocation should reuse fd=1 (smallest free)
        let fd_new = table.alloc(MockFile::new(99));
        assert_eq!(fd_new, fd1, "should reuse the smallest closed fd");

        let _ = (fd0, fd2);
    }

    #[test]
    fn test_close_invalid() {
        let mut table = FdTable::new();
        assert!(
            !table.close(0),
            "closing non-existent fd should return false"
        );
    }

    #[test]
    fn test_count() {
        let mut table = FdTable::new();
        assert_eq!(table.count(), 0);
        let fd0 = table.alloc(MockFile::new(0));
        let fd1 = table.alloc(MockFile::new(1));
        assert_eq!(table.count(), 2);
        table.close(fd0);
        assert_eq!(table.count(), 1);
        table.close(fd1);
        assert_eq!(table.count(), 0);
    }

    #[test]
    fn test_write_through_fd() {
        let mut table = FdTable::new();
        let file = MockFile::new(0);
        let fd = table.alloc(file);
        let f = table.get(fd).unwrap();
        let n = f.write(b"hello");
        assert_eq!(n, 5);
    }
}
