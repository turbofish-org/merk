use crate::worker;

error_chain! {
    foreign_links {
        RocksDB(rocksdb::Error);
        Bincode(bincode::Error);
        WorkerSend(std::sync::mpsc::SendError<worker::Request>);
        WorkerRecv(std::sync::mpsc::RecvError);
    }
}
