extern fn foopy() {}

static f: extern "C" fn() = foopy;
static s: S = S { f: foopy };

struct S {
    f: extern "C" fn()
}

pub fn main() {
    assert!(foopy as extern "C" fn() == f);
    assert!(f == s.f);
}
