#!/usr/bin/env python
#
# Copyright 2011-2013 The Rust Project Developers. See the COPYRIGHT
# file at the top-level directory of this distribution and at
# http://rust-lang.org/COPYRIGHT.
#
# Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
# http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
# <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.

# This script uses the following Unicode tables:
# - DerivedCoreProperties.txt
# - DerivedNormalizationProps.txt
# - EastAsianWidth.txt
# - auxiliary/GraphemeBreakProperty.txt
# - auxiliary/WordBreakProperty.txt
# - PropList.txt
# - ReadMe.txt
# - Scripts.txt
# - UnicodeData.txt
#
# Since this should not require frequent updates, we just store this
# out-of-line and check the unicode.rs file into git.

import fileinput, re, os, sys, operator

preamble = '''// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// NOTE: The following code was generated by "src/etc/unicode.py", do not edit directly

#![allow(missing_docs, non_upper_case_globals, non_snake_case)]
'''

# Mapping taken from Table 12 from:
# http://www.unicode.org/reports/tr44/#General_Category_Values
expanded_categories = {
    'Lu': ['LC', 'L'], 'Ll': ['LC', 'L'], 'Lt': ['LC', 'L'],
    'Lm': ['L'], 'Lo': ['L'],
    'Mn': ['M'], 'Mc': ['M'], 'Me': ['M'],
    'Nd': ['N'], 'Nl': ['N'], 'No': ['No'],
    'Pc': ['P'], 'Pd': ['P'], 'Ps': ['P'], 'Pe': ['P'],
    'Pi': ['P'], 'Pf': ['P'], 'Po': ['P'],
    'Sm': ['S'], 'Sc': ['S'], 'Sk': ['S'], 'So': ['S'],
    'Zs': ['Z'], 'Zl': ['Z'], 'Zp': ['Z'],
    'Cc': ['C'], 'Cf': ['C'], 'Cs': ['C'], 'Co': ['C'], 'Cn': ['C'],
}

# these are the surrogate codepoints, which are not valid rust characters
surrogate_codepoints = (0xd800, 0xdfff)

def fetch(f):
    if not os.path.exists(os.path.basename(f)):
        os.system("curl -O http://www.unicode.org/Public/UNIDATA/%s"
                  % f)

    if not os.path.exists(os.path.basename(f)):
        sys.stderr.write("cannot load %s" % f)
        exit(1)

def is_surrogate(n):
    return surrogate_codepoints[0] <= n <= surrogate_codepoints[1]

def load_unicode_data(f):
    fetch(f)
    gencats = {}
    upperlower = {}
    lowerupper = {}
    combines = {}
    canon_decomp = {}
    compat_decomp = {}

    udict = {};
    range_start = -1;
    for line in fileinput.input(f):
        data = line.split(';');
        if len(data) != 15:
            continue
        cp = int(data[0], 16);
        if is_surrogate(cp):
            continue
        if range_start >= 0:
            for i in xrange(range_start, cp):
                udict[i] = data;
            range_start = -1;
        if data[1].endswith(", First>"):
            range_start = cp;
            continue;
        udict[cp] = data;

    for code in udict:
        [code_org, name, gencat, combine, bidi,
         decomp, deci, digit, num, mirror,
         old, iso, upcase, lowcase, titlecase ] = udict[code];

        # generate char to char direct common and simple conversions
        # uppercase to lowercase
        if gencat == "Lu" and lowcase != "" and code_org != lowcase:
            upperlower[code] = int(lowcase, 16)

        # lowercase to uppercase
        if gencat == "Ll" and upcase != "" and code_org != upcase:
            lowerupper[code] = int(upcase, 16)

        # store decomposition, if given
        if decomp != "":
            if decomp.startswith('<'):
                seq = []
                for i in decomp.split()[1:]:
                    seq.append(int(i, 16))
                compat_decomp[code] = seq
            else:
                seq = []
                for i in decomp.split():
                    seq.append(int(i, 16))
                canon_decomp[code] = seq

        # place letter in categories as appropriate
        for cat in [gencat, "Assigned"] + expanded_categories.get(gencat, []):
            if cat not in gencats:
                gencats[cat] = []
            gencats[cat].append(code)

        # record combining class, if any
        if combine != "0":
            if combine not in combines:
                combines[combine] = []
            combines[combine].append(code)

    # generate Not_Assigned from Assigned
    gencats["Cn"] = gen_unassigned(gencats["Assigned"])
    # Assigned is not a real category
    del(gencats["Assigned"])
    # Other contains Not_Assigned
    gencats["C"].extend(gencats["Cn"])
    gencats = group_cats(gencats)
    combines = to_combines(group_cats(combines))

    return (canon_decomp, compat_decomp, gencats, combines, lowerupper, upperlower)

