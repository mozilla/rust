// The compiler shouldn't ICE in this case.
static A: &'static [u32] = &[1];
static B: [u32; 1] = [0; A.len()];
//~^ ERROR [E0013]
//~| ERROR `core::slice::<impl [T]>::len` is not yet stable as a const fn

fn main() {}
