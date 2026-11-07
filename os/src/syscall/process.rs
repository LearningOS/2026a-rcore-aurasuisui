//! Process management syscalls
use crate::{
    task::{exit_current_and_run_next, suspend_current_and_run_next},
    timer::get_time_us,
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// get time with second and microsecond
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    unsafe {
        *ts = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
    }
    0
}

/// Check whether `addr` points into the user address space (ch3 has no
/// address-space isolation yet, so a range check is enough).
fn user_ptr_valid(addr: usize) -> bool {
    addr != 0 && addr < crate::config::MEMORY_END
}

/// sys_trace(trace_request, id, data)
///
/// - trace_request == 0 (Read): read a byte from user address `id`
/// - trace_request == 1 (Write): write byte `data` to user address `id`
/// - trace_request == 2 (Syscall): return how many times syscall `id` was called
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace");
    match trace_request {
        0 => {
            if !user_ptr_valid(id) {
                return -1;
            }
            unsafe { *(id as *const u8) as isize }
        }
        1 => {
            if !user_ptr_valid(id) {
                return -1;
            }
            unsafe {
                *(id as *mut u8) = data as u8;
            }
            0
        }
        2 => crate::task::current_syscall_count(id) as isize,
        _ => -1,
    }
}
