use std::mem;

fn eq_ref<T>(x: &T, y: &T) -> bool {
    x as *const _ == y as *const _
}

fn f() -> i32 { 42 }

fn main() {
    // int-ptr-int
    assert_eq!(1 as *const i32 as usize, 1);
    assert_eq!((1 as *const i32).wrapping_offset(4) as usize, 1 + 4*4);

    {   // ptr-int-ptr
        let x = 13;
        let mut y = &x as &_ as *const _ as usize;
        y += 13;
        y -= 13;
        let y = y as *const _;
        assert!(eq_ref(&x, unsafe { &*y }));
    }

    {   // fnptr-int-fnptr
        let x : fn() -> i32 = f;
        let y : *mut u8 = unsafe { mem::transmute(x as fn() -> i32) };
        let mut y = y as usize;
        y += 13;
        y -= 13;
        let x : fn() -> i32 = unsafe { mem::transmute(y as *mut u8) };
        assert_eq!(x(), 42);
    }
}