def group_cats(cats):
    cats_out = {}
    for cat in cats:
        cats_out[cat] = group_cat(cats[cat])
    return cats_out

def group_cat(cat):
    cat_out = []
    letters = sorted(set(cat))
    cur_start = letters.pop(0)
    cur_end = cur_start
    for letter in letters:
        assert letter > cur_end, \
            "cur_end: %s, letter: %s" % (hex(cur_end), hex(letter))
        if letter == cur_end + 1:
            cur_end = letter
        else:
            cat_out.append((cur_start, cur_end))
            cur_start = cur_end = letter
    cat_out.append((cur_start, cur_end))
    return cat_out

def ungroup_cat(cat):
    cat_out = []
    for (lo, hi) in cat:
        while lo <= hi:
            cat_out.append(lo)
            lo += 1
    return cat_out

def gen_unassigned(assigned):
    assigned = set(assigned)
    return ([i for i in range(0, 0xd800) if i not in assigned] +
            [i for i in range(0xe000, 0x110000) if i not in assigned])

def to_combines(combs):
    combs_out = []
    for comb in combs:
        for (lo, hi) in combs[comb]:
            combs_out.append((lo, hi, comb))
    combs_out.sort(key=lambda comb: comb[0])
    return combs_out

def format_table_content(f, content, indent):
    line = " "*indent
    first = True
    for chunk in content.split(","):
        if len(line) + len(chunk) < 98:
            if first:
                line += chunk
            else:
                line += ", " + chunk
            first = False
        else:
            f.write(line + ",\n")
            line = " "*indent + chunk
    f.write(line)

def load_properties(f, interestingprops):
    fetch(f)
    props = {}
    re1 = re.compile("^([0-9A-F]+) +; (\w+)")
    re2 = re.compile("^([0-9A-F]+)\.\.([0-9A-F]+) +; (\w+)")

    for line in fileinput.input(os.path.basename(f)):
        prop = None
        d_lo = 0
        d_hi = 0
        m = re1.match(line)
        if m:
            d_lo = m.group(1)
            d_hi = m.group(1)
            prop = m.group(2)
        else:
            m = re2.match(line)
            if m:
                d_lo = m.group(1)
                d_hi = m.group(2)
                prop = m.group(3)
            else:
                continue
        if interestingprops and prop not in interestingprops:
            continue
        d_lo = int(d_lo, 16)
        d_hi = int(d_hi, 16)
        if prop not in props:
            props[prop] = []
        props[prop].append((d_lo, d_hi))
    return props

# load all widths of want_widths, except those in except_cats
def load_east_asian_width(want_widths, except_cats):
    f = "EastAsianWidth.txt"
    fetch(f)
    widths = {}
    re1 = re.compile("^([0-9A-F]+);(\w+) +# (\w+)")
    re2 = re.compile("^([0-9A-F]+)\.\.([0-9A-F]+);(\w+) +# (\w+)")

    for line in fileinput.input(f):
        width = None
        d_lo = 0
        d_hi = 0
        cat = None
        m = re1.match(line)
        if m:
            d_lo = m.group(1)
            d_hi = m.group(1)
            width = m.group(2)
            cat = m.group(3)
        else:
            m = re2.match(line)
            if m:
                d_lo = m.group(1)
                d_hi = m.group(2)
                width = m.group(3)
                cat = m.group(4)
            else:
                continue
        if cat in except_cats or width not in want_widths:
            continue
        d_lo = int(d_lo, 16)
        d_hi = int(d_hi, 16)
        if width not in widths:
            widths[width] = []
        widths[width].append((d_lo, d_hi))
    return widths

