//! Process management syscalls
use crate::mm::{
    translated_byte_buffer,
    PageTable,
    PTEFlags,
    VirtAddr,
};
use crate::task::{
    change_program_brk,
    current_user_token,
    exit_current_and_run_next,
    suspend_current_and_run_next,
    get_syscall_count,
    mmap_current,
    munmap_current,
};
use crate::timer::get_time_us;

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

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");

    let us = get_time_us();

    let time_val = TimeVal {
        sec: us / 1_000_000,
        usec: us % 1_000_000,
    };

    // Convert TimeVal into bytes.
    // This avoids assuming that the user buffer is
    // physically contiguous.
    let mut bytes = [0u8; core::mem::size_of::<TimeVal>()];
    let word_size = core::mem::size_of::<usize>();

    bytes[..word_size].copy_from_slice(&time_val.sec.to_ne_bytes());
    bytes[word_size..].copy_from_slice(&time_val.usec.to_ne_bytes());

    // Translate the user virtual address into
    // page-local physical memory slices.
    let buffers = translated_byte_buffer(
        current_user_token(),
        ts as *const u8,
        bytes.len(),
    );

    let mut offset = 0;
    for buffer in buffers {
        let len = buffer.len();
        buffer.copy_from_slice(&bytes[offset..offset + len]);
        offset += len;
    }

    0
}

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace");

    match trace_request {
        // Read one byte from user virtual memory.
        0 => {
            let page_table = PageTable::from_token(current_user_token());
            let va = VirtAddr::from(id);
            let vpn = va.floor();

            let Some(pte) = page_table.translate(vpn) else {
                return -1;
            };

            let flags = pte.flags();

            if !pte.is_valid()
                || !flags.contains(PTEFlags::U)
                || !flags.contains(PTEFlags::R)
            {
                return -1;
            }

            let bytes = pte.ppn().get_bytes_array();
            bytes[va.page_offset()] as isize
        }

        // Write one byte into user virtual memory.
        1 => {
            let page_table = PageTable::from_token(current_user_token());
            let va = VirtAddr::from(id);
            let vpn = va.floor();

            let Some(pte) = page_table.translate(vpn) else {
                return -1;
            };

            let flags = pte.flags();

            if !pte.is_valid()
                || !flags.contains(PTEFlags::U)
                || !flags.contains(PTEFlags::W)
            {
                return -1;
            }

            let bytes = pte.ppn().get_bytes_array();
            bytes[va.page_offset()] = data as u8;
            0
        }

        // Count syscalls made by the current task.
        2 => get_syscall_count(id)
            .map(|count| count as isize)
            .unwrap_or(-1),

        _ => -1,
    }
}

/// Map anonymous memory into the current user's address space.
pub fn sys_mmap(start: usize, len: usize, prot: usize) -> isize {
    trace!("kernel: sys_mmap");
    mmap_current(start, len, prot)
}

/// Remove a mapping from the current user's address space.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap");
    munmap_current(start, len)
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
