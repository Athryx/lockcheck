use std::sync::{Mutex, Arc, MutexGuard, LockResult};

struct Okay1;
fn okay1() {
    let mutex = Mutex::new(Okay1);
    let _guard = mutex.lock();
}

struct Deadlock1;
fn deadlock1() {
    let mutex = Mutex::new(Deadlock1);
    let _guard1 = mutex.lock();
    let _guard2 = mutex.lock();
}

struct Okay2;
fn okay2() {
    let mutex = Mutex::new(Okay2);
    let guard1 = mutex.lock();
    drop(guard1);
    let _guard2 = mutex.lock();
}

struct Deadlock2;
fn deadlock2() {
    let mutex = Arc::new(Mutex::new(Deadlock2));
    let _guard1 = mutex.lock();
    let _guard2 = mutex.lock();
}

struct Deadlock3;
fn deadlock3(n: i32) {
    let mutex = Mutex::new(Deadlock3);
    let _guard1 = mutex.lock();

    if n == 2 {
        let _guard2 = mutex.lock();
    } else {
        println!("happy birthday");
    }
}

struct Deadlock4;
fn deadlock4(n: i32) {
    let mutex = Mutex::new(Deadlock4);
    let _guard1 = mutex.lock();

    if n == 2 {
        println!("happy birthday");
    } else if n == 3 {
        let _guard2 = mutex.lock();
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
        let _guard1 = mutex1.lock();
        let _guard2 = mutex2.lock();
    } else {
        let _guard1 = mutex1.lock();
        let _guard2 = mutex2.lock();
    }
}

struct Deadlock5a;
struct Deadlock5b;
fn deadlock5(n: i32) {
    let mutex1 = Mutex::new(Deadlock5a);
    let mutex2 = Mutex::new(Deadlock5b);

    if n == 4 {
        let _guard1 = mutex1.lock();
        let _guard2 = mutex2.lock();
    } else {
        let _guard2 = mutex2.lock();
        let _guard1 = mutex1.lock();
    }
}

struct Deadlock6;
fn deadlock6() {
    let mutex = Mutex::new(Deadlock6);

    let _guard2 = {
        let _guard1 = mutex.lock();
        let _guard2 = _guard1;
        _guard2
    };

    let _guard3 = mutex.lock();
}

struct Okay6;
fn okay6() {
    let mutex = Mutex::new(Okay6);

    {
        let _guard1 = mutex.lock();
    }

    let _guard2 = mutex.lock();
}

struct Okay7;
fn okay7() {
    let mutex = Mutex::new(Okay7);

    loop {
        let _guard = mutex.lock();
    }
}

struct Okay8;
fn okay8() {
    let mutex = Mutex::new(Okay8);

    for a in [true, false, true, false] {
        if a {
            let _guard1 = mutex.lock();
        } else {
            let _guard2 = mutex.lock();
        }
    }
}

struct Test<'a> {
    _guard: LockResult<MutexGuard<'a, Deadlock9>>,
}

struct Deadlock9;
fn deadlock9() {
    let mutex = Mutex::new(Deadlock9);

    let _guard = Test {
        _guard: mutex.lock(),
    };

    let _guard2 = mutex.lock();
}

struct Test2<'a> {
    _guard: LockResult<MutexGuard<'a, Okay9>>,
}

struct Okay9;
fn okay9() {
    let mutex = Mutex::new(Okay9);

    let guard = Test2 {
        _guard: mutex.lock(),
    };

    drop(guard);

    let _guard2 = mutex.lock();
}

struct Deadlock10;
fn deadlock10() {
    let mutex = Mutex::new(Deadlock10);

    let _guard1 = mutex.lock().unwrap();
    let _guard2 = mutex.lock().unwrap();
}

struct Deadlock11;
fn deadlock11() {
    fn inner() {
        let mutex = Mutex::new(Deadlock11);

        let _guard1 = mutex.lock();
        let _guard2 = mutex.lock();
    }
}

struct Deadlock12;
fn deadlock12() {
    let a = || {
        let mutex = Mutex::new(Deadlock12);

        let _guard1 = mutex.lock();
        let _guard2 = mutex.lock();
    };
    a();
}

struct Deadlock13a;
struct Deadlock13b;
fn deadlock13() {
    let mutex = Mutex::new(Deadlock13a);
    let mutex2 = Mutex::new(Deadlock13b);

    let guard = mutex.lock();

    fn inner(mutex: &Mutex<Deadlock13b>) {
        let _guard = mutex.lock();
    }

    inner(&mutex2);
    drop(guard);

    let _guard2 = mutex2.lock();
    let _guard3 = mutex.lock();
}

struct Okay13a;
struct Okay13b;
fn okay13() {
    let mutex = Mutex::new(Okay13a);
    let mutex2 = Mutex::new(Okay13b);

    let guard = mutex.lock();

    fn inner(mutex: &Mutex<Okay13b>) {
        let _guard = mutex.lock();
    }

    inner(&mutex2);
    drop(guard);

    let _guard2 = mutex.lock();
    let _guard3 = mutex2.lock();
}

struct CustomGuard<'a, T>(MutexGuard<'a, T>);

struct Deadlock14;
fn deadlock14() {
    fn inner(mutex: &Mutex<Deadlock14>) -> CustomGuard<Deadlock14> {
        CustomGuard(mutex.lock().unwrap())
    }

    let mutex = Mutex::new(Deadlock14);
    let _guard1 = inner(&mutex);
    let _guard2 = inner(&mutex);
}

fn main() {}