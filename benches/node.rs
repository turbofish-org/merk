#![feature(test)]

extern crate test;

use merk::*;

macro_rules! bench_decode {
    ($name:ident, $length:expr) => {
        #[bench]
        fn $name(b: &mut test::Bencher) {
            let node = Node::new(b"", &[123; $length]);
            let bytes = node.encode().unwrap();
            b.iter(|| Node::decode(b"", &bytes).unwrap());
        }
    };
}

bench_decode!(bench_decode_0B, 0);
bench_decode!(bench_decode_50B, 50);
bench_decode!(bench_decode_500B, 500);
bench_decode!(bench_decode_5000B, 5000);

macro_rules! bench_encode {
    ($name:ident, $length:expr) => {
        #[bench]
        fn $name(b: &mut test::Bencher) {
            let node = Node::new(b"", &[123; $length]);
            b.iter(|| node.encode().unwrap());
        }
    };
}

bench_encode!(bench_encode_0B, 0);
bench_encode!(bench_encode_50B, 50);
bench_encode!(bench_encode_500B, 500);
bench_encode!(bench_encode_5000B, 5000);