// Test unreachable_code lint for `try {}` block ok-wrapping. See issues #54165, #63324.

// compile-flags: --edition 2018
// check-pass
#![feature(try_blocks)]
#![warn(unreachable_code)]

fn err() -> Result<u32, ()> {
    Err(())
}

// In the following cases unreachable code is autogenerated and should not be reported.

fn test_ok_wrapped_divergent_expr_1() {
    let res: Result<u32, ()> = try {
        loop {
            err()?;
        }
    };
    println!("res: {:?}", res);
}

fn test_ok_wrapped_divergent_expr_2() {
    let _: Result<u32, ()> = try {
        return
    };
}

fn test_autogenerated_unit_after_divergent_expr() {
    let _: Result<(), ()> = try {
        return;
    };
}

// In the following cases unreachable code should be reported.

fn test_try_block_after_divergent_stmt() {
    let _: Result<u32, ()> = {
        return;

        try {
            loop {
                err()?;
            }
        }
        // ~^^^^^ WARNING unreachable expression
    };
}

fn test_wrapped_divergent_expr() {
    let _: Result<u32, ()> = {
        Err(return)
        // ~^ WARNING unreachable call
    };
}

fn test_expr_after_divergent_stmt_in_try_block() {
    let res: Result<u32, ()> = try {
        loop {
            err()?;
        }

        42
        // ~^ WARNING unreachable expression
    };
    println!("res: {:?}", res);
}

fn main() {
    test_ok_wrapped_divergent_expr_1();
    test_ok_wrapped_divergent_expr_2();
    test_autogenerated_unit_after_divergent_expr();
    test_try_block_after_divergent_stmt();
    test_wrapped_divergent_expr();
    test_expr_after_divergent_stmt_in_try_block();
}
