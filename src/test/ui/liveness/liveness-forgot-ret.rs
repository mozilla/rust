fn god_exists(a: isize) -> bool { return god_exists(a); }

fn f(a: isize) -> isize { if god_exists(a) { return 5; }; }
//~^ ERROR mismatched types

fn main() { f(12); }
