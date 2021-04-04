// Check to see if we can get parameters from an @argsfile file
//
// check-pass
// compile-flags: --cfg cmdline_set @{{src-base}}/commandline-argfile.args

#[cfg(not(cmdline_set))]
compile_error!("cmdline_set not set");

#[cfg(not(unbroken))]
compile_error!("unbroken not set");

fn main() {
}
