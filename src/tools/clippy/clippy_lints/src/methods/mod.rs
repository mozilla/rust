mod bind_instead_of_map;
mod bytes_nth;
mod clone_on_copy;
mod clone_on_ref_ptr;
mod expect_fun_call;
mod expect_used;
mod filetype_is_file;
mod filter_flat_map;
mod filter_map;
mod filter_map_flat_map;
mod filter_map_identity;
mod filter_map_map;
mod filter_map_next;
mod filter_next;
mod flat_map_identity;
mod from_iter_instead_of_collect;
mod get_unwrap;
mod implicit_clone;
mod inefficient_to_string;
mod inspect_for_each;
mod into_iter_on_ref;
mod iter_cloned_collect;
mod iter_count;
mod iter_next_slice;
mod iter_nth;
mod iter_nth_zero;
mod iter_skip_next;
mod iterator_step_by_zero;
mod manual_saturating_arithmetic;
mod map_collect_result_unit;
mod map_flatten;
mod map_unwrap_or;
mod ok_expect;
mod option_as_ref_deref;
mod option_map_or_none;
mod option_map_unwrap_or;
mod or_fun_call;
mod search_is_some;
mod single_char_insert_string;
mod single_char_pattern;
mod single_char_push_string;
mod skip_while_next;
mod string_extend_chars;
mod suspicious_map;
mod uninit_assumed_init;
mod unnecessary_filter_map;
mod unnecessary_fold;
mod unnecessary_lazy_eval;
mod unwrap_used;
mod useless_asref;
mod wrong_self_convention;
mod zst_offset;

use bind_instead_of_map::BindInsteadOfMap;
use if_chain::if_chain;
use rustc_ast::ast;
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_hir::{TraitItem, TraitItemKind};
use rustc_lint::{LateContext, LateLintPass, Lint, LintContext};
use rustc_middle::lint::in_external_macro;
use rustc_middle::ty::{self, TraitRef, Ty, TyS};
use rustc_semver::RustcVersion;
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::symbol::{sym, SymbolStr};
use rustc_typeck::hir_ty_to_ty;

use crate::utils::{
    contains_return, contains_ty, get_trait_def_id, implements_trait, in_macro, is_copy, is_type_diagnostic_item,
    iter_input_pats, match_def_path, match_qpath, method_calls, method_chain_args, paths, return_ty,
    single_segment_path, snippet_with_applicability, span_lint, span_lint_and_help, span_lint_and_sugg, SpanlessEq,
};

declare_clippy_lint! {
    /// **What it does:** Checks for `.unwrap()` calls on `Option`s and on `Result`s.
    ///
    /// **Why is this bad?** It is better to handle the `None` or `Err` case,
    /// or at least call `.expect(_)` with a more helpful message. Still, for a lot of
    /// quick-and-dirty code, `unwrap` is a good choice, which is why this lint is
    /// `Allow` by default.
    ///
    /// `result.unwrap()` will let the thread panic on `Err` values.
    /// Normally, you want to implement more sophisticated error handling,
    /// and propagate errors upwards with `?` operator.
    ///
    /// Even if you want to panic on errors, not all `Error`s implement good
    /// messages on display. Therefore, it may be beneficial to look at the places
    /// where they may get displayed. Activate this lint to do just that.
    ///
    /// **Known problems:** None.
    ///
    /// **Examples:**
    /// ```rust
    /// # let opt = Some(1);
    ///
    /// // Bad
    /// opt.unwrap();
    ///
    /// // Good
    /// opt.expect("more helpful message");
    /// ```
    ///
    /// // or
    ///
    /// ```rust
    /// # let res: Result<usize, ()> = Ok(1);
    ///
    /// // Bad
    /// res.unwrap();
    ///
    /// // Good
    /// res.expect("more helpful message");
    /// ```
    pub UNWRAP_USED,
    restriction,
    "using `.unwrap()` on `Result` or `Option`, which should at least get a better message using `expect()`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for `.expect()` calls on `Option`s and `Result`s.
    ///
    /// **Why is this bad?** Usually it is better to handle the `None` or `Err` case.
    /// Still, for a lot of quick-and-dirty code, `expect` is a good choice, which is why
    /// this lint is `Allow` by default.
    ///
    /// `result.expect()` will let the thread panic on `Err`
    /// values. Normally, you want to implement more sophisticated error handling,
    /// and propagate errors upwards with `?` operator.
    ///
    /// **Known problems:** None.
    ///
    /// **Examples:**
    /// ```rust,ignore
    /// # let opt = Some(1);
    ///
    /// // Bad
    /// opt.expect("one");
    ///
    /// // Good
    /// let opt = Some(1);
    /// opt?;
    /// ```
    ///
    /// // or
    ///
    /// ```rust
    /// # let res: Result<usize, ()> = Ok(1);
    ///
    /// // Bad
    /// res.expect("one");
    ///
    /// // Good
    /// res?;
    /// # Ok::<(), ()>(())
    /// ```
    pub EXPECT_USED,
    restriction,
    "using `.expect()` on `Result` or `Option`, which might be better handled"
}

declare_clippy_lint! {
    /// **What it does:** Checks for methods that should live in a trait
    /// implementation of a `std` trait (see [llogiq's blog
    /// post](http://llogiq.github.io/2015/07/30/traits.html) for further
    /// information) instead of an inherent implementation.
    ///
    /// **Why is this bad?** Implementing the traits improve ergonomics for users of
    /// the code, often with very little cost. Also people seeing a `mul(...)`
    /// method
    /// may expect `*` to work equally, so you should have good reason to disappoint
    /// them.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// struct X;
    /// impl X {
    ///     fn add(&self, other: &X) -> X {
    ///         // ..
    /// # X
    ///     }
    /// }
    /// ```
    pub SHOULD_IMPLEMENT_TRAIT,
    style,
    "defining a method that should be implementing a std trait"
}

declare_clippy_lint! {
    /// **What it does:** Checks for methods with certain name prefixes and which
    /// doesn't match how self is taken. The actual rules are:
    ///
    /// |Prefix |`self` taken          |
    /// |-------|----------------------|
    /// |`as_`  |`&self` or `&mut self`|
    /// |`from_`| none                 |
    /// |`into_`|`self`                |
    /// |`is_`  |`&self` or none       |
    /// |`to_`  |`&self`               |
    ///
    /// **Why is this bad?** Consistency breeds readability. If you follow the
    /// conventions, your users won't be surprised that they, e.g., need to supply a
    /// mutable reference to a `as_..` function.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// # struct X;
    /// impl X {
    ///     fn as_str(self) -> &'static str {
    ///         // ..
    /// # ""
    ///     }
    /// }
    /// ```
    pub WRONG_SELF_CONVENTION,
    style,
    "defining a method named with an established prefix (like \"into_\") that takes `self` with the wrong convention"
}

declare_clippy_lint! {
    /// **What it does:** This is the same as
    /// [`wrong_self_convention`](#wrong_self_convention), but for public items.
    ///
    /// **Why is this bad?** See [`wrong_self_convention`](#wrong_self_convention).
    ///
    /// **Known problems:** Actually *renaming* the function may break clients if
    /// the function is part of the public interface. In that case, be mindful of
    /// the stability guarantees you've given your users.
    ///
    /// **Example:**
    /// ```rust
    /// # struct X;
    /// impl<'a> X {
    ///     pub fn as_str(self) -> &'a str {
    ///         "foo"
    ///     }
    /// }
    /// ```
    pub WRONG_PUB_SELF_CONVENTION,
    restriction,
    "defining a public method named with an established prefix (like \"into_\") that takes `self` with the wrong convention"
}

declare_clippy_lint! {
    /// **What it does:** Checks for usage of `ok().expect(..)`.
    ///
    /// **Why is this bad?** Because you usually call `expect()` on the `Result`
    /// directly to get a better error message.
    ///
    /// **Known problems:** The error type needs to implement `Debug`
    ///
    /// **Example:**
    /// ```rust
    /// # let x = Ok::<_, ()>(());
    ///
    /// // Bad
    /// x.ok().expect("why did I do this again?");
    ///
    /// // Good
    /// x.expect("why did I do this again?");
    /// ```
    pub OK_EXPECT,
    style,
    "using `ok().expect()`, which gives worse error messages than calling `expect` directly on the Result"
}

