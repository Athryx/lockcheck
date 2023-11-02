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

    if (n == 4) {
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

    if (n == 4) {
        let guard1 = mutex1.lock();
        let guard2 = mutex2.lock();
    } else {
        let guard2 = mutex2.lock();
        let guard1 = mutex1.lock();
    }
}

fn main() {}