// The Computer Language Benchmarks Game
// http://benchmarksgame.alioth.debian.org/
//
// contributed by the Rust Project Developers

// Copyright (c) 2014 The Rust Project Developers
//
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions
// are met:
//
// - Redistributions of source code must retain the above copyright
//   notice, this list of conditions and the following disclaimer.
//
// - Redistributions in binary form must reproduce the above copyright
//   notice, this list of conditions and the following disclaimer in
//   the documentation and/or other materials provided with the
//   distribution.
//
// - Neither the name of "The Computer Language Benchmarks Game" nor
//   the name of "The Computer Language Shootout Benchmarks" nor the
//   names of its contributors may be used to endorse or promote
//   products derived from this software without specific prior
//   written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
// "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
// LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS
// FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE
// COPYRIGHT OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT,
// INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
// HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED
// OF THE POSSIBILITY OF SUCH DAMAGE.

// ignore-android: FIXME(#10393) hangs without output

#![feature(box_syntax, std_misc, collections)]

use std::ascii::OwnedAsciiExt;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::io;
use std::slice;
use std::sync::Arc;
use std::thread;

static TABLE: [u8;4] = [ 'A' as u8, 'C' as u8, 'G' as u8, 'T' as u8 ];
static TABLE_SIZE: usize = 2 << 16;

static OCCURRENCES: [&'static str;5] = [
    "GGT",
    "GGTA",
    "GGTATT",
    "GGTATTTTAATT",
    "GGTATTTTAATTTATAGT",
];

// Code implementation

#[derive(Copy, Clone, PartialEq, PartialOrd, Ord, Eq)]
struct Code(u64);

impl Code {
    fn hash(&self) -> u64 {
        let Code(ret) = *self;
        return ret;
    }

    fn push_char(&self, c: u8) -> Code {
        Code((self.hash() << 2) + (pack_symbol(c) as u64))
    }

    fn rotate(&self, c: u8, frame: usize) -> Code {
        Code(self.push_char(c).hash() & ((1 << (2 * frame)) - 1))
    }

    fn pack(string: &str) -> Code {
        string.bytes().fold(Code(0), |a, b| a.push_char(b))
    }

    fn unpack(&self, frame: usize) -> String {
        let mut key = self.hash();
        let mut result = Vec::new();
        for _ in 0..frame {
            result.push(unpack_symbol((key as u8) & 3));
            key >>= 2;
        }

        result.reverse();
        String::from_utf8(result).unwrap()
    }
}

// Hash table implementation

trait TableCallback {
    fn f(&self, entry: &mut Entry);
}

struct BumpCallback;

impl TableCallback for BumpCallback {
    fn f(&self, entry: &mut Entry) {
        entry.count += 1;
    }
}

struct PrintCallback(&'static str);

impl TableCallback for PrintCallback {
    fn f(&self, entry: &mut Entry) {
        let PrintCallback(s) = *self;
        println!("{}\t{}", entry.count, s);
    }
}

struct Entry {
    code: Code,
    count: usize,
    next: Option<Box<Entry>>,
}

struct Table {
    items: Vec<Option<Box<Entry>>>
}

struct Items<'a> {
    cur: Option<&'a Entry>,
    items: slice::Iter<'a, Option<Box<Entry>>>,
}

impl Table {
    fn new() -> Table {
        Table {
            items: (0..TABLE_SIZE).map(|_| None).collect()
        }
    }

    fn search_remainder<C:TableCallback>(item: &mut Entry, key: Code, c: C) {
        match item.next {
            None => {
                let mut entry: Box<_> = box Entry {
                    code: key,
                    count: 0,
                    next: None,
                };
                c.f(&mut *entry);
                item.next = Some(entry);
            }
            Some(ref mut entry) => {
                if entry.code == key {
                    c.f(&mut **entry);
                    return;
                }

                Table::search_remainder(&mut **entry, key, c)
            }
        }
    }

