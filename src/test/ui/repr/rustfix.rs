// run-rustfix
#[repr(align = r#"1"#)] //~ ERROR incorrect `repr(align)` attribute format
                        //~| ERROR incorrect `repr(align)` attribute format
struct A;

#[repr(align = r###"foo"###)] //~ ERROR incorrect `repr(align)` attribute format
                              //~| ERROR incorrect `repr(align)` attribute format
struct B;

#[repr(align = 1)] //~ ERROR incorrect `repr(align)` attribute format
                   //~| ERROR incorrect `repr(align)` attribute format
struct C;

#[repr(C(1, "", true))] //~ ERROR invalid `repr(C)` attribute: no arguments expected
                        //~| ERROR invalid `repr(C)` attribute: no arguments expected
struct D;

#[repr(C())] //~ ERROR invalid `repr(C)` attribute: no arguments expected
             //~| ERROR invalid `repr(C)` attribute: no arguments expected
struct E;

#[repr(align("1"))] //~ ERROR invalid `repr(align)` attribute
                    //~| ERROR invalid `repr(align)` attribute
struct F;

#[repr(align(r############"1"############))] //~ ERROR invalid `repr(align)` attribute
                                             //~| ERROR invalid `repr(align)` attribute
struct G;

fn main() {}
