#![feature(specialization)]
#![feature(optin_builtin_traits)]

// Negative impl for u32 cannot "specialize" the base impl.
trait MyTrait { }
impl<T> MyTrait for T { }
impl !MyTrait for u32 { } //~ ERROR conflicting implementations

// The second impl specializes the first, no error.
trait MyTrait2 { }
impl<T> MyTrait2 for T { }
impl MyTrait2 for u32 { }

fn main() { }
