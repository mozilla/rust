// run-pass

#![feature(box_syntax)]
#![feature(intrinsics)]

mod rusti {
    extern "rust-intrinsic" {
        pub fn move_val_init<T>(dst: *mut T, src: T);
    }
}

pub fn main() {
    unsafe {
        // sanity check
        check_drops_state(0, None);

        let mut x: Option<Box<D>> = Some(box D(1));
        assert_eq!(x.as_ref().unwrap().0, 1);

        // A normal overwrite, to demonstrate `check_drops_state`.
        x = Some(box D(2));

        // At this point, one destructor has run, because the
        // overwrite of `x` drops its initial value.
        check_drops_state(1, Some(1));

        let mut y: Option<Box<D>> = std::mem::zeroed();

        // An initial binding does not overwrite anything.
        check_drops_state(1, Some(1));

        // Since `y` has been initialized via the `init` intrinsic, it
        // would be unsound to directly overwrite its value via normal
        // assignment.
        //
        // The code currently generated by the compiler is overly
        // accepting, however, in that it will check if `y` is itself
        // null and thus avoid the unsound action of attempting to
        // free null. In other words, if we were to do a normal
        // assignment like `y = box D(4);` here, it probably would not
        // crash today. But the plan is that it may well crash in the
        // future, (I believe).

        // `x` is moved here; the manner in which this is tracked by the
        // compiler is hidden.
        rusti::move_val_init(&mut y, x);

        // But what we *can* observe is how many times the destructor
        // for `D` is invoked, and what the last value we saw was
        // during such a destructor call. We do so after the end of
        // this scope.

        assert_eq!(y.as_ref().unwrap().0, 2);
        y.as_mut().unwrap().0 = 3;
        assert_eq!(y.as_ref().unwrap().0, 3);

        check_drops_state(1, Some(1));
    }

    check_drops_state(2, Some(3));
}

static mut NUM_DROPS: i32 = 0;
static mut LAST_DROPPED: Option<i32> = None;

fn check_drops_state(num_drops: i32, last_dropped: Option<i32>) {
    unsafe {
        assert_eq!(NUM_DROPS, num_drops);
        assert_eq!(LAST_DROPPED, last_dropped);
    }
}

struct D(i32);
impl Drop for D {
    fn drop(&mut self) {
        unsafe {
            NUM_DROPS += 1;
            LAST_DROPPED = Some(self.0);
        }
    }
}
