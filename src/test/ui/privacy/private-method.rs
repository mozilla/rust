// error-pattern:method `nap` is private

mod kitties {
    pub struct cat {
        meows : usize,

        how_hungry : isize,
    }

    impl cat {
        fn nap(&self) {}
    }

    pub fn cat(in_x : usize, in_y : isize) -> cat {
        cat {
            meows: in_x,
            how_hungry: in_y
        }
    }
}

fn main() {
  let nyan : kitties::cat = kitties::cat(52, 99);
  nyan.nap();
}
