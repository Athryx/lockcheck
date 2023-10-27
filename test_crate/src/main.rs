use std::sync::{Mutex, Arc};

fn test(mutex: &Mutex<u8>) {
    let guard = mutex.lock();
}

fn okay1() {
    let mutex = Mutex::new(0u8);
    let guard = mutex.lock();
}

fn deadlock1() {
    let mutex = Mutex::new(0i32);
    let guard1 = mutex.lock();
    let guard2 = mutex.lock();
}

fn okay2() {
    let mutex = Mutex::new(0u32);
    let guard1 = mutex.lock();
    drop(guard1);
    let guard2 = mutex.lock();
}

fn deadlock2() {
    let mutex = Arc::new(Mutex::new(0i32));
    let guard1 = mutex.lock();
    let guard2 = mutex.lock();
}

fn main() {}