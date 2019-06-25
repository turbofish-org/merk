use std::sync::mpsc;
use crate::ops::worker;

error_chain! {
    foreign_links {
        RocksDB(rocksdb::Error);
        Bincode(bincode::Error);
        WorkerSend(mpsc::SendError<worker::Request>);
        WorkerRecv(mpsc::RecvError);
    }
}