declare_clippy_lint! {
    /// **What it does:** Checks for usage of `option.map(_).unwrap_or(_)` or `option.map(_).unwrap_or_else(_)` or
    /// `result.map(_).unwrap_or_else(_)`.
    ///
    /// **Why is this bad?** Readability, these can be written more concisely (resp.) as
    /// `option.map_or(_, _)`, `option.map_or_else(_, _)` and `result.map_or_else(_, _)`.
    ///
    /// **Known problems:** The order of the arguments is not in execution order
    ///
    /// **Examples:**
    /// ```rust
    /// # let x = Some(1);
    ///
    /// // Bad
    /// x.map(|a| a + 1).unwrap_or(0);
    ///
    /// // Good
    /// x.map_or(0, |a| a + 1);
    /// ```
    ///
    /// // or
    ///
    /// ```rust
    /// # let x: Result<usize, ()> = Ok(1);
    /// # fn some_function(foo: ()) -> usize { 1 }
    ///
    /// // Bad
    /// x.map(|a| a + 1).unwrap_or_else(some_function);
    ///
    /// // Good
    /// x.map_or_else(some_function, |a| a + 1);
    /// ```
    pub MAP_UNWRAP_OR,
    pedantic,
    "using `.map(f).unwrap_or(a)` or `.map(f).unwrap_or_else(func)`, which are more succinctly expressed as `map_or(a, f)` or `map_or_else(a, f)`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for usage of `_.map_or(None, _)`.
    ///
    /// **Why is this bad?** Readability, this can be written more concisely as
    /// `_.and_then(_)`.
    ///
    /// **Known problems:** The order of the arguments is not in execution order.
    ///
    /// **Example:**
    /// ```rust
    /// # let opt = Some(1);
    ///
    /// // Bad
    /// opt.map_or(None, |a| Some(a + 1));
    ///
    /// // Good
    /// opt.and_then(|a| Some(a + 1));
    /// ```
    pub OPTION_MAP_OR_NONE,
    style,
    "using `Option.map_or(None, f)`, which is more succinctly expressed as `and_then(f)`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for usage of `_.map_or(None, Some)`.
    ///
    /// **Why is this bad?** Readability, this can be written more concisely as
    /// `_.ok()`.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    ///
    /// Bad:
    /// ```rust
    /// # let r: Result<u32, &str> = Ok(1);
    /// assert_eq!(Some(1), r.map_or(None, Some));
    /// ```
    ///
    /// Good:
    /// ```rust
    /// # let r: Result<u32, &str> = Ok(1);
    /// assert_eq!(Some(1), r.ok());
    /// ```
    pub RESULT_MAP_OR_INTO_OPTION,
    style,
    "using `Result.map_or(None, Some)`, which is more succinctly expressed as `ok()`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for usage of `_.and_then(|x| Some(y))`, `_.and_then(|x| Ok(y))` or
    /// `_.or_else(|x| Err(y))`.
    ///
    /// **Why is this bad?** Readability, this can be written more concisely as
    /// `_.map(|x| y)` or `_.map_err(|x| y)`.
    ///
    /// **Known problems:** None
    ///
    /// **Example:**
    ///
    /// ```rust
    /// # fn opt() -> Option<&'static str> { Some("42") }
    /// # fn res() -> Result<&'static str, &'static str> { Ok("42") }
    /// let _ = opt().and_then(|s| Some(s.len()));
    /// let _ = res().and_then(|s| if s.len() == 42 { Ok(10) } else { Ok(20) });
    /// let _ = res().or_else(|s| if s.len() == 42 { Err(10) } else { Err(20) });
    /// ```
    ///
    /// The correct use would be:
    ///
    /// ```rust
    /// # fn opt() -> Option<&'static str> { Some("42") }
    /// # fn res() -> Result<&'static str, &'static str> { Ok("42") }
    /// let _ = opt().map(|s| s.len());
    /// let _ = res().map(|s| if s.len() == 42 { 10 } else { 20 });
    /// let _ = res().map_err(|s| if s.len() == 42 { 10 } else { 20 });
    /// ```
    pub BIND_INSTEAD_OF_MAP,
    complexity,
    "using `Option.and_then(|x| Some(y))`, which is more succinctly expressed as `map(|x| y)`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for usage of `_.filter(_).next()`.
    ///
    /// **Why is this bad?** Readability, this can be written more concisely as
    /// `_.find(_)`.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// # let vec = vec![1];
    /// vec.iter().filter(|x| **x == 0).next();
    /// ```
    /// Could be written as
    /// ```rust
    /// # let vec = vec![1];
    /// vec.iter().find(|x| **x == 0);
    /// ```
    pub FILTER_NEXT,
    complexity,
    "using `filter(p).next()`, which is more succinctly expressed as `.find(p)`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for usage of `_.skip_while(condition).next()`.
    ///
    /// **Why is this bad?** Readability, this can be written more concisely as
    /// `_.find(!condition)`.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// # let vec = vec![1];
    /// vec.iter().skip_while(|x| **x == 0).next();
    /// ```
    /// Could be written as
    /// ```rust
    /// # let vec = vec![1];
    /// vec.iter().find(|x| **x != 0);
    /// ```
    pub SKIP_WHILE_NEXT,
    complexity,
    "using `skip_while(p).next()`, which is more succinctly expressed as `.find(!p)`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for usage of `_.map(_).flatten(_)` on `Iterator` and `Option`
    ///
    /// **Why is this bad?** Readability, this can be written more concisely as
    /// `_.flat_map(_)`
    ///
    /// **Known problems:**
    ///
    /// **Example:**
    /// ```rust
    /// let vec = vec![vec![1]];
    ///
    /// // Bad
    /// vec.iter().map(|x| x.iter()).flatten();
    ///
    /// // Good
    /// vec.iter().flat_map(|x| x.iter());
    /// ```
    pub MAP_FLATTEN,
    pedantic,
    "using combinations of `flatten` and `map` which can usually be written as a single method call"
}

declare_clippy_lint! {
    /// **What it does:** Checks for usage of `_.filter(_).map(_)`,
    /// `_.filter(_).flat_map(_)`, `_.filter_map(_).flat_map(_)` and similar.
    ///
    /// **Why is this bad?** Readability, this can be written more concisely as
    /// `_.filter_map(_)`.
    ///
    /// **Known problems:** Often requires a condition + Option/Iterator creation
    /// inside the closure.
    ///
    /// **Example:**
    /// ```rust
    /// let vec = vec![1];
    ///
    /// // Bad
    /// vec.iter().filter(|x| **x == 0).map(|x| *x * 2);
    ///
    /// // Good
    /// vec.iter().filter_map(|x| if *x == 0 {
    ///     Some(*x * 2)
    /// } else {
    ///     None
    /// });
    /// ```
    pub FILTER_MAP,
    pedantic,
    "using combinations of `filter`, `map`, `filter_map` and `flat_map` which can usually be written as a single method call"
}

declare_clippy_lint! {
    /// **What it does:** Checks for usage of `_.filter(_).map(_)` that can be written more simply
    /// as `filter_map(_)`.
    ///
    /// **Why is this bad?** Redundant code in the `filter` and `map` operations is poor style and
    /// less performant.
    ///
    /// **Known problems:** None.
    ///
     /// **Example:**
    /// Bad:
    /// ```rust
    /// (0_i32..10)
    ///     .filter(|n| n.checked_add(1).is_some())
    ///     .map(|n| n.checked_add(1).unwrap());
    /// ```
    ///
    /// Good:
    /// ```rust
    /// (0_i32..10).filter_map(|n| n.checked_add(1));
    /// ```
    pub MANUAL_FILTER_MAP,
    complexity,
    "using `_.filter(_).map(_)` in a way that can be written more simply as `filter_map(_)`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for usage of `_.find(_).map(_)` that can be written more simply
    /// as `find_map(_)`.
    ///
    /// **Why is this bad?** Redundant code in the `find` and `map` operations is poor style and
    /// less performant.
    ///
    /// **Known problems:** None.
    ///
     /// **Example:**
    /// Bad:
    /// ```rust
    /// (0_i32..10)
    ///     .find(|n| n.checked_add(1).is_some())
    ///     .map(|n| n.checked_add(1).unwrap());
    /// ```
    ///
    /// Good:
    /// ```rust
    /// (0_i32..10).find_map(|n| n.checked_add(1));
    /// ```
    pub MANUAL_FIND_MAP,
    complexity,
    "using `_.find(_).map(_)` in a way that can be written more simply as `find_map(_)`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for usage of `_.filter_map(_).next()`.
    ///
    /// **Why is this bad?** Readability, this can be written more concisely as
    /// `_.find_map(_)`.
    ///
    /// **Known problems:** None
    ///
    /// **Example:**
    /// ```rust
    ///  (0..3).filter_map(|x| if x == 2 { Some(x) } else { None }).next();
    /// ```
    /// Can be written as
    ///
    /// ```rust
    ///  (0..3).find_map(|x| if x == 2 { Some(x) } else { None });
    /// ```
    pub FILTER_MAP_NEXT,
    pedantic,
    "using combination of `filter_map` and `next` which can usually be written as a single method call"
}

declare_clippy_lint! {
    /// **What it does:** Checks for usage of `flat_map(|x| x)`.
    ///
    /// **Why is this bad?** Readability, this can be written more concisely by using `flatten`.
    ///
    /// **Known problems:** None
    ///
    /// **Example:**
    /// ```rust
    /// # let iter = vec![vec![0]].into_iter();
    /// iter.flat_map(|x| x);
    /// ```
    /// Can be written as
    /// ```rust
    /// # let iter = vec![vec![0]].into_iter();
    /// iter.flatten();
    /// ```
    pub FLAT_MAP_IDENTITY,
    complexity,
    "call to `flat_map` where `flatten` is sufficient"
}

declare_clippy_lint! {
    /// **What it does:** Checks for an iterator or string search (such as `find()`,
    /// `position()`, or `rposition()`) followed by a call to `is_some()`.
    ///
    /// **Why is this bad?** Readability, this can be written more concisely as
    /// `_.any(_)` or `_.contains(_)`.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// # let vec = vec![1];
    /// vec.iter().find(|x| **x == 0).is_some();
    /// ```
    /// Could be written as
    /// ```rust
    /// # let vec = vec![1];
    /// vec.iter().any(|x| *x == 0);
    /// ```
    pub SEARCH_IS_SOME,
    complexity,
    "using an iterator or string search followed by `is_some()`, which is more succinctly expressed as a call to `any()` or `contains()`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for usage of `.chars().next()` on a `str` to check
    /// if it starts with a given char.
    ///
    /// **Why is this bad?** Readability, this can be written more concisely as
    /// `_.starts_with(_)`.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// let name = "foo";
    /// if name.chars().next() == Some('_') {};
    /// ```
    /// Could be written as
    /// ```rust
    /// let name = "foo";
    /// if name.starts_with('_') {};
    /// ```
    pub CHARS_NEXT_CMP,
    style,
    "using `.chars().next()` to check if a string starts with a char"
}

declare_clippy_lint! {
    /// **What it does:** Checks for calls to `.or(foo(..))`, `.unwrap_or(foo(..))`,
    /// etc., and suggests to use `or_else`, `unwrap_or_else`, etc., or
    /// `unwrap_or_default` instead.
    ///
    /// **Why is this bad?** The function will always be called and potentially
    /// allocate an object acting as the default.
    ///
    /// **Known problems:** If the function has side-effects, not calling it will
    /// change the semantic of the program, but you shouldn't rely on that anyway.
    ///
    /// **Example:**
    /// ```rust
    /// # let foo = Some(String::new());
    /// foo.unwrap_or(String::new());
    /// ```
    /// this can instead be written:
    /// ```rust
    /// # let foo = Some(String::new());
    /// foo.unwrap_or_else(String::new);
    /// ```
    /// or
    /// ```rust
    /// # let foo = Some(String::new());
    /// foo.unwrap_or_default();
    /// ```
    pub OR_FUN_CALL,
    perf,
    "using any `*or` method with a function call, which suggests `*or_else`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for calls to `.expect(&format!(...))`, `.expect(foo(..))`,
    /// etc., and suggests to use `unwrap_or_else` instead
    ///
    /// **Why is this bad?** The function will always be called.
    ///
    /// **Known problems:** If the function has side-effects, not calling it will
    /// change the semantics of the program, but you shouldn't rely on that anyway.
    ///
    /// **Example:**
    /// ```rust
    /// # let foo = Some(String::new());
    /// # let err_code = "418";
    /// # let err_msg = "I'm a teapot";
    /// foo.expect(&format!("Err {}: {}", err_code, err_msg));
    /// ```
    /// or
    /// ```rust
    /// # let foo = Some(String::new());
    /// # let err_code = "418";
    /// # let err_msg = "I'm a teapot";
    /// foo.expect(format!("Err {}: {}", err_code, err_msg).as_str());
    /// ```
    /// this can instead be written:
    /// ```rust
    /// # let foo = Some(String::new());
    /// # let err_code = "418";
    /// # let err_msg = "I'm a teapot";
    /// foo.unwrap_or_else(|| panic!("Err {}: {}", err_code, err_msg));
    /// ```
    pub EXPECT_FUN_CALL,
    perf,
    "using any `expect` method with a function call"
}

