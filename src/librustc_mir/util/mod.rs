// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use core::unicode::property::Pattern_White_Space;
use rustc::ty;
use syntax_pos::Span;

pub mod borrowck_errors;
pub mod elaborate_drops;
pub mod def_use;
pub mod patch;

mod alignment;
mod graphviz;
pub(crate) mod pretty;
pub mod liveness;
pub mod collect_writes;

pub use self::alignment::is_disaligned;
pub use self::pretty::{dump_enabled, dump_mir, write_mir_pretty, PassWhere};
pub use self::graphviz::{write_mir_graphviz};
pub use self::graphviz::write_node_label as write_graphviz_node_label;

/// If possible, suggest replacing `ref` with `ref mut`.
pub fn suggest_ref_mut<'cx, 'gcx, 'tcx>(
    tcx: ty::TyCtxt<'cx, 'gcx, 'tcx>,
    pattern_span: Span,
) -> Option<(Span, String)> {
    let hi_src = tcx.sess.codemap().span_to_snippet(pattern_span).unwrap();
    if hi_src.starts_with("ref")
        && hi_src["ref".len()..].starts_with(Pattern_White_Space)
    {
        let replacement = format!("ref mut{}", &hi_src["ref".len()..]);
        Some((pattern_span, replacement))
    } else {
        None
    }
}
