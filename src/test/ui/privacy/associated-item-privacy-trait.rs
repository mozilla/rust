// ignore-tidy-linelength

#![feature(decl_macro, associated_type_defaults)]
#![allow(unused, private_in_public)]

mod priv_trait {
    trait PrivTr {
        fn method(&self) {}
        const CONST: u8 = 0;
        type AssocTy = u8;
    }
    pub struct Pub;
    impl PrivTr for Pub {}
    pub trait PubTr: PrivTr {}

    pub macro mac() {
        let value = <Pub as PrivTr>::method;
        //~^ ERROR method `priv_trait::PrivTr::method` is private
        value;
        //~^ ERROR method `priv_trait::PrivTr::method` is private
        Pub.method();
        //~^ ERROR method `priv_trait::PrivTr::method` is private
        <Pub as PrivTr>::CONST;
        //~^ ERROR associated constant `PrivTr::CONST` is private
        let _: <Pub as PrivTr>::AssocTy;
        //~^ ERROR trait `priv_trait::PrivTr` is private
        //~| ERROR trait `priv_trait::PrivTr` is private
        pub type InSignatureTy = <Pub as PrivTr>::AssocTy;
        //~^ ERROR trait `priv_trait::PrivTr` is private
        pub trait InSignatureTr: PrivTr {}
        //~^ ERROR trait `priv_trait::PrivTr` is private
        impl PrivTr for u8 {}
        //~^ ERROR trait `priv_trait::PrivTr` is private
    }
}
fn priv_trait() {
    priv_trait::mac!();
}

mod priv_signature {
    pub trait PubTr {
        fn method(&self, arg: Priv) {}
    }
    struct Priv;
    pub struct Pub;
    impl PubTr for Pub {}

    pub macro mac() {
        let value = <Pub as PubTr>::method;
        //~^ ERROR struct `priv_signature::Priv` is private
        value;
        //~^ ERROR struct `priv_signature::Priv` is private
        Pub.method(loop {});
        //~^ ERROR struct `priv_signature::Priv` is private
    }
}
fn priv_signature() {
    priv_signature::mac!();
}

mod priv_substs {
    pub trait PubTr {
        fn method<T>(&self) {}
    }
    struct Priv;
    pub struct Pub;
    impl PubTr for Pub {}

    pub macro mac() {
        let value = <Pub as PubTr>::method::<Priv>;
        //~^ ERROR struct `priv_substs::Priv` is private
        value;
        //~^ ERROR struct `priv_substs::Priv` is private
        Pub.method::<Priv>();
        //~^ ERROR struct `priv_substs::Priv` is private
    }
}
fn priv_substs() {
    priv_substs::mac!();
}

mod priv_parent_substs {
    pub trait PubTr<T = Priv> {
        fn method(&self) {}
        const CONST: u8 = 0;
        type AssocTy = u8;
    }
    struct Priv;
    pub struct Pub;
    impl PubTr<Priv> for Pub {}
    impl PubTr<Pub> for Priv {}

    pub macro mac() {
        let value = <Pub as PubTr>::method;
        //~^ ERROR struct `priv_parent_substs::Priv` is private
        value;
        //~^ ERROR struct `priv_parent_substs::Priv` is private
        let value = <Pub as PubTr<_>>::method;
        //~^ ERROR struct `priv_parent_substs::Priv` is private
        value;
        //~^ ERROR struct `priv_parent_substs::Priv` is private
        Pub.method();
        //~^ ERROR struct `priv_parent_substs::Priv` is private

        let value = <Priv as PubTr<_>>::method;
        //~^ ERROR struct `priv_parent_substs::Priv` is private
        value;
        //~^ ERROR struct `priv_parent_substs::Priv` is private
        Priv.method();
        //~^ ERROR struct `priv_parent_substs::Priv` is private

        <Pub as PubTr>::CONST;
        //~^ ERROR struct `priv_parent_substs::Priv` is private
        <Pub as PubTr<_>>::CONST;
        //~^ ERROR struct `priv_parent_substs::Priv` is private
        <Priv as PubTr<_>>::CONST;
        //~^ ERROR struct `priv_parent_substs::Priv` is private

        let _: <Pub as PubTr>::AssocTy;
        //~^ ERROR struct `priv_parent_substs::Priv` is private
        //~| ERROR struct `priv_parent_substs::Priv` is private
        let _: <Pub as PubTr<_>>::AssocTy;
        //~^ ERROR struct `priv_parent_substs::Priv` is private
        //~| ERROR struct `priv_parent_substs::Priv` is private
        let _: <Priv as PubTr<_>>::AssocTy;
        //~^ ERROR struct `priv_parent_substs::Priv` is private
        //~| ERROR struct `priv_parent_substs::Priv` is private

        pub type InSignatureTy1 = <Pub as PubTr>::AssocTy;
        //~^ ERROR struct `priv_parent_substs::Priv` is private
        pub type InSignatureTy2 = <Priv as PubTr<Pub>>::AssocTy;
        //~^ ERROR struct `priv_parent_substs::Priv` is private
        impl PubTr for u8 {}
        //~^ ERROR struct `priv_parent_substs::Priv` is private
    }
}
fn priv_parent_substs() {
    priv_parent_substs::mac!();
}

fn main() {}