declare_clippy_lint! {
    /// **What it does:** Checks for usage of `.clone()` on a `Copy` type.
    ///
    /// **Why is this bad?** The only reason `Copy` types implement `Clone` is for
    /// generics, not for using the `clone` method on a concrete type.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// 42u64.clone();
    /// ```
    pub CLONE_ON_COPY,
    complexity,
    "using `clone` on a `Copy` type"
}

declare_clippy_lint! {
    /// **What it does:** Checks for usage of `.clone()` on a ref-counted pointer,
    /// (`Rc`, `Arc`, `rc::Weak`, or `sync::Weak`), and suggests calling Clone via unified
    /// function syntax instead (e.g., `Rc::clone(foo)`).
    ///
    /// **Why is this bad?** Calling '.clone()' on an Rc, Arc, or Weak
    /// can obscure the fact that only the pointer is being cloned, not the underlying
    /// data.
    ///
    /// **Example:**
    /// ```rust
    /// # use std::rc::Rc;
    /// let x = Rc::new(1);
    ///
    /// // Bad
    /// x.clone();
    ///
    /// // Good
    /// Rc::clone(&x);
    /// ```
    pub CLONE_ON_REF_PTR,
    restriction,
    "using 'clone' on a ref-counted pointer"
}

declare_clippy_lint! {
    /// **What it does:** Checks for usage of `.clone()` on an `&&T`.
    ///
    /// **Why is this bad?** Cloning an `&&T` copies the inner `&T`, instead of
    /// cloning the underlying `T`.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// fn main() {
    ///     let x = vec![1];
    ///     let y = &&x;
    ///     let z = y.clone();
    ///     println!("{:p} {:p}", *y, z); // prints out the same pointer
    /// }
    /// ```
    pub CLONE_DOUBLE_REF,
    correctness,
    "using `clone` on `&&T`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for usage of `.to_string()` on an `&&T` where
    /// `T` implements `ToString` directly (like `&&str` or `&&String`).
    ///
    /// **Why is this bad?** This bypasses the specialized implementation of
    /// `ToString` and instead goes through the more expensive string formatting
    /// facilities.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// // Generic implementation for `T: Display` is used (slow)
    /// ["foo", "bar"].iter().map(|s| s.to_string());
    ///
    /// // OK, the specialized impl is used
    /// ["foo", "bar"].iter().map(|&s| s.to_string());
    /// ```
    pub INEFFICIENT_TO_STRING,
    pedantic,
    "using `to_string` on `&&T` where `T: ToString`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for `new` not returning a type that contains `Self`.
    ///
    /// **Why is this bad?** As a convention, `new` methods are used to make a new
    /// instance of a type.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// In an impl block:
    /// ```rust
    /// # struct Foo;
    /// # struct NotAFoo;
    /// impl Foo {
    ///     fn new() -> NotAFoo {
    /// # NotAFoo
    ///     }
    /// }
    /// ```
    ///
    /// ```rust
    /// # struct Foo;
    /// struct Bar(Foo);
    /// impl Foo {
    ///     // Bad. The type name must contain `Self`
    ///     fn new() -> Bar {
    /// # Bar(Foo)
    ///     }
    /// }
    /// ```
    ///
    /// ```rust
    /// # struct Foo;
    /// # struct FooError;
    /// impl Foo {
    ///     // Good. Return type contains `Self`
    ///     fn new() -> Result<Foo, FooError> {
    /// # Ok(Foo)
    ///     }
    /// }
    /// ```
    ///
    /// Or in a trait definition:
    /// ```rust
    /// pub trait Trait {
    ///     // Bad. The type name must contain `Self`
    ///     fn new();
    /// }
    /// ```
    ///
    /// ```rust
    /// pub trait Trait {
    ///     // Good. Return type contains `Self`
    ///     fn new() -> Self;
    /// }
    /// ```
    pub NEW_RET_NO_SELF,
    style,
    "not returning type containing `Self` in a `new` method"
}

declare_clippy_lint! {
    /// **What it does:** Checks for string methods that receive a single-character
    /// `str` as an argument, e.g., `_.split("x")`.
    ///
    /// **Why is this bad?** Performing these methods using a `char` is faster than
    /// using a `str`.
    ///
    /// **Known problems:** Does not catch multi-byte unicode characters.
    ///
    /// **Example:**
    /// ```rust,ignore
    /// // Bad
    /// _.split("x");
    ///
    /// // Good
    /// _.split('x');
    pub SINGLE_CHAR_PATTERN,
    perf,
    "using a single-character str where a char could be used, e.g., `_.split(\"x\")`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for calling `.step_by(0)` on iterators which panics.
    ///
    /// **Why is this bad?** This very much looks like an oversight. Use `panic!()` instead if you
    /// actually intend to panic.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust,should_panic
    /// for x in (0..100).step_by(0) {
    ///     //..
    /// }
    /// ```
    pub ITERATOR_STEP_BY_ZERO,
    correctness,
    "using `Iterator::step_by(0)`, which will panic at runtime"
}

declare_clippy_lint! {
    /// **What it does:** Checks for the use of `iter.nth(0)`.
    ///
    /// **Why is this bad?** `iter.next()` is equivalent to
    /// `iter.nth(0)`, as they both consume the next element,
    ///  but is more readable.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    ///
    /// ```rust
    /// # use std::collections::HashSet;
    /// // Bad
    /// # let mut s = HashSet::new();
    /// # s.insert(1);
    /// let x = s.iter().nth(0);
    ///
    /// // Good
    /// # let mut s = HashSet::new();
    /// # s.insert(1);
    /// let x = s.iter().next();
    /// ```
    pub ITER_NTH_ZERO,
    style,
    "replace `iter.nth(0)` with `iter.next()`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for use of `.iter().nth()` (and the related
    /// `.iter_mut().nth()`) on standard library types with O(1) element access.
    ///
    /// **Why is this bad?** `.get()` and `.get_mut()` are more efficient and more
    /// readable.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// let some_vec = vec![0, 1, 2, 3];
    /// let bad_vec = some_vec.iter().nth(3);
    /// let bad_slice = &some_vec[..].iter().nth(3);
    /// ```
    /// The correct use would be:
    /// ```rust
    /// let some_vec = vec![0, 1, 2, 3];
    /// let bad_vec = some_vec.get(3);
    /// let bad_slice = &some_vec[..].get(3);
    /// ```
    pub ITER_NTH,
    perf,
    "using `.iter().nth()` on a standard library type with O(1) element access"
}

declare_clippy_lint! {
    /// **What it does:** Checks for use of `.skip(x).next()` on iterators.
    ///
    /// **Why is this bad?** `.nth(x)` is cleaner
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// let some_vec = vec![0, 1, 2, 3];
    /// let bad_vec = some_vec.iter().skip(3).next();
    /// let bad_slice = &some_vec[..].iter().skip(3).next();
    /// ```
    /// The correct use would be:
    /// ```rust
    /// let some_vec = vec![0, 1, 2, 3];
    /// let bad_vec = some_vec.iter().nth(3);
    /// let bad_slice = &some_vec[..].iter().nth(3);
    /// ```
    pub ITER_SKIP_NEXT,
    style,
    "using `.skip(x).next()` on an iterator"
}

declare_clippy_lint! {
    /// **What it does:** Checks for use of `.get().unwrap()` (or
    /// `.get_mut().unwrap`) on a standard library type which implements `Index`
    ///
    /// **Why is this bad?** Using the Index trait (`[]`) is more clear and more
    /// concise.
    ///
    /// **Known problems:** Not a replacement for error handling: Using either
    /// `.unwrap()` or the Index trait (`[]`) carries the risk of causing a `panic`
    /// if the value being accessed is `None`. If the use of `.get().unwrap()` is a
    /// temporary placeholder for dealing with the `Option` type, then this does
    /// not mitigate the need for error handling. If there is a chance that `.get()`
    /// will be `None` in your program, then it is advisable that the `None` case
    /// is handled in a future refactor instead of using `.unwrap()` or the Index
    /// trait.
    ///
    /// **Example:**
    /// ```rust
    /// let mut some_vec = vec![0, 1, 2, 3];
    /// let last = some_vec.get(3).unwrap();
    /// *some_vec.get_mut(0).unwrap() = 1;
    /// ```
    /// The correct use would be:
    /// ```rust
    /// let mut some_vec = vec![0, 1, 2, 3];
    /// let last = some_vec[3];
    /// some_vec[0] = 1;
    /// ```
    pub GET_UNWRAP,
    restriction,
    "using `.get().unwrap()` or `.get_mut().unwrap()` when using `[]` would work instead"
}

declare_clippy_lint! {
    /// **What it does:** Checks for the use of `.extend(s.chars())` where s is a
    /// `&str` or `String`.
    ///
    /// **Why is this bad?** `.push_str(s)` is clearer
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// let abc = "abc";
    /// let def = String::from("def");
    /// let mut s = String::new();
    /// s.extend(abc.chars());
    /// s.extend(def.chars());
    /// ```
    /// The correct use would be:
    /// ```rust
    /// let abc = "abc";
    /// let def = String::from("def");
    /// let mut s = String::new();
    /// s.push_str(abc);
    /// s.push_str(&def);
    /// ```
    pub STRING_EXTEND_CHARS,
    style,
    "using `x.extend(s.chars())` where s is a `&str` or `String`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for the use of `.cloned().collect()` on slice to
    /// create a `Vec`.
    ///
    /// **Why is this bad?** `.to_vec()` is clearer
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// let s = [1, 2, 3, 4, 5];
    /// let s2: Vec<isize> = s[..].iter().cloned().collect();
    /// ```
    /// The better use would be:
    /// ```rust
    /// let s = [1, 2, 3, 4, 5];
    /// let s2: Vec<isize> = s.to_vec();
    /// ```
    pub ITER_CLONED_COLLECT,
    style,
    "using `.cloned().collect()` on slice to create a `Vec`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for usage of `_.chars().last()` or
    /// `_.chars().next_back()` on a `str` to check if it ends with a given char.
    ///
    /// **Why is this bad?** Readability, this can be written more concisely as
    /// `_.ends_with(_)`.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// # let name = "_";
    ///
    /// // Bad
    /// name.chars().last() == Some('_') || name.chars().next_back() == Some('-');
    ///
    /// // Good
    /// name.ends_with('_') || name.ends_with('-');
    /// ```
    pub CHARS_LAST_CMP,
    style,
    "using `.chars().last()` or `.chars().next_back()` to check if a string ends with a char"
}

