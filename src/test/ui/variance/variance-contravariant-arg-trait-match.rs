// ignore-compare-mode-nll

#![allow(dead_code)]

// Test that even when `T` is only used in contravariant position, it
// is treated as invariant.

trait Get<T> {
    fn get(&self, t: T);
}

fn get_min_from_max<'min, 'max, G>()
    where 'max : 'min, G : Get<&'max i32>
{
    impls_get::<G,&'min i32>() //~ ERROR mismatched types
}

fn get_max_from_min<'min, 'max, G>()
    where 'max : 'min, G : Get<&'min i32>
{
    // Previously OK, but now an error because traits are invariant:

    impls_get::<G,&'max i32>() //~ ERROR mismatched types
}

fn impls_get<G,T>() where G : Get<T> { }

fn main() { }
