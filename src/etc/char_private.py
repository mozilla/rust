#!/usr/bin/env python
#
# Copyright 2011-2016 The Rust Project Developers. See the COPYRIGHT
# file at the top-level directory of this distribution and at
# http://rust-lang.org/COPYRIGHT.
#
# Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
# http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
# <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.

# This script uses the following Unicode tables:
# - UnicodeData.txt


from collections import namedtuple
import csv
import os
import subprocess

NUM_CODEPOINTS=0x110000

def to_ranges(iter):
    current = None
    for i in iter:
        if current is None or i != current[1] or i in (0x10000, 0x20000):
            if current is not None:
                yield tuple(current)
            current = [i, i + 1]
        else:
            current[1] += 1
    if current is not None:
        yield tuple(current)

def get_escaped(codepoints):
    for c in codepoints:
        if (c.class_ or "Cn") in "Cc Cf Cs Co Cn Zl Zp Zs".split() and c.value != ord(' '):
            yield c.value

def get_file(f):
    try:
        return open(os.path.basename(f))
    except FileNotFoundError:
        subprocess.run(["curl", "-O", f], check=True)
        return open(os.path.basename(f))

Codepoint = namedtuple('Codepoint', 'value class_')

def get_codepoints(f):
    r = csv.reader(f, delimiter=";")
    prev_codepoint = 0
    class_first = None
    for row in r:
        codepoint = int(row[0], 16)
        name = row[1]
        class_ = row[2]

        if class_first is not None:
            if not name.endswith("Last>"):
                raise ValueError("Missing Last after First")

        for c in range(prev_codepoint + 1, codepoint):
            yield Codepoint(c, class_first)

        class_first = None
        if name.endswith("First>"):
            class_first = class_

        yield Codepoint(codepoint, class_)
        prev_codepoint = codepoint

    if class_first != None:
        raise ValueError("Missing Last after First")

    for c in range(prev_codepoint + 1, NUM_CODEPOINTS):
        yield Codepoint(c, None)

def main():
    file = get_file("http://www.unicode.org/Public/UNIDATA/UnicodeData.txt")

    codepoints = get_codepoints(file)

    CUTOFF=0x10000
    singletons0 = []
    singletons1 = []
    normal0 = []
    normal1 = []
    extra = []

    for a, b in to_ranges(get_escaped(codepoints)):
        if a > 2 * CUTOFF:
            extra.append((a, b - a))
        elif a == b - 1:
            if a & CUTOFF:
                singletons1.append(a & ~CUTOFF)
            else:
                singletons0.append(a)
        elif a == b - 2:
            if a & CUTOFF:
                singletons1.append(a & ~CUTOFF)
                singletons1.append((a + 1) & ~CUTOFF)
            else:
                singletons0.append(a)
                singletons0.append(a + 1)
        else:
            if a >= 2 * CUTOFF:
                extra.append((a, b - a))
            elif a & CUTOFF:
                normal1.append((a & ~CUTOFF, b - a))
            else:
                normal0.append((a, b - a))

    print("""\
// Copyright 2012-2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// NOTE: The following code was generated by "src/etc/char_private.py",
//       do not edit directly!

use slice::SliceExt;

fn check(x: u16, singletons: &[u16], normal: &[u16]) -> bool {
    for &s in singletons {
        if x == s {
            return false;
        } else if x < s {
            break;
        }
    }
    for w in normal.chunks(2) {
        let start = w[0];
        let len = w[1];
        let difference = (x as i32) - (start as i32);
        if 0 <= difference {
            if difference < len as i32 {
                return false;
            }
        } else {
            break;
        }
    }
    true
}

pub fn is_printable(x: char) -> bool {
    let x = x as u32;
    let lower = x as u16;
    if x < 0x10000 {
        check(lower, SINGLETONS0, NORMAL0)
    } else if x < 0x20000 {
        check(lower, SINGLETONS1, NORMAL1)
    } else {\
""")
    for a, b in extra:
        print("        if 0x{:x} <= x && x < 0x{:x} {{".format(a, a + b))
        print("            return false;")
        print("        }")
    print("""\
        true
    }
}\
""")
    print()
    print("const SINGLETONS0: &'static [u16] = &[")
    for s in singletons0:
        print("    0x{:x},".format(s))
    print("];")
    print("const SINGLETONS1: &'static [u16] = &[")
    for s in singletons1:
        print("    0x{:x},".format(s))
    print("];")
    print("const NORMAL0: &'static [u16] = &[")
    for a, b in normal0:
        print("    0x{:x}, 0x{:x},".format(a, b))
    print("];")
    print("const NORMAL1: &'static [u16] = &[")
    for a, b in normal1:
        print("    0x{:x}, 0x{:x},".format(a, b))
    print("];")

if __name__ == '__main__':
    main()
