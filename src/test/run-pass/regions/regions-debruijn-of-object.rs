// run-pass
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(non_camel_case_types)]

// pretty-expanded FIXME(#23616):

struct ctxt<'tcx> {
    x: &'tcx i32
}

trait AstConv<'tcx> {
    fn tcx<'a>(&'a self) -> &'a ctxt<'tcx>;
}

fn foo(conv: &AstConv) { }

fn bar<'tcx>(conv: &AstConv<'tcx>) {
    foo(conv)
}

fn main() { }
