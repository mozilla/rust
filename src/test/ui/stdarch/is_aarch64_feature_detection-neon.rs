// only-aarch64

#[cfg(target_arch = "aarch64")]
fn main() {
    is_aarch64_feature_detected!("neon");
    //~^ ERROR use of unstable library feature
}

#[cfg(not(target_arch = "aarch64"))]
fn main() {}
