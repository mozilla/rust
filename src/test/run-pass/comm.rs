// -*- rust -*-

io fn main() {
  let port[int] p = port();
  auto child_task = spawn child(chan(p));
  let int y;
  y <- p;
  log "received";
  log y;
  check (y == 10);
}

io fn child(chan[int] c) {
  c <| 10;
}

