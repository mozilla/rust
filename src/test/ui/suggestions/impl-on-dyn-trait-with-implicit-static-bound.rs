// run-rustfix
#![allow(dead_code)]

mod foo {
    trait OtherTrait<'a> {}
    impl<'a> OtherTrait<'a> for &'a () {}

    trait ObjectTrait {}
    trait MyTrait {
        fn use_self(&self) -> &();
    }
    trait Irrelevant {}

    impl MyTrait for dyn ObjectTrait {
        fn use_self(&self) -> &() { panic!() }
    }
    impl Irrelevant for dyn ObjectTrait {}

    fn use_it<'a>(val: &'a dyn ObjectTrait) -> impl OtherTrait<'a> + 'a {
        val.use_self() //~ ERROR E0759
    }
}

mod bar {
    trait ObjectTrait {}
    trait MyTrait {
        fn use_self(&self) -> &();
    }
    trait Irrelevant {}

    impl MyTrait for dyn ObjectTrait {
        fn use_self(&self) -> &() { panic!() }
    }
    impl Irrelevant for dyn ObjectTrait {}

    fn use_it<'a>(val: &'a dyn ObjectTrait) -> &'a () {
        val.use_self() //~ ERROR E0767
    }
}

mod baz {
    trait ObjectTrait {}
    trait MyTrait {
        fn use_self(&self) -> &();
    }
    trait Irrelevant {}

    impl MyTrait for Box<dyn ObjectTrait> {
        fn use_self(&self) -> &() { panic!() }
    }
    impl Irrelevant for Box<dyn ObjectTrait> {}

    fn use_it<'a>(val: &'a Box<dyn ObjectTrait + 'a>) -> &'a () {
        val.use_self() //~ ERROR E0767
    }
}

mod bat {
    trait OtherTrait<'a> {}
    impl<'a> OtherTrait<'a> for &'a () {}

    trait ObjectTrait {}

    impl dyn ObjectTrait {
        fn use_self(&self) -> &() { panic!() }
    }

    fn use_it<'a>(val: &'a dyn ObjectTrait) -> impl OtherTrait<'a> + 'a {
        val.use_self() //~ ERROR E0767
    }
}

mod ban {
    trait OtherTrait<'a> {}
    impl<'a> OtherTrait<'a> for &'a () {}

    trait ObjectTrait {}
    trait MyTrait {
        fn use_self(&self) -> &();
    }
    trait Irrelevant {}

    impl MyTrait for dyn ObjectTrait {
        fn use_self(&self) -> &() { panic!() }
    }
    impl Irrelevant for dyn ObjectTrait {}

    fn use_it<'a>(val: &'a dyn ObjectTrait) -> impl OtherTrait<'a> {
        val.use_self() //~ ERROR E0759
    }
}


fn main() {}