def escape_char(c):
    return "'\\u{%x}'" % c

def emit_bsearch_range_table(f):
    f.write("""
fn bsearch_range_table(c: char, r: &'static [(char,char)]) -> bool {
    use core::cmp::Ordering::{Equal, Less, Greater};
    use core::slice::SliceExt;
    r.binary_search_by(|&(lo,hi)| {
        if lo <= c && c <= hi { Equal }
        else if hi < c { Less }
        else { Greater }
    }).is_ok()
}\n
""")

def emit_table(f, name, t_data, t_type = "&'static [(char, char)]", is_pub=True,
        pfun=lambda x: "(%s,%s)" % (escape_char(x[0]), escape_char(x[1])), is_const=True):
    pub_string = "const"
    if not is_const:
        pub_string = "let"
    if is_pub:
        pub_string = "pub " + pub_string
    f.write("    %s %s: %s = &[\n" % (pub_string, name, t_type))
    data = ""
    first = True
    for dat in t_data:
        if not first:
            data += ","
        first = False
        data += pfun(dat)
    format_table_content(f, data, 8)
    f.write("\n    ];\n\n")

def emit_property_module(f, mod, tbl, emit):
    f.write("pub mod %s {\n" % mod)
    for cat in sorted(emit):
        emit_table(f, "%s_table" % cat, tbl[cat])
        f.write("    pub fn %s(c: char) -> bool {\n" % cat)
        f.write("        super::bsearch_range_table(c, %s_table)\n" % cat)
        f.write("    }\n\n")
    f.write("}\n\n")

def emit_conversions_module(f, lowerupper, upperlower):
    f.write("pub mod conversions {")
    f.write("""
    use core::cmp::Ordering::{Equal, Less, Greater};
    use core::slice::SliceExt;
    use core::option::Option;
    use core::option::Option::{Some, None};
    use core::result::Result::{Ok, Err};

    pub fn to_lower(c: char) -> char {
        match bsearch_case_table(c, LuLl_table) {
          None        => c,
          Some(index) => LuLl_table[index].1
        }
    }

    pub fn to_upper(c: char) -> char {
        match bsearch_case_table(c, LlLu_table) {
            None        => c,
            Some(index) => LlLu_table[index].1
        }
    }

    fn bsearch_case_table(c: char, table: &'static [(char, char)]) -> Option<usize> {
        match table.binary_search_by(|&(key, _)| {
            if c == key { Equal }
            else if key < c { Less }
            else { Greater }
        }) {
            Ok(i) => Some(i),
            Err(_) => None,
        }
    }

""")
    emit_table(f, "LuLl_table",
        sorted(upperlower.iteritems(), key=operator.itemgetter(0)), is_pub=False)
    emit_table(f, "LlLu_table",
        sorted(lowerupper.iteritems(), key=operator.itemgetter(0)), is_pub=False)
    f.write("}\n\n")

