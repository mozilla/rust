// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use rustc::mir::repr::*;

/// Update basic block ids in all terminators using the given replacements,
/// useful e.g. after removal of several basic blocks to update all terminators
/// in a single pass
pub fn update_basic_block_ids(mir: &mut Mir, replacements: &[BasicBlock]) {
    for bb in mir.all_basic_blocks() {
        for target in mir.basic_block_data_mut(bb).terminator.successors_mut() {
            *target = replacements[target.index()];
        }
    }
}

/// Run a function on every reachable basic block, then delete any unreachable blocks.
/// The function given should not add or remove any blocks from the control flow graph.
pub fn adjust_reachable_basic_blocks<F: FnMut(&mut Mir, BasicBlock)>(mir: &mut Mir, mut func: F) {
    let mut seen = vec![false; mir.basic_blocks.len()];

    let mut worklist = vec![START_BLOCK];
    while let Some(bb) = worklist.pop() {
        func(mir, bb);

        for succ in mir.basic_block_data(bb).terminator.successors() {
            if !seen[succ.index()] {
                seen[succ.index()] = true;
                worklist.push(*succ);
            }
        }
    }

    // These blocks are always required.
    seen[START_BLOCK.index()] = true;
    seen[END_BLOCK.index()] = true;
    seen[DIVERGE_BLOCK.index()] = true;

    retain_basic_blocks(mir, &seen);
}

/// Mass removal of basic blocks to keep the ID-remapping cheap.
pub fn retain_basic_blocks(mir: &mut Mir, keep: &[bool]) {
    let num_blocks = mir.basic_blocks.len();

    // Check that we have a usage flag for every block
    assert_eq!(num_blocks, keep.len());

    let first_dead = match keep.iter().position(|&k| !k) {
        None => return,
        Some(first_dead) => first_dead,
    };

    // `replacements` maps the old block ids to the new ones
    let mut replacements: Vec<_> = (0..num_blocks).map(BasicBlock::new).collect();

    let mut dead = 0;
    for i in first_dead..num_blocks {
        if keep[i] {
            replacements[i] = BasicBlock::new(i - dead);
            mir.basic_blocks.swap(i, i - dead);
        } else {
            dead += 1;
        }
    }
    mir.basic_blocks.truncate(num_blocks - dead);

    update_basic_block_ids(mir, &replacements);
}
