use merk::*;

#[test]
fn simple_put() {
    let mut merk = Merk::new("./test_merk_simple_put.db").unwrap();
    let batch: Vec<(&[u8], &[u8])> = vec![
        (b"key", b"value"),
        (b"key2", b"value2"),
    ];
    merk.put_batch(&batch).unwrap();
    merk.delete().unwrap();
}
