extern crate rocksdb;
extern crate bincode;

error_chain! {
    foreign_links {
        RocksDB(rocksdb::Error);
        Bincode(bincode::Error);
    }
}
