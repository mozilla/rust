// normalize-stderr-test: "existed:.*\(" -> "existed: $$FILE_NOT_FOUND_MSG ("

// test that errors in a (selection) of macros don't kill compilation
// immediately, so that we get more errors listed at a time.

#![feature(asm, llvm_asm)]
#![feature(trace_macros, concat_idents)]
#![feature(derive_default_enum)]

#[derive(Default)] //~ ERROR no default declared
enum NoDeclaredDefault {
    Foo,
    Bar,
}

#[derive(Default)] //~ ERROR multiple declared defaults
enum MultipleDefaults {
    #[default]
    Foo,
    #[default]
    Bar,
    #[default]
    Baz,
}

#[derive(Default)]
enum ExtraDeriveTokens {
    // FIXME(jhpratt) only one of these errors will be present in the final PR
    #[default = 1] //~ ERROR malformed `default` attribute input
    //~^ ERROR `#[default]` attribute does not accept a value
    Foo,
}

#[derive(Default)]
enum TwoDefaultAttrs {
    #[default]
    #[default]
    Foo, //~ERROR multiple `#[default]` attributes
    Bar,
}

#[derive(Default)]
enum ManyDefaultAttrs {
    #[default]
    #[default]
    #[default]
    #[default]
    Foo, //~ERROR multiple `#[default]` attributes
    Bar,
}

#[derive(Default)]
enum DefaultHasFields {
    #[default]
    Foo {}, //~ ERROR `#[default]` may only be used on unit variants
    Bar,
}

#[derive(Default)]
enum NonExhaustiveDefault {
    #[default]
    #[non_exhaustive]
    Foo, //~ ERROR default variant must be exhaustive
    Bar,
}

fn main() {
    asm!(invalid); //~ ERROR
    llvm_asm!(invalid); //~ ERROR

    concat_idents!("not", "idents"); //~ ERROR

    option_env!(invalid); //~ ERROR
    env!(invalid); //~ ERROR
    env!(foo, abr, baz); //~ ERROR
    env!("RUST_HOPEFULLY_THIS_DOESNT_EXIST"); //~ ERROR

    format!(invalid); //~ ERROR

    include!(invalid); //~ ERROR

    include_str!(invalid); //~ ERROR
    include_str!("i'd be quite surprised if a file with this name existed"); //~ ERROR
    include_bytes!(invalid); //~ ERROR
    include_bytes!("i'd be quite surprised if a file with this name existed"); //~ ERROR

    trace_macros!(invalid); //~ ERROR
}
