
// error-pattern: unresolved name
mod circ1 {
  import circ1::*;
  export f1;
  export f2;
  export common;
  fn f1() {
    log "f1";
  }
  fn common() -> uint {
    ret 0u;
  }
}

mod circ2 {
  import circ2::*;
  export f1;
  export f2;
  export common;
  fn f2() {
    log "f2";
  }
  fn common() -> uint {
    ret 1u;
  }
}

mod test {
  import circ1::*;

  fn test() {
    f1066();
  }
}