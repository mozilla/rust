﻿// (This line has BOM so it's ignored by compiletest for directives)
//
// build-fail
// compile-flags: --json=diagnostic-short --error-format=json
// ignore-tidy-cr

#[path = "json-bom-plus-crlf-multifile-aux.rs"]
mod json_bom_plus_crlf_multifile_aux;

fn main() {
    json_bom_plus_crlf_multifile_aux::test();
}