declare_clippy_lint! {
    /// **What it does:** Checks for usage of `.as_ref()` or `.as_mut()` where the
    /// types before and after the call are the same.
    ///
    /// **Why is this bad?** The call is unnecessary.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// # fn do_stuff(x: &[i32]) {}
    /// let x: &[i32] = &[1, 2, 3, 4, 5];
    /// do_stuff(x.as_ref());
    /// ```
    /// The correct use would be:
    /// ```rust
    /// # fn do_stuff(x: &[i32]) {}
    /// let x: &[i32] = &[1, 2, 3, 4, 5];
    /// do_stuff(x);
    /// ```
    pub USELESS_ASREF,
    complexity,
    "using `as_ref` where the types before and after the call are the same"
}

declare_clippy_lint! {
    /// **What it does:** Checks for using `fold` when a more succinct alternative exists.
    /// Specifically, this checks for `fold`s which could be replaced by `any`, `all`,
    /// `sum` or `product`.
    ///
    /// **Why is this bad?** Readability.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// let _ = (0..3).fold(false, |acc, x| acc || x > 2);
    /// ```
    /// This could be written as:
    /// ```rust
    /// let _ = (0..3).any(|x| x > 2);
    /// ```
    pub UNNECESSARY_FOLD,
    style,
    "using `fold` when a more succinct alternative exists"
}

declare_clippy_lint! {
    /// **What it does:** Checks for `filter_map` calls which could be replaced by `filter` or `map`.
    /// More specifically it checks if the closure provided is only performing one of the
    /// filter or map operations and suggests the appropriate option.
    ///
    /// **Why is this bad?** Complexity. The intent is also clearer if only a single
    /// operation is being performed.
    ///
    /// **Known problems:** None
    ///
    /// **Example:**
    /// ```rust
    /// let _ = (0..3).filter_map(|x| if x > 2 { Some(x) } else { None });
    ///
    /// // As there is no transformation of the argument this could be written as:
    /// let _ = (0..3).filter(|&x| x > 2);
    /// ```
    ///
    /// ```rust
    /// let _ = (0..4).filter_map(|x| Some(x + 1));
    ///
    /// // As there is no conditional check on the argument this could be written as:
    /// let _ = (0..4).map(|x| x + 1);
    /// ```
    pub UNNECESSARY_FILTER_MAP,
    complexity,
    "using `filter_map` when a more succinct alternative exists"
}

declare_clippy_lint! {
    /// **What it does:** Checks for `into_iter` calls on references which should be replaced by `iter`
    /// or `iter_mut`.
    ///
    /// **Why is this bad?** Readability. Calling `into_iter` on a reference will not move out its
    /// content into the resulting iterator, which is confusing. It is better just call `iter` or
    /// `iter_mut` directly.
    ///
    /// **Known problems:** None
    ///
    /// **Example:**
    ///
    /// ```rust
    /// // Bad
    /// let _ = (&vec![3, 4, 5]).into_iter();
    ///
    /// // Good
    /// let _ = (&vec![3, 4, 5]).iter();
    /// ```
    pub INTO_ITER_ON_REF,
    style,
    "using `.into_iter()` on a reference"
}

declare_clippy_lint! {
    /// **What it does:** Checks for calls to `map` followed by a `count`.
    ///
    /// **Why is this bad?** It looks suspicious. Maybe `map` was confused with `filter`.
    /// If the `map` call is intentional, this should be rewritten. Or, if you intend to
    /// drive the iterator to completion, you can just use `for_each` instead.
    ///
    /// **Known problems:** None
    ///
    /// **Example:**
    ///
    /// ```rust
    /// let _ = (0..3).map(|x| x + 2).count();
    /// ```
    pub SUSPICIOUS_MAP,
    complexity,
    "suspicious usage of map"
}

declare_clippy_lint! {
    /// **What it does:** Checks for `MaybeUninit::uninit().assume_init()`.
    ///
    /// **Why is this bad?** For most types, this is undefined behavior.
    ///
    /// **Known problems:** For now, we accept empty tuples and tuples / arrays
    /// of `MaybeUninit`. There may be other types that allow uninitialized
    /// data, but those are not yet rigorously defined.
    ///
    /// **Example:**
    ///
    /// ```rust
    /// // Beware the UB
    /// use std::mem::MaybeUninit;
    ///
    /// let _: usize = unsafe { MaybeUninit::uninit().assume_init() };
    /// ```
    ///
    /// Note that the following is OK:
    ///
    /// ```rust
    /// use std::mem::MaybeUninit;
    ///
    /// let _: [MaybeUninit<bool>; 5] = unsafe {
    ///     MaybeUninit::uninit().assume_init()
    /// };
    /// ```
    pub UNINIT_ASSUMED_INIT,
    correctness,
    "`MaybeUninit::uninit().assume_init()`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for `.checked_add/sub(x).unwrap_or(MAX/MIN)`.
    ///
    /// **Why is this bad?** These can be written simply with `saturating_add/sub` methods.
    ///
    /// **Example:**
    ///
    /// ```rust
    /// # let y: u32 = 0;
    /// # let x: u32 = 100;
    /// let add = x.checked_add(y).unwrap_or(u32::MAX);
    /// let sub = x.checked_sub(y).unwrap_or(u32::MIN);
    /// ```
    ///
    /// can be written using dedicated methods for saturating addition/subtraction as:
    ///
    /// ```rust
    /// # let y: u32 = 0;
    /// # let x: u32 = 100;
    /// let add = x.saturating_add(y);
    /// let sub = x.saturating_sub(y);
    /// ```
    pub MANUAL_SATURATING_ARITHMETIC,
    style,
    "`.chcked_add/sub(x).unwrap_or(MAX/MIN)`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for `offset(_)`, `wrapping_`{`add`, `sub`}, etc. on raw pointers to
    /// zero-sized types
    ///
    /// **Why is this bad?** This is a no-op, and likely unintended
    ///
    /// **Known problems:** None
    ///
    /// **Example:**
    /// ```rust
    /// unsafe { (&() as *const ()).offset(1) };
    /// ```
    pub ZST_OFFSET,
    correctness,
    "Check for offset calculations on raw pointers to zero-sized types"
}

declare_clippy_lint! {
    /// **What it does:** Checks for `FileType::is_file()`.
    ///
    /// **Why is this bad?** When people testing a file type with `FileType::is_file`
    /// they are testing whether a path is something they can get bytes from. But
    /// `is_file` doesn't cover special file types in unix-like systems, and doesn't cover
    /// symlink in windows. Using `!FileType::is_dir()` is a better way to that intention.
    ///
    /// **Example:**
    ///
    /// ```rust
    /// # || {
    /// let metadata = std::fs::metadata("foo.txt")?;
    /// let filetype = metadata.file_type();
    ///
    /// if filetype.is_file() {
    ///     // read file
    /// }
    /// # Ok::<_, std::io::Error>(())
    /// # };
    /// ```
    ///
    /// should be written as:
    ///
    /// ```rust
    /// # || {
    /// let metadata = std::fs::metadata("foo.txt")?;
    /// let filetype = metadata.file_type();
    ///
    /// if !filetype.is_dir() {
    ///     // read file
    /// }
    /// # Ok::<_, std::io::Error>(())
    /// # };
    /// ```
    pub FILETYPE_IS_FILE,
    restriction,
    "`FileType::is_file` is not recommended to test for readable file type"
}

declare_clippy_lint! {
    /// **What it does:** Checks for usage of `_.as_ref().map(Deref::deref)` or it's aliases (such as String::as_str).
    ///
    /// **Why is this bad?** Readability, this can be written more concisely as
    /// `_.as_deref()`.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// # let opt = Some("".to_string());
    /// opt.as_ref().map(String::as_str)
    /// # ;
    /// ```
    /// Can be written as
    /// ```rust
    /// # let opt = Some("".to_string());
    /// opt.as_deref()
    /// # ;
    /// ```
    pub OPTION_AS_REF_DEREF,
    complexity,
    "using `as_ref().map(Deref::deref)`, which is more succinctly expressed as `as_deref()`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for usage of `iter().next()` on a Slice or an Array
    ///
    /// **Why is this bad?** These can be shortened into `.get()`
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// # let a = [1, 2, 3];
    /// # let b = vec![1, 2, 3];
    /// a[2..].iter().next();
    /// b.iter().next();
    /// ```
    /// should be written as:
    /// ```rust
    /// # let a = [1, 2, 3];
    /// # let b = vec![1, 2, 3];
    /// a.get(2);
    /// b.get(0);
    /// ```
    pub ITER_NEXT_SLICE,
    style,
    "using `.iter().next()` on a sliced array, which can be shortened to just `.get()`"
}

declare_clippy_lint! {
    /// **What it does:** Warns when using `push_str`/`insert_str` with a single-character string literal
    /// where `push`/`insert` with a `char` would work fine.
    ///
    /// **Why is this bad?** It's less clear that we are pushing a single character.
    ///
    /// **Known problems:** None
    ///
    /// **Example:**
    /// ```rust
    /// let mut string = String::new();
    /// string.insert_str(0, "R");
    /// string.push_str("R");
    /// ```
    /// Could be written as
    /// ```rust
    /// let mut string = String::new();
    /// string.insert(0, 'R');
    /// string.push('R');
    /// ```
    pub SINGLE_CHAR_ADD_STR,
    style,
    "`push_str()` or `insert_str()` used with a single-character string literal as parameter"
}

