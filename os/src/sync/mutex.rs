//! Mutex (spin-like and blocking(sleep))

use super::{deadlock, UPSafeCell};
use crate::task::TaskControlBlock;
use crate::task::{block_current_and_run_next, suspend_current_and_run_next};
use crate::task::{current_task, wakeup_task};
use alloc::{collections::VecDeque, sync::Arc};

/// Mutex trait
pub trait Mutex: Sync + Send {
    /// Lock the mutex
    fn lock(&self);
    /// Lock the mutex, returning `-0xdead` when a deadlock is detected.
    fn lock_check(&self) -> isize {
        self.lock();
        0
    }
    /// Unlock the mutex
    fn unlock(&self);
}

/// Spinlock Mutex struct
pub struct MutexSpin {
    locked: UPSafeCell<bool>,
}

impl MutexSpin {
    /// Create a new spinlock mutex
    pub fn new() -> Self {
        Self {
            locked: unsafe { UPSafeCell::new(false) },
        }
    }
}

impl Mutex for MutexSpin {
    /// Lock the spinlock mutex
    fn lock(&self) {
        trace!("kernel: MutexSpin::lock");
        loop {
            let mut locked = self.locked.exclusive_access();
            if *locked {
                drop(locked);
                suspend_current_and_run_next();
                continue;
            } else {
                *locked = true;
                return;
            }
        }
    }

    fn unlock(&self) {
        trace!("kernel: MutexSpin::unlock");
        let mut locked = self.locked.exclusive_access();
        *locked = false;
    }
}

/// Blocking Mutex struct
pub struct MutexBlocking {
    inner: UPSafeCell<MutexBlockingInner>,
}

pub struct MutexBlockingInner {
    locked: bool,
    wait_queue: VecDeque<Arc<TaskControlBlock>>,
    /// the task currently holding this mutex
    owner: Option<Arc<TaskControlBlock>>,
}

impl MutexBlocking {
    /// Create a new blocking mutex
    pub fn new() -> Self {
        trace!("kernel: MutexBlocking::new");
        Self {
            inner: unsafe {
                UPSafeCell::new(MutexBlockingInner {
                    locked: false,
                    wait_queue: VecDeque::new(),
                    owner: None,
                })
            },
        }
    }
}

impl Mutex for MutexBlocking {
    /// lock the blocking mutex
    fn lock(&self) {
        self.lock_check();
    }

    /// lock the blocking mutex, detecting deadlocks
    fn lock_check(&self) -> isize {
        trace!("kernel: MutexBlocking::lock");
        let mut mutex_inner = self.inner.exclusive_access();
        let current = current_task().unwrap();
        if mutex_inner.locked {
            if deadlock::enabled() {
                let cur_key = deadlock::key(&current);
                if let Some(owner) = mutex_inner.owner.clone() {
                    let owner_key = deadlock::key(&owner);
                    if deadlock::would_deadlock(cur_key, owner_key) {
                        return -0xdead;
                    }
                    deadlock::add_edge(cur_key, owner_key);
                }
            }
            mutex_inner.wait_queue.push_back(current);
            drop(mutex_inner);
            block_current_and_run_next();
        } else {
            mutex_inner.locked = true;
            mutex_inner.owner = Some(current);
        }
        0
    }

    /// unlock the blocking mutex
    fn unlock(&self) {
        trace!("kernel: MutexBlocking::unlock");
        let mut mutex_inner = self.inner.exclusive_access();
        assert!(mutex_inner.locked);
        if let Some(waking_task) = mutex_inner.wait_queue.pop_front() {
            deadlock::remove_edges_of(deadlock::key(&waking_task));
            mutex_inner.owner = Some(waking_task.clone());
            wakeup_task(waking_task);
        } else {
            mutex_inner.locked = false;
            mutex_inner.owner = None;
        }
    }
}
