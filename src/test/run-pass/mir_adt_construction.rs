use std::fmt;

#[repr(C)]
enum CEnum {
    Hello = 30,
    World = 60
}

fn test1(c: CEnum) -> i32 {
    let c2 = CEnum::Hello;
    match (c, c2) {
        (CEnum::Hello, CEnum::Hello) => 42,
        (CEnum::World, CEnum::Hello) => 0,
        _ => 1
    }
}

#[repr(packed)]
struct Pakd {
    a: u64,
    b: u32,
    c: u16,
    d: u8,
    e: ()
}

// It is unsafe to use #[derive(Debug)] on a packed struct because the code generated by the derive
// macro takes references to the fields instead of accessing them directly.
impl fmt::Debug for Pakd {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // It's important that we load the fields into locals by-value here. This will do safe
        // unaligned loads into the locals, then pass references to the properly-aligned locals to
        // the formatting code.
        let Pakd { a, b, c, d, e } = *self;
        f.debug_struct("Pakd")
            .field("a", &a)
            .field("b", &b)
            .field("c", &c)
            .field("d", &d)
            .field("e", &e)
            .finish()
    }
}

// It is unsafe to use #[derive(PartialEq)] on a packed struct because the code generated by the
// derive macro takes references to the fields instead of accessing them directly.
impl PartialEq for Pakd {
    fn eq(&self, other: &Pakd) -> bool {
        self.a == other.a &&
            self.b == other.b &&
            self.c == other.c &&
            self.d == other.d &&
            self.e == other.e
    }
}

impl Drop for Pakd {
    fn drop(&mut self) {}
}

fn test2() -> Pakd {
    Pakd { a: 42, b: 42, c: 42, d: 42, e: () }
}

#[derive(PartialEq, Debug)]
struct TupleLike(u64, u32);

fn test3() -> TupleLike {
    TupleLike(42, 42)
}

fn test4(x: fn(u64, u32) -> TupleLike) -> (TupleLike, TupleLike) {
    let y = TupleLike;
    (x(42, 84), y(42, 84))
}

fn test5(x: fn(u32) -> Option<u32>) -> (Option<u32>, Option<u32>) {
    let y = Some;
    (x(42), y(42))
}

fn main() {
    assert_eq!(test1(CEnum::Hello), 42);
    assert_eq!(test1(CEnum::World), 0);
    assert_eq!(test2(), Pakd { a: 42, b: 42, c: 42, d: 42, e: () });
    assert_eq!(test3(), TupleLike(42, 42));
    let t4 = test4(TupleLike);
    assert_eq!(t4.0, t4.1);
    let t5 = test5(Some);
    assert_eq!(t5.0, t5.1);
}