declare_clippy_lint! {
    /// **What it does:** As the counterpart to `or_fun_call`, this lint looks for unnecessary
    /// lazily evaluated closures on `Option` and `Result`.
    ///
    /// This lint suggests changing the following functions, when eager evaluation results in
    /// simpler code:
    ///  - `unwrap_or_else` to `unwrap_or`
    ///  - `and_then` to `and`
    ///  - `or_else` to `or`
    ///  - `get_or_insert_with` to `get_or_insert`
    ///  - `ok_or_else` to `ok_or`
    ///
    /// **Why is this bad?** Using eager evaluation is shorter and simpler in some cases.
    ///
    /// **Known problems:** It is possible, but not recommended for `Deref` and `Index` to have
    /// side effects. Eagerly evaluating them can change the semantics of the program.
    ///
    /// **Example:**
    ///
    /// ```rust
    /// // example code where clippy issues a warning
    /// let opt: Option<u32> = None;
    ///
    /// opt.unwrap_or_else(|| 42);
    /// ```
    /// Use instead:
    /// ```rust
    /// let opt: Option<u32> = None;
    ///
    /// opt.unwrap_or(42);
    /// ```
    pub UNNECESSARY_LAZY_EVALUATIONS,
    style,
    "using unnecessary lazy evaluation, which can be replaced with simpler eager evaluation"
}

declare_clippy_lint! {
    /// **What it does:** Checks for usage of `_.map(_).collect::<Result<(), _>()`.
    ///
    /// **Why is this bad?** Using `try_for_each` instead is more readable and idiomatic.
    ///
    /// **Known problems:** None
    ///
    /// **Example:**
    ///
    /// ```rust
    /// (0..3).map(|t| Err(t)).collect::<Result<(), _>>();
    /// ```
    /// Use instead:
    /// ```rust
    /// (0..3).try_for_each(|t| Err(t));
    /// ```
    pub MAP_COLLECT_RESULT_UNIT,
    style,
    "using `.map(_).collect::<Result<(),_>()`, which can be replaced with `try_for_each`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for `from_iter()` function calls on types that implement the `FromIterator`
    /// trait.
    ///
    /// **Why is this bad?** It is recommended style to use collect. See
    /// [FromIterator documentation](https://doc.rust-lang.org/std/iter/trait.FromIterator.html)
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    ///
    /// ```rust
    /// use std::iter::FromIterator;
    ///
    /// let five_fives = std::iter::repeat(5).take(5);
    ///
    /// let v = Vec::from_iter(five_fives);
    ///
    /// assert_eq!(v, vec![5, 5, 5, 5, 5]);
    /// ```
    /// Use instead:
    /// ```rust
    /// let five_fives = std::iter::repeat(5).take(5);
    ///
    /// let v: Vec<i32> = five_fives.collect();
    ///
    /// assert_eq!(v, vec![5, 5, 5, 5, 5]);
    /// ```
    pub FROM_ITER_INSTEAD_OF_COLLECT,
    style,
    "use `.collect()` instead of `::from_iter()`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for usage of `inspect().for_each()`.
    ///
    /// **Why is this bad?** It is the same as performing the computation
    /// inside `inspect` at the beginning of the closure in `for_each`.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    ///
    /// ```rust
    /// [1,2,3,4,5].iter()
    /// .inspect(|&x| println!("inspect the number: {}", x))
    /// .for_each(|&x| {
    ///     assert!(x >= 0);
    /// });
    /// ```
    /// Can be written as
    /// ```rust
    /// [1,2,3,4,5].iter()
    /// .for_each(|&x| {
    ///     println!("inspect the number: {}", x);
    ///     assert!(x >= 0);
    /// });
    /// ```
    pub INSPECT_FOR_EACH,
    complexity,
    "using `.inspect().for_each()`, which can be replaced with `.for_each()`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for usage of `filter_map(|x| x)`.
    ///
    /// **Why is this bad?** Readability, this can be written more concisely by using `flatten`.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    ///
    /// ```rust
    /// # let iter = vec![Some(1)].into_iter();
    /// iter.filter_map(|x| x);
    /// ```
    /// Use instead:
    /// ```rust
    /// # let iter = vec![Some(1)].into_iter();
    /// iter.flatten();
    /// ```
    pub FILTER_MAP_IDENTITY,
    complexity,
    "call to `filter_map` where `flatten` is sufficient"
}

declare_clippy_lint! {
    /// **What it does:** Checks for the use of `.bytes().nth()`.
    ///
    /// **Why is this bad?** `.as_bytes().get()` is more efficient and more
    /// readable.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    ///
    /// ```rust
    /// // Bad
    /// let _ = "Hello".bytes().nth(3);
    ///
    /// // Good
    /// let _ = "Hello".as_bytes().get(3);
    /// ```
    pub BYTES_NTH,
    style,
    "replace `.bytes().nth()` with `.as_bytes().get()`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for the usage of `_.to_owned()`, `vec.to_vec()`, or similar when calling `_.clone()` would be clearer.
    ///
    /// **Why is this bad?** These methods do the same thing as `_.clone()` but may be confusing as
    /// to why we are calling `to_vec` on something that is already a `Vec` or calling `to_owned` on something that is already owned.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    ///
    /// ```rust
    /// let a = vec![1, 2, 3];
    /// let b = a.to_vec();
    /// let c = a.to_owned();
    /// ```
    /// Use instead:
    /// ```rust
    /// let a = vec![1, 2, 3];
    /// let b = a.clone();
    /// let c = a.clone();
    /// ```
    pub IMPLICIT_CLONE,
    pedantic,
    "implicitly cloning a value by invoking a function on its dereferenced type"
}

declare_clippy_lint! {
    /// **What it does:** Checks for the use of `.iter().count()`.
    ///
    /// **Why is this bad?** `.len()` is more efficient and more
    /// readable.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    ///
    /// ```rust
    /// // Bad
    /// let some_vec = vec![0, 1, 2, 3];
    /// let _ = some_vec.iter().count();
    /// let _ = &some_vec[..].iter().count();
    ///
    /// // Good
    /// let some_vec = vec![0, 1, 2, 3];
    /// let _ = some_vec.len();
    /// let _ = &some_vec[..].len();
    /// ```
    pub ITER_COUNT,
    complexity,
    "replace `.iter().count()` with `.len()`"
}

pub struct Methods {
    msrv: Option<RustcVersion>,
}

impl Methods {
    #[must_use]
    pub fn new(msrv: Option<RustcVersion>) -> Self {
        Self { msrv }
    }
}

impl_lint_pass!(Methods => [
    UNWRAP_USED,
    EXPECT_USED,
    SHOULD_IMPLEMENT_TRAIT,
    WRONG_SELF_CONVENTION,
    WRONG_PUB_SELF_CONVENTION,
    OK_EXPECT,
    MAP_UNWRAP_OR,
    RESULT_MAP_OR_INTO_OPTION,
    OPTION_MAP_OR_NONE,
    BIND_INSTEAD_OF_MAP,
    OR_FUN_CALL,
    EXPECT_FUN_CALL,
    CHARS_NEXT_CMP,
    CHARS_LAST_CMP,
    CLONE_ON_COPY,
    CLONE_ON_REF_PTR,
    CLONE_DOUBLE_REF,
    INEFFICIENT_TO_STRING,
    NEW_RET_NO_SELF,
    SINGLE_CHAR_PATTERN,
    SINGLE_CHAR_ADD_STR,
    SEARCH_IS_SOME,
    FILTER_NEXT,
    SKIP_WHILE_NEXT,
    FILTER_MAP,
    FILTER_MAP_IDENTITY,
    MANUAL_FILTER_MAP,
    MANUAL_FIND_MAP,
    FILTER_MAP_NEXT,
    FLAT_MAP_IDENTITY,
    MAP_FLATTEN,
    ITERATOR_STEP_BY_ZERO,
    ITER_NEXT_SLICE,
    ITER_COUNT,
    ITER_NTH,
    ITER_NTH_ZERO,
    BYTES_NTH,
    ITER_SKIP_NEXT,
    GET_UNWRAP,
    STRING_EXTEND_CHARS,
    ITER_CLONED_COLLECT,
    USELESS_ASREF,
    UNNECESSARY_FOLD,
    UNNECESSARY_FILTER_MAP,
    INTO_ITER_ON_REF,
    SUSPICIOUS_MAP,
    UNINIT_ASSUMED_INIT,
    MANUAL_SATURATING_ARITHMETIC,
    ZST_OFFSET,
    FILETYPE_IS_FILE,
    OPTION_AS_REF_DEREF,
    UNNECESSARY_LAZY_EVALUATIONS,
    MAP_COLLECT_RESULT_UNIT,
    FROM_ITER_INSTEAD_OF_COLLECT,
    INSPECT_FOR_EACH,
    IMPLICIT_CLONE
]);

impl<'tcx> LateLintPass<'tcx> for Methods {
    #[allow(clippy::too_many_lines)]
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx hir::Expr<'_>) {
        if in_macro(expr.span) {
            return;
        }

        let (method_names, arg_lists, method_spans) = method_calls(expr, 2);
        let method_names: Vec<SymbolStr> = method_names.iter().map(|s| s.as_str()).collect();
        let method_names: Vec<&str> = method_names.iter().map(|s| &**s).collect();

