//! # Mutex Shared State
//!
//! In this exercise, you will use `Arc<Mutex<T>>` to safely share and modify data between multiple threads.
//!
//! ## Concepts
//! - `Mutex<T>` mutex protects shared data
//! - `Arc<T>` atomic reference counting enables cross-thread sharing
//! - `lock()` acquires the lock and accesses data


use std::sync::{Arc, Mutex};
use std::thread;

/// Increment a counter concurrently using `n_threads` threads.
/// Each thread increments the counter `count_per_thread` times.
/// Returns the final counter value.
///
/// Hint: Use `Arc<Mutex<usize>>` as the shared counter.

pub fn concurrent_counter(n_threads: usize, count_per_thread: usize) -> usize {
    let mut count = Arc::new(Mutex::new(0));
    //Arc 负责共享 count，Mutex 负责同步即加锁
    //Arc 和 Mutex 是“拥有数据的容器类型”，必须通过 new 来构造它们内部的数据
    //即 Mutex 新建的是一个可以带锁的值，而 Arc 新建的是一个可以共享的指针
    //使用 Arc 需要使用 Arc::new(),使用 Mutex需要使用 Mutex::new()
    //使用下面的都需要使用 new::来在堆上为变量分配内存
    //     Box<T>       堆分配
    //     Arc<T>       共享所有权
    //     Mutex<T>     并发保护
    //     RefCell<T>   内部可变性
    //     Vec<T>      动态数组

    let mut handles = vec![];
    for _ in 0..n_threads {
        let count = Arc::clone(&count);
        //clone 得到的是一个新的 Arc（拥有所有权），而不是 &Arc

        //使用 Arc::clone(&count) 
        //  是为了防止 Arc::clone(count) 会直接转移 count 的所有权

        //线程需要拥有数据，是因为线程可能比主线程更晚结束，主线程结束会释放变量
        //  因此子线程需要拥有数据，防止资源被提前释放产生悬垂引用

        //线程需要拥有数据，同时不能拿走变量的所有权，因此需要使用 Arc 智能指针来
        //  共享变量。同时使用 clone 来共享变量，clone 之后还是指向同一个数据
        //  只是多个 Arc 指针
        let handle = thread::spawn(move || {
            for i in 0..count_per_thread {
                let mut guard = count.lock().unwrap();
                *guard += 1;

                //这是 count.lock()返回的类型
                //  Result<MutexGuard<'_, T>, PoisonError<MutexGuard<'_, T>>>
                //  只有当锁被污染，即某个线程在持有锁的时候 panic，
                //  此时修改到一半，数据可能不安全，导致别的线程都无法使用锁

                //match count.lock() {
                //  Ok(guard) => { ... }
                //  Err(poisoned) => {
                //     let guard = poisoned.into_inner(); // 继续用
                //  }
                //这样可以恢复数据的访问权，恢复后的值是发生 panic 那一刻的数据，
                //  即当前内存里的真实值（可能是半更新状态）
            }

                //count.lock() 返回的类型是 Result，所以需要 unwrap 取出里面的
                //没有拿到锁的话，Mutex 不允许使用当前变量
                //count.lock()尝试获取变量的锁，使得当前只允许我自己使用这个变量
                //如果此时别人持有锁，当前线程会阻塞等待，直至这个锁被释放
                //并不是忙等，不会持续占用 CPU
                // 1. 尝试获取锁
                // 2. 如果失败 → 线程挂起（sleep）
                // 3. 等待操作系统唤醒
                //在 Mutex 离开作用域之后，lock 会自动 drop
                //使用 *号 是因为 count.lock() 返回的类型是
        });
        handles.push(handle);
    }
    //不能直接在 for 循环内使用 handle.join().unwrap()，这样需要线程 1 执行完毕后
    //  才会执行线程 2 ，变成了并行操作了，这个线程的意义就没了，
    //  统一join 可以保证线程并发
    for handle in handles {
        handle.join().unwrap();
    }

    //
    let result = *count.lock().unwrap();
    result
}

/// Add elements to a shared vector concurrently using multiple threads.
/// Each thread pushes its own id (0..n_threads) to the vector.
/// Returns the sorted vector.
///
/// Hint: Use `Arc<Mutex<Vec<usize>>>`.
pub fn concurrent_collect(n_threads: usize) -> Vec<usize> {
    let res = Arc::new(Mutex::new(vec![]));
    let mut handles = vec![];
    //这里不能使用 _ 来代替 i，_ 是不使用的变的标记，既然标记了就不能再使用了
    for i in 0..n_threads {
        let res = Arc::clone(&res);
        let handle = thread::spawn(move || {
            let mut guard = res.lock().unwrap();
            guard.push(i);
            //这里不需要使用 * 来解引用，是因为方法调用的时候，
            //就自动调用 Deref 进行解引用，将其
        });

        handles.push(handle);
    }
    for handle in handles {
        handle.join().unwrap();
    }
    let mut result = res.lock().unwrap().clone();
    //lock().unwrap() 返回的是MutexGuard类型
    //  因为当前的 MutexGuard 没有实现 clone 方法，但是实现了 Deref 方法
    //  所以编译器会自动解引用，得到 &Vec 类型，&Vec 类型可以直接调用 clone 进行复制
    //  此时得到的 result 就是 Vec<usize> 类型
    result.sort();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter_single_thread() {
        assert_eq!(concurrent_counter(1, 100), 100);
    }

    #[test]
    fn test_counter_multi_thread() {
        assert_eq!(concurrent_counter(10, 100), 1000);
    }

    #[test]
    fn test_counter_zero() {
        assert_eq!(concurrent_counter(5, 0), 0);
    }

    #[test]
    fn test_collect() {
        let result = concurrent_collect(5);
        assert_eq!(result, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_collect_single() {
        assert_eq!(concurrent_collect(1), vec![0]);
    }
}
