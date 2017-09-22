// Copyright 2012-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use hir::def_id::{CrateNum, DefId, DefIndex};
use ty::{self, Ty, TyCtxt};
use ty::maps::queries;
use ty::subst::Substs;

use std::hash::Hash;
use syntax_pos::symbol::InternedString;

/// Query configuration and description traits.

pub trait QueryConfig {
    type Key: Eq + Hash + Clone;
    type Value;
}

pub(super) trait QueryDescription: QueryConfig {
    fn describe(tcx: TyCtxt, key: Self::Key) -> String;
}

impl<M: QueryConfig<Key=DefId>> QueryDescription for M {
    default fn describe(tcx: TyCtxt, def_id: DefId) -> String {
        format!("processing `{}`", tcx.item_path_str(def_id))
    }
}

impl<'tcx> QueryDescription for queries::is_copy_raw<'tcx> {
    fn describe(_tcx: TyCtxt, env: ty::ParamEnvAnd<'tcx, Ty<'tcx>>) -> String {
        format!("computing whether `{}` is `Copy`", env.value)
    }
}

impl<'tcx> QueryDescription for queries::is_sized_raw<'tcx> {
    fn describe(_tcx: TyCtxt, env: ty::ParamEnvAnd<'tcx, Ty<'tcx>>) -> String {
        format!("computing whether `{}` is `Sized`", env.value)
    }
}

impl<'tcx> QueryDescription for queries::is_freeze_raw<'tcx> {
    fn describe(_tcx: TyCtxt, env: ty::ParamEnvAnd<'tcx, Ty<'tcx>>) -> String {
        format!("computing whether `{}` is freeze", env.value)
    }
}

impl<'tcx> QueryDescription for queries::needs_drop_raw<'tcx> {
    fn describe(_tcx: TyCtxt, env: ty::ParamEnvAnd<'tcx, Ty<'tcx>>) -> String {
        format!("computing whether `{}` needs drop", env.value)
    }
}

impl<'tcx> QueryDescription for queries::layout_raw<'tcx> {
    fn describe(_tcx: TyCtxt, env: ty::ParamEnvAnd<'tcx, Ty<'tcx>>) -> String {
        format!("computing layout of `{}`", env.value)
    }
}

impl<'tcx> QueryDescription for queries::super_predicates_of<'tcx> {
    fn describe(tcx: TyCtxt, def_id: DefId) -> String {
        format!("computing the supertraits of `{}`",
                tcx.item_path_str(def_id))
    }
}

impl<'tcx> QueryDescription for queries::type_param_predicates<'tcx> {
    fn describe(tcx: TyCtxt, (_, def_id): (DefId, DefId)) -> String {
        let id = tcx.hir.as_local_node_id(def_id).unwrap();
        format!("computing the bounds for type parameter `{}`",
                tcx.hir.ty_param_name(id))
    }
}

impl<'tcx> QueryDescription for queries::coherent_trait<'tcx> {
    fn describe(tcx: TyCtxt, (_, def_id): (CrateNum, DefId)) -> String {
        format!("coherence checking all impls of trait `{}`",
                tcx.item_path_str(def_id))
    }
}

impl<'tcx> QueryDescription for queries::crate_inherent_impls<'tcx> {
    fn describe(_: TyCtxt, k: CrateNum) -> String {
        format!("all inherent impls defined in crate `{:?}`", k)
    }
}

impl<'tcx> QueryDescription for queries::crate_inherent_impls_overlap_check<'tcx> {
    fn describe(_: TyCtxt, _: CrateNum) -> String {
        format!("check for overlap between inherent impls defined in this crate")
    }
}

impl<'tcx> QueryDescription for queries::crate_variances<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("computing the variances for items in this crate")
    }
}

impl<'tcx> QueryDescription for queries::mir_shims<'tcx> {
    fn describe(tcx: TyCtxt, def: ty::InstanceDef<'tcx>) -> String {
        format!("generating MIR shim for `{}`",
                tcx.item_path_str(def.def_id()))
    }
}

impl<'tcx> QueryDescription for queries::privacy_access_levels<'tcx> {
    fn describe(_: TyCtxt, _: CrateNum) -> String {
        format!("privacy access levels")
    }
}

impl<'tcx> QueryDescription for queries::typeck_item_bodies<'tcx> {
    fn describe(_: TyCtxt, _: CrateNum) -> String {
        format!("type-checking all item bodies")
    }
}

impl<'tcx> QueryDescription for queries::reachable_set<'tcx> {
    fn describe(_: TyCtxt, _: CrateNum) -> String {
        format!("reachability")
    }
}

impl<'tcx> QueryDescription for queries::const_eval<'tcx> {
    fn describe(tcx: TyCtxt, key: ty::ParamEnvAnd<'tcx, (DefId, &'tcx Substs<'tcx>)>) -> String {
        format!("const-evaluating `{}`", tcx.item_path_str(key.value.0))
    }
}

impl<'tcx> QueryDescription for queries::mir_keys<'tcx> {
    fn describe(_: TyCtxt, _: CrateNum) -> String {
        format!("getting a list of all mir_keys")
    }
}