        match method_names.as_slice() {
            ["unwrap", "get"] => get_unwrap::check(cx, expr, arg_lists[1], false),
            ["unwrap", "get_mut"] => get_unwrap::check(cx, expr, arg_lists[1], true),
            ["unwrap", ..] => unwrap_used::check(cx, expr, arg_lists[0]),
            ["expect", "ok"] => ok_expect::check(cx, expr, arg_lists[1]),
            ["expect", ..] => expect_used::check(cx, expr, arg_lists[0]),
            ["unwrap_or", "map"] => option_map_unwrap_or::check(cx, expr, arg_lists[1], arg_lists[0], method_spans[1]),
            ["unwrap_or_else", "map"] => {
                if !map_unwrap_or::check(cx, expr, arg_lists[1], arg_lists[0], self.msrv.as_ref()) {
                    unnecessary_lazy_eval::check(cx, expr, arg_lists[0], "unwrap_or");
                }
            },
            ["map_or", ..] => option_map_or_none::check(cx, expr, arg_lists[0]),
            ["and_then", ..] => {
                let biom_option_linted = bind_instead_of_map::OptionAndThenSome::check(cx, expr, arg_lists[0]);
                let biom_result_linted = bind_instead_of_map::ResultAndThenOk::check(cx, expr, arg_lists[0]);
                if !biom_option_linted && !biom_result_linted {
                    unnecessary_lazy_eval::check(cx, expr, arg_lists[0], "and");
                }
            },
            ["or_else", ..] => {
                if !bind_instead_of_map::ResultOrElseErrInfo::check(cx, expr, arg_lists[0]) {
                    unnecessary_lazy_eval::check(cx, expr, arg_lists[0], "or");
                }
            },
            ["next", "filter"] => filter_next::check(cx, expr, arg_lists[1]),
            ["next", "skip_while"] => skip_while_next::check(cx, expr, arg_lists[1]),
            ["next", "iter"] => iter_next_slice::check(cx, expr, arg_lists[1]),
            ["map", "filter"] => filter_map::check(cx, expr, false),
            ["map", "filter_map"] => filter_map_map::check(cx, expr, arg_lists[1], arg_lists[0]),
            ["next", "filter_map"] => filter_map_next::check(cx, expr, arg_lists[1], self.msrv.as_ref()),
            ["map", "find"] => filter_map::check(cx, expr, true),
            ["flat_map", "filter"] => filter_flat_map::check(cx, expr, arg_lists[1], arg_lists[0]),
            ["flat_map", "filter_map"] => filter_map_flat_map::check(cx, expr, arg_lists[1], arg_lists[0]),
            ["flat_map", ..] => flat_map_identity::check(cx, expr, arg_lists[0], method_spans[0]),
            ["flatten", "map"] => map_flatten::check(cx, expr, arg_lists[1]),
            ["is_some", "find"] => search_is_some::check(cx, expr, "find", arg_lists[1], arg_lists[0], method_spans[1]),
            ["is_some", "position"] => {
                search_is_some::check(cx, expr, "position", arg_lists[1], arg_lists[0], method_spans[1])
            },
            ["is_some", "rposition"] => {
                search_is_some::check(cx, expr, "rposition", arg_lists[1], arg_lists[0], method_spans[1])
            },
            ["extend", ..] => string_extend_chars::check(cx, expr, arg_lists[0]),
            ["count", "into_iter"] => iter_count::check(cx, expr, &arg_lists[1], "into_iter"),
            ["count", "iter"] => iter_count::check(cx, expr, &arg_lists[1], "iter"),
            ["count", "iter_mut"] => iter_count::check(cx, expr, &arg_lists[1], "iter_mut"),
            ["nth", "iter"] => iter_nth::check(cx, expr, &arg_lists, false),
            ["nth", "iter_mut"] => iter_nth::check(cx, expr, &arg_lists, true),
            ["nth", "bytes"] => bytes_nth::check(cx, expr, &arg_lists[1]),
            ["nth", ..] => iter_nth_zero::check(cx, expr, arg_lists[0]),
            ["step_by", ..] => iterator_step_by_zero::check(cx, expr, arg_lists[0]),
            ["next", "skip"] => iter_skip_next::check(cx, expr, arg_lists[1]),
            ["collect", "cloned"] => iter_cloned_collect::check(cx, expr, arg_lists[1]),
            ["as_ref"] => useless_asref::check(cx, expr, "as_ref", arg_lists[0]),
            ["as_mut"] => useless_asref::check(cx, expr, "as_mut", arg_lists[0]),
            ["fold", ..] => unnecessary_fold::check(cx, expr, arg_lists[0], method_spans[0]),
            ["filter_map", ..] => {
                unnecessary_filter_map::check(cx, expr, arg_lists[0]);
                filter_map_identity::check(cx, expr, arg_lists[0], method_spans[0]);
            },
            ["count", "map"] => suspicious_map::check(cx, expr),
            ["assume_init"] => uninit_assumed_init::check(cx, &arg_lists[0][0], expr),
            ["unwrap_or", arith @ ("checked_add" | "checked_sub" | "checked_mul")] => {
                manual_saturating_arithmetic::check(cx, expr, &arg_lists, &arith["checked_".len()..])
            },
            ["add" | "offset" | "sub" | "wrapping_offset" | "wrapping_add" | "wrapping_sub"] => {
                zst_offset::check(cx, expr, arg_lists[0])
            },
            ["is_file", ..] => filetype_is_file::check(cx, expr, arg_lists[0]),
            ["map", "as_ref"] => {
                option_as_ref_deref::check(cx, expr, arg_lists[1], arg_lists[0], false, self.msrv.as_ref())
            },
            ["map", "as_mut"] => {
                option_as_ref_deref::check(cx, expr, arg_lists[1], arg_lists[0], true, self.msrv.as_ref())
            },
            ["unwrap_or_else", ..] => unnecessary_lazy_eval::check(cx, expr, arg_lists[0], "unwrap_or"),
            ["get_or_insert_with", ..] => unnecessary_lazy_eval::check(cx, expr, arg_lists[0], "get_or_insert"),
            ["ok_or_else", ..] => unnecessary_lazy_eval::check(cx, expr, arg_lists[0], "ok_or"),
            ["collect", "map"] => map_collect_result_unit::check(cx, expr, arg_lists[1], arg_lists[0]),
            ["for_each", "inspect"] => inspect_for_each::check(cx, expr, method_spans[1]),
            ["to_owned", ..] => implicit_clone::check(cx, expr, sym::ToOwned),
            ["to_os_string", ..] => implicit_clone::check(cx, expr, sym::OsStr),
            ["to_path_buf", ..] => implicit_clone::check(cx, expr, sym::Path),
            ["to_vec", ..] => implicit_clone::check(cx, expr, sym::slice),
            _ => {},
        }

        match expr.kind {
            hir::ExprKind::Call(ref func, ref args) => {
                if let hir::ExprKind::Path(path) = &func.kind {
                    if match_qpath(path, &["from_iter"]) {
                        from_iter_instead_of_collect::check(cx, expr, args);
                    }
                }
            },
            hir::ExprKind::MethodCall(ref method_call, ref method_span, ref args, _) => {
                or_fun_call::check(cx, expr, *method_span, &method_call.ident.as_str(), args);
                expect_fun_call::check(cx, expr, *method_span, &method_call.ident.as_str(), args);

                let self_ty = cx.typeck_results().expr_ty_adjusted(&args[0]);
                if args.len() == 1 && method_call.ident.name == sym::clone {
                    clone_on_copy::check(cx, expr, &args[0], self_ty);
                    clone_on_ref_ptr::check(cx, expr, &args[0]);
                }
                if args.len() == 1 && method_call.ident.name == sym!(to_string) {
                    inefficient_to_string::check(cx, expr, &args[0], self_ty);
                }

                if let Some(fn_def_id) = cx.typeck_results().type_dependent_def_id(expr.hir_id) {
                    if match_def_path(cx, fn_def_id, &paths::PUSH_STR) {
                        single_char_push_string::check(cx, expr, args);
                    } else if match_def_path(cx, fn_def_id, &paths::INSERT_STR) {
                        single_char_insert_string::check(cx, expr, args);
                    }
                }

                match self_ty.kind() {
                    ty::Ref(_, ty, _) if *ty.kind() == ty::Str => {
                        for &(method, pos) in &PATTERN_METHODS {
                            if method_call.ident.name.as_str() == method && args.len() > pos {
                                single_char_pattern::check(cx, expr, &args[pos]);
                            }
                        }
                    },
                    ty::Ref(..) if method_call.ident.name == sym::into_iter => {
                        into_iter_on_ref::check(cx, expr, self_ty, *method_span);
                    },
                    _ => (),
                }
            },
            hir::ExprKind::Binary(op, ref lhs, ref rhs)
                if op.node == hir::BinOpKind::Eq || op.node == hir::BinOpKind::Ne =>
            {
                let mut info = BinaryExprInfo {
                    expr,
                    chain: lhs,
                    other: rhs,
                    eq: op.node == hir::BinOpKind::Eq,
                };
                lint_binary_expr_with_method_call(cx, &mut info);
            }
            _ => (),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, impl_item: &'tcx hir::ImplItem<'_>) {
        if in_external_macro(cx.sess(), cx.tcx.hir().span_with_body(impl_item.hir_id())) {
            return;
        }
        let name = impl_item.ident.name.as_str();
        let parent = cx.tcx.hir().get_parent_item(impl_item.hir_id());
        let item = cx.tcx.hir().expect_item(parent);
        let self_ty = cx.tcx.type_of(item.def_id);

        // if this impl block implements a trait, lint in trait definition instead
        if let hir::ItemKind::Impl(hir::Impl { of_trait: Some(_), .. }) = item.kind {
            return;
        }

        if_chain! {
            if let hir::ImplItemKind::Fn(ref sig, id) = impl_item.kind;
            if let Some(first_arg) = iter_input_pats(&sig.decl, cx.tcx.hir().body(id)).next();

            let method_sig = cx.tcx.fn_sig(impl_item.def_id);
            let method_sig = cx.tcx.erase_late_bound_regions(method_sig);

            let first_arg_ty = &method_sig.inputs().iter().next();

            // check conventions w.r.t. conversion method names and predicates
            if let Some(first_arg_ty) = first_arg_ty;

            then {
                if cx.access_levels.is_exported(impl_item.hir_id()) {
                    // check missing trait implementations
                    for method_config in &TRAIT_METHODS {
                        if name == method_config.method_name &&
                            sig.decl.inputs.len() == method_config.param_count &&
                            method_config.output_type.matches(cx, &sig.decl.output) &&
                            method_config.self_kind.matches(cx, self_ty, first_arg_ty) &&
                            fn_header_equals(method_config.fn_header, sig.header) &&
                            method_config.lifetime_param_cond(&impl_item)
                        {
                            let impl_item_span = cx.tcx.hir().span_with_body(impl_item.hir_id());
                            span_lint_and_help(
                                cx,
                                SHOULD_IMPLEMENT_TRAIT,
                                impl_item_span,
                                &format!(
                                    "method `{}` can be confused for the standard trait method `{}::{}`",
                                    method_config.method_name,
                                    method_config.trait_name,
                                    method_config.method_name
                                ),
                                None,
                                &format!(
                                    "consider implementing the trait `{}` or choosing a less ambiguous method name",
                                    method_config.trait_name
                                )
                            );
                        }
                    }
                }

                wrong_self_convention::check(
                    cx,
                    &name,
                    item.vis.node.is_pub(),
                    self_ty,
                    first_arg_ty,
                    cx.tcx.hir().span(first_arg.pat.hir_id)
                );
            }
        }

        if let hir::ImplItemKind::Fn(_, _) = impl_item.kind {
            let ret_ty = return_ty(cx, impl_item.hir_id());

            // walk the return type and check for Self (this does not check associated types)
            if contains_ty(ret_ty, self_ty) {
                return;
            }

            // if return type is impl trait, check the associated types
            if let ty::Opaque(def_id, _) = *ret_ty.kind() {
                // one of the associated types must be Self
                for &(predicate, _span) in cx.tcx.explicit_item_bounds(def_id) {
                    if let ty::PredicateKind::Projection(projection_predicate) = predicate.kind().skip_binder() {
                        // walk the associated type and check for Self
                        if contains_ty(projection_predicate.ty, self_ty) {
                            return;
                        }
                    }
                }
            }

            if name == "new" && !TyS::same_type(ret_ty, self_ty) {
                span_lint(
                    cx,
                    NEW_RET_NO_SELF,
                    cx.tcx.hir().span_with_body(impl_item.hir_id()),
                    "methods called `new` usually return `Self`",
                );
            }
        }
    }

