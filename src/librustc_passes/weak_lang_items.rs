//! Validity checking for weak lang items

use rustc::middle::lang_items;
use rustc::middle::lang_items::whitelisted;
use rustc::session::config;

use rustc::hir::map::Map;
use rustc::ty::TyCtxt;
use rustc_data_structures::fx::FxHashSet;
use rustc_errors::struct_span_err;
use rustc_hir as hir;
use rustc_hir::intravisit::{self, NestedVisitorMap, Visitor};
use rustc_hir::weak_lang_items::WEAK_ITEMS_REFS;
use rustc_span::symbol::Symbol;
use rustc_span::Span;

struct Context<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    items: &'a mut lang_items::LanguageItems,
}

/// Checks the crate for usage of weak lang items, returning a vector of all the
/// language items required by this crate, but not defined yet.
pub fn check_crate<'tcx>(tcx: TyCtxt<'tcx>, items: &mut lang_items::LanguageItems) {
    // These are never called by user code, they're generated by the compiler.
    // They will never implicitly be added to the `missing` array unless we do
    // so here.
    if items.eh_personality().is_none() {
        items.missing.push(lang_items::EhPersonalityLangItem);
    }
    if tcx.sess.target.target.options.custom_unwind_resume & items.eh_unwind_resume().is_none() {
        items.missing.push(lang_items::EhUnwindResumeLangItem);
    }

    {
        let mut cx = Context { tcx, items };
        tcx.hir().krate().visit_all_item_likes(&mut cx.as_deep_visitor());
    }
    verify(tcx, items);
}

fn verify<'tcx>(tcx: TyCtxt<'tcx>, items: &lang_items::LanguageItems) {
    // We only need to check for the presence of weak lang items if we're
    // emitting something that's not an rlib.
    let needs_check = tcx.sess.crate_types.borrow().iter().any(|kind| match *kind {
        config::CrateType::Dylib
        | config::CrateType::ProcMacro
        | config::CrateType::Cdylib
        | config::CrateType::Executable
        | config::CrateType::Staticlib => true,
        config::CrateType::Rlib => false,
    });
    if !needs_check {
        return;
    }

    let mut missing = FxHashSet::default();
    for &cnum in tcx.crates().iter() {
        for &item in tcx.missing_lang_items(cnum).iter() {
            missing.insert(item);
        }
    }

    for (name, &item) in WEAK_ITEMS_REFS.iter() {
        if missing.contains(&item) && !whitelisted(tcx, item) && items.require(item).is_err() {
            if item == lang_items::PanicImplLangItem {
                tcx.sess.err("`#[panic_handler]` function required, but not found");
            } else if item == lang_items::OomLangItem {
                tcx.sess.err("`#[alloc_error_handler]` function required, but not found");
            } else {
                tcx.sess.err(&format!("language item required, but not found: `{}`", name));
            }
        }
    }
}

impl<'a, 'tcx> Context<'a, 'tcx> {
    fn register(&mut self, name: Symbol, span: Span) {
        if let Some(&item) = WEAK_ITEMS_REFS.get(&name) {
            if self.items.require(item).is_err() {
                self.items.missing.push(item);
            }
        } else {
            struct_span_err!(self.tcx.sess, span, E0264, "unknown external lang item: `{}`", name)
                .emit();
        }
    }
}

impl<'a, 'tcx, 'v> Visitor<'v> for Context<'a, 'tcx> {
    type Map = Map<'v>;

    fn nested_visit_map<'this>(&'this mut self) -> NestedVisitorMap<'this, Map<'v>> {
        NestedVisitorMap::None
    }

    fn visit_foreign_item(&mut self, i: &hir::ForeignItem<'_>) {
        if let Some((lang_item, _)) = hir::lang_items::extract(&i.attrs) {
            self.register(lang_item, i.span);
        }
        intravisit::walk_foreign_item(self, i)
    }
}