    fn lookup<C:TableCallback>(&mut self, key: Code, c: C) {
        let index = key.hash() % (TABLE_SIZE as u64);

        {
            if self.items[index as usize].is_none() {
                let mut entry: Box<_> = box Entry {
                    code: key,
                    count: 0,
                    next: None,
                };
                c.f(&mut *entry);
                self.items[index as usize] = Some(entry);
                return;
            }
        }

        {
            let entry = self.items[index as usize].as_mut().unwrap();
            if entry.code == key {
                c.f(&mut **entry);
                return;
            }

            Table::search_remainder(&mut **entry, key, c)
        }
    }

    fn iter(&self) -> Items {
        Items { cur: None, items: self.items.iter() }
    }
}

impl<'a> Iterator for Items<'a> {
    type Item = &'a Entry;

    fn next(&mut self) -> Option<&'a Entry> {
        let ret = match self.cur {
            None => {
                let i;
                loop {
                    match self.items.next() {
                        None => return None,
                        Some(&None) => {}
                        Some(&Some(ref a)) => { i = &**a; break }
                    }
                }
                self.cur = Some(&*i);
                &*i
            }
            Some(c) => c
        };
        match ret.next {
            None => { self.cur = None; }
            Some(ref next) => { self.cur = Some(&**next); }
        }
        return Some(ret);
    }
}

// Main program

fn pack_symbol(c: u8) -> u8 {
    match c as char {
        'A' => 0,
        'C' => 1,
        'G' => 2,
        'T' => 3,
        _ => panic!("{}", c as char),
    }
}

fn unpack_symbol(c: u8) -> u8 {
    TABLE[c as usize]
}

fn generate_frequencies(mut input: &[u8], frame: usize) -> Table {
    let mut frequencies = Table::new();
    if input.len() < frame { return frequencies; }
    let mut code = Code(0);

    // Pull first frame.
    for _ in 0..frame {
        code = code.push_char(input[0]);
        input = &input[1..];
    }
    frequencies.lookup(code, BumpCallback);

    while !input.is_empty() && input[0] != ('>' as u8) {
        code = code.rotate(input[0], frame);
        frequencies.lookup(code, BumpCallback);
        input = &input[1..];
    }
    frequencies
}

fn print_frequencies(frequencies: &Table, frame: usize) {
    let mut vector = Vec::new();
    for entry in frequencies.iter() {
        vector.push((entry.count, entry.code));
    }
    vector.sort();

    let mut total_count = 0;
    for &(count, _) in &vector {
        total_count += count;
    }

    for &(count, key) in vector.iter().rev() {
        println!("{} {:.3}",
                 key.unpack(frame),
                 (count as f32 * 100.0) / (total_count as f32));
    }
    println!("");
}

fn print_occurrences(frequencies: &mut Table, occurrence: &'static str) {
    frequencies.lookup(Code::pack(occurrence), PrintCallback(occurrence))
}

fn get_sequence<R: BufRead>(r: &mut R, key: &str) -> Vec<u8> {
    let mut res = Vec::new();
    for l in r.lines().map(|l| l.unwrap())
        .skip_while(|l| key != &l[..key.len()]).skip(1)
    {
        res.push_all(l.trim().as_bytes());
    }
    res.into_ascii_uppercase()
}

fn main() {
    let input = if env::var_os("RUST_BENCH").is_some() {
        let f = File::open("shootout-k-nucleotide.data").unwrap();
        get_sequence(&mut io::BufReader::new(f), ">THREE")
    } else {
        let stdin = io::stdin();
        let mut stdin = stdin.lock();
        get_sequence(&mut stdin, ">THREE")
    };
    let input = Arc::new(input);

    let nb_freqs: Vec<_> = (1..3).map(|i| {
        let input = input.clone();
        (i, thread::spawn(move|| generate_frequencies(&input, i)))
    }).collect();
    let occ_freqs: Vec<_> = OCCURRENCES.iter().map(|&occ| {
        let input = input.clone();
        thread::spawn(move|| generate_frequencies(&input, occ.len()))
    }).collect();

    for (i, freq) in nb_freqs {
        print_frequencies(&freq.join().unwrap(), i);
    }
    for (&occ, freq) in OCCURRENCES.iter().zip(occ_freqs.into_iter()) {
        print_occurrences(&mut freq.join().unwrap(), occ);
    }
}
