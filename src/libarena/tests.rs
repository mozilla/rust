extern crate test;
use test::Bencher;
use super::{TypedArena, DroplessArena, SyncDroplessArena};
use std::cell::Cell;
use std::iter;

#[allow(dead_code)]
#[derive(Debug, Eq, PartialEq)]
struct Point {
    x: i32,
    y: i32,
    z: i32,
}

#[test]
fn test_arena_alloc_nested_typed() {
    struct Inner {
        value: u8,
    }
    struct Outer<'a> {
        inner: &'a Inner,
    }
    enum EI<'e> {
        I(Inner),
        O(Outer<'e>),
    }

    struct Wrap<'a>(TypedArena<EI<'a>>);

    impl<'a> Wrap<'a> {
        fn alloc_inner<F: Fn() -> Inner>(&self, f: F) -> &Inner {
            let r: &EI<'_> = self.0.alloc(EI::I(f()));
            if let &EI::I(ref i) = r {
                i
            } else {
                panic!("mismatch");
            }
        }
        fn alloc_outer<F: Fn() -> Outer<'a>>(&self, f: F) -> &Outer<'_> {
            let r: &EI<'_> = self.0.alloc(EI::O(f()));
            if let &EI::O(ref o) = r {
                o
            } else {
                panic!("mismatch");
            }
        }
    }

    let arena = Wrap(TypedArena::default());

    let result = arena.alloc_outer(|| Outer {
        inner: arena.alloc_inner(|| Inner { value: 10 }),
    });

    assert_eq!(result.inner.value, 10);
}

#[test]
fn test_arena_alloc_nested_dropless() {
    struct Inner {
        value: u8,
    }
    struct Outer<'a> {
        inner: &'a Inner,
    }
    enum EI<'e> {
        I(Inner),
        O(Outer<'e>),
    }

    struct Wrap(DroplessArena);

    impl Wrap {
        fn alloc_inner<F: Fn() -> Inner>(&self, f: F) -> &Inner {
            let r: &EI<'_> = self.0.alloc(EI::I(f()));
            if let &EI::I(ref i) = r {
                i
            } else {
                panic!("mismatch");
            }
        }
        fn alloc_outer<'a, F: Fn() -> Outer<'a>>(&'a self, f: F) -> &Outer<'a> {
            let r: &EI<'_> = self.0.alloc(EI::O(f()));
            if let &EI::O(ref o) = r {
                o
            } else {
                panic!("mismatch");
            }
        }
    }

    let arena = Wrap(DroplessArena::default());

    let result = arena.alloc_outer(|| Outer {
        inner: arena.alloc_inner(|| Inner { value: 10 }),
    });

    assert_eq!(result.inner.value, 10);
}

#[test]
fn test_arena_alloc_nested_sync() {
    struct Inner {
        value: u8,
    }
    struct Outer<'a> {
        inner: &'a Inner,
    }
    enum EI<'e> {
        I(Inner),
        O(Outer<'e>),
    }

    struct Wrap(SyncDroplessArena);

    impl Wrap {
        fn alloc_inner<F: Fn() -> Inner>(&self, f: F) -> &Inner {
            let r: &EI<'_> = self.0.alloc(EI::I(f()));
            if let &EI::I(ref i) = r {
                i
            } else {
                panic!("mismatch");
            }
        }
        fn alloc_outer<'a, F: Fn() -> Outer<'a>>(&'a self, f: F) -> &Outer<'a> {
            let r: &EI<'_> = self.0.alloc(EI::O(f()));
            if let &EI::O(ref o) = r {
                o
            } else {
                panic!("mismatch");
            }
        }
    }

    let arena = Wrap(SyncDroplessArena::default());

    let result = arena.alloc_outer(|| Outer {
        inner: arena.alloc_inner(|| Inner { value: 10 }),
    });

    assert_eq!(result.inner.value, 10);
}

