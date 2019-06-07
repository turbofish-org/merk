# merk

*High-performance Merkle key/value store*

[![Build Status](https://travis-ci.org/nomic-io/merk.svg?branch=master)](https://travis-ci.org/nomic-io/merk)
[![Crate](https://img.shields.io/crates/v/merk.svg)](https://crates.io/crates/merk)
[![API](https://docs.rs/merk/badge.svg)](https://docs.rs/merk)

Merk is a crypto key/value store - more specifically, it's a Merkle AVL tree built on top of RocksDB (Facebook's fork of LevelDB).

Its priorities are performance and reliability. While Merk was designed to be the state database for blockchains, it can also be used anywhere an auditable key/value store is needed.

**NOTE:** This crate is still in early development and not fully implemented yet.

### FEATURES:
- **Fast reads/writes** - Reads have no overhead compared to a normal RocksDB store, and writes are optimized for batch operations (e.g. blocks in a blockchain).
- **Fast proof generation** - Since Merk implements an AVL tree rather than a trie, it is very efficient to create and verify proofs for ranges of keys.
- **Concurrency** - Unlike most other Merkle stores, all operations utilize all available cores - giving huge performance gains and allowing nodes to scale along with Moore's Law.
- **Replication** - The tree is optimized to efficiently build proofs of large chunks, allowing for nodes to download the entire state (e.g. "state syncing").
- **Checkpointing** - Merk can create checkpoints on disk (an immutable view of the entire store at a certain point in time) without blocking, so there are no delays in availability or liveness.
- **Web-friendly** - Being written in Rust means it is easy to run the proof-verification code in browsers with WebAssembly, allowing for light-clients that can verify data for themselves.
- **Fits any Profile** - Performant on RAM-constrained Raspberry Pi's and beefy validator rigs alike.

## Usage

**Install:**
```
cargo add merk
```

**Example:**
```rust
extern crate merk;
use merk::*;

// load or create a Merk store at the given path
let mut merk = Merk::open("./merk.db").unwrap();

// apply some operations
let mut batch: Vec<TreeBatchEntry> = vec![
    (b"key", TreeOp::Put(b"value")),
    (b"key2", TreeOp::Put(b"value2")),
    (b"key3", TreeOp::Put(b"value3")),
    (b"key4", TreeOp::Delete)
];
merk.apply(&mut batch).unwrap();
```

## Status

Merk is currently experimental but developing fast, and is intended to be used in production soon in [LotionJS](https://github.com/nomic-io/lotion).

## Benchmarks

Average performance on my 2017 Macbook Pro, on a store with at least 1M keys, with no concurrency:
- *Random inserts:* ~22,000 per second
- *Random updates:* ~19,000 per second
- *Random reads:* ~117,000 per second
- *Sequential inserts:* ~181,000 per second
- *Sequential updates:* ~174,000 per second
- *Sequential reads:* ~350,000 per second
- *RAM usage:* ~30MB average, ~60MB max

This is just the first pass - we can do much better!

*TODO: generate more scientific benchmarks, with comparisons to alternatives*

## Algorithm Details

*TODO*