#! /bin/sh

# Copyright 2014 The Rust Project Developers. See the COPYRIGHT
# file at the top-level directory of this distribution and at
# http://rust-lang.org/COPYRIGHT.
#
# Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
# http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
# <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
# option.

set -e

clean_tmp() {
    if [ -n "$g_outf" ]; then
        echo "ERROR: failed to create \"$g_outf\""
    fi
    if [ -d "$g_tmpdir" -a -O "$g_tmpdir" ]; then
        echo "Cleanup: rm -rf \"$g_tmpdir\""
        rm -rf "$g_tmpdir" && exit 0
    fi
    echo "WARNING: could not remove \"$g_tmpdir\""
}

trap clean_tmp EXIT;

check_file() {
    infile=$1
    if [ ! -r "$infile" ]; then
        echo "ERROR: could not find file \"$infile\" which should be in the top directory."
        echo "Did you run this script in the top directory?"
        exit 10
    fi
}

git_final_commit() {
    infile=$1
    echo "// git log -n1 '$infile'"
    # don't use "--" so that git returns an error
    git log -n1 "$infile" > /dev/null || { echo "ERROR: failed to run git" >&2 ; exit 30; }
    git log -n1 "$infile" | while read line; do
        if [ -n "$line" ]; then
            echo "// $line"
        else
            echo //
        fi
    done
    echo
}

generate() {
    g_outf=$1

    tmpoutf=$g_tmpdir/$g_outf
    tmpcomb=$g_tmpdir/combined.txt
    tmpdeflate=$g_tmpdir/deflate.dat
    tmpod=$g_tmpdir/deflate.od.txt
    tmparr=$g_tmpdir/deflate.od.sed.txt

    mkdir -p ${tmpoutf%/*}

    check_file COPYRIGHT
    check_file LICENSE-MIT
    check_file LICENSE-APACHE
    check_file AUTHORS.txt

    # Open a temporary output file.
    # It will be (atomically) renamed to the final output file.
    exec 3>"$tmpoutf"
    cat >&3 <<EOS
// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option.

EOS
    echo "// This file (text.rs) was generated by src/etc/copyright2rs.sh at" >&3
    echo "// $(date -R)" >&3
    echo >&3

    git_final_commit COPYRIGHT >&3
    git_final_commit AUTHORS.txt >&3

    echo "pub const COPYRIGHT: &'static [u8] = &[" >&3

    # Process each step w/o pipe so that "-e" can catch an error in intermediate step.s
    cat > "$tmpcomb" <<EOS
Four license documents COPYRIGHT, LICENSE-MIT, LICENSE-APACHE, and
AUTHORS.txt combined as one:

EOS
    for f in COPYRIGHT LICENSE-MIT LICENSE-APACHE AUTHORS.txt; do
        echo "---- $f ----" >> "$tmpcomb"
        cat "$f" >> "$tmpcomb"
        echo >> "$tmpcomb"
    done
    python -c 'import sys,zlib; sys.stdout.write(zlib.compress(sys.stdin.read(), 9))' \
        < "$tmpcomb" > "$tmpdeflate"
    od -t x1 -A n < "$tmpdeflate" > "$tmpod"
    sed -re 's/ (\w+)/0x\1,/g' < "$tmpod" >&3
    echo "];" >&3
    exec 3<&-

    mv "$tmpoutf" "$g_outf"
    echo "Successfully generated \"$g_outf\""
    g_outf=''
}

main() {
    g_tmpdir=$(mktemp -d) || { echo "ERROR: mktemp(1) failed to create a temp directory"; exit 5; }
    echo "Created a temp dir \"$g_tmpdir\""

    generate src/librustc_trans/driver/license/text.rs
    generate src/librustdoc/license/text.rs
}

main
