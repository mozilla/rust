// We must mark a variable whose initialization fails due to an
// abort statement as StorageDead.

// EMIT_MIR issue_49232.main.mir_map.0.mir
fn main() {
    loop {
        let beacon = {
            match true {
                false => 4,
                true => break,
            }
        };
        drop(&beacon);
    }
}
