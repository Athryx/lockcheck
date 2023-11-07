# Lockcheck

To build, you must install the rustc-dev component

  rustup component add --toolchain nightly rust-src rustc-dev llvm-tools-preview

# TODO

Improve multiple passes

rwlock
condvar
reentrant mutex
perhaps just even a more general way to specify blocking behavior of a concurrency primitive,
and have the cargo lockcheck parse these rules to be very flexible and work with most primitives

make external crates work better

For diverging terminators, maybe print warning if lock is held and they are reached
Detect loops in basic blocks (of course impossible to do right, probable don't do this)

## Easy Stuff (Ideally Finish Before Interview)

- More testing
- Analyse closures as well (closures' fn impl don't have a defid so currently they are skipped)
- Improve config file
    - allow specifying multiple lock methods
    - allow specifying if lock method returns result or not

## Cargo Stuff

- Maybe allow passing more arguments to cargo lockcheck
- Fix random file thing being generated

## Analysis

- Properly handle projections of locals
    - This will also fix issue of handling derefs
        - Still in general a hard problem to handle derefs, more work needed
- Handle mutexes with generic parameters depending on other generics

## Messages

- Include a note section in the error message with the correct order to lock locks

## Done

- Analyse functions where guards are passed into
- Analyse functions guards are returned from
- Print error messages in order they occur in file, not some random order based on what hashmap iter decides

# Notes

rust version:
rustc 1.75.0-nightly (475c71da0 2023-10-11)