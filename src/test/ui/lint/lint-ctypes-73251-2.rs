// revisions: min_tait full_tait
#![feature(min_type_alias_impl_trait)]
#![cfg_attr(full_tait, feature(type_alias_impl_trait))]
//[full_tait]~^ WARN incomplete
#![deny(improper_ctypes)]

pub trait TraitA {
    type Assoc;
}

impl TraitA for u32 {
    type Assoc = u32;
}

pub trait TraitB {
    type Assoc;
}

impl<T> TraitB for T where T: TraitA {
    type Assoc = <T as TraitA>::Assoc;
}

type AliasA = impl TraitA<Assoc = u32>;

type AliasB = impl TraitB<Assoc = AliasA>;

fn use_of_a() -> AliasA { 3 }

fn use_of_b() -> AliasB { 3 }

extern "C" {
    pub fn lint_me() -> <AliasB as TraitB>::Assoc; //~ ERROR: uses type `impl TraitA`
}

fn main() {}
