// compile-pass

       //! Test with [Foo::baz], [Bar::foo], ...
     //! , [Uniooon::X] and [Qux::Z].
       //!
      //! , [Uniooon::X] and [Qux::Z].

       /// [Qux:Y]
pub struct Foo {
    pub bar: usize,
}

/// Foo
/// bar [BarA] bar
/// baz
pub fn a() {}

/**
 * Foo
 * bar [BarB] bar
 * baz
 */
pub fn b() {}

/** Foo

bar [BarC] bar
baz

    let bar_c_1 = 0;
    let bar_c_2 = 0;
    let g = [bar_c_1];
    let h = g[bar_c_2];

*/
pub fn c() {}

#[doc = "Foo\nbar [BarD] bar\nbaz"]
pub fn d() {}

macro_rules! f {
    ($f:expr) => {
        #[doc = $f]
        pub fn f() {}
    }
}
f!("Foo\nbar [BarF] bar\nbaz");
