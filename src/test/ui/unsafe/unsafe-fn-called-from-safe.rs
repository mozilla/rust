unsafe fn f() { return; }

fn main() {
    f(); //~ ERROR call to unsafe function is unsafe
}