impl<'tcx> QueryDescription for queries::symbol_name<'tcx> {
    fn describe(_tcx: TyCtxt, instance: ty::Instance<'tcx>) -> String {
        format!("computing the symbol for `{}`", instance)
    }
}

impl<'tcx> QueryDescription for queries::describe_def<'tcx> {
    fn describe(_: TyCtxt, _: DefId) -> String {
        bug!("describe_def")
    }
}

impl<'tcx> QueryDescription for queries::def_span<'tcx> {
    fn describe(_: TyCtxt, _: DefId) -> String {
        bug!("def_span")
    }
}


impl<'tcx> QueryDescription for queries::lookup_stability<'tcx> {
    fn describe(_: TyCtxt, _: DefId) -> String {
        bug!("stability")
    }
}

impl<'tcx> QueryDescription for queries::lookup_deprecation_entry<'tcx> {
    fn describe(_: TyCtxt, _: DefId) -> String {
        bug!("deprecation")
    }
}

impl<'tcx> QueryDescription for queries::item_attrs<'tcx> {
    fn describe(_: TyCtxt, _: DefId) -> String {
        bug!("item_attrs")
    }
}

impl<'tcx> QueryDescription for queries::is_exported_symbol<'tcx> {
    fn describe(_: TyCtxt, _: DefId) -> String {
        bug!("is_exported_symbol")
    }
}

impl<'tcx> QueryDescription for queries::fn_arg_names<'tcx> {
    fn describe(_: TyCtxt, _: DefId) -> String {
        bug!("fn_arg_names")
    }
}

impl<'tcx> QueryDescription for queries::impl_parent<'tcx> {
    fn describe(_: TyCtxt, _: DefId) -> String {
        bug!("impl_parent")
    }
}

impl<'tcx> QueryDescription for queries::trait_of_item<'tcx> {
    fn describe(_: TyCtxt, _: DefId) -> String {
        bug!("trait_of_item")
    }
}

impl<'tcx> QueryDescription for queries::item_body_nested_bodies<'tcx> {
    fn describe(tcx: TyCtxt, def_id: DefId) -> String {
        format!("nested item bodies of `{}`", tcx.item_path_str(def_id))
    }
}

impl<'tcx> QueryDescription for queries::const_is_rvalue_promotable_to_static<'tcx> {
    fn describe(tcx: TyCtxt, def_id: DefId) -> String {
        format!("const checking if rvalue is promotable to static `{}`",
            tcx.item_path_str(def_id))
    }
}

impl<'tcx> QueryDescription for queries::is_mir_available<'tcx> {
    fn describe(tcx: TyCtxt, def_id: DefId) -> String {
        format!("checking if item is mir available: `{}`",
            tcx.item_path_str(def_id))
    }
}

impl<'tcx> QueryDescription for queries::trait_impls_of<'tcx> {
    fn describe(tcx: TyCtxt, def_id: DefId) -> String {
        format!("trait impls of `{}`", tcx.item_path_str(def_id))
    }
}

impl<'tcx> QueryDescription for queries::is_object_safe<'tcx> {
    fn describe(tcx: TyCtxt, def_id: DefId) -> String {
        format!("determine object safety of trait `{}`", tcx.item_path_str(def_id))
    }
}

impl<'tcx> QueryDescription for queries::is_const_fn<'tcx> {
    fn describe(tcx: TyCtxt, def_id: DefId) -> String {
        format!("checking if item is const fn: `{}`", tcx.item_path_str(def_id))
    }
}

impl<'tcx> QueryDescription for queries::dylib_dependency_formats<'tcx> {
    fn describe(_: TyCtxt, _: CrateNum) -> String {
        "dylib dependency formats of crate".to_string()
    }
}

impl<'tcx> QueryDescription for queries::is_panic_runtime<'tcx> {
    fn describe(_: TyCtxt, _: CrateNum) -> String {
        "checking if the crate is_panic_runtime".to_string()
    }
}

impl<'tcx> QueryDescription for queries::is_compiler_builtins<'tcx> {
    fn describe(_: TyCtxt, _: CrateNum) -> String {
        "checking if the crate is_compiler_builtins".to_string()
    }
}

impl<'tcx> QueryDescription for queries::has_global_allocator<'tcx> {
    fn describe(_: TyCtxt, _: CrateNum) -> String {
        "checking if the crate has_global_allocator".to_string()
    }
}

impl<'tcx> QueryDescription for queries::extern_crate<'tcx> {
    fn describe(_: TyCtxt, _: DefId) -> String {
        "getting crate's ExternCrateData".to_string()
    }
}

impl<'tcx> QueryDescription for queries::lint_levels<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("computing the lint levels for items in this crate")
    }
}

impl<'tcx> QueryDescription for queries::specializes<'tcx> {
    fn describe(_tcx: TyCtxt, _: (DefId, DefId)) -> String {
        format!("computing whether impls specialize one another")
    }
}

impl<'tcx> QueryDescription for queries::in_scope_traits_map<'tcx> {
    fn describe(_tcx: TyCtxt, _: DefIndex) -> String {
        format!("traits in scope at a block")
    }
}

impl<'tcx> QueryDescription for queries::is_no_builtins<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("test whether a crate has #![no_builtins]")
    }
}

impl<'tcx> QueryDescription for queries::panic_strategy<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("query a crate's configured panic strategy")
    }
}

impl<'tcx> QueryDescription for queries::is_profiler_runtime<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("query a crate is #![profiler_runtime]")
    }
}

impl<'tcx> QueryDescription for queries::is_sanitizer_runtime<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("query a crate is #![sanitizer_runtime]")
    }
}

impl<'tcx> QueryDescription for queries::exported_symbol_ids<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("looking up the exported symbols of a crate")
    }
}

impl<'tcx> QueryDescription for queries::native_libraries<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("looking up the native libraries of a linked crate")
    }
}

impl<'tcx> QueryDescription for queries::plugin_registrar_fn<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("looking up the plugin registrar for a crate")
    }
}

impl<'tcx> QueryDescription for queries::derive_registrar_fn<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("looking up the derive registrar for a crate")
    }
}

impl<'tcx> QueryDescription for queries::crate_disambiguator<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("looking up the disambiguator a crate")
    }
}

impl<'tcx> QueryDescription for queries::crate_hash<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("looking up the hash a crate")
    }
}

impl<'tcx> QueryDescription for queries::original_crate_name<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("looking up the original name a crate")
    }
}

impl<'tcx> QueryDescription for queries::implementations_of_trait<'tcx> {
    fn describe(_tcx: TyCtxt, _: (CrateNum, DefId)) -> String {
        format!("looking up implementations of a trait in a crate")
    }
}

impl<'tcx> QueryDescription for queries::all_trait_implementations<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("looking up all (?) trait implementations")
    }
}

impl<'tcx> QueryDescription for queries::link_args<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("looking up link arguments for a crate")
    }
}

impl<'tcx> QueryDescription for queries::named_region_map<'tcx> {
    fn describe(_tcx: TyCtxt, _: DefIndex) -> String {
        format!("looking up a named region")
    }
}

impl<'tcx> QueryDescription for queries::is_late_bound_map<'tcx> {
    fn describe(_tcx: TyCtxt, _: DefIndex) -> String {
        format!("testing if a region is late boudn")
    }
}

impl<'tcx> QueryDescription for queries::object_lifetime_defaults_map<'tcx> {
    fn describe(_tcx: TyCtxt, _: DefIndex) -> String {
        format!("looking up lifetime defaults for a region")
    }
}

impl<'tcx> QueryDescription for queries::dep_kind<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("fetching what a dependency looks like")
    }
}

impl<'tcx> QueryDescription for queries::crate_name<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("fetching what a crate is named")
    }
}

impl<'tcx> QueryDescription for queries::get_lang_items<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("calculating the lang items map")
    }
}

impl<'tcx> QueryDescription for queries::defined_lang_items<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("calculating the lang items defined in a crate")
    }
}

impl<'tcx> QueryDescription for queries::missing_lang_items<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("calculating the missing lang items in a crate")
    }
}

impl<'tcx> QueryDescription for queries::visible_parent_map<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("calculating the visible parent map")
    }
}

impl<'tcx> QueryDescription for queries::missing_extern_crate_item<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("seeing if we're missing an `extern crate` item for this crate")
    }
}

impl<'tcx> QueryDescription for queries::used_crate_source<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("looking at the source for a crate")
    }
}

impl<'tcx> QueryDescription for queries::postorder_cnums<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("generating a postorder list of CrateNums")
    }
}

impl<'tcx> QueryDescription for queries::maybe_unused_extern_crates<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("looking up all possibly unused extern crates")
    }
}

impl<'tcx> QueryDescription for queries::stability_index<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("calculating the stability index for the local crate")
    }
}

impl<'tcx> QueryDescription for queries::all_crate_nums<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("fetching all foreign CrateNum instances")
    }
}

impl<'tcx> QueryDescription for queries::exported_symbols<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("exported_symbols")
    }
}

impl<'tcx> QueryDescription for queries::collect_and_partition_translation_items<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("collect_and_partition_translation_items")
    }
}

impl<'tcx> QueryDescription for queries::codegen_unit<'tcx> {
    fn describe(_tcx: TyCtxt, _: InternedString) -> String {
        format!("codegen_unit")
    }
}

impl<'tcx> QueryDescription for queries::compile_codegen_unit<'tcx> {
    fn describe(_tcx: TyCtxt, _: InternedString) -> String {
        format!("compile_codegen_unit")
    }
}

impl<'tcx> QueryDescription for queries::output_filenames<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("output_filenames")
    }
}

impl<'tcx> QueryDescription for queries::has_clone_closures<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("seeing if the crate has enabled `Clone` closures")
    }
}

impl<'tcx> QueryDescription for queries::has_copy_closures<'tcx> {
    fn describe(_tcx: TyCtxt, _: CrateNum) -> String {
        format!("seeing if the crate has enabled `Copy` closures")
    }
}
