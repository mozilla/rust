// Regression test for #62988

// check-pass

#![feature(type_alias_impl_trait)]

trait MyTrait {
    type AssocType: Send;
    fn ret(&self) -> Self::AssocType;
}

impl MyTrait for () {
    type AssocType = impl Send;
    fn ret(&self) -> Self::AssocType {
        ()
    }
}

impl<'a> MyTrait for &'a () {
    type AssocType = impl Send;
    fn ret(&self) -> Self::AssocType {
        ()
    }
}

trait MyLifetimeTrait<'a> {
    type AssocType: Send + 'a;
    fn ret(&self) -> Self::AssocType;
}

impl<'a> MyLifetimeTrait<'a> for &'a () {
    type AssocType = impl Send + 'a;
    fn ret(&self) -> Self::AssocType {
        *self
    }
}

fn main() {}
