// error-pattern:declaration of `thpppt` shadows
enum ack { thpppt, ffff, }

fn main() {
  let thpppt: int = 42;
  log(debug, thpppt);
}
