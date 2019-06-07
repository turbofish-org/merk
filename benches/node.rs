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

bench_decode!(bench_decode_0_bytes, 0);
bench_decode!(bench_decode_50_bytes, 50);
bench_decode!(bench_decode_500_bytes, 500);
bench_decode!(bench_decode_5000_bytes, 5000);

macro_rules! bench_encode {
    ($name:ident, $length:expr) => {
        #[bench]
        fn $name(b: &mut test::Bencher) {
            let node = Node::new(b"", &[123; $length]);
            b.iter(|| node.encode().unwrap());
        }
    };
}

bench_encode!(bench_encode_0_bytes, 0);
bench_encode!(bench_encode_50_bytes, 50);
bench_encode!(bench_encode_500_bytes, 500);
bench_encode!(bench_encode_5000_bytes, 5000);