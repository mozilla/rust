/// This declares a list of types which can be allocated by `Arena`.
///
/// The `few` modifier will cause allocation to use the shared arena and recording the destructor.
/// This is faster and more memory efficient if there's only a few allocations of the type.
/// Leaving `few` out will cause the type to get its own dedicated `TypedArena` which is
/// faster and more memory efficient if there is lots of allocations.
///
/// Specifying the `decode` modifier will add decode impls for &T and &[T] where T is the type
/// listed. These impls will appear in the implement_ty_decoder! macro.
#[macro_export]
macro_rules! arena_types {
    ($macro:path, $args:tt, $tcx:lifetime) => (
        $macro!($args, [
            [] layouts: rustc_target::abi::Layout,
            // AdtDef are interned and compared by address
            [] adt_def: rustc_middle::ty::AdtDef,
            [decode] tables: rustc_middle::ty::TypeckTables<$tcx>,
            [] const_allocs: rustc_middle::mir::interpret::Allocation,
            // Required for the incremental on-disk cache
            [few, decode] mir_keys: rustc_hir::def_id::DefIdSet,
            [] region_scope_tree: rustc_middle::middle::region::ScopeTree,
            [] dropck_outlives:
                rustc_middle::infer::canonical::Canonical<'tcx,
                    rustc_middle::infer::canonical::QueryResponse<'tcx,
                        rustc_middle::traits::query::DropckOutlivesResult<'tcx>
                    >
                >,
            [] normalize_projection_ty:
                rustc_middle::infer::canonical::Canonical<'tcx,
                    rustc_middle::infer::canonical::QueryResponse<'tcx,
                        rustc_middle::traits::query::NormalizationResult<'tcx>
                    >
                >,
            [] implied_outlives_bounds:
                rustc_middle::infer::canonical::Canonical<'tcx,
                    rustc_middle::infer::canonical::QueryResponse<'tcx,
                        Vec<rustc_middle::traits::query::OutlivesBound<'tcx>>
                    >
                >,
            [] type_op_subtype:
                rustc_middle::infer::canonical::Canonical<'tcx,
                    rustc_middle::infer::canonical::QueryResponse<'tcx, ()>
                >,
            [] type_op_normalize_poly_fn_sig:
                rustc_middle::infer::canonical::Canonical<'tcx,
                    rustc_middle::infer::canonical::QueryResponse<'tcx, rustc_middle::ty::PolyFnSig<'tcx>>
                >,
            [] type_op_normalize_fn_sig:
                rustc_middle::infer::canonical::Canonical<'tcx,
                    rustc_middle::infer::canonical::QueryResponse<'tcx, rustc_middle::ty::FnSig<'tcx>>
                >,
            [] type_op_normalize_predicate:
                rustc_middle::infer::canonical::Canonical<'tcx,
                    rustc_middle::infer::canonical::QueryResponse<'tcx, rustc_middle::ty::Predicate<'tcx>>
                >,
            [] type_op_normalize_ty:
                rustc_middle::infer::canonical::Canonical<'tcx,
                    rustc_middle::infer::canonical::QueryResponse<'tcx, rustc_middle::ty::Ty<'tcx>>
                >,
            [few] all_traits: Vec<rustc_hir::def_id::DefId>,
            [few] privacy_access_levels: rustc_middle::middle::privacy::AccessLevels,
            [few] foreign_module: rustc_middle::middle::cstore::ForeignModule,
            [few] foreign_modules: Vec<rustc_middle::middle::cstore::ForeignModule>,
            [] upvars: rustc_data_structures::fx::FxIndexMap<rustc_hir::HirId, rustc_hir::Upvar>,
            [] object_safety_violations: rustc_middle::traits::ObjectSafetyViolation,
            [] codegen_unit: rustc_middle::mir::mono::CodegenUnit<$tcx>,
            [] attribute: rustc_ast::ast::Attribute,
            [] name_set: rustc_data_structures::fx::FxHashSet<rustc_ast::ast::Name>,
            [] hir_id_set: rustc_hir::HirIdSet,
            // Sub-parts of the TypeckTables
            [decode] concrete_opaque_types: rustc_data_structures::fx::FxHashMap<rustc_hir::def_id::DefId, rustc_middle::ty::ResolvedOpaqueTy<$tcx>>,
            [decode] upvar_list: rustc_middle::ty::UpvarListMap,
            [decode] user_provided_sigs: rustc_data_structures::fx::FxHashMap<rustc_hir::def_id::DefId, rustc_middle::ty::CanonicalPolyFnSig<$tcx>>,

            // Interned types
            [] tys: rustc_middle::ty::TyS<$tcx>,

            // HIR query types
            [few] indexed_hir: rustc_middle::hir::map::IndexedHir<$tcx>,
            [few] hir_definitions: rustc_hir::definitions::Definitions,
            [] hir_owner: rustc_middle::hir::Owner<$tcx>,
            [] hir_owner_nodes: rustc_middle::hir::OwnerNodes<$tcx>,
        ], $tcx);
    )
}

arena_types!(arena::declare_arena, [], 'tcx);
