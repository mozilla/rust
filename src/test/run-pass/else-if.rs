fn main() {
  if (1 == 2) {
    check(false);
  } else if (2 == 3) {
    check(false);
  } else if (3 == 4) {
    check(false);
  } else {
    check(true);
  }


  if (1 == 2) {
    check(false);
  } else if (2 == 2) {
    check(true);
  }

  if (1 == 2) {
    check(false);
  } else if (2 == 2) {
    if (1 == 1) {
      check(true);
    } else {
      if (2 == 1) {
        check(false);
      } else {
        check(false);
      }
    }
  }

  if (1 == 2) {
    check(false);
  } else {
    if (1 == 2) {
      check(false);
    } else {
      check(true);
    }
  }
}