#[test]
fn test_arena_alloc_nested_iter() {
    struct Inner {
        value: u8,
    }
    struct Outer<'a> {
        inner: &'a Inner,
    }
    enum EI<'e> {
        I(Inner),
        O(Outer<'e>),
    }

    struct Wrap<'a>(TypedArena<EI<'a>>);

    impl<'a> Wrap<'a> {
        fn alloc_inner<F: Fn() -> Inner>(&self, f: F) -> &Inner {
            let r: &[EI<'_>] = self.0.alloc_from_iter(iter::once_with(|| EI::I(f())));
            if let &[EI::I(ref i)] = r {
                i
            } else {
                panic!("mismatch");
            }
        }
        fn alloc_outer<F: Fn() -> Outer<'a>>(&self, f: F) -> &Outer<'_> {
            let r: &[EI<'_>] = self.0.alloc_from_iter(iter::once_with(|| EI::O(f())));
            if let &[EI::O(ref o)] = r {
                o
            } else {
                panic!("mismatch");
            }
        }
    }

    let arena = Wrap(TypedArena::default());

    let result = arena.alloc_outer(|| Outer {
        inner: arena.alloc_inner(|| Inner { value: 10 }),
    });

    assert_eq!(result.inner.value, 10);
}

#[test]
pub fn test_copy() {
    let arena = TypedArena::default();
    for _ in 0..100000 {
        arena.alloc(Point { x: 1, y: 2, z: 3 });
    }

    let arena = DroplessArena::default();
    for _ in 0..100000 {
        arena.alloc(Point { x: 1, y: 2, z: 3 });
    }

    let arena = SyncDroplessArena::default();
    for _ in 0..100000 {
        arena.alloc(Point { x: 1, y: 2, z: 3 });
    }
}

#[test]
pub fn test_align() {
    #[repr(align(32))]
    struct AlignedPoint(Point);

    let arena = TypedArena::default();
    for _ in 0..100000 {
        let ptr = arena.alloc(AlignedPoint(Point { x: 1, y: 2, z: 3 }));
        assert_eq!((ptr as *const _ as usize) & 31, 0);
    }
}

#[bench]
pub fn bench_copy(b: &mut Bencher) {
    let arena = TypedArena::default();
    b.iter(|| arena.alloc(Point { x: 1, y: 2, z: 3 }))
}

#[bench]
pub fn bench_copy_nonarena(b: &mut Bencher) {
    b.iter(|| {
        let _: Box<_> = Box::new(Point { x: 1, y: 2, z: 3 });
    })
}

#[allow(dead_code)]
struct Noncopy {
    string: String,
    array: Vec<i32>,
}

#[test]
pub fn test_noncopy() {
    let arena = TypedArena::default();
    for _ in 0..100000 {
        arena.alloc(Noncopy {
            string: "hello world".to_string(),
            array: vec![1, 2, 3, 4, 5],
        });
    }
}

#[test]
pub fn test_typed_arena_zero_sized() {
    let arena = TypedArena::default();
    for _ in 0..100000 {
        arena.alloc(());
    }
}

#[test]
pub fn test_typed_arena_clear() {
    let mut arena = TypedArena::default();
    for _ in 0..10 {
        arena.clear();
        for _ in 0..10000 {
            arena.alloc(Point { x: 1, y: 2, z: 3 });
        }
    }
}

#[bench]
pub fn bench_typed_arena_clear(b: &mut Bencher) {
    let mut arena = TypedArena::default();
    b.iter(|| {
        arena.alloc(Point { x: 1, y: 2, z: 3 });
        arena.clear();
    })
}

// Drop tests

struct DropCounter<'a> {
    count: &'a Cell<u32>,
}

impl Drop for DropCounter<'_> {
    fn drop(&mut self) {
        self.count.set(self.count.get() + 1);
    }
}

#[test]
fn test_typed_arena_drop_count() {
    let counter = Cell::new(0);
    {
        let arena: TypedArena<DropCounter<'_>> = TypedArena::default();
        for _ in 0..100 {
            // Allocate something with drop glue to make sure it doesn't leak.
            arena.alloc(DropCounter { count: &counter });
        }
    };
    assert_eq!(counter.get(), 100);
}

#[test]
fn test_typed_arena_drop_on_clear() {
    let counter = Cell::new(0);
    let mut arena: TypedArena<DropCounter<'_>> = TypedArena::default();
    for i in 0..10 {
        for _ in 0..100 {
            // Allocate something with drop glue to make sure it doesn't leak.
            arena.alloc(DropCounter { count: &counter });
        }
        arena.clear();
        assert_eq!(counter.get(), i * 100 + 100);
    }
}

struct DropOrder<'a> {
    rank: u32,
    count: &'a Cell<u32>,
}

impl Drop for DropOrder<'_> {
    fn drop(&mut self) {
        assert_eq!(self.rank, self.count.get());
        self.count.set(self.count.get() + 1);
    }
}

#[test]
fn test_typed_arena_drop_order() {
    let counter = Cell::new(0);
    {
        let arena: TypedArena<DropOrder<'_>> = TypedArena::default();
        for rank in 0..100 {
            // Allocate something with drop glue to make sure it doesn't leak.
            arena.alloc(DropOrder { rank, count: &counter });
        }
    };
    assert_eq!(counter.get(), 100);
}

thread_local! {
    static DROP_COUNTER: Cell<u32> = Cell::new(0)
}

struct SmallDroppable;

impl Drop for SmallDroppable {
    fn drop(&mut self) {
        DROP_COUNTER.with(|c| c.set(c.get() + 1));
    }
}

#[test]
fn test_typed_arena_drop_small_count() {
    DROP_COUNTER.with(|c| c.set(0));
    {
        let arena: TypedArena<SmallDroppable> = TypedArena::default();
        for _ in 0..100 {
            // Allocate something with drop glue to make sure it doesn't leak.
            arena.alloc(SmallDroppable);
        }
        // dropping
    };
    assert_eq!(DROP_COUNTER.with(|c| c.get()), 100);
}

#[bench]
pub fn bench_noncopy(b: &mut Bencher) {
    let arena = TypedArena::default();
    b.iter(|| {
        arena.alloc(Noncopy {
            string: "hello world".to_string(),
            array: vec![1, 2, 3, 4, 5],
        })
    })
}

#[bench]
pub fn bench_noncopy_nonarena(b: &mut Bencher) {
    b.iter(|| {
        let _: Box<_> = Box::new(Noncopy {
            string: "hello world".to_string(),
            array: vec![1, 2, 3, 4, 5],
        });
    })
}
