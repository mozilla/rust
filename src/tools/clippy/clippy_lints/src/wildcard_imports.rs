use crate::utils::{in_macro, snippet, snippet_with_applicability, span_lint_and_sugg};
use if_chain::if_chain;
use rustc_errors::Applicability;
use rustc_hir::{
    def::{DefKind, Res},
    Item, ItemKind, UseKind,
};
use rustc_lint::{LateContext, LateLintPass};
use rustc_session::{declare_lint_pass, declare_tool_lint};
use rustc_span::BytePos;

declare_clippy_lint! {
    /// **What it does:** Checks for `use Enum::*`.
    ///
    /// **Why is this bad?** It is usually better style to use the prefixed name of
    /// an enumeration variant, rather than importing variants.
    ///
    /// **Known problems:** Old-style enumerations that prefix the variants are
    /// still around.
    ///
    /// **Example:**
    /// ```rust
    /// use std::cmp::Ordering::*;
    /// ```
    pub ENUM_GLOB_USE,
    pedantic,
    "use items that import all variants of an enum"
}

declare_clippy_lint! {
    /// **What it does:** Checks for wildcard imports `use _::*`.
    ///
    /// **Why is this bad?** wildcard imports can polute the namespace. This is especially bad if
    /// you try to import something through a wildcard, that already has been imported by name from
    /// a different source:
    ///
    /// ```rust,ignore
    /// use crate1::foo; // Imports a function named foo
    /// use crate2::*; // Has a function named foo
    ///
    /// foo(); // Calls crate1::foo
    /// ```
    ///
    /// This can lead to confusing error messages at best and to unexpected behavior at worst.
    ///
    /// Note that this will not warn about wildcard imports from modules named `prelude`; many
    /// crates (including the standard library) provide modules named "prelude" specifically
    /// designed for wildcard import.
    ///
    /// **Known problems:** If macros are imported through the wildcard, this macro is not included
    /// by the suggestion and has to be added by hand.
    ///
    /// Applying the suggestion when explicit imports of the things imported with a glob import
    /// exist, may result in `unused_imports` warnings.
    ///
    /// **Example:**
    ///
    /// Bad:
    /// ```rust,ignore
    /// use crate1::*;
    ///
    /// foo();
    /// ```
    ///
    /// Good:
    /// ```rust,ignore
    /// use crate1::foo;
    ///
    /// foo();
    /// ```
    pub WILDCARD_IMPORTS,
    pedantic,
    "lint `use _::*` statements"
}

declare_lint_pass!(WildcardImports => [ENUM_GLOB_USE, WILDCARD_IMPORTS]);

impl LateLintPass<'_, '_> for WildcardImports {
    fn check_item(&mut self, cx: &LateContext<'_, '_>, item: &Item<'_>) {
        if item.vis.node.is_pub() || item.vis.node.is_pub_restricted() {
            return;
        }
        if_chain! {
            if !in_macro(item.span);
            if let ItemKind::Use(use_path, UseKind::Glob) = &item.kind;
            // don't lint prelude glob imports
            if !use_path.segments.iter().last().map_or(false, |ps| ps.ident.as_str() == "prelude");
            let used_imports = cx.tcx.names_imported_by_glob_use(item.hir_id.owner.to_def_id());
            if !used_imports.is_empty(); // Already handled by `unused_imports`
            then {
                let mut applicability = Applicability::MachineApplicable;
                let import_source_snippet = snippet_with_applicability(cx, use_path.span, "..", &mut applicability);
                let (span, braced_glob) = if import_source_snippet.is_empty() {
                    // This is a `_::{_, *}` import
                    // In this case `use_path.span` is empty and ends directly in front of the `*`,
                    // so we need to extend it by one byte.
                    (
                        use_path.span.with_hi(use_path.span.hi() + BytePos(1)),
                        true,
                    )
                } else {
                    // In this case, the `use_path.span` ends right before the `::*`, so we need to
                    // extend it up to the `*`. Since it is hard to find the `*` in weird
                    // formattings like `use _ ::  *;`, we extend it up to, but not including the
                    // `;`. In nested imports, like `use _::{inner::*, _}` there is no `;` and we
                    // can just use the end of the item span
                    let mut span = use_path.span.with_hi(item.span.hi());
                    if snippet(cx, span, "").ends_with(';') {
                        span = use_path.span.with_hi(item.span.hi() - BytePos(1));
                    }
                    (
                        span,
                        false,
                    )
                };

                let imports_string = if used_imports.len() == 1 {
                    used_imports.iter().next().unwrap().to_string()
                } else {
                    let mut imports = used_imports
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>();
                    imports.sort();
                    if braced_glob {
                        imports.join(", ")
                    } else {
                        format!("{{{}}}", imports.join(", "))
                    }
                };

                let sugg = if braced_glob {
                    imports_string
                } else {
                    format!("{}::{}", import_source_snippet, imports_string)
                };

                let (lint, message) = if let Res::Def(DefKind::Enum, _) = use_path.res {
                    (ENUM_GLOB_USE, "usage of wildcard import for enum variants")
                } else {
                    (WILDCARD_IMPORTS, "usage of wildcard import")
                };

                span_lint_and_sugg(
                    cx,
                    lint,
                    span,
                    message,
                    "try",
                    sugg,
                    applicability,
                );
            }
        }
    }
}
