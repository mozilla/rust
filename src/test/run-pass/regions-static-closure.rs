struct closure_box<'a> {
    cl: Box<FnMut() + 'a>,
}

fn box_it<'a>(x: Box<FnMut() + 'a>) -> closure_box<'a> {
    closure_box {cl: x}
}

fn call_static_closure(mut cl: closure_box<'static>) {
    (cl.cl)();
}

pub fn main() {
    let cl_box = box_it(Box::new(|| println!("Hello, world!")));
    call_static_closure(cl_box);
}
