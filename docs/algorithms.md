# Merk - A High-Performance Merkle AVL Tree

**Matt Bell ([@mappum](https://twitter.com/mappum))** • [Nomic Hodlings, Inc.](https://nomic.io)

v0.0.0 - *June 27, 2019*

## Introduction

Merk is a Merkle AVL tree designed for performance, running on top of a backing key/value store such as RocksDB. Notable features include concurrent operations for higher throughput, an optimized key/value layout for performant usage of the backing store, and efficient proof generation to enable bulk tree replication.

*Note that this document is meant to be a way to grok how Merk works, rather than an authoritative specification.*

## Algorithm Overview

The Merk tree was inspired by [`tendermint/iavl`](https://github.com/tendermint/iavl) from the [Tendermint](https://tendermint.com) team but makes various fundamental design changes in the name of performance.

### Tree Structure

#### Nodes and Hashing

In many Merkle tree designs, only leaf nodes contain key/value pairs (inner nodes only contain child hashes). To contrast, every node in a Merk tree contains a key and a value, including inner nodes.

Each node contains a "kv hash", which is the hash of its key/value pair, in addition to its child hashes. The hash of the node is just the hash of the concatenation of these three hashes:
```
kv_hash = H(key, value)
node_hash = H(kv_hash, left_child_hash, right_child_hash)
```
Note that the `left_child_hash` and/or `right_child_hash` values may be null since it is possible for the node to have no children or only one child.

In our implementation, the hash function used is Blake2b (chosen for its performance characteristics) but this choice is trivially swappable.

#### In-Memory Representation

Trees are structured in memory sparsely so that not all nodes need to be retained, but can be fetched from the backing store as needed when operating on the tree. Applications can make decisions about when to retain and when to prune nodes from this graph to make time/memory tradeoffs.

The following are simplified forms of the tree structures from the Rust implementation:
```rust
struct Link {
    key: Vec<u8>,
    hash: Hash,
    height: u8
}

struct Node {
    key: Vec<u8>,
    value: Vec<u8>,
    kv_hash: Hash,
    left: Option<Link>,
    right: Option<Link>
}

struct Tree {
    node: Node,
    left: Option<Box<Tree>>,
    right: Option<Box<Tree>>
}
```

The `Link` struct contains the required data to reference a child node which may not be loaded in memory, `Node` represents a single loaded node's data, and `Tree` is a container which connects nodes into a graph by containing heap references to its children.

The sparse tree graph can be in a few states:
- A node does not have a child, in which case both its `node.right` and `tree.right` will be `None`.
- A node has a child, but the child is not currently loaded in memory. In this case, `node.right` will represent a valid `Link` structure which can be used to lookup the child node in the backing store, but `tree.right` will be `None`.
- A node has a child which is loaded in memory, in which case both `node.right` and `tree.right` will be populated.

#### Database Representation

In the backing key/value store, nodes are stored using their key/value pair key as the database key, and a binary encoding that contains the fields in the above `Node` structure - minus the `key` field since that is already implied by the database entry.

Storing nodes by key is an important optimization, and is the reason why inner nodes each have a key/value pair. The implication is that reading a key does not require traversing through the tree structure but only requires a single read in the backing key/value store, meaning there is little overhead versus using the backing store without a tree structure. Additionally, we can efficiently iterate through nodes in the tree in their in-order traversal just by iterating by key in the backing store (which RocksDB and LevelDB are optimized for).

This means we lose the "I" compared to the IAVL library - immutability. Since now we operate the tree nodes in-place in the backing store, we don't by default have views of past states of the tree. However, in our implementation we replicate this functionality with RocksDB's snapshot and checkpoint features which provide a consistent view of the store at a certain point in history - either ephemerally in memory or persistently on disk.

### Operations

Operating on a Merk tree is optimized for batches - in the real world we will only be updating the tree once per block, applying a batch of many changes from many transactions at the same time.

#### Concurrent Batch Operator

To mutate the tree, we apply batches of operations, each of which can either be `Put(key, value)` or `Delete(key)`.

Batches of operations are expected to be sorted by key, with every key appearing only once. Our implementation provides an `apply` method which sorts the batch and checks for duplicate keys, and an `apply_unchecked` method which skips the sorting/checking step for performance reasons when the caller has already ensured the batch is sorted.

The algorithm to apply these operations to the tree is called recursively on each relevant node. 

*Simplified pseudocode for the operation algorithm:*
- Given a node and a batch of operations:
  - Binary search for the current node's key in the batch:
    - If this node's key is found in the batch at index `i`:
      - Apply the operation to this node:
        - If operation is `Put`, update its `value` and `kv_hash`
        - If the operation is `Delete`, perform a traditional BST node removal
      - Split the batch into left and right sub-batches (excluding the operation we just applied):
        - Left batch from batch start to index `i`
        - Right batch from index `i + 1` to the end of the batch
    - If this node's key is not found in the batch, but could be inserted at index `i` maintaining sorted order:
      - Split the batch into left and right sub-batches:
        - Left batch from batch start to index `i`
        - Right batch from index `i` to the end of the batch
  - Recurse:
    - Apply the left sub-batch to this node's left child
    - Apply the right sub-batch to this node's right child
  - Balance:
    - If after recursing the left and right subtrees are unbalanced (their heights differ by more than 1), perform an AVL tree rotation (possibly more than one)
  - Recompute node's hash based on hash of its updated children and `kv_hash`, then return

This batch application of operations can happen concurrently - recursing into the left and right subtrees of a node are two fully independent operations (operations on one subtree will never involve reading or writing to/from any of the nodes on the other subtree). This means we have an *implicit lock* - we don't need to coordinate with mutexes but only need to wait for both the left side and right side to finish their operations.

### Proofs

Merk was designed with efficient proofs in mind, both for application queries (e.g. a user checking their account balance) and bulk tree replication (a.k.a. "state syncing") between untrusted nodes.

#### Structure

Merk proofs are a list of stack-based operators and node data, with 3 possible operators: `Push(node)`, `Parent`, and `Child`. A stream of these operators can be processed by a verifier in order to reconstruct a sparse representation of part of the tree, in a way where the data can be verified against a known root hash.

The value of `node` in a `Push` operation can be one of three types:
  - `Hash(hash)` - The hash of a node
  - `KVHash(hash)` - The key/value hash of a node
  - `KV(key, value)` - The key and value of a node
  
This proof format can be encoded in a binary format and has negligible space overhead for efficient transport over the network.

#### Verification

A verifier can process a proof by maintaining a stack of connected tree nodes, and executing the operators in order:
  - `Push(node)` - Push some node data onto the stack.
  - `Child` - Pop a value from the stack, `child`. Pop another value from the stack, `parent`. Set `child` as the right child of `parent`, and push the combined result back on the stack.
  - `Parent` - Pop a value from the stack, `parent`. Pop another value from the stack, `child`. Set `child` as the left child of `parent`, and push the combined result back on the stack.

Proof verification will fail if e.g. `Child` or `Parent` try to pop a value from the stack but the stack is empty, `Child` or `Parent` try to overwrite an existing child, or the proof does not result in exactly one stack item.

This proof language can be used to specify any possible set or subset of the tree's data in a way that can be reconstructed efficiently by the verifier. Proofs can contain either an arbitrary set of selected key/value pairs (e.g. in an application query), or contiguous tree chunks (when replicating the tree). After processing an entire proof, the verifier should have derived a root hash which can be compared to the root hash they expect (e.g. the one validators committed to in consensus), and have a set of proven key/value pairs.

Note that this can be computed in a streaming fashion, e.g. while downloading the proof, which makes the required memory for verification very low even for large proofs. However, the verifier cannot tell if the proof is valid until finishing the entire proof, so very large proofs should be broken up into multiple proofs of smaller size.

#### Generation

Efficient proof generation is important since nodes will likely receive a high volume of queries and constantly be serving proofs, essentially providing an API service to end-user application clients, as well as servicing demand for replication when new nodes come onto the network.

Nodes can generate proofs for a set of keys by traversing through the tree from the root and building up the required proof branches. Much like the batch operator aglorithm, this algorithm takes a batch of sorted, unique keys as input.

*Simplified pseudocode for proof generation (based on an in-order traversal):*
  - Given a node and a batch of keys to include in the proof:
    - If the batch is empty, append `Push(Hash(node_hash))` to the proof and return
    - Binary search the for the current node's key in the batch:
      - If this node's key is found in the batch at index `i`:
        - Partition the batch into left and right sub-batches at index `i` (excluding index `i`)
      - If this node's key is not found in the batch, but could be inserted at index `i` maintaining sorted order:
        - Partition the batch into left and right sub-batches at index `i`
    - Recurse left: if it exists, query the left child using the left sub-batch (appending some operators to the proof)
    - Append proof operator:
      - If this node's key is in the batch, append `Push(KV(key, value))` to the proof
      - If this node's key is not in the batch, append `Push(KVHash(kv_hash))` to the proof
    - If the left child exists, append `Parent` to the proof
    - Recurse right: if it exists, query the right child using the right sub-batch (appending some operators to the proof), then append `Child` to the proof


Since RocksDB allows concurrent reading from a consistent snapshot/checkpoint, nodes can concurrently generate proofs on all cores to service a higher volume of queries, even if our algorithm isn't designed for concurrency.

#### Binary Format

We can efficiently encode these proofs by encoding each operator as follows:
```
Push(Hash(hash)) => 0x01 <20-byte hash>
Push(KVHash(hash)) => 0x02 <20-byte hash>
Push(KV(key, value)) => 0x03 <1-byte key length> <n-byte key> <4-byte value length> <n-byte value>
Parent => 0x10
Child => 0x11
```

This results in a compact binary representation, with a very small space overhead (roughly 2 bytes per node in the proof, plus 5 bytes per key/value pair).

#### Efficient Proofs for Ranges of Keys

An alternate, optimized proof generation can be used when generating proofs for large contiguous subtrees, e.g. chunks for tree replication. This works by iterating through keys in the backing store (which is much faster than random lookups).

Based on some early benchmarks, I estimate that typical server hardware should be able to generate this kind of range proof at a rate of hundreds of MB/s, which means the bottleneck for bulk replication will likely be bandwidth rather than CPU. To improve performance further, these proofs can be cached and trivially served by a CDN or a P2P swarm (each node of which can easily verify the chunks they pass around).

Due to the tree structure we already use, key-order iteration gives us an entire valid subtree (or up to three disjoint subtrees, which will be illustrated below):

```
      4
    /   \
  2       6
 / \     / \
1   3   5   7
```

Consider this simple tree, where each digit is the key of a node. If we iterated in key-order from 1 to 7, we would receive sufficient data for an entire contiguous tree. However, if we only take range 3 to 4 (inclusive), nodes 3 and 4 are disjoint and we need to fetch node 2 to join them for a valid proof.

If we took range 3 to 5, we would now have 3 disjoint graphs. However, no matter the size of the tree, it is only possible to have up to 3 of these disjoint graphs, and we can always join them by traversing from the root to the edges of the range to ensure all of these "range boundary nodes" are included in the proof.

*Pseudocode for the range proof generation algorithm:*

- Given a tree and a range of keys to prove:
  - Create a stack of keys (initially empty)
  - **Left boundary:** traverse down from the tree root to the start of the key range:
    - For any node visited which has a key less than the start of the range:
      - If the node has a left child, append `Push(Hash(left_child_hash))` to the proof
      - Append `Push(KVHash(kv_hash))` to the proof
      - If the node has a left child, append `Parent` to the proof
      - If the node's right child is inside the range, push the right child's key onto the key stack
  - **Range iteration:** for every key/value entry within the query range in the backing store:
    - If the current node has a left child which is outside the query range, append `Push(Hash(left_child_hash))` to the proof
    - Append `Push(KV(key, value))` to the proof
    - If the current node has a left child, append `Parent` to the proof
    - If the current node has a right child which is inside the query range, push the right child's key onto the key stack
    - Else:
      - If the current node has a right child (outside the query range), append `Push(Hash(right_child_hash)), Child` to the proof
      - While the current node's key is greater than or equal to the key at the top of the key stack, append `Child` to the proof and pop from the key stack
  - **Right boundary:** post-order traverse from the tree root to the end of the key range:
    - For any node visited which has a key greater than the end of the range:
      - Append `Push(KVHash(kv_hash))` to the proof
      - If the node has a left child, append `Parent` to the proof
      - If the node has a right child, append `Push(Hash(right_child_hash)), Child` to the proof
      - While the current node's key is greater than or equal to the key at the top of the key stack, append `Child` to the proof and pop from the key stack

Note that this algorithm produces the proof in a streaming fashion and has very little memory requirements (the only overhead is the key stack, which will be small even for extremely large trees since its length is a maximum of `log N`).

#### Example Proofs

Let's walk through a concrete proof example. Consider the following tree:

```
       5
      / \
    /     \
  2        9
 / \      /  \
1   4    7    11
   /    / \   /
  3    6   8 10
```

*Small proof:*

First, let's create a proof for a small part of the tree. Let's say the user makes a query for keys `1, 2, 3, 4`.

If we follow our proof generation algorithm, we should get a proof that looks like this:
```
Push(KV(1, <value of 1>)),
Push(KV(2, <value of 2>)),
Parent,
Push(KV(3, <value of 3>)),
Push(KV(4, <value of 4>)),
Parent,
Child,
Push(KVHash(<kv_hash of 5>)),
Parent,
Push(Hash(<hash of 9>)),
Child
```

Let's step through verification to show that this proof works. We'll create a verification stack, which starts out empty, and walk through each operator in the proof, in order:

```
Stack: (empty)
```
We will push a key/value pair on the stack, creating a node. However, note that for verification purposes this node will only need to contain the kv_hash which we will compute at this step.
```
Operator: Push(KV(1, <value of 1>))

Stack:
1
```
```
Operator: Push(KV(2, <value of 2>))

Stack:
1
2
```
Now we connect nodes 1 and 2, with 2 as the parent.
```
Operator: Parent

Stack:
  2
 /
1
```
```
Operator: Push(KV(3, <value of 3>))

Stack:
  2
 /
1
3
```
```
Operator: Push(KV(4, <value of 4>))

Stack:
  2
 /
1
3
4
```
```
Operator: Parent

Stack:
  2
 /
1
  4
 /
3
```
Now connect these two graphs with 4 as the child of 2.
```
Operator: Child

Stack:
  2
 / \
1   4
   /
  3
```
Since the user isn't querying the data from node 5, we only need its kv_hash.
```
Operator: Push(KVHash(<kv_hash of 5>))

Stack:
  2
 / \
1   4
   /
  3
5
```
```
Operator: Parent

Stack:
    5
   /
  2
 / \
1   4
   /
  3
```
We only need the hash of node 9.
```
Operator: Push(Hash(<hash of 9>))

Stack:
    5
   /
  2
 / \
1   4
   /
  3
9
```
```
Operator: Child

Stack:
    5
   / \
  2   9
 / \
1   4
   /
  3
```

Now after going through all these steps, we have sufficient knowlege of the tree's structure and data to compute node hashes in order to verify. At the end, we will have computed a hash for node 5 (the root), and we verify by comparing this hash to the one we expected.