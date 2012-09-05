// error-pattern: copying a noncopyable value

// Test that a class with a non-copyable field can't be
// copied
struct bar {
  let x: int;
  drop {}
}

fn bar(x:int) -> bar {
    bar {
        x: x
    }
}

struct foo {
  let i: int;
  let j: bar;
}

fn foo(i:int) -> foo {
    foo {
        i: i,
        j: bar(5)
    }
}

fn main() { let x <- foo(10); let y = x; log(error, x); }
