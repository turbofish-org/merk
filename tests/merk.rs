#![feature(test)]

extern crate test;

use merk::*;

#[test]
fn simple_put() {
    let mut merk = Merk::open("./test_merk_simple_put.db").unwrap();
    let batch: Vec<TreeBatchEntry> = vec![
        (b"key", TreeOp::Put(b"value")),
        (b"key2", TreeOp::Put(b"value2")),
        (b"key3", TreeOp::Put(b"value3"))
    ];
    merk.apply(&batch).unwrap();
    merk.destroy().unwrap();
}
