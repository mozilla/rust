pub mod module {
    pub mod sub_module {
        pub mod sub_sub_module {
            pub fn foo() {}
        }
        pub fn bar() {}
    }
    pub fn whatever() {}
}

pub fn foobar() {}

pub type Alias = u32;

pub struct Foo {
    pub x: Alias,
}

impl Foo {
    pub fn a_method(&self) {}
}

// This is used to ensure the line numbers are correctly set.
/// ```
/// # fn main() {
/// let x = 12;
/// let y = 13;
/// # let z = 14;
///
/// println!("hello");
/// # }
/// ```
pub trait Trait {
    type X;
    const Y: u32;

    // This is used to ensure the line numbers are correctly set.
    /// ```
    /// fn main() {
    /// let x = 12;
    /// let y = 13;
    /// let z = 14;
    ///
    /// println!("hello");
    /// }
    /// ```
    fn foo() {}
}

impl Trait for Foo {
    type X = u32;
    const Y: u32 = 0;
}

impl implementors::Whatever for Foo {}
