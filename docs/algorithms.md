# Merk - A High-Performance Merkle AVL Tree

**Matt Bell ([@mappum](https://twitter.com/mappum))** • [Nomic Hodlings, Inc.](https://nomic.io)

v0.0.0 - *June 27, 2019*

## Abstract

Merk is a Merkle AVL tree designed for performance, running on top of a backing key/value store such as RocksDB. Notable features include concurrent operations for higher throughput, an optimized key/value layout for efficient usage of the backing store, and efficient proof generation to enable bulk tree replication.

*Note that this document is meant to be a way to grok how Merk works, rather than trying to be an authoritative specification.*

## Algorithm Overview

The Merk tree was inspired by [`tendermint/iavl`](https://github.com/tendermint/iavl) from the [Tendermint](https://tendermint.com) team but makes various fundamental design changes in the name of higher performance.

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

To mutate the tree, we apply batches of operations, which can either be `Put(key, value)` or `Delete(key)`.

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
    - If this node's key is not found in the batch:
      - Split the batch into left and right sub-batches:
        - Left batch from batch start to index `i`
        - Right batch from index `i` to the end of the batch
  - Recurse:
    - Apply the left sub-batch to this node's left child
    - Apply the right sub-batch to this node's right child
  - Balance:
    - If after recursing the left and right subtrees are unbalanced (their heights differ by more than 1), perform an AVL tree rotation (possibly more than one)
  - Recompute node's hash based on hash of its children and `kv_hash`

This batch application of operations can happen concurrently - recursing into the left and right subtrees of a node are two fully independent operations (operations on one subtree will never involve reading or writing to/from any of the nodes on the other subtree). This means we have an *implicit lock* - we don't need to coordinate with mutexes but only need to wait for both the left side and right side to finish their operations.

### Proofs

#### Structure

#### Generation

#### Verification

#### Binary Format

#### Bulk Tree Replication (State Syncing)

## Comparisons

### IAVL

