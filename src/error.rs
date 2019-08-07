error_chain! {
    foreign_links {
        RocksDB(rocksdb::Error);
    }
}
