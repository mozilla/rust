// run-pass
#![feature(format_args_capture)]

fn main() {
    named_argument_takes_precedence_to_captured();
    panic_with_single_argument_does_not_get_formatted();
    panic_with_multiple_arguments_is_formatted();
    formatting_parameters_can_be_captured();
}

fn named_argument_takes_precedence_to_captured() {
    let foo = "captured";
    let s = format!("{foo}", foo="named");
    assert_eq!(&s, "named");

    let s = format!("{foo}-{foo}-{foo}", foo="named");
    assert_eq!(&s, "named-named-named");

    let s = format!("{}-{bar}-{foo}", "positional", bar="named");
    assert_eq!(&s, "positional-named-captured");
}

fn panic_with_single_argument_does_not_get_formatted() {
    // panic! with a single argument does not perform string formatting.
    // RFC #2795 suggests that this may need to change so that captured arguments are formatted.
    // For stability reasons this will need to part of an edition change.

    let msg = std::panic::catch_unwind(|| {
        panic!("{foo}");
    }).unwrap_err();

    assert_eq!(msg.downcast_ref::<&str>(), Some(&"{foo}"))
}

fn panic_with_multiple_arguments_is_formatted() {
    let foo = "captured";

    let msg = std::panic::catch_unwind(|| {
        panic!("{}-{bar}-{foo}", "positional", bar="named");
    }).unwrap_err();

    assert_eq!(msg.downcast_ref::<String>(), Some(&"positional-named-captured".to_string()))
}

fn formatting_parameters_can_be_captured() {
    let width = 9;
    let precision = 3;

    let x = 7.0;

    let s = format!("{x:width$}");
    assert_eq!(&s, "        7");

    let s = format!("{x:<width$}");
    assert_eq!(&s, "7        ");

    let s = format!("{x:-^width$}");
    assert_eq!(&s, "----7----");

    let s = format!("{x:-^width$.precision$}");
    assert_eq!(&s, "--7.000--");
}