    fn check_trait_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx TraitItem<'_>) {
        let item_span = cx.tcx.hir().span_with_body(item.hir_id());
        if in_external_macro(cx.tcx.sess, item_span) {
            return;
        }

        if_chain! {
            if let TraitItemKind::Fn(ref sig, _) = item.kind;
            if let Some(first_arg_ty) = sig.decl.inputs.iter().next();
            let first_arg_span = cx.tcx.hir().span(first_arg_ty.hir_id);
            let first_arg_ty = hir_ty_to_ty(cx.tcx, first_arg_ty);
            let self_ty = TraitRef::identity(cx.tcx, item.def_id.to_def_id()).self_ty();

            then {
                wrong_self_convention::check(
                    cx,
                    &item.ident.name.as_str(),
                    false,
                    self_ty,
                    first_arg_ty,
                    first_arg_span
                );
            }
        }

        if_chain! {
            if item.ident.name == sym::new;
            if let TraitItemKind::Fn(_, _) = item.kind;
            let ret_ty = return_ty(cx, item.hir_id());
            let self_ty = TraitRef::identity(cx.tcx, item.def_id.to_def_id()).self_ty();
            if !contains_ty(ret_ty, self_ty);

            then {
                span_lint(
                    cx,
                    NEW_RET_NO_SELF,
                    item_span,
                    "methods called `new` usually return `Self`",
                );
            }
        }
    }

    extract_msrv_attr!(LateContext);
}

fn derefs_to_slice<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx hir::Expr<'tcx>,
    ty: Ty<'tcx>,
) -> Option<&'tcx hir::Expr<'tcx>> {
    fn may_slice<'a>(cx: &LateContext<'a>, ty: Ty<'a>) -> bool {
        match ty.kind() {
            ty::Slice(_) => true,
            ty::Adt(def, _) if def.is_box() => may_slice(cx, ty.boxed_ty()),
            ty::Adt(..) => is_type_diagnostic_item(cx, ty, sym::vec_type),
            ty::Array(_, size) => size
                .try_eval_usize(cx.tcx, cx.param_env)
                .map_or(false, |size| size < 32),
            ty::Ref(_, inner, _) => may_slice(cx, inner),
            _ => false,
        }
    }

    if let hir::ExprKind::MethodCall(ref path, _, ref args, _) = expr.kind {
        if path.ident.name == sym::iter && may_slice(cx, cx.typeck_results().expr_ty(&args[0])) {
            Some(&args[0])
        } else {
            None
        }
    } else {
        match ty.kind() {
            ty::Slice(_) => Some(expr),
            ty::Adt(def, _) if def.is_box() && may_slice(cx, ty.boxed_ty()) => Some(expr),
            ty::Ref(_, inner, _) => {
                if may_slice(cx, inner) {
                    Some(expr)
                } else {
                    None
                }
            },
            _ => None,
        }
    }
}

/// Used for `lint_binary_expr_with_method_call`.
#[derive(Copy, Clone)]
struct BinaryExprInfo<'a> {
    expr: &'a hir::Expr<'a>,
    chain: &'a hir::Expr<'a>,
    other: &'a hir::Expr<'a>,
    eq: bool,
}

/// Checks for the `CHARS_NEXT_CMP` and `CHARS_LAST_CMP` lints.
fn lint_binary_expr_with_method_call(cx: &LateContext<'_>, info: &mut BinaryExprInfo<'_>) {
    macro_rules! lint_with_both_lhs_and_rhs {
        ($func:ident, $cx:expr, $info:ident) => {
            if !$func($cx, $info) {
                ::std::mem::swap(&mut $info.chain, &mut $info.other);
                if $func($cx, $info) {
                    return;
                }
            }
        };
    }

    lint_with_both_lhs_and_rhs!(lint_chars_next_cmp, cx, info);
    lint_with_both_lhs_and_rhs!(lint_chars_last_cmp, cx, info);
    lint_with_both_lhs_and_rhs!(lint_chars_next_cmp_with_unwrap, cx, info);
    lint_with_both_lhs_and_rhs!(lint_chars_last_cmp_with_unwrap, cx, info);
}

/// Wrapper fn for `CHARS_NEXT_CMP` and `CHARS_LAST_CMP` lints.
fn lint_chars_cmp(
    cx: &LateContext<'_>,
    info: &BinaryExprInfo<'_>,
    chain_methods: &[&str],
    lint: &'static Lint,
    suggest: &str,
) -> bool {
    if_chain! {
        if let Some(args) = method_chain_args(info.chain, chain_methods);
        if let hir::ExprKind::Call(ref fun, ref arg_char) = info.other.kind;
        if arg_char.len() == 1;
        if let hir::ExprKind::Path(ref qpath) = fun.kind;
        if let Some(segment) = single_segment_path(qpath);
        if segment.ident.name == sym::Some;
        then {
            let mut applicability = Applicability::MachineApplicable;
            let self_ty = cx.typeck_results().expr_ty_adjusted(&args[0][0]).peel_refs();

            if *self_ty.kind() != ty::Str {
                return false;
            }

            span_lint_and_sugg(
                cx,
                lint,
                info.expr.span,
                &format!("you should use the `{}` method", suggest),
                "like this",
                format!("{}{}.{}({})",
                        if info.eq { "" } else { "!" },
                        snippet_with_applicability(cx, args[0][0].span, "..", &mut applicability),
                        suggest,
                        snippet_with_applicability(cx, arg_char[0].span, "..", &mut applicability)),
                applicability,
            );

            return true;
        }
    }

    false
}

/// Checks for the `CHARS_NEXT_CMP` lint.
fn lint_chars_next_cmp<'tcx>(cx: &LateContext<'tcx>, info: &BinaryExprInfo<'_>) -> bool {
    lint_chars_cmp(cx, info, &["chars", "next"], CHARS_NEXT_CMP, "starts_with")
}

/// Checks for the `CHARS_LAST_CMP` lint.
fn lint_chars_last_cmp<'tcx>(cx: &LateContext<'tcx>, info: &BinaryExprInfo<'_>) -> bool {
    if lint_chars_cmp(cx, info, &["chars", "last"], CHARS_LAST_CMP, "ends_with") {
        true
    } else {
        lint_chars_cmp(cx, info, &["chars", "next_back"], CHARS_LAST_CMP, "ends_with")
    }
}

/// Wrapper fn for `CHARS_NEXT_CMP` and `CHARS_LAST_CMP` lints with `unwrap()`.
fn lint_chars_cmp_with_unwrap<'tcx>(
    cx: &LateContext<'tcx>,
    info: &BinaryExprInfo<'_>,
    chain_methods: &[&str],
    lint: &'static Lint,
    suggest: &str,
) -> bool {
    if_chain! {
        if let Some(args) = method_chain_args(info.chain, chain_methods);
        if let hir::ExprKind::Lit(ref lit) = info.other.kind;
        if let ast::LitKind::Char(c) = lit.node;
        then {
            let mut applicability = Applicability::MachineApplicable;
            span_lint_and_sugg(
                cx,
                lint,
                info.expr.span,
                &format!("you should use the `{}` method", suggest),
                "like this",
                format!("{}{}.{}('{}')",
                        if info.eq { "" } else { "!" },
                        snippet_with_applicability(cx, args[0][0].span, "..", &mut applicability),
                        suggest,
                        c),
                applicability,
            );

            true
        } else {
            false
        }
    }
}

/// Checks for the `CHARS_NEXT_CMP` lint with `unwrap()`.
fn lint_chars_next_cmp_with_unwrap<'tcx>(cx: &LateContext<'tcx>, info: &BinaryExprInfo<'_>) -> bool {
    lint_chars_cmp_with_unwrap(cx, info, &["chars", "next", "unwrap"], CHARS_NEXT_CMP, "starts_with")
}

/// Checks for the `CHARS_LAST_CMP` lint with `unwrap()`.
fn lint_chars_last_cmp_with_unwrap<'tcx>(cx: &LateContext<'tcx>, info: &BinaryExprInfo<'_>) -> bool {
    if lint_chars_cmp_with_unwrap(cx, info, &["chars", "last", "unwrap"], CHARS_LAST_CMP, "ends_with") {
        true
    } else {
        lint_chars_cmp_with_unwrap(cx, info, &["chars", "next_back", "unwrap"], CHARS_LAST_CMP, "ends_with")
    }
}

fn get_hint_if_single_char_arg(
    cx: &LateContext<'_>,
    arg: &hir::Expr<'_>,
    applicability: &mut Applicability,
) -> Option<String> {
    if_chain! {
        if let hir::ExprKind::Lit(lit) = &arg.kind;
        if let ast::LitKind::Str(r, style) = lit.node;
        let string = r.as_str();
        if string.chars().count() == 1;
        then {
            let snip = snippet_with_applicability(cx, arg.span, &string, applicability);
            let ch = if let ast::StrStyle::Raw(nhash) = style {
                let nhash = nhash as usize;
                // for raw string: r##"a"##
                &snip[(nhash + 2)..(snip.len() - 1 - nhash)]
            } else {
                // for regular string: "a"
                &snip[1..(snip.len() - 1)]
            };
            let hint = format!("'{}'", if ch == "'" { "\\'" } else { ch });
            Some(hint)
        } else {
            None
        }
    }
}

const FN_HEADER: hir::FnHeader = hir::FnHeader {
    unsafety: hir::Unsafety::Normal,
    constness: hir::Constness::NotConst,
    asyncness: hir::IsAsync::NotAsync,
    abi: rustc_target::spec::abi::Abi::Rust,
};

