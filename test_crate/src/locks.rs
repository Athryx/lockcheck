use std::sync::Mutex;

fn deadlock() {
    let a = Mutex::new(1);
    let _guard = a.lock();
    let _guard2 = a.lock();
}

fn deadlock2<T>(a: T) {
    let lock = Mutex::new(a);
    let _guard = lock.lock();
    let _guard = lock.lock();
}

fn deadlock3a<U, V>(a: U, b: V) {
    let lock1 = Mutex::new(a);
    let lock2 = Mutex::new(b);
    let _guard1 = lock1.lock();
    let _guard2 = lock2.lock();
}

fn deadlock3b<U, V>(a: U, b: V) {
    let lock1 = Mutex::new(a);
    let lock2 = Mutex::new(b);
    let _guard2 = lock2.lock();
    let _guard1 = lock1.lock();
}