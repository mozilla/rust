// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use super::super::test::TestGraph;

use super::*;

#[test]
fn diamond() {
    let graph = TestGraph::new(0, &[
        (0, 1),
        (0, 2),
        (1, 3),
        (2, 3),
    ]);

    let dominators = dominators(&graph).unwrap();
    let immediate_dominators = dominators.all_immediate_dominators();
    assert_eq!(immediate_dominators[0], Some(0));
    assert_eq!(immediate_dominators[1], Some(0));
    assert_eq!(immediate_dominators[2], Some(0));
    assert_eq!(immediate_dominators[3], Some(0));
}

#[test]
fn paper() {
    // example from the paper (with 0 exit node added):
    let graph = TestGraph::new(6, &[
        (3, 0), // this is the added edge
        (6, 5),
        (6, 4),
        (5, 1),
        (4, 2),
        (4, 3),
        (1, 2),
        (2, 3),
        (3, 2),
        (2, 1),
    ]);

    let dominators = dominators(&graph).unwrap();
    let immediate_dominators = dominators.all_immediate_dominators();
    assert_eq!(immediate_dominators[1], Some(6));
    assert_eq!(immediate_dominators[2], Some(6));
    assert_eq!(immediate_dominators[3], Some(6));
    assert_eq!(immediate_dominators[4], Some(6));
    assert_eq!(immediate_dominators[5], Some(6));
    assert_eq!(immediate_dominators[6], Some(6));
}

#[test]
#[should_panic(expected = "called `Result::unwrap()` on an `Err` value: UnreachableNode")]
fn no_start() {
    // Test error handling for graphs without a start node
    // 0 -> 1
    //      v
    // 2 -> 3
    // Dominators for this graph are undefined because there is
    // no start node which every path begins with
    let graph = TestGraph::new(0, &[
        (0, 1),
        (1, 3),
        (2, 3),
    ]);
    // this should panic:
    let dominators = dominators(&graph).unwrap();
    assert_eq!(dominators.is_dominated_by(1, 0), false);
}

#[test]
fn infinite_loop() {
    // Test handling of infinite loops
    // 0 -> 1 -> 4
    // v
    // 2 -> 3
    // ^ -  v
    let graph = TestGraph::new(0, &[
        (0, 1),
        (0, 2),
        (1, 4),
        (2, 3),
        (3, 2),
    ]);
    let dominators = dominators(&graph).unwrap();
    assert!(dominators.is_dominated_by(1, 0));
    assert!(dominators.is_dominated_by(4, 0));
    assert!(dominators.is_dominated_by(2, 0));
    assert!(dominators.is_dominated_by(3, 0));
    assert!(dominators.is_dominated_by(3, 2));
    assert!(!dominators.is_dominated_by(2, 3));
}

#[test]
#[should_panic(expected = "called `Result::unwrap()` on an `Err` value: UnreachableNode")]
fn transpose_infinite_loop() {
    // If we transpose the graph from `infinite_loop`
    // we get a graph with an unreachable loop 
    // in this case there are unreachable nodes and dominators
    // should return a error.
    // This is simulating transposing the Mir CFG
    // 0 <- 1 <- 4
    // ^
    // 2 <- 3
    // v -  ^
    let graph = TestGraph::new(4, &[
        (1, 0),
        (2, 0),
        (4, 1),
        (3, 2),
        (2, 3),
    ]);
    let dominators = dominators(&graph).unwrap(); // should panic
    assert!(dominators.is_dominated_by(1, 4)); // should never get here
}