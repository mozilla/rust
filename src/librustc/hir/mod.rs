//! HIR datatypes. See the [rustc guide] for more info.
//!
//! [rustc guide]: https://rust-lang.github.io/rustc-guide/hir.html

pub mod check_attr;
pub mod exports;
pub mod map;

use crate::ty::query::Providers;
use crate::ty::TyCtxt;
use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_hir::print;
use rustc_hir::Crate;
use rustc_hir::HirId;
use std::ops::Deref;

/// A wrapper type which allows you to access HIR.
#[derive(Clone)]
pub struct Hir<'tcx> {
    tcx: TyCtxt<'tcx>,
    map: &'tcx map::Map<'tcx>,
}

impl<'tcx> Hir<'tcx> {
    pub fn krate(&self) -> &'tcx Crate<'tcx> {
        self.tcx.hir_crate(LOCAL_CRATE)
    }
}

impl<'tcx> Deref for Hir<'tcx> {
    type Target = &'tcx map::Map<'tcx>;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl<'hir> print::PpAnn for Hir<'hir> {
    fn nested(&self, state: &mut print::State<'_>, nested: print::Nested) {
        self.map.nested(state, nested)
    }
}

impl<'tcx> TyCtxt<'tcx> {
    #[inline(always)]
    pub fn hir(self) -> Hir<'tcx> {
        Hir { tcx: self, map: &self.hir_map }
    }

    pub fn hir_id_parent_module(self, id: HirId) -> DefId {
        self.parent_module(DefId::local(id.owner))
    }
}

pub fn provide(providers: &mut Providers<'_>) {
    providers.parent_module = |tcx, id| {
        let hir = tcx.hir();
        hir.get_module_parent(hir.as_local_hir_id(id).unwrap())
    };
    providers.hir_crate = |tcx, _| tcx.hir_map.untracked_krate();
    map::provide(providers);
}