def emit_break_module(f, break_table, break_cats, name):
    Name = name.capitalize()
    f.write("""pub mod %s {
    use core::slice::SliceExt;
    pub use self::%sCat::*;
    use core::result::Result::{Ok, Err};

    #[allow(non_camel_case_types)]
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub enum %sCat {
""" % (name, Name, Name))

    break_cats.append("Any")
    break_cats.sort()
    for cat in break_cats:
        f.write(("        %sC_" % Name[0]) + cat + ",\n")
    f.write("""    }

    fn bsearch_range_value_table(c: char, r: &'static [(char, char, %sCat)]) -> %sCat {
        use core::cmp::Ordering::{Equal, Less, Greater};
        match r.binary_search_by(|&(lo, hi, _)| {
            if lo <= c && c <= hi { Equal }
            else if hi < c { Less }
            else { Greater }
        }) {
            Ok(idx) => {
                let (_, _, cat) = r[idx];
                cat
            }
            Err(_) => %sC_Any
        }
    }

    pub fn %s_category(c: char) -> %sCat {
        bsearch_range_value_table(c, %s_cat_table)
    }

""" % (Name, Name, Name[0], name, Name, name))

    emit_table(f, "%s_cat_table" % name, break_table, "&'static [(char, char, %sCat)]" % Name,
        pfun=lambda x: "(%s,%s,%sC_%s)" % (escape_char(x[0]), escape_char(x[1]), Name[0], x[2]),
        is_pub=False, is_const=True)
    f.write("}\n")

def emit_charwidth_module(f, width_table):
    f.write("pub mod charwidth {\n")
    f.write("    use core::option::Option;\n")
    f.write("    use core::option::Option::{Some, None};\n")
    f.write("    use core::slice::SliceExt;\n")
    f.write("    use core::result::Result::{Ok, Err};\n")
    f.write("""
    fn bsearch_range_value_table(c: char, is_cjk: bool, r: &'static [(char, char, u8, u8)]) -> u8 {
        use core::cmp::Ordering::{Equal, Less, Greater};
        match r.binary_search_by(|&(lo, hi, _, _)| {
            if lo <= c && c <= hi { Equal }
            else if hi < c { Less }
            else { Greater }
        }) {
            Ok(idx) => {
                let (_, _, r_ncjk, r_cjk) = r[idx];
                if is_cjk { r_cjk } else { r_ncjk }
            }
            Err(_) => 1
        }
    }
""")

    f.write("""
    pub fn width(c: char, is_cjk: bool) -> Option<usize> {
        match c as usize {
            _c @ 0 => Some(0),          // null is zero width
            cu if cu < 0x20 => None,    // control sequences have no width
            cu if cu < 0x7F => Some(1), // ASCII
            cu if cu < 0xA0 => None,    // more control sequences
            _ => Some(bsearch_range_value_table(c, is_cjk, charwidth_table) as usize)
        }
    }

""")

    f.write("    // character width table. Based on Markus Kuhn's free wcwidth() implementation,\n")
    f.write("    //     http://www.cl.cam.ac.uk/~mgk25/ucs/wcwidth.c\n")
    emit_table(f, "charwidth_table", width_table, "&'static [(char, char, u8, u8)]", is_pub=False,
            pfun=lambda x: "(%s,%s,%s,%s)" % (escape_char(x[0]), escape_char(x[1]), x[2], x[3]))
    f.write("}\n\n")

