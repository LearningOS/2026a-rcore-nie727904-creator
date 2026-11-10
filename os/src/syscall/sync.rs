use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;
use alloc::vec::Vec;
/// sleep syscall
pub fn sys_sleep(ms: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_sleep",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}
/// mutex create syscall
pub fn sys_mutex_create(blocking: bool) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        process_inner.mutex_owner[id] = None;
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
        process_inner.mutex_owner.push(None);
        process_inner.mutex_list.len() as isize - 1
    }
}
/// mutex lock syscall
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    let task = current_task().unwrap();
    let tid = task
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid;

    let process = current_process();

    let mutex = {
        let inner = process.inner_exclusive_access();

        if mutex_id >= inner.mutex_list.len() {
            return -1;
        }

        let mutex = match &inner.mutex_list[mutex_id] {
            Some(mutex) => Arc::clone(mutex),
            None => return -1,
        };

        // A thread cannot acquire a mutex it already owns.
        if inner.deadlock_detection_enabled
            && inner.mutex_owner[mutex_id] == Some(tid)
        {
            return -0xdead;
        }

        mutex
    };

    // Never hold the PCB borrow while potentially blocking.
    mutex.lock();

    let mut inner = process.inner_exclusive_access();
    inner.mutex_owner[mutex_id] = Some(tid);

    0
}

/// mutex unlock syscall
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    let process = current_process();

    let mutex = {
        let inner = process.inner_exclusive_access();

        if mutex_id >= inner.mutex_list.len() {
            return -1;
        }

        match &inner.mutex_list[mutex_id] {
            Some(mutex) => Arc::clone(mutex),
            None => return -1,
        }
    };

    mutex.unlock();

    let mut inner = process.inner_exclusive_access();
    inner.mutex_owner[mutex_id] = None;

    0
}
/// semaphore create syscall
pub fn sys_semaphore_create(res_count: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        process_inner.semaphore_list.len() - 1
    };
    id as isize
}
/// semaphore up syscall
pub fn sys_semaphore_up(sem_id: usize) -> isize {
    let task = current_task().unwrap();
    let tid = task.inner_exclusive_access().res.as_ref().unwrap().tid;
    let process = current_process();

    let sem = {
        let inner = process.inner_exclusive_access();
        match inner.semaphore_list.get(sem_id) {
            Some(Some(sem)) => Arc::clone(sem),
            _ => return -1,
        }
    };

    sem.up();

    let mut inner = process.inner_exclusive_access();
    if inner.deadlock_detection_enabled {
        if let Some(row) = inner.sem_allocations.get_mut(tid) {
            if let Some(held) = row.get_mut(sem_id) {
                if *held > 0 {
                    *held -= 1;
                }
            }
        }
    }
    0
}
/// semaphore down syscall
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    let task = current_task().unwrap();
    let tid = task.inner_exclusive_access().res.as_ref().unwrap().tid;
    let process = current_process();

    let sem = {
        let mut inner = process.inner_exclusive_access();
        let sem = match inner.semaphore_list.get(sem_id) {
            Some(Some(sem)) => Arc::clone(sem),
            _ => return -1,
        };

        if inner.deadlock_detection_enabled {
            // Only grow the tables; never discard other threads' records.
            if inner.sem_allocations.len() <= tid {
                inner.sem_allocations.resize_with(tid + 1, Vec::new);
            }
            if inner.sem_requests.len() <= tid {
                inner.sem_requests.resize(tid + 1, None);
            }

            let resource_count = inner.semaphore_list.len();
            for row in inner.sem_allocations.iter_mut() {
                row.resize(resource_count, 0);
            }

            // An immediately available resource does not create a wait.
            if sem.inner.exclusive_access().count <= 0 {
                inner.sem_requests[tid] = Some(sem_id);

                if inner.semaphore_deadlocked(tid) {
                    inner.sem_requests[tid] = None;
                    return -0xdead;
                }
            }
        }

        sem
    };

    // Release PCB borrow before possibly blocking.
    sem.down();

    let mut inner = process.inner_exclusive_access();
    if inner.deadlock_detection_enabled {
        inner.sem_requests[tid] = None;
        inner.sem_allocations[tid][sem_id] += 1;
    }
    0
}
/// condvar create syscall
pub fn sys_condvar_create() -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}
/// condvar signal syscall
pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_signal",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}
/// condvar wait syscall
pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_wait",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex);
    0
}
/// enable deadlock detection syscall
///
/// YOUR JOB: Implement deadlock detection, but might not all in this syscall
/// Enable or disable deadlock detection for the current process.
pub fn sys_enable_deadlock_detect(enabled: usize) -> isize {
    if enabled > 1 {
        return -1;
    }

    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    inner.deadlock_detection_enabled = enabled == 1;

    0
}