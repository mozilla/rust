#![feature(self_in_typedefs)]
#![feature(untagged_unions)]

#![allow(dead_code)]

enum A<'a, T: 'a>
where
    Self: Send, T: PartialEq<Self>
{
    Foo(&'a Self),
    Bar(T),
}

struct B<'a, T: 'a>
where
    Self: Send, T: PartialEq<Self>
{
    foo: &'a Self,
    bar: T,
}

union C<'a, T: 'a>
where
    Self: Send, T: PartialEq<Self>
{
    foo: &'a Self,
    bar: T,
}

fn main() {}
