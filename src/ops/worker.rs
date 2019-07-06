// use std::thread;
// use std::sync::mpsc::{
//     channel,
//     sync_channel,
//     Sender,
//     Receiver
// };

// use crate::error::Result;

// pub struct Worker {
//     handle: thread::JoinHandle<()>,
//     input: Sender<Request>,
//     output: Receiver<Response>
// }

// pub enum Request {
//     Apply,
//     Kill
// }

// pub enum Response {
//     Apply
// }

// impl Worker {
//     pub fn new() -> Worker {
//         let (input_tx, input_rx) = channel::<Request>();
//         let (output_tx, output_rx) = sync_channel::<Response>(0);

//         let handle = thread::spawn(move || {
//             // TODO: call a setup function, which gives us a GetNodeFn

//             let input = input_rx;
//             let output = output_tx;

//             loop {
//                 // block until we have work to do
//                 let req = input.recv().unwrap();
                
//                 match req {
//                     Request::Apply => {
//                         println!("got work");
//                     },
//                     Request::Kill => break
//                 }

//                 output.send(Response::Apply).unwrap();
//             }
//         });

//         Worker {
//             handle,
//             input: input_tx,
//             output: output_rx
//         }
//     }

//     pub fn exec(&self) -> Result<Response> {
//         self.input.send(Request::Apply)?;
//         Ok(self.output.recv()?)
//     }
// }

// impl Drop for Worker {
//     fn drop(&mut self) {
//         self.input.send(Request::Kill).unwrap();
//     }
// }

// pub struct Pool {
//     workers: Vec<Worker>
// }

// impl Pool {
//     pub fn new(worker_count: usize) -> Pool {
//         let workers = Vec::with_capacity(worker_count);

//         for i in 0..worker_count {
//             // let worker = Worker::new()
//             // workers.push(worker);
//         }

//         Pool { workers }
//     }
// }

// #[test]
// fn worker_test() {
//     let worker = Worker::new();
//     worker.exec();
// }