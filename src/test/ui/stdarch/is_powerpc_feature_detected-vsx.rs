// only-powerpc

#[cfg(target_arch = "powerpc")]
fn main() {
    is_powerpc_feature_detected!("vsx");
    //~^ ERROR use of unstable library feature
}

#[cfg(not(target_arch = "powerpc"))]
fn main() {}
