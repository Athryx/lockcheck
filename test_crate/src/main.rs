use std::sync::{Mutex, Arc, MutexGuard, LockResult};
use parking_lot::RwLock;

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

struct Deadlock3;
fn deadlock3(n: i32) {
    let mutex = Mutex::new(Deadlock3);
    let guard1 = mutex.lock();

    if n == 2 {
        let guard2 = mutex.lock();
    } else {
        println!("happy birthday");
    }
}

struct Deadlock4;
fn deadlock4(n: i32) {
    let mutex = Mutex::new(Deadlock4);
    let guard1 = mutex.lock();

    if n == 2 {
        println!("happy birthday");
    } else if n == 3 {
        let guard2 = mutex.lock();
    } else {
        println!("sad day");
    }
}

struct Okay5a;
struct Okay5b;
fn okay5(n: i32) {
    let mutex1 = Mutex::new(Okay5a);
    let mutex2 = Mutex::new(Okay5b);

    if n == 4 {
        let guard1 = mutex1.lock();
        let guard2 = mutex2.lock();
    } else {
        let guard1 = mutex1.lock();
        let guard2 = mutex2.lock();
    }
}

struct Deadlock5a;
struct Deadlock5b;
fn deadlock5(n: i32) {
    let mutex1 = Mutex::new(Deadlock5a);
    let mutex2 = Mutex::new(Deadlock5b);

    if n == 4 {
        let guard1 = mutex1.lock();
        let guard2 = mutex2.lock();
    } else {
        let guard2 = mutex2.lock();
        let guard1 = mutex1.lock();
    }
}

struct Deadlock6;
fn deadlock6() {
    let mutex = Mutex::new(Deadlock6);

    let guard2 = {
        let guard1 = mutex.lock();
        let guard2 = guard1;
        guard2
    };

    let guard3 = mutex.lock();
}

struct Okay6;
fn okay6() {
    let mutex = Mutex::new(Okay6);

    {
        let guard1 = mutex.lock();
    }

    let guard2 = mutex.lock();
}

struct Okay7;
fn okay7() {
    let mutex = Mutex::new(Okay7);

    loop {
        let guard = mutex.lock();
    }
}

struct Okay8;
fn okay8() {
    let mutex = Mutex::new(Okay8);

    for a in [true, false, true, false] {
        if a {
            let guard1 = mutex.lock();
        } else {
            let guard2 = mutex.lock();
        }
    }
}

struct Test<'a> {
    guard: LockResult<MutexGuard<'a, Deadlock9>>,
}

struct Deadlock9;
fn deadlock9() {
    let mutex = Mutex::new(Deadlock9);

    let guard = Test {
        guard: mutex.lock(),
    };

    let guard2 = mutex.lock();
}

struct Test2<'a> {
    guard: LockResult<MutexGuard<'a, Okay9>>,
}

struct Okay9;
fn okay9() {
    let mutex = Mutex::new(Okay9);

    let guard = Test2 {
        guard: mutex.lock(),
    };

    drop(guard);

    let guard2 = mutex.lock();
}

struct Deadlock10;
fn deadlock10() {
    let mutex = Mutex::new(Deadlock10);

    let guard1 = mutex.lock().unwrap();
    let guard2 = mutex.lock().unwrap();
}

struct Deadlock11;
fn deadlock11() {
    fn inner() {
        let mutex = Mutex::new(Deadlock11);

        let guard1 = mutex.lock();
        let guard2 = mutex.lock();
    }
}

struct Deadlock12;
fn deadlock12() {
    let a = || {
        let mutex = Mutex::new(Deadlock12);

        let guard1 = mutex.lock();
        let guard2 = mutex.lock();
    };
    a();
}

struct Deadlock13a;
struct Deadlock13b;
fn deadlock13() {
    let mutex = Mutex::new(Deadlock13a);
    let mutex2 = Mutex::new(Deadlock13b);

    let guard = mutex.lock();

    fn inner() {
        let mutex = Mutex::new(Deadlock13b);

        let guard = mutex.lock();
    }

    inner();
    drop(guard);

    let guard2 = mutex2.lock();
    let guard3 = mutex.lock();
}

fn main() {}