// run-pass

#![feature(core_intrinsics)]
#![feature(const_fn)]
#![allow(dead_code)]

const fn type_name_wrapper<T>(_: &T) -> &'static str {
    unsafe { std::intrinsics::type_name::<T>() }
}

struct Struct<TA, TB, TC> {
    a: TA,
    b: TB,
    c: TC,
}

type StructInstantiation = Struct<i8, f64, bool>;

const CONST_STRUCT: StructInstantiation = StructInstantiation {
    a: 12,
    b: 13.7,
    c: false,
};

const CONST_STRUCT_NAME: &'static str = type_name_wrapper(&CONST_STRUCT);

fn main() {
    println!("{}", CONST_STRUCT_NAME);

    let non_const_struct = StructInstantiation {
        a: 87,
        b: 65.99,
        c: true,
    };
    let non_const_struct_name = type_name_wrapper(&non_const_struct);

    println!("{}", non_const_struct_name);
}
