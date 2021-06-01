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

crate type DefIdExampleKey = String;
crate type FnCallLocations = FxHashMap<String, Vec<(usize, usize)>>;
crate type AllCallLocations = FxHashMap<DefIdExampleKey, FnCallLocations>;

/// Visitor for traversing a crate and finding instances of function calls.
struct FindCalls<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    map: Map<'tcx>,

    /// Workspace-relative path to the root of the crate. Used to remember
    /// which example a particular call came from.
    file_name: String,

    /// Data structure to accumulate call sites across all examples.
    calls: &'a mut AllCallLocations,
}

crate fn def_id_example_key(tcx: TyCtxt<'_>, def_id: DefId) -> DefIdExampleKey {
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
        let types = self.tcx.typeck(ex.hir_id.owner);
        let (ty, span) = match ex.kind {
            hir::ExprKind::Call(f, _) => (types.node_type(f.hir_id), ex.span),
            hir::ExprKind::MethodCall(_, _, _, span) => {
                let types = self.tcx.typeck(ex.hir_id.owner);
                let def_id = types.type_dependent_def_id(ex.hir_id).unwrap();
                (self.tcx.type_of(def_id), span)
            }
            _ => {
                return;
            }
        };

        // Save call site if the function resovles to a concrete definition
        if let ty::FnDef(def_id, _) = ty.kind() {
            let key = def_id_example_key(self.tcx, *def_id);
            let entries = self.calls.entry(key).or_insert_with(FxHashMap::default);
            entries
                .entry(self.file_name.clone())
                .or_insert_with(Vec::new)
                .push((span.lo().0 as usize, span.hi().0 as usize));
        }
    }
}

crate fn run(options: Options) -> interface::Result<()> {
    let config = make_rustc_config(&options);
    interface::run_compiler(config, |compiler| {
        compiler.enter(|queries| {
            let mut global_ctxt = queries.global_ctxt()?.take();
            global_ctxt
                .enter(|tcx| {
                    let mut calls = FxHashMap::default();
                    let mut finder = FindCalls {
                        calls: &mut calls,
                        tcx,
                        map: tcx.hir(),
                        file_name: format!("{}", options.input.display()),
                    };
                    tcx.hir().krate().visit_all_item_likes(&mut finder.as_deep_visitor());

                    let calls_json = serde_json::to_string(&calls).map_err(|e| format!("{}", e))?;
                    fs::write(options.scrape_examples.as_ref().unwrap(), &calls_json)
                        .map_err(|e| format!("{}", e))?;

                    Ok(())
                })
                .map_err(|e: String| {
                    eprintln!("{}", e);
                    rustc_errors::ErrorReported
                })
        })
    })

    // // FIXME(wcrichto): is there a more robust way to get arguments than split(" ")?
    // let mut args = example.split(" ").map(|s| s.to_owned()).collect::<Vec<_>>();
    // let file_name = args[0].clone();
    // args.insert(0, "_".to_string());

    // // FIXME(wcrichto): is there any setup / cleanup that needs to be performed
    // // here upon the invocation of rustc_driver?
    // debug!("Scraping examples from krate {} with args:\n{:?}", krate, args);
    // let mut callbacks =
    //   Callbacks { calls: FxHashMap::default(), file_name, krate: krate.to_string() };
    // rustc_driver::RunCompiler::new(&args, &mut callbacks).run()?;
}
