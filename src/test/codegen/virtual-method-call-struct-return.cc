// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#include <stdlib.h>

struct Stuff {
  size_t a;
  double b;
};

struct Struct {
  virtual Stuff method() = 0;
};

extern "C"
size_t test(Struct &s) {
  return s.method().a;
}
