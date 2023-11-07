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