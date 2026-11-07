//! A simple wait-for graph based deadlock detector.
//!
//! Every task is identified by the address of its `Arc<TaskControlBlock>`.
//! When a task blocks waiting for a lock held by another task, an edge
//! `waiter -> holder` is recorded.  A lock request is rejected with
//! `-0xdead` when granting it would close a cycle in this graph.

use super::UPSafeCell;
use crate::task::TaskControlBlock;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use lazy_static::*;

lazy_static! {
    static ref WAIT_GRAPH: UPSafeCell<Vec<(usize, usize)>> =
        unsafe { UPSafeCell::new(Vec::new()) };
}

static ENABLED: AtomicBool = AtomicBool::new(false);

/// Enable or disable deadlock detection.
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::SeqCst);
}

/// Whether deadlock detection is enabled.
pub fn enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}

/// A unique key for a task.
pub fn key(task: &Arc<TaskControlBlock>) -> usize {
    Arc::as_ptr(task) as usize
}

/// Record that `waiter` is blocked waiting for a resource held by `holder`.
pub fn add_edge(waiter: usize, holder: usize) {
    if waiter == holder {
        return;
    }
    WAIT_GRAPH.exclusive_access().push((waiter, holder));
}

/// Remove all edges whose waiter is `waiter` (the task has been woken up).
pub fn remove_edges_of(waiter: usize) {
    WAIT_GRAPH
        .exclusive_access()
        .retain(|(w, _)| *w != waiter);
}

/// Is there a chain of wait-for edges from `from` to `to`?
pub fn has_path(from: usize, to: usize) -> bool {
    let graph = WAIT_GRAPH.exclusive_access();
    let mut stack = alloc::vec![from];
    let mut visited: Vec<usize> = Vec::new();
    while let Some(node) = stack.pop() {
        if node == to {
            return true;
        }
        if visited.contains(&node) {
            continue;
        }
        visited.push(node);
        for (w, h) in graph.iter() {
            if *w == node && !visited.contains(h) {
                stack.push(*h);
            }
        }
    }
    false
}

/// Would making `waiter` wait for `holder` introduce a deadlock?
pub fn would_deadlock(waiter: usize, holder: usize) -> bool {
    waiter == holder || has_path(holder, waiter)
}
