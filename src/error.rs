use std::cell;

error_chain! {
    foreign_links {
        RocksDB(rocksdb::Error);
        Bincode(bincode::Error);
        CellBorrow(cell::BorrowError);
        CellBorrowMut(cell::BorrowMutError);
    }
}
