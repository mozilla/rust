// Regression test for issue #1362
//   (without out that fix the location will be bogus)
fn main() {
  let x: uint = 20; //! ERROR mismatched types
}
// NOTE: Do not add any extra lines as the line number the error is
// on is significant; an error later in the source file might not
// trigger the bug.
