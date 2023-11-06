# Plan 1

- Scan mir for all instances where lock is constructed
    - When lock is found, traverse all mir basic blocks where lock is used until lock is dropped
    - If a call to lock is found, put in map the BasicBlock where it is called, and the id of the lock construction
- Next, go through map of basic block where locks is called, and traverse until lock guard is dropped
    - If the map contains the basic block that we visit, record in the map for the current block that the visited block is its child

Use rustc dataflow analysis

# Plan 2

For now only scan for deadlocks which occur entirely in 1 function

- Scan mir of each function, looking for call where function call defid matches lock def id
    - Find an argument to the function whith defid that matches mutex
        - Use the 1 generic parameter (mutex generic type) as the lock class for this invocation
    - Record basic block id and lock class for current invocation and continue scanning

- Now scan through all lock invocations
    - For each invocation, track all other invocations that occur while guard is alive
        - TODO: figure out how to do this
        - The lock class of all other invocations found are added to dependant lock classes set

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