// error-pattern: mismatched types

fn f(int x) -> int {
  ret x;
}

fn main() {
  auto taskf = spawn f(10);
}