struct ShouldImplTraitCase {
    trait_name: &'static str,
    method_name: &'static str,
    param_count: usize,
    fn_header: hir::FnHeader,
    // implicit self kind expected (none, self, &self, ...)
    self_kind: SelfKind,
    // checks against the output type
    output_type: OutType,
    // certain methods with explicit lifetimes can't implement the equivalent trait method
    lint_explicit_lifetime: bool,
}
impl ShouldImplTraitCase {
    const fn new(
        trait_name: &'static str,
        method_name: &'static str,
        param_count: usize,
        fn_header: hir::FnHeader,
        self_kind: SelfKind,
        output_type: OutType,
        lint_explicit_lifetime: bool,
    ) -> ShouldImplTraitCase {
        ShouldImplTraitCase {
            trait_name,
            method_name,
            param_count,
            fn_header,
            self_kind,
            output_type,
            lint_explicit_lifetime,
        }
    }

    fn lifetime_param_cond(&self, impl_item: &hir::ImplItem<'_>) -> bool {
        self.lint_explicit_lifetime
            || !impl_item.generics.params.iter().any(|p| {
                matches!(
                    p.kind,
                    hir::GenericParamKind::Lifetime {
                        kind: hir::LifetimeParamKind::Explicit
                    }
                )
            })
    }
}

#[rustfmt::skip]
const TRAIT_METHODS: [ShouldImplTraitCase; 30] = [
    ShouldImplTraitCase::new("std::ops::Add", "add",  2,  FN_HEADER,  SelfKind::Value,  OutType::Any, true),
    ShouldImplTraitCase::new("std::convert::AsMut", "as_mut",  1,  FN_HEADER,  SelfKind::RefMut,  OutType::Ref, true),
    ShouldImplTraitCase::new("std::convert::AsRef", "as_ref",  1,  FN_HEADER,  SelfKind::Ref,  OutType::Ref, true),
    ShouldImplTraitCase::new("std::ops::BitAnd", "bitand",  2,  FN_HEADER,  SelfKind::Value,  OutType::Any, true),
    ShouldImplTraitCase::new("std::ops::BitOr", "bitor",  2,  FN_HEADER,  SelfKind::Value,  OutType::Any, true),
    ShouldImplTraitCase::new("std::ops::BitXor", "bitxor",  2,  FN_HEADER,  SelfKind::Value,  OutType::Any, true),
    ShouldImplTraitCase::new("std::borrow::Borrow", "borrow",  1,  FN_HEADER,  SelfKind::Ref,  OutType::Ref, true),
    ShouldImplTraitCase::new("std::borrow::BorrowMut", "borrow_mut",  1,  FN_HEADER,  SelfKind::RefMut,  OutType::Ref, true),
    ShouldImplTraitCase::new("std::clone::Clone", "clone",  1,  FN_HEADER,  SelfKind::Ref,  OutType::Any, true),
    ShouldImplTraitCase::new("std::cmp::Ord", "cmp",  2,  FN_HEADER,  SelfKind::Ref,  OutType::Any, true),
    // FIXME: default doesn't work
    ShouldImplTraitCase::new("std::default::Default", "default",  0,  FN_HEADER,  SelfKind::No,  OutType::Any, true),
    ShouldImplTraitCase::new("std::ops::Deref", "deref",  1,  FN_HEADER,  SelfKind::Ref,  OutType::Ref, true),
    ShouldImplTraitCase::new("std::ops::DerefMut", "deref_mut",  1,  FN_HEADER,  SelfKind::RefMut,  OutType::Ref, true),
    ShouldImplTraitCase::new("std::ops::Div", "div",  2,  FN_HEADER,  SelfKind::Value,  OutType::Any, true),
    ShouldImplTraitCase::new("std::ops::Drop", "drop",  1,  FN_HEADER,  SelfKind::RefMut,  OutType::Unit, true),
    ShouldImplTraitCase::new("std::cmp::PartialEq", "eq",  2,  FN_HEADER,  SelfKind::Ref,  OutType::Bool, true),
    ShouldImplTraitCase::new("std::iter::FromIterator", "from_iter",  1,  FN_HEADER,  SelfKind::No,  OutType::Any, true),
    ShouldImplTraitCase::new("std::str::FromStr", "from_str",  1,  FN_HEADER,  SelfKind::No,  OutType::Any, true),
    ShouldImplTraitCase::new("std::hash::Hash", "hash",  2,  FN_HEADER,  SelfKind::Ref,  OutType::Unit, true),
    ShouldImplTraitCase::new("std::ops::Index", "index",  2,  FN_HEADER,  SelfKind::Ref,  OutType::Ref, true),
    ShouldImplTraitCase::new("std::ops::IndexMut", "index_mut",  2,  FN_HEADER,  SelfKind::RefMut,  OutType::Ref, true),
    ShouldImplTraitCase::new("std::iter::IntoIterator", "into_iter",  1,  FN_HEADER,  SelfKind::Value,  OutType::Any, true),
    ShouldImplTraitCase::new("std::ops::Mul", "mul",  2,  FN_HEADER,  SelfKind::Value,  OutType::Any, true),
    ShouldImplTraitCase::new("std::ops::Neg", "neg",  1,  FN_HEADER,  SelfKind::Value,  OutType::Any, true),
    ShouldImplTraitCase::new("std::iter::Iterator", "next",  1,  FN_HEADER,  SelfKind::RefMut,  OutType::Any, false),
    ShouldImplTraitCase::new("std::ops::Not", "not",  1,  FN_HEADER,  SelfKind::Value,  OutType::Any, true),
    ShouldImplTraitCase::new("std::ops::Rem", "rem",  2,  FN_HEADER,  SelfKind::Value,  OutType::Any, true),
    ShouldImplTraitCase::new("std::ops::Shl", "shl",  2,  FN_HEADER,  SelfKind::Value,  OutType::Any, true),
    ShouldImplTraitCase::new("std::ops::Shr", "shr",  2,  FN_HEADER,  SelfKind::Value,  OutType::Any, true),
    ShouldImplTraitCase::new("std::ops::Sub", "sub",  2,  FN_HEADER,  SelfKind::Value,  OutType::Any, true),
];

#[rustfmt::skip]
const PATTERN_METHODS: [(&str, usize); 17] = [
    ("contains", 1),
    ("starts_with", 1),
    ("ends_with", 1),
    ("find", 1),
    ("rfind", 1),
    ("split", 1),
    ("rsplit", 1),
    ("split_terminator", 1),
    ("rsplit_terminator", 1),
    ("splitn", 2),
    ("rsplitn", 2),
    ("matches", 1),
    ("rmatches", 1),
    ("match_indices", 1),
    ("rmatch_indices", 1),
    ("trim_start_matches", 1),
    ("trim_end_matches", 1),
];

#[derive(Clone, Copy, PartialEq, Debug)]
enum SelfKind {
    Value,
    Ref,
    RefMut,
    No,
}

impl SelfKind {
    fn matches<'a>(self, cx: &LateContext<'a>, parent_ty: Ty<'a>, ty: Ty<'a>) -> bool {
        fn matches_value<'a>(cx: &LateContext<'a>, parent_ty: Ty<'_>, ty: Ty<'_>) -> bool {
            if ty == parent_ty {
                true
            } else if ty.is_box() {
                ty.boxed_ty() == parent_ty
            } else if is_type_diagnostic_item(cx, ty, sym::Rc) || is_type_diagnostic_item(cx, ty, sym::Arc) {
                if let ty::Adt(_, substs) = ty.kind() {
                    substs.types().next().map_or(false, |t| t == parent_ty)
                } else {
                    false
                }
            } else {
                false
            }
        }

        fn matches_ref<'a>(cx: &LateContext<'a>, mutability: hir::Mutability, parent_ty: Ty<'a>, ty: Ty<'a>) -> bool {
            if let ty::Ref(_, t, m) = *ty.kind() {
                return m == mutability && t == parent_ty;
            }

            let trait_path = match mutability {
                hir::Mutability::Not => &paths::ASREF_TRAIT,
                hir::Mutability::Mut => &paths::ASMUT_TRAIT,
            };

            let trait_def_id = match get_trait_def_id(cx, trait_path) {
                Some(did) => did,
                None => return false,
            };
            implements_trait(cx, ty, trait_def_id, &[parent_ty.into()])
        }

        match self {
            Self::Value => matches_value(cx, parent_ty, ty),
            Self::Ref => matches_ref(cx, hir::Mutability::Not, parent_ty, ty) || ty == parent_ty && is_copy(cx, ty),
            Self::RefMut => matches_ref(cx, hir::Mutability::Mut, parent_ty, ty),
            Self::No => ty != parent_ty,
        }
    }

    #[must_use]
    fn description(self) -> &'static str {
        match self {
            Self::Value => "self by value",
            Self::Ref => "self by reference",
            Self::RefMut => "self by mutable reference",
            Self::No => "no self",
        }
    }
}

#[derive(Clone, Copy)]
enum OutType {
    Unit,
    Bool,
    Any,
    Ref,
}

impl OutType {
    fn matches(self, cx: &LateContext<'_>, ty: &hir::FnRetTy<'_>) -> bool {
        let is_unit = |ty: &hir::Ty<'_>| SpanlessEq::new(cx).eq_ty_kind(&ty.kind, &hir::TyKind::Tup(&[]));
        match (self, ty) {
            (Self::Unit, &hir::FnRetTy::DefaultReturn(_)) => true,
            (Self::Unit, &hir::FnRetTy::Return(ref ty)) if is_unit(ty) => true,
            (Self::Bool, &hir::FnRetTy::Return(ref ty)) if is_bool(ty) => true,
            (Self::Any, &hir::FnRetTy::Return(ref ty)) if !is_unit(ty) => true,
            (Self::Ref, &hir::FnRetTy::Return(ref ty)) => matches!(ty.kind, hir::TyKind::Rptr(_, _)),
            _ => false,
        }
    }
}

fn is_bool(ty: &hir::Ty<'_>) -> bool {
    if let hir::TyKind::Path(ref p) = ty.kind {
        match_qpath(p, &["bool"])
    } else {
        false
    }
}

fn fn_header_equals(expected: hir::FnHeader, actual: hir::FnHeader) -> bool {
    expected.constness == actual.constness
        && expected.unsafety == actual.unsafety
        && expected.asyncness == actual.asyncness
}
