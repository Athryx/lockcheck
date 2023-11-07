use std::sync::{Mutex, MutexGuard};

// analysis should continue 
fn return_guard(mut guard: MutexGuard<usize>) -> MutexGuard<usize> {
    *guard += 4;
    guard
}

// analysis should stop the analysing the current path when this function is encountered
fn drop_guard(mut guard: MutexGuard<usize>) {
    *guard += 3;
}

fn lock_mutex(mutex: &Mutex<usize>) {
    let mut guard = mutex.lock().unwrap();
    *guard -= 1;
}

fn test() {
    let mutex = Mutex::new(0usize);
    // analysis should start at this invocation
    let guard = mutex.lock().unwrap();

    // lockcheck will look inside this function and see the guard is returned
    let guard2 = return_guard(guard);

    lock_mutex(&mutex);

    drop_guard(guard2);
}
