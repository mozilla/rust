//! Construction of MIR from HIR.
//!
//! This crate also contains the match exhaustiveness and usefulness checking.
#![feature(array_windows)]
#![feature(box_patterns)]
#![feature(box_syntax)]
#![feature(const_panic)]
#![feature(control_flow_enum)]
#![feature(crate_visibility_modifier)]
#![feature(bool_to_option)]
#![feature(iter_zip)]
#![feature(once_cell)]
#![feature(min_specialization)]
#![feature(trusted_step)]
#![recursion_limit = "256"]

#[macro_use]
extern crate tracing;
#[macro_use]
extern crate rustc_middle;

mod build;
mod check_unsafety;
mod lints;
pub mod thir;

use rustc_middle::ty::query::Providers;

pub fn provide(providers: &mut Providers) {
    providers.check_match = thir::pattern::check_match;
    providers.lit_to_const = thir::constant::lit_to_const;
    providers.mir_built = build::mir_built;
    providers.thir_check_unsafety = check_unsafety::thir_check_unsafety;
    providers.thir_check_unsafety_for_const_arg = check_unsafety::thir_check_unsafety_for_const_arg;
    providers.thir_body = thir::cx::thir_body;
}