def emit_norm_module(f, canon, compat, combine, norm_props):
    canon_keys = canon.keys()
    canon_keys.sort()

    compat_keys = compat.keys()
    compat_keys.sort()

    canon_comp = {}
    comp_exclusions = norm_props["Full_Composition_Exclusion"]
    for char in canon_keys:
        if True in map(lambda (lo, hi): lo <= char <= hi, comp_exclusions):
            continue
        decomp = canon[char]
        if len(decomp) == 2:
            if not canon_comp.has_key(decomp[0]):
                canon_comp[decomp[0]] = []
            canon_comp[decomp[0]].append( (decomp[1], char) )
    canon_comp_keys = canon_comp.keys()
    canon_comp_keys.sort()

    f.write("pub mod normalization {\n")

    def mkdata_fun(table):
        def f(char):
            data = "(%s,&[" % escape_char(char)
            first = True
            for d in table[char]:
                if not first:
                    data += ","
                first = False
                data += escape_char(d)
            data += "])"
            return data
        return f

    f.write("    // Canonical decompositions\n")
    emit_table(f, "canonical_table", canon_keys, "&'static [(char, &'static [char])]",
        pfun=mkdata_fun(canon))

    f.write("    // Compatibility decompositions\n")
    emit_table(f, "compatibility_table", compat_keys, "&'static [(char, &'static [char])]",
        pfun=mkdata_fun(compat))

    def comp_pfun(char):
        data = "(%s,&[" % escape_char(char)
        canon_comp[char].sort(lambda x, y: x[0] - y[0])
        first = True
        for pair in canon_comp[char]:
            if not first:
                data += ","
            first = False
            data += "(%s,%s)" % (escape_char(pair[0]), escape_char(pair[1]))
        data += "])"
        return data

    f.write("    // Canonical compositions\n")
    emit_table(f, "composition_table", canon_comp_keys,
        "&'static [(char, &'static [(char, char)])]", pfun=comp_pfun)

    f.write("""
    fn bsearch_range_value_table(c: char, r: &'static [(char, char, u8)]) -> u8 {
        use core::cmp::Ordering::{Equal, Less, Greater};
        use core::slice::SliceExt;
        use core::result::Result::{Ok, Err};
        match r.binary_search_by(|&(lo, hi, _)| {
            if lo <= c && c <= hi { Equal }
            else if hi < c { Less }
            else { Greater }
        }) {
            Ok(idx) => {
                let (_, _, result) = r[idx];
                result
            }
            Err(_) => 0
        }
    }\n
""")

    emit_table(f, "combining_class_table", combine, "&'static [(char, char, u8)]", is_pub=False,
            pfun=lambda x: "(%s,%s,%s)" % (escape_char(x[0]), escape_char(x[1]), x[2]))

    f.write("    pub fn canonical_combining_class(c: char) -> u8 {\n"
        + "        bsearch_range_value_table(c, combining_class_table)\n"
        + "    }\n")

    f.write("""
}

""")

def remove_from_wtable(wtable, val):
    wtable_out = []
    while wtable:
        if wtable[0][1] < val:
            wtable_out.append(wtable.pop(0))
        elif wtable[0][0] > val:
            break
        else:
            (wt_lo, wt_hi, width, width_cjk) = wtable.pop(0)
            if wt_lo == wt_hi == val:
                continue
            elif wt_lo == val:
                wtable_out.append((wt_lo+1, wt_hi, width, width_cjk))
            elif wt_hi == val:
                wtable_out.append((wt_lo, wt_hi-1, width, width_cjk))
            else:
                wtable_out.append((wt_lo, val-1, width, width_cjk))
                wtable_out.append((val+1, wt_hi, width, width_cjk))
    if wtable:
        wtable_out.extend(wtable)
    return wtable_out



def optimize_width_table(wtable):
    wtable_out = []
    w_this = wtable.pop(0)
    while wtable:
        if w_this[1] == wtable[0][0] - 1 and w_this[2:3] == wtable[0][2:3]:
            w_tmp = wtable.pop(0)
            w_this = (w_this[0], w_tmp[1], w_tmp[2], w_tmp[3])
        else:
            wtable_out.append(w_this)
            w_this = wtable.pop(0)
    wtable_out.append(w_this)
    return wtable_out

