/*!

Rust MIR: a lowered representation of Rust.

*/

#![feature(array_windows)]
#![feature(assert_matches)]
#![feature(associated_type_defaults)]
#![feature(bool_to_option)]
#![feature(box_patterns)]
#![feature(box_syntax)]
#![feature(const_panic)]
#![feature(control_flow_enum)]
#![feature(crate_visibility_modifier)]
#![feature(decl_macro)]
#![feature(exact_size_is_empty)]
#![feature(in_band_lifetimes)]
#![feature(iter_zip)]
#![feature(map_try_insert)]
#![feature(min_specialization)]
#![feature(slice_ptr_len)]
#![feature(slice_ptr_get)]
#![feature(option_get_or_insert_default)]
#![feature(once_cell)]
#![feature(never_type)]
#![feature(stmt_expr_attributes)]
#![feature(trait_alias)]
#![feature(trusted_len)]
#![feature(trusted_step)]
#![feature(try_blocks)]

#[macro_use]
extern crate tracing;
#[macro_use]
extern crate rustc_middle;

pub mod const_eval;
pub mod dataflow;
pub mod interpret;
pub mod monomorphize;
pub mod transform;
pub mod util;

use rustc_middle::ty::query::Providers;

pub fn provide(providers: &mut Providers) {
    const_eval::provide(providers);
    monomorphize::partitioning::provide(providers);
    monomorphize::polymorphize::provide(providers);
    providers.eval_to_const_value_raw = const_eval::eval_to_const_value_raw_provider;
    providers.eval_to_allocation_raw = const_eval::eval_to_allocation_raw_provider;
    providers.const_caller_location = const_eval::const_caller_location;
    providers.destructure_const = |tcx, param_env_and_value| {
        let (param_env, value) = param_env_and_value.into_parts();
        const_eval::destructure_const(tcx, param_env, value)
    };
    providers.const_to_valtree = |tcx, param_env_and_value| {
        let (param_env, raw) = param_env_and_value.into_parts();
        const_eval::const_to_valtree(tcx, param_env, raw)
    };
    providers.deref_const = |tcx, param_env_and_value| {
        let (param_env, value) = param_env_and_value.into_parts();
        const_eval::deref_const(tcx, param_env, value)
    };
}
