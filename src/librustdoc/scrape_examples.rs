//! This module analyzes provided crates to find examples of uses for items in the
//! current crate being documented.

use crate::config::Options;
use crate::doctest::make_rustc_config;
use rustc_data_structures::fx::FxHashMap;
use rustc_hir::{
    self as hir,
    intravisit::{self, Visitor},
};
use rustc_interface::interface;
use rustc_middle::hir::map::Map;
use rustc_middle::ty::{self, TyCtxt};
use rustc_span::def_id::DefId;
use std::fs;

crate type DefIdCallKey = String;
crate type FnCallLocations = FxHashMap<String, Vec<(usize, usize)>>;
crate type AllCallLocations = FxHashMap<DefIdCallKey, FnCallLocations>;

/// Visitor for traversing a crate and finding instances of function calls.
struct FindCalls<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    map: Map<'tcx>,

    /// Workspace-relative path to the root of the crate. Used to remember
    /// which example a particular call came from.
    file_path: String,

    /// Data structure to accumulate call sites across all examples.
    calls: &'a mut AllCallLocations,
}

crate fn def_id_call_key(tcx: TyCtxt<'_>, def_id: DefId) -> DefIdCallKey {
    format!(
        "{}{}",
        tcx.crate_name(def_id.krate).to_ident_string(),
        tcx.def_path(def_id).to_string_no_crate_verbose()
    )
}

impl<'a, 'tcx> Visitor<'tcx> for FindCalls<'a, 'tcx>
where
    'tcx: 'a,
{
    type Map = Map<'tcx>;

    fn nested_visit_map(&mut self) -> intravisit::NestedVisitorMap<Self::Map> {
        intravisit::NestedVisitorMap::OnlyBodies(self.map)
    }

    fn visit_expr(&mut self, ex: &'tcx hir::Expr<'tcx>) {
        intravisit::walk_expr(self, ex);

        // Get type of function if expression is a function call
        let (ty, span) = match ex.kind {
            hir::ExprKind::Call(f, _) => {
                let types = self.tcx.typeck(ex.hir_id.owner);
                (types.node_type(f.hir_id), ex.span)
            }
            hir::ExprKind::MethodCall(_, _, _, span) => {
                let types = self.tcx.typeck(ex.hir_id.owner);
                let def_id = types.type_dependent_def_id(ex.hir_id).unwrap();
                (self.tcx.type_of(def_id), span)
            }
            _ => {
                return;
            }
        };

        // Save call site if the function resolves to a concrete definition
        if let ty::FnDef(def_id, _) = ty.kind() {
            let key = def_id_call_key(self.tcx, *def_id);
            let entries = self.calls.entry(key).or_insert_with(FxHashMap::default);
            entries
                .entry(self.file_path.clone())
                .or_insert_with(Vec::new)
                .push((span.lo().0 as usize, span.hi().0 as usize));
        }
    }
}

crate fn run(options: Options) -> interface::Result<()> {
    let inner = move || {
        let config = make_rustc_config(&options);

        // Get input file path as relative to workspace root
        let file_path = options
            .input
            .strip_prefix(options.workspace_root.as_ref().unwrap())
            .map_err(|e| format!("{}", e))?;

        interface::run_compiler(config, |compiler| {
            compiler.enter(|queries| {
                let mut global_ctxt = queries.global_ctxt().unwrap().take();
                global_ctxt.enter(|tcx| {
                    // Run call-finder on all items
                    let mut calls = FxHashMap::default();
                    let mut finder = FindCalls {
                        calls: &mut calls,
                        tcx,
                        map: tcx.hir(),
                        file_path: file_path.display().to_string(),
                    };
                    tcx.hir().krate().visit_all_item_likes(&mut finder.as_deep_visitor());

                    // Save output JSON to provided path
                    let calls_json = serde_json::to_string(&calls).map_err(|e| format!("{}", e))?;
                    fs::write(options.scrape_examples.as_ref().unwrap(), &calls_json)
                        .map_err(|e| format!("{}", e))?;

                    Ok(())
                })
            })
        })
    };

    inner().map_err(|e: String| {
        eprintln!("{}", e);
        rustc_errors::ErrorReported
    })
}
