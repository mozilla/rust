

#[abi = "cdecl"]
native mod rustrt {
    fn unsupervise();
}

#[abi = "cdecl"]
#[nolink]
native mod bar { }

#[abi = "cdecl"]
#[nolink]
native mod zed { }

#[abi = "cdecl"]
#[nolink]
native mod libc {
    fn write(fd: int, buf: *u8, count: ctypes::size_t) -> ctypes::ssize_t;
}

#[abi = "cdecl"]
#[nolink]
native mod baz { }

fn main(args: [str]) { }