if __name__ == "__main__":
    r = "tables.rs"
    if os.path.exists(r):
        os.remove(r)
    with open(r, "w") as rf:
        # write the file's preamble
        rf.write(preamble)

        # download and parse all the data
        fetch("ReadMe.txt")
        with open("ReadMe.txt") as readme:
            pattern = "for Version (\d+)\.(\d+)\.(\d+) of the Unicode"
            unicode_version = re.search(pattern, readme.read()).groups()
        rf.write("""
/// The version of [Unicode](http://www.unicode.org/)
/// that the unicode parts of `CharExt` and `UnicodeStrPrelude` traits are based on.
pub const UNICODE_VERSION: (u64, u64, u64) = (%s, %s, %s);
""" % unicode_version)
        (canon_decomp, compat_decomp, gencats, combines,
                lowerupper, upperlower) = load_unicode_data("UnicodeData.txt")
        want_derived = ["XID_Start", "XID_Continue", "Alphabetic", "Lowercase", "Uppercase"]
        derived = load_properties("DerivedCoreProperties.txt", want_derived)
        scripts = load_properties("Scripts.txt", [])
        props = load_properties("PropList.txt",
                ["White_Space", "Join_Control", "Noncharacter_Code_Point"])
        norm_props = load_properties("DerivedNormalizationProps.txt",
                     ["Full_Composition_Exclusion"])

        # bsearch_range_table is used in all the property modules below
        emit_bsearch_range_table(rf)

        # category tables
        for (name, cat, pfuns) in ("general_category", gencats, ["N", "Cc"]), \
                                  ("derived_property", derived, want_derived), \
                                  ("property", props, ["White_Space"]):
            emit_property_module(rf, name, cat, pfuns)

        # normalizations and conversions module
        emit_norm_module(rf, canon_decomp, compat_decomp, combines, norm_props)
        emit_conversions_module(rf, lowerupper, upperlower)

        ### character width module
        width_table = []
        for zwcat in ["Me", "Mn", "Cf"]:
            width_table.extend(map(lambda (lo, hi): (lo, hi, 0, 0), gencats[zwcat]))
        width_table.append((4448, 4607, 0, 0))

        # get widths, except those that are explicitly marked zero-width above
        ea_widths = load_east_asian_width(["W", "F", "A"], ["Me", "Mn", "Cf"])
        # these are doublewidth
        for dwcat in ["W", "F"]:
            width_table.extend(map(lambda (lo, hi): (lo, hi, 2, 2), ea_widths[dwcat]))
        width_table.extend(map(lambda (lo, hi): (lo, hi, 1, 2), ea_widths["A"]))

        width_table.sort(key=lambda w: w[0])

        # soft hyphen is not zero width in preformatted text; it's used to indicate
        # a hyphen inserted to facilitate a linebreak.
        width_table = remove_from_wtable(width_table, 173)

        # optimize the width table by collapsing adjacent entities when possible
        width_table = optimize_width_table(width_table)
        emit_charwidth_module(rf, width_table)

        ### grapheme cluster module
        # from http://www.unicode.org/reports/tr29/#Grapheme_Cluster_Break_Property_Values
        grapheme_cats = load_properties("auxiliary/GraphemeBreakProperty.txt", [])

        # Control
        #  Note 1:
        # This category also includes Cs (surrogate codepoints), but Rust's `char`s are
        # Unicode Scalar Values only, and surrogates are thus invalid `char`s.
        # Thus, we have to remove Cs from the Control category
        #  Note 2:
        # 0x0a and 0x0d (CR and LF) are not in the Control category for Graphemes.
        # However, the Graphemes iterator treats these as a special case, so they
        # should be included in grapheme_cats["Control"] for our implementation.
        grapheme_cats["Control"] = group_cat(list(
            (set(ungroup_cat(grapheme_cats["Control"]))
             | set(ungroup_cat(grapheme_cats["CR"]))
             | set(ungroup_cat(grapheme_cats["LF"])))
            - set(ungroup_cat([surrogate_codepoints]))))
        del(grapheme_cats["CR"])
        del(grapheme_cats["LF"])

        grapheme_table = []
        for cat in grapheme_cats:
            grapheme_table.extend([(x, y, cat) for (x, y) in grapheme_cats[cat]])
        grapheme_table.sort(key=lambda w: w[0])
        emit_break_module(rf, grapheme_table, grapheme_cats.keys(), "grapheme")
        rf.write("\n")

        word_cats = load_properties("auxiliary/WordBreakProperty.txt", [])
        word_table = []
        for cat in word_cats:
            word_table.extend([(x, y, cat) for (x, y) in word_cats[cat]])
        word_table.sort(key=lambda w: w[0])
        emit_break_module(rf, word_table, word_cats.keys(), "word")
