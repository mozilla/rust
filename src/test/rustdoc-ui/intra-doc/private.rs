// check-pass
// revisions: public private
// [private]compile-flags: --document-private-items

/// docs [DontDocMe] [DontDocMe::f]
//~^ WARNING public documentation for `DocMe` links to private item `DontDocMe`
//~| WARNING public documentation for `DocMe` links to private item `DontDocMe::f`
// FIXME: for [private] we should also make sure the link was actually generated
pub struct DocMe;
struct DontDocMe;

impl DontDocMe {
    fn f() {}
}
