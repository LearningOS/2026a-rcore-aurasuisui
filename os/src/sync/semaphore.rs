//! Semaphore

use crate::sync::{deadlock, UPSafeCell};
use crate::task::{block_current_and_run_next, current_task, wakeup_task, TaskControlBlock};
use alloc::{collections::VecDeque, sync::Arc, vec::Vec};

/// semaphore structure
pub struct Semaphore {
    /// semaphore inner
    pub inner: UPSafeCell<SemaphoreInner>,
}

pub struct SemaphoreInner {
    pub count: isize,
    pub wait_queue: VecDeque<Arc<TaskControlBlock>>,
    /// tasks that currently hold a unit of this semaphore
    pub holders: Vec<Arc<TaskControlBlock>>,
}

impl Semaphore {
    /// Create a new semaphore
    pub fn new(res_count: usize) -> Self {
        trace!("kernel: Semaphore::new");
        Self {
            inner: unsafe {
                UPSafeCell::new(SemaphoreInner {
                    count: res_count as isize,
                    wait_queue: VecDeque::new(),
                    holders: Vec::new(),
                })
            },
        }
    }

    /// up operation of semaphore
    pub fn up(&self) {
        trace!("kernel: Semaphore::up");
        let mut inner = self.inner.exclusive_access();
        let current = current_task().unwrap();
        if let Some(pos) = inner.holders.iter().position(|t| Arc::ptr_eq(t, &current)) {
            inner.holders.remove(pos);
        }
        inner.count += 1;
        if inner.count <= 0 {
            if let Some(task) = inner.wait_queue.pop_front() {
                deadlock::remove_edges_of(deadlock::key(&task));
                wakeup_task(task);
            }
        }
    }

    /// down operation of semaphore
    ///
    /// Returns 0 on success, or `-0xdead` when granting the request would deadlock.
    pub fn down(&self) -> isize {
        trace!("kernel: Semaphore::down");
        let mut inner = self.inner.exclusive_access();
        let current = current_task().unwrap();
        if inner.count > 0 {
            inner.count -= 1;
            inner.holders.push(current);
            return 0;
        }
        if deadlock::enabled() {
            let cur_key = deadlock::key(&current);
            for holder in inner.holders.iter() {
                let holder_key = deadlock::key(holder);
                if deadlock::would_deadlock(cur_key, holder_key) {
                    return -0xdead;
                }
            }
            for holder in inner.holders.iter() {
                deadlock::add_edge(cur_key, deadlock::key(holder));
            }
        }
        inner.count -= 1;
        inner.wait_queue.push_back(current.clone());
        drop(inner);
        block_current_and_run_next();
        self.inner.exclusive_access().holders.push(current);
        0
    }
}
