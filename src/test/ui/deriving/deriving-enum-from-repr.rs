// Test that FromRepr can be derived to convert an int-repr'd enum into its repr.

// run-pass
// gate-test-enum_as_repr

// AsRepr is auto-derived in stage2, and that is tested in the -automatic version of this test.
// ignore-stage2


#![feature(enum_as_repr)]

#![deny(unreachable_patterns)]

use std::enums::{AsRepr, FromRepr, TryFromReprError};

#[derive(AsRepr, FromRepr, Debug, PartialEq, Eq)]
#[repr(u8)]
enum PositiveNumber {
    Zero,
    One,
}

#[derive(AsRepr, FromRepr, Debug, PartialEq, Eq)]
#[repr(i8)]
enum Number {
    MinusOne = -1,
    Zero,
    One,
    Four = 4,
}

fn main() {
    let n = PositiveNumber::try_from_repr(0_u8);
    assert_eq!(n, Ok(PositiveNumber::Zero));
    let n = PositiveNumber::try_from_repr(1_u8);
    assert_eq!(n, Ok(PositiveNumber::One));
    let n = PositiveNumber::try_from_repr(3_u8);
    assert_eq!(n, Err(TryFromReprError(3_u8)));

    unsafe {
        let n = PositiveNumber::from_repr(0_u8);
        assert_eq!(n, PositiveNumber::Zero);
        let n = PositiveNumber::from_repr(1_u8);
        assert_eq!(n, PositiveNumber::One);
    }
}

#[derive(AsRepr, FromRepr)]
#[repr(u8)]
enum ExhaustiveVariants {
    Variant0,
    Variant1,
    Variant2,
    Variant3,
    Variant4,
    Variant5,
    Variant6,
    Variant7,
    Variant8,
    Variant9,
    Variant10,
    Variant11,
    Variant12,
    Variant13,
    Variant14,
    Variant15,
    Variant16,
    Variant17,
    Variant18,
    Variant19,
    Variant20,
    Variant21,
    Variant22,
    Variant23,
    Variant24,
    Variant25,
    Variant26,
    Variant27,
    Variant28,
    Variant29,
    Variant30,
    Variant31,
    Variant32,
    Variant33,
    Variant34,
    Variant35,
    Variant36,
    Variant37,
    Variant38,
    Variant39,
    Variant40,
    Variant41,
    Variant42,
    Variant43,
    Variant44,
    Variant45,
    Variant46,
    Variant47,
    Variant48,
    Variant49,
    Variant50,
    Variant51,
    Variant52,
    Variant53,
    Variant54,
    Variant55,
    Variant56,
    Variant57,
    Variant58,
    Variant59,
    Variant60,
    Variant61,
    Variant62,
    Variant63,
    Variant64,
    Variant65,
    Variant66,
    Variant67,
    Variant68,
    Variant69,
    Variant70,
    Variant71,
    Variant72,
    Variant73,
    Variant74,
    Variant75,
    Variant76,
    Variant77,
    Variant78,
    Variant79,
    Variant80,
    Variant81,
    Variant82,
    Variant83,
    Variant84,
    Variant85,
    Variant86,
    Variant87,
    Variant88,
    Variant89,
    Variant90,
    Variant91,
    Variant92,
    Variant93,
    Variant94,
    Variant95,
    Variant96,
    Variant97,
    Variant98,
    Variant99,
    Variant100,
    Variant101,
    Variant102,
    Variant103,
    Variant104,
    Variant105,
    Variant106,
    Variant107,
    Variant108,
    Variant109,
    Variant110,
    Variant111,
    Variant112,
    Variant113,
    Variant114,
    Variant115,
    Variant116,
    Variant117,
    Variant118,
    Variant119,
    Variant120,
    Variant121,
    Variant122,
    Variant123,
    Variant124,
    Variant125,
    Variant126,
    Variant127,
    Variant128,
    Variant129,
    Variant130,
    Variant131,
    Variant132,
    Variant133,
    Variant134,
    Variant135,
    Variant136,
    Variant137,
    Variant138,
    Variant139,
    Variant140,
    Variant141,
    Variant142,
    Variant143,
    Variant144,
    Variant145,
    Variant146,
    Variant147,
    Variant148,
    Variant149,
    Variant150,
    Variant151,
    Variant152,
    Variant153,
    Variant154,
    Variant155,
    Variant156,
    Variant157,
    Variant158,
    Variant159,
    Variant160,
    Variant161,
    Variant162,
    Variant163,
    Variant164,
    Variant165,
    Variant166,
    Variant167,
    Variant168,
    Variant169,
    Variant170,
    Variant171,
    Variant172,
    Variant173,
    Variant174,
    Variant175,
    Variant176,
    Variant177,
    Variant178,
    Variant179,
    Variant180,
    Variant181,
    Variant182,
    Variant183,
    Variant184,
    Variant185,
    Variant186,
    Variant187,
    Variant188,
    Variant189,
    Variant190,
    Variant191,
    Variant192,
    Variant193,
    Variant194,
    Variant195,
    Variant196,
    Variant197,
    Variant198,
    Variant199,
    Variant200,
    Variant201,
    Variant202,
    Variant203,
    Variant204,
    Variant205,
    Variant206,
    Variant207,
    Variant208,
    Variant209,
    Variant210,
    Variant211,
    Variant212,
    Variant213,
    Variant214,
    Variant215,
    Variant216,
    Variant217,
    Variant218,
    Variant219,
    Variant220,
    Variant221,
    Variant222,
    Variant223,
    Variant224,
    Variant225,
    Variant226,
    Variant227,
    Variant228,
    Variant229,
    Variant230,
    Variant231,
    Variant232,
    Variant233,
    Variant234,
    Variant235,
    Variant236,
    Variant237,
    Variant238,
    Variant239,
    Variant240,
    Variant241,
    Variant242,
    Variant243,
    Variant244,
    Variant245,
    Variant246,
    Variant247,
    Variant248,
    Variant249,
    Variant250,
    Variant251,
    Variant252,
    Variant253,
    Variant254,
    Variant255,
}
