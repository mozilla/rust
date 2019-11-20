// ignore-tidy-tab

#![deny(non_printable_ascii)]

// Tests if warning is emitted if a string literal contains
// non-printable ascii characters

// edit this test in a hex editor because some
// symbols probably won't be printed in your editor
fn main() {
    let _ = "00abc"; //~ ERROR non-printable ASCII character in literal
    let _ = "lorem ipsum\n\t";
    let _ = b"00"; //~ ERROR non-printable ASCII character in literal
    let _ = br"00\n"; //~ ERROR non-printable ASCII character in literal
    let _ = "00\n"; //~ ERROR non-printable ASCII character in literal
    let _ = b"Hello,
    \tworld!";
    let _ = r"00\n"; //~ ERROR non-printable ASCII character in literal
    let _ = r"	 	"; //~ ERROR non-printable ASCII character in literal
    let _ = b""; //~ ERROR non-printable ASCII character in literal
    let _ = br"	 ";
}
