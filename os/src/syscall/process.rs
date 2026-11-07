//! Process management syscalls
use crate::config::PAGE_SIZE;
use crate::mm::{PageTable, VirtPageNum};
use crate::task::{
    change_program_brk, current_user_token, exit_current_and_run_next, suspend_current_and_run_next,
};

/// Copy `buf.len()` bytes from user memory starting at `va`.
/// Returns false if any page in the range is unmapped or unreadable.
fn user_read(va: usize, buf: &mut [u8]) -> bool {
    // Only the lower half of SV39 (user space) may be touched; this also
    // rejects addresses such as `isize::MAX` that alias the trampoline page.
    if va >= (1usize << 38) || va.checked_add(buf.len()).is_none() || va + buf.len() > (1usize << 38)
    {
        return false;
    }
    let token = current_user_token();
    let page_table = PageTable::from_token(token);
    let mut copied = 0;
    while copied < buf.len() {
        let addr = va + copied;
        let pte = match page_table.translate(VirtPageNum::from(addr / PAGE_SIZE)) {
            Some(pte) if pte.readable() => pte,
            _ => return false,
        };
        let offset = addr % PAGE_SIZE;
        let n = core::cmp::min(PAGE_SIZE - offset, buf.len() - copied);
        let pa = pte.ppn().0 * PAGE_SIZE + offset;
        unsafe {
            core::ptr::copy_nonoverlapping(pa as *const u8, buf[copied..].as_mut_ptr(), n);
        }
        copied += n;
    }
    true
}

/// Copy `data` into user memory starting at `va`.
/// Returns false if any page in the range is unmapped or unwritable.
fn user_write(va: usize, data: &[u8]) -> bool {
    if va >= (1usize << 38)
        || va.checked_add(data.len()).is_none()
        || va + data.len() > (1usize << 38)
    {
        return false;
    }
    let token = current_user_token();
    let page_table = PageTable::from_token(token);
    let mut copied = 0;
    while copied < data.len() {
        let addr = va + copied;
        let pte = match page_table.translate(VirtPageNum::from(addr / PAGE_SIZE)) {
            Some(pte) if pte.writable() => pte,
            _ => return false,
        };
        let offset = addr % PAGE_SIZE;
        let n = core::cmp::min(PAGE_SIZE - offset, data.len() - copied);
        let pa = pte.ppn().0 * PAGE_SIZE + offset;
        unsafe {
            core::ptr::copy_nonoverlapping(data[copied..].as_ptr(), pa as *mut u8, n);
        }
        copied += n;
    }
    true
}

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
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
    let us = crate::timer::get_time_us();
    let time_val = TimeVal {
        sec: us / 1_000_000,
        usec: us % 1_000_000,
    };
    let bytes = unsafe {
        core::slice::from_raw_parts(
            &time_val as *const TimeVal as *const u8,
            core::mem::size_of::<TimeVal>(),
        )
    };
    if user_write(ts as usize, bytes) {
        0
    } else {
        -1
    }
}

/// sys_trace: read/write user memory or query syscall counters
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace");
    match trace_request {
        0 => {
            let mut byte = [0u8; 1];
            if user_read(id, &mut byte) {
                byte[0] as isize
            } else {
                -1
            }
        }
        1 => {
            if user_write(id, &[data as u8]) {
                0
            } else {
                -1
            }
        }
        2 => crate::task::current_syscall_count(id) as isize,
        _ => -1,
    }
}

/// map anonymous memory
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    trace!("kernel: sys_mmap");
    crate::task::mmap(start, len, port)
}

/// unmap memory
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap");
    crate::task::munmap(start, len)
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
