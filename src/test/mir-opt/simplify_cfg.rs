// Test that the goto chain starting from bb0 is collapsed.

fn main() {
    loop {
        if bar() {
            break;
        }
    }
}

#[inline(never)]
fn bar() -> bool {
    true
}

// END RUST SOURCE
// START rustc.main.SimplifyCfg-initial.before.mir
//     bb0: {
//         goto -> bb1;
//     }
//     bb1: {
//         falseUnwind -> [real: bb2, cleanup: bb11];
//     }
//     ...
//     bb9: {
//         ...
//         goto -> bb1;
//     }
// END rustc.main.SimplifyCfg-initial.before.mir
// START rustc.main.SimplifyCfg-initial.after.mir
//     bb0: {
//         falseUnwind -> [real: bb1, cleanup: bb6];
//     }
//     ...
//     bb4: {
//         ...
//         goto -> bb0;
//     }
// END rustc.main.SimplifyCfg-initial.after.mir
// START rustc.main.SimplifyCfg-early-opt.before.mir
//     bb0: {
//         goto -> bb1;
//     }
//     bb1: {
//         StorageLive(_2);
//         _2 = const bar() -> bb2;
//     }
// END rustc.main.SimplifyCfg-early-opt.before.mir
// START rustc.main.SimplifyCfg-early-opt.after.mir
//     bb0: {
//         StorageLive(_2);
//         _2 = const bar() -> bb1;
//     }
// END rustc.main.SimplifyCfg-early-opt.after.mir
