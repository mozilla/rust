// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Test that we correctly compute the changes to drop obligations
// from matches.

// All opening remarks from drop-obligations-1.rs also apply here.
// in particular:
//
// Note that this is not a true compile-fail test, in the sense that
// the code below is not meant to have any actual errors that are
// being detected by the comipiler. We are just re-purposing the
// compiletest error parsing as an easy way to annotate a test file
// with the expected operations from guts of the compiler.

// In all of the tests below, the arguments to the test are
// functions that:
//
// - c: create an instance (usually of `T`)

pub enum E<X,Y> { Zero, One(X), Two(X,Y), }

fn drop<T>(t: T) { }

#[rustc_drop_obligations]
fn matching_enum_copy_left<C:Copy,T>(c: fn() -> E<C,T>) {
    // `matching_enum_by_move` shows how variant matching manipulates
    // the drop obligation state.

    {
        let e : E<C,T>;        //~  ERROR        move removes drop-obl `$(local e)`

        e = c();               //~  ERROR     assignment adds drop-obl `$(local e)`

        // (match arms do not have their own spans, which partly
        // explains why all the bindings for each arm are reported as
        // being scoped by the match itself.)

        match e {
            Zero => {          //~  ERROR     match whitelists drop-obl `$(local e)`

            }

            // All of the fields (`c_0`) are `Copy`, so matching this
            // variant does not remove the drop-obligation, but rather
            // expands the whiteslist in the same way that `Zero`
            // above does.

            One(c_0) => {      //~  ERROR     match whitelists drop-obl `$(local e)`

                // `c_0` is Copy, so dropping it has no effect on the
                // drop-obligations.
                drop(c_0);
            }

            // `d_0` is `Copy` but `t_1` is not, so this does not
            // whitelist `e`; but it *only* manipulates the
            // drop-obligations associated with `t_1` in the matched
            // variant (and not anything associated with `d_0`).

            Two(d_0, t_1) => { //~ ERROR     refinement removes drop-obl `$(local e)`
                               //~^ ERROR           match adds drop-obl `($(local e)->Two).#1u`
                               //~^^ ERROR        move removes drop-obl `($(local e)->Two).#1u`
                               //~^^^ ERROR     assignment adds drop-obl `$(local t_1)`

                // `d_0` is Copy, so dropping it has no effect on the
                // drop-obligations.
                drop(d_0);

                drop(t_1);     //~  ERROR          move removes drop-obl `$(local t_1)`
            }

            // `c_0` and `d_0` are both `Copy`, so the scope-end here
            // does not treat them as part of the drop-obligations.

        }                      //~ ERROR    scope-end removes drop-obl `$(local t_1)`

        // The first component of variant `Two` is `Copy`, so it is not part of the
        // removed drop obligations here.

    }                          //~  ERROR     scope-end removes drop-obl `$(local e)`
                               //~^ ERROR    scope-end removes drop-obl `($(local e)->Zero)`
                               //~^^ ERROR   scope-end removes drop-obl `($(local e)->One)`
                               //~^^^ ERROR  scope-end removes drop-obl `($(local e)->Two).#1u`
}

#[rustc_drop_obligations]
fn matching_enum_copy_right<C:Copy,T>(c: fn() -> E<T,C>) {
    // `matching_enum_by_move` shows how variant matching manipulates
    // the drop obligation state.

    {
        let e : E<T,C>;        //~  ERROR        move removes drop-obl `$(local e)`

        e = c();               //~  ERROR     assignment adds drop-obl `$(local e)`

        // (match arms do not have their own spans, which partly
        // explains why all the bindings for each arm are reported as
        // being scoped by the match itself.)

        match e {
            Zero => {          //~  ERROR    match whitelists drop-obl `$(local e)`

            }

            One(s_0) => {      //~  ERROR  refinement removes drop-obl `$(local e)`
                               //~^ ERROR         match adds drop-obl `($(local e)->One).#0u`
                               //~^^ ERROR      move removes drop-obl `($(local e)->One).#0u`
                               //~^^^ ERROR   assignment adds drop-obl `$(local s_0)`

                drop(s_0);      //~  ERROR       move removes drop-obl `$(local s_0)`

            }

            // `d_1`, the 2nd component of `Two` aka (..->Two).#1u, is
            // `Copy`, so it is not part of the drop obligations.

            Two(t_0, d_1) => { //~  ERROR   refinement removes drop-obl `$(local e)`
                               //~^ ERROR          match adds drop-obl `($(local e)->Two).#0u`
                               //~^^ ERROR       move removes drop-obl `($(local e)->Two).#0u`
                               //~^^^ ERROR    assignment adds drop-obl `$(local t_0)`

                drop(t_0);     //~  ERROR         move removes drop-obl `$(local t_0)`
            }
        }                      //~   ERROR scope-end removes drop-obl `$(local s_0)`
                               //~^  ERROR scope-end removes drop-obl `$(local t_0)`


    }                          //~  ERROR       scope-end removes drop-obl `$(local e)`
                               //~^ ERROR      scope-end removes drop-obl `($(local e)->Zero)`
                               //~^^ ERROR     scope-end removes drop-obl `($(local e)->One).#0u`
                               //~^^^ ERROR    scope-end removes drop-obl `($(local e)->Two).#0u`
}

fn main() { }
