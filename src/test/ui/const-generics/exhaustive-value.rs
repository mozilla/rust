// revisions: full min
#![cfg_attr(full, feature(const_generics))]
#![cfg_attr(full, allow(incomplete_features))]
#![cfg_attr(min, feature(min_const_generics))]

trait Foo<const N: u8> {
    fn test() {}
}
impl Foo<0> for () {}
impl Foo<1> for () {}
impl Foo<2> for () {}
impl Foo<3> for () {}
impl Foo<4> for () {}
impl Foo<5> for () {}
impl Foo<6> for () {}
impl Foo<7> for () {}
impl Foo<8> for () {}
impl Foo<9> for () {}
impl Foo<10> for () {}
impl Foo<11> for () {}
impl Foo<12> for () {}
impl Foo<13> for () {}
impl Foo<14> for () {}
impl Foo<15> for () {}
impl Foo<16> for () {}
impl Foo<17> for () {}
impl Foo<18> for () {}
impl Foo<19> for () {}
impl Foo<20> for () {}
impl Foo<21> for () {}
impl Foo<22> for () {}
impl Foo<23> for () {}
impl Foo<24> for () {}
impl Foo<25> for () {}
impl Foo<26> for () {}
impl Foo<27> for () {}
impl Foo<28> for () {}
impl Foo<29> for () {}
impl Foo<30> for () {}
impl Foo<31> for () {}
impl Foo<32> for () {}
impl Foo<33> for () {}
impl Foo<34> for () {}
impl Foo<35> for () {}
impl Foo<36> for () {}
impl Foo<37> for () {}
impl Foo<38> for () {}
impl Foo<39> for () {}
impl Foo<40> for () {}
impl Foo<41> for () {}
impl Foo<42> for () {}
impl Foo<43> for () {}
impl Foo<44> for () {}
impl Foo<45> for () {}
impl Foo<46> for () {}
impl Foo<47> for () {}
impl Foo<48> for () {}
impl Foo<49> for () {}
impl Foo<50> for () {}
impl Foo<51> for () {}
impl Foo<52> for () {}
impl Foo<53> for () {}
impl Foo<54> for () {}
impl Foo<55> for () {}
impl Foo<56> for () {}
impl Foo<57> for () {}
impl Foo<58> for () {}
impl Foo<59> for () {}
impl Foo<60> for () {}
impl Foo<61> for () {}
impl Foo<62> for () {}
impl Foo<63> for () {}
impl Foo<64> for () {}
impl Foo<65> for () {}
impl Foo<66> for () {}
impl Foo<67> for () {}
impl Foo<68> for () {}
impl Foo<69> for () {}
impl Foo<70> for () {}
impl Foo<71> for () {}
impl Foo<72> for () {}
impl Foo<73> for () {}
impl Foo<74> for () {}
impl Foo<75> for () {}
impl Foo<76> for () {}
impl Foo<77> for () {}
impl Foo<78> for () {}
impl Foo<79> for () {}
impl Foo<80> for () {}
impl Foo<81> for () {}
impl Foo<82> for () {}
impl Foo<83> for () {}
impl Foo<84> for () {}
impl Foo<85> for () {}
impl Foo<86> for () {}
impl Foo<87> for () {}
impl Foo<88> for () {}
impl Foo<89> for () {}
impl Foo<90> for () {}
impl Foo<91> for () {}
impl Foo<92> for () {}
impl Foo<93> for () {}
impl Foo<94> for () {}
impl Foo<95> for () {}
impl Foo<96> for () {}
impl Foo<97> for () {}
impl Foo<98> for () {}
impl Foo<99> for () {}
impl Foo<100> for () {}
impl Foo<101> for () {}
impl Foo<102> for () {}
impl Foo<103> for () {}
impl Foo<104> for () {}
impl Foo<105> for () {}
impl Foo<106> for () {}
impl Foo<107> for () {}
impl Foo<108> for () {}
impl Foo<109> for () {}
impl Foo<110> for () {}
impl Foo<111> for () {}
impl Foo<112> for () {}
impl Foo<113> for () {}
impl Foo<114> for () {}
impl Foo<115> for () {}
impl Foo<116> for () {}
impl Foo<117> for () {}
impl Foo<118> for () {}
impl Foo<119> for () {}
impl Foo<120> for () {}
impl Foo<121> for () {}
impl Foo<122> for () {}
impl Foo<123> for () {}
impl Foo<124> for () {}
impl Foo<125> for () {}
impl Foo<126> for () {}
impl Foo<127> for () {}
impl Foo<128> for () {}
impl Foo<129> for () {}
impl Foo<130> for () {}
impl Foo<131> for () {}
impl Foo<132> for () {}
impl Foo<133> for () {}
impl Foo<134> for () {}
impl Foo<135> for () {}
impl Foo<136> for () {}
impl Foo<137> for () {}
impl Foo<138> for () {}
impl Foo<139> for () {}
impl Foo<140> for () {}
impl Foo<141> for () {}
impl Foo<142> for () {}
impl Foo<143> for () {}
impl Foo<144> for () {}
impl Foo<145> for () {}
impl Foo<146> for () {}
impl Foo<147> for () {}
impl Foo<148> for () {}
impl Foo<149> for () {}
impl Foo<150> for () {}
impl Foo<151> for () {}
impl Foo<152> for () {}
impl Foo<153> for () {}
impl Foo<154> for () {}
impl Foo<155> for () {}
impl Foo<156> for () {}
impl Foo<157> for () {}
impl Foo<158> for () {}
impl Foo<159> for () {}
impl Foo<160> for () {}
impl Foo<161> for () {}
impl Foo<162> for () {}
impl Foo<163> for () {}
impl Foo<164> for () {}
impl Foo<165> for () {}
impl Foo<166> for () {}
impl Foo<167> for () {}
impl Foo<168> for () {}
impl Foo<169> for () {}
impl Foo<170> for () {}
impl Foo<171> for () {}
impl Foo<172> for () {}
impl Foo<173> for () {}
impl Foo<174> for () {}
impl Foo<175> for () {}
impl Foo<176> for () {}
impl Foo<177> for () {}
impl Foo<178> for () {}
impl Foo<179> for () {}
impl Foo<180> for () {}
impl Foo<181> for () {}
impl Foo<182> for () {}
impl Foo<183> for () {}
impl Foo<184> for () {}
impl Foo<185> for () {}
impl Foo<186> for () {}
impl Foo<187> for () {}
impl Foo<188> for () {}
impl Foo<189> for () {}
impl Foo<190> for () {}
impl Foo<191> for () {}
impl Foo<192> for () {}
impl Foo<193> for () {}
impl Foo<194> for () {}
impl Foo<195> for () {}
impl Foo<196> for () {}
impl Foo<197> for () {}
impl Foo<198> for () {}
impl Foo<199> for () {}
impl Foo<200> for () {}
impl Foo<201> for () {}
impl Foo<202> for () {}
impl Foo<203> for () {}
impl Foo<204> for () {}
impl Foo<205> for () {}
impl Foo<206> for () {}
impl Foo<207> for () {}
impl Foo<208> for () {}
impl Foo<209> for () {}
impl Foo<210> for () {}
impl Foo<211> for () {}
impl Foo<212> for () {}
impl Foo<213> for () {}
impl Foo<214> for () {}
impl Foo<215> for () {}
impl Foo<216> for () {}
impl Foo<217> for () {}
impl Foo<218> for () {}
impl Foo<219> for () {}
impl Foo<220> for () {}
impl Foo<221> for () {}
impl Foo<222> for () {}
impl Foo<223> for () {}
impl Foo<224> for () {}
impl Foo<225> for () {}
impl Foo<226> for () {}
impl Foo<227> for () {}
impl Foo<228> for () {}
impl Foo<229> for () {}
impl Foo<230> for () {}
impl Foo<231> for () {}
impl Foo<232> for () {}
impl Foo<233> for () {}
impl Foo<234> for () {}
impl Foo<235> for () {}
impl Foo<236> for () {}
impl Foo<237> for () {}
impl Foo<238> for () {}
impl Foo<239> for () {}
impl Foo<240> for () {}
impl Foo<241> for () {}
impl Foo<242> for () {}
impl Foo<243> for () {}
impl Foo<244> for () {}
impl Foo<245> for () {}
impl Foo<246> for () {}
impl Foo<247> for () {}
impl Foo<248> for () {}
impl Foo<249> for () {}
impl Foo<250> for () {}
impl Foo<251> for () {}
impl Foo<252> for () {}
impl Foo<253> for () {}
impl Foo<254> for () {}
impl Foo<255> for () {}

fn foo<const N: u8>() {
    <() as Foo<N>>::test() //~ ERROR the trait bound `(): Foo<N>`
}

fn main() {
    foo::<7>();
}
