- Scan mir for all instances where lock is constructed
    - When lock is found, traverse all mir basic blocks where lock is used until lock is dropped
    - If a call to lock is found, put in map the BasicBlock where it is called, and the id of the lock construction
- Next, go through map of basic block where locks is called, and traverse until lock guard is dropped
    - If the map contains the basic block that we visit, record in the map for the current block that the visited block is its child

Use rustc dataflow analysis