use std::sync::{Mutex, Arc};

struct Okay1;
fn okay1() {
    let mutex = Mutex::new(Okay1);
    let guard = mutex.lock();
}

struct Deadlock1;
fn deadlock1() {
    let mutex = Mutex::new(Deadlock1);
    let guard1 = mutex.lock();
    let guard2 = mutex.lock();
}

struct Okay2;
fn okay2() {
    let mutex = Mutex::new(Okay2);
    let guard1 = mutex.lock();
    drop(guard1);
    let guard2 = mutex.lock();
}

struct Deadlock2;
fn deadlock2() {
    let mutex = Arc::new(Mutex::new(Deadlock2));
    let guard1 = mutex.lock();
    let guard2 = mutex.lock();
}

fn main() {}