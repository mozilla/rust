// xfail-test FIXME: Needs metadata encoding

class cat {
  priv {
    let mutable meows : uint;
  }

  let how_hungry : int;

  new(in_x : uint, in_y : int) { meows = in_x; how_hungry = in_y; }
}

fn main() {
  let nyan : cat = cat(52u, 99);
  let kitty = cat(1000u, 2);
  assert(nyan.how_hungry == 99);
  assert(kitty.how_hungry == 2);
}
