#![feature(repr_simd)]
#![feature(test)]
#![feature(benches_btree_set)]

extern crate rand;
extern crate rand_xorshift;
extern crate test;

mod btree;
mod linked_list;
mod string;
mod str;
mod slice;
mod vec;
mod vec_deque;
