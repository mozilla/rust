// -*- rust -*-

io fn main() {
  let port[int] p = port();
  auto c = chan(p);
  let int y;

  auto task1 = spawn child(c);
  y <- p;
  log "received 1";
  log y;
  check (y == 10);

  auto task2 = spawn child(c);
  y <- p;
  log "received 2";
  log y;
  check (y == 10);
}

io fn child(chan[int] c) {
  c <| 10;
}
