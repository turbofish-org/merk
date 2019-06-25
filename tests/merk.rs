// #![feature(test)]

// extern crate test;

// use merk::*;
// use merk::proof;

// #[test]
// fn merk_simple_put() {
//     let mut merk = Merk::open("./test_merk_simple_put.db").unwrap();
//     let mut batch: Vec<TreeBatchEntry> = vec![
//         (b"key", Op::Put(b"value")),
//         (b"key2", Op::Put(b"value2")),
//         (b"key3", Op::Put(b"value3"))
//     ];
//     merk.apply(&mut batch).unwrap();
//     merk.destroy().unwrap();
// }

// #[test]
// fn merk_range_inclusive() {
//     let mut merk = Merk::open("./test_merk_range.db").unwrap();
//     let mut batch: Vec<TreeBatchEntry> = vec![
//         (b"key", Op::Put(b"value")),
//         (b"key2", Op::Put(b"value2")),
//         (b"key3", Op::Put(b"value3"))
//     ];
//     merk.apply(&mut batch).unwrap();

//     let mut i = 0;
//     merk.map_range(b"key", b"key3", &mut |node: Node| {
//         let expected_key = batch[i].0;
//         assert_eq!(node.key, expected_key);
//         i += 1;
//     }).unwrap();
//     assert_eq!(i, 3);

//     merk.destroy().unwrap();
// }

// #[test]
// fn merk_proof() {
//     let mut merk = Merk::open("./test_merk_proof.db").unwrap();
//     let mut batch: Vec<TreeBatchEntry> = vec![
//         (b"key1", Op::Put(b"value1")),
//         (b"key2", Op::Put(b"value2")),
//         (b"key3", Op::Put(b"value3")),
//         (b"key4", Op::Put(b"value4")),
//         (b"key5", Op::Put(b"value5")),
//         (b"key6", Op::Put(b"value6"))
//     ];
//     merk.apply(&mut batch).unwrap();

//     let proof = merk.proof(b"key", b"key6").unwrap();

//     proof::verify(
//         // TODO: use merk root_hash function instead of hardcoding?
//         &[164, 172, 235, 50, 254, 105, 16, 195, 220, 18, 217, 39, 44, 215, 194, 160, 253, 84, 27, 75],
//         &proof
//     ).unwrap();

//     merk.destroy().unwrap();
// }

// #[test]
// fn merk_delete_1k() {
//     let mut merk = Merk::open("./test_merk_delete_1k.db").unwrap();

//     let mut keys: Vec<[u8; 4]> = Vec::with_capacity(1001);
//     let mut batch: Vec<TreeBatchEntry> = Vec::with_capacity(1001);
//     for i in 0..=1000 {
//         keys.push((i as u32).to_be_bytes());
//     }
//     for i in 0..=1000 {
//         batch.push((&keys[i], Op::Put(b"xyz")));
//     }
//     merk.apply(&mut batch).unwrap();

//     batch.clear();
//     for i in 0..1000 {
//         batch.push((&keys[i], Op::Delete));
//     }
//     merk.apply(&mut batch).unwrap();

//     assert_eq!(&merk.tree.as_ref().unwrap().key, &keys[1000]);
//     assert_eq!(&merk.tree.as_ref().unwrap().height(), &(1 as u8));

//     merk.destroy().unwrap();
// }

// #[test]
// fn merk_load() {
//     let mut keys: Vec<[u8; 4]> = Vec::with_capacity(100);
//     for i in 0..100 {
//         keys.push((i as u32).to_be_bytes());
//     }

//     {
//         let mut merk = Merk::open("./test_merk_load.db").unwrap();

//         let mut batch: Vec<TreeBatchEntry> = Vec::with_capacity(100);
//         for i in 0..100 {
//             batch.push((&keys[i], Op::Put(b"xyz")));
//         }
//         merk.apply(&mut batch).unwrap();
//     }

//     {
//         let merk = Merk::open("./test_merk_load.db").unwrap();

//         for key in keys.iter() {
//             assert_eq!(merk.get(key).unwrap(), b"xyz");
//         }

//         merk.destroy().unwrap();
//     }
// }