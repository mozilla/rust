use rustc_ast::{self as ast, MetaItem};
use rustc_middle::ty;
use rustc_session::Session;
use rustc_span::symbol::{sym, Symbol};

pub(crate) use self::drop_flag_effects::*;
pub use self::framework::{
    fmt, lattice, visit_results, Analysis, AnalysisDomain, Backward, Direction, Engine, Forward,
    GenKill, GenKillAnalysis, JoinSemiLattice, Results, ResultsCursor, ResultsRefCursor,
    ResultsVisitable, ResultsVisitor,
};

use self::move_paths::MoveData;

pub mod drop_flag_effects;
mod framework;
pub mod impls;
pub mod move_paths;

pub(crate) mod indexes {
    pub(crate) use super::move_paths::MovePathIndex;
}

pub struct MoveDataParamEnv<'tcx> {
    pub move_data: MoveData<'tcx>,
    pub param_env: ty::ParamEnv<'tcx>,
}

pub(crate) fn has_rustc_mir_with(
    sess: &Session,
    attrs: &[ast::Attribute],
    name: Symbol,
) -> Option<MetaItem> {
    for attr in attrs {
        if sess.check_name(attr, sym::rustc_mir) {
            let items = attr.meta_item_list();
            for item in items.iter().flat_map(|l| l.iter()) {
                match item.meta_item() {
                    Some(mi) if mi.has_name(name) => return Some(mi.clone()),
                    _ => continue,
                }
            }
        }
    }
    None
}
