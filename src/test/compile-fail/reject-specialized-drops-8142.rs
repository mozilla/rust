// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Issue 8142: Test that Drop impls cannot be specialized beyond the
// predicates attached to the struct/enum definition itself.

trait Bound { fn foo(&self) { } }
struct K<'l1,'l2> { x: &'l1 i8, y: &'l2 u8 }
//~^ NOTE The same requirement must be part of the struct/enum definition
struct L<'l1,'l2> { x: &'l1 i8, y: &'l2 u8 }
//~^ NOTE The same requirement must be part of the struct/enum definition
struct M<'m> { x: &'m i8 }
struct N<'n> { x: &'n i8 }
//~^ NOTE the lifetime 'n as defined on the struct
struct O<To> { x: *const To }
struct P<Tp> { x: *const Tp }
//~^ NOTE Use same sequence of generic type and region parameters that is on the struct/enum
struct Q<Tq> { x: *const Tq }
//~^ NOTE The same requirement must be part of the struct/enum definition
struct R<Tr> { x: *const Tr }
//~^ NOTE The same requirement must be part of the struct/enum definition
struct S<Ts:Bound> { x: *const Ts }
struct T<'t,Ts:'t> { x: &'t Ts }
struct U;
struct V<Tva, Tvb> { x: *const Tva, y: *const Tvb }
//~^ NOTE Use same sequence of generic type and region parameters that is on the struct/enum
struct W<'l1, 'l2> { x: &'l1 i8, y: &'l2 u8 }
//~^ NOTE the lifetime cannot outlive the lifetime 'l2 as defined on the struct
//~| NOTE the lifetime must be valid for the lifetime 'l1 as defined on the struct

impl<'al,'adds_bnd:'al> Drop for K<'al,'adds_bnd> {                        // REJECT
    //~^ ERROR The requirement `'adds_bnd : 'al` is added only by the Drop impl.
    fn drop(&mut self) { } }

impl<'al,'adds_bnd>     Drop for L<'al,'adds_bnd> where 'adds_bnd:'al {    // REJECT
    //~^ ERROR The requirement `'adds_bnd : 'al` is added only by the Drop impl.
    fn drop(&mut self) { } }

impl<'ml>               Drop for M<'ml>         { fn drop(&mut self) { } } // ACCEPT

impl                    Drop for N<'static>     { fn drop(&mut self) { } } // REJECT
//~^ ERROR mismatched types
//~| expected type `N<'n>`
//~|    found type `N<'static>`
//~| NOTE lifetime mismatch
//~| NOTE ..does not necessarily outlive the static lifetime

impl<Cok_nobound> Drop for O<Cok_nobound> { fn drop(&mut self) { } } // ACCEPT

impl              Drop for P<i8>          { fn drop(&mut self) { } } // REJECT
//~^ ERROR Implementations of Drop cannot be specialized

impl<Adds_bnd:Bound> Drop for Q<Adds_bnd> { fn drop(&mut self) { } } // REJECT
//~^ ERROR The requirement `Adds_bnd: Bound` is added only by the Drop impl.

impl<'rbnd,Adds_rbnd:'rbnd> Drop for R<Adds_rbnd> { fn drop(&mut self) { } } // REJECT
//~^ ERROR The requirement `Adds_rbnd : 'rbnd` is added only by the Drop impl.

impl<Bs:Bound>    Drop for S<Bs>          { fn drop(&mut self) { } } // ACCEPT

impl<'t,Bt:'t>    Drop for T<'t,Bt>       { fn drop(&mut self) { } } // ACCEPT

impl              Drop for U              { fn drop(&mut self) { } } // ACCEPT

impl<One>         Drop for V<One,One>     { fn drop(&mut self) { } } // REJECT
//~^ ERROR Implementations of Drop cannot be specialized

impl<'lw>         Drop for W<'lw,'lw>     { fn drop(&mut self) { } } // REJECT
//~^ ERROR cannot infer an appropriate lifetime for lifetime parameter `'lw` due to conflicting
//~| ERROR cannot infer an appropriate lifetime for lifetime parameter `'lw` due to conflicting
//~| ERROR cannot infer an appropriate lifetime for lifetime parameter `'lw` due to conflicting
//~| NOTE cannot infer an appropriate lifetime
//~| NOTE ...so that types are compatible (expected W<'l1, 'l2>, found W<'_, '_>)
//~| NOTE ...so that types are compatible (expected W<'l1, 'l2>, found W<'_, '_>)

pub fn main() { }
