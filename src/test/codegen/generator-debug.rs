// Verify debuginfo for generators:
//  - Each variant points to the file and line of its yield point
//  - The generator types and variants are marked artificial
//  - Captured vars from the source are not marked artificial
//
// ignore-tidy-linelength
// compile-flags: -C debuginfo=2 --edition=2018

#![feature(generators, generator_trait)]
use std::ops::Generator;

fn generator_test() -> impl Generator<Yield = i32, Return = ()> {
    || {
        yield 0;
        let s = String::from("foo");
        yield 1;
    }
}

async fn foo() {}
async fn async_fn_test() {
    foo().await;
    let s = String::from("foo");
    foo().await;
}

// FIXME: We need "checksum" to prevent matching with the wrong (duplicate) file
//        metadata, even when -C codegen-units=1.
// CHECK:      [[FILE:!.*]] = !DIFile(filename: "{{.*}}/generator-debug.rs", {{.*}}, checksum:

// CHECK:      [[GEN:!.*]] = !DICompositeType(tag: DW_TAG_structure_type, name: "generator-0", scope: [[FN:![0-9]*]],
// CHECK-SAME: flags: DIFlagArtificial
// CHECK:      [[FN]] = !DINamespace(name: "generator_test"
// CHECK:      [[VARIANT:!.*]] = !DICompositeType(tag: DW_TAG_variant_part, scope: [[FN]],
// CHECK-SAME: flags: DIFlagArtificial
// CHECK-SAME: discriminator: [[DISC:![0-9]*]]
// CHECK:      {{!.*}} = !DIDerivedType(tag: DW_TAG_member, name: "0", scope: [[VARIANT]],
// CHECK-SAME: file: [[FILE]], line: 13,
// CHECK-SAME: flags: DIFlagArtificial
// CHECK:      {{!.*}} = !DICompositeType(tag: DW_TAG_structure_type, name: "Unresumed", scope: [[GEN]],
// CHECK-SAME: flags: DIFlagArtificial
// CHECK:      {{!.*}} = !DIDerivedType(tag: DW_TAG_member, name: "1", scope: [[VARIANT]],
// CHECK-SAME: file: [[FILE]], line: 17,
// CHECK-SAME: flags: DIFlagArtificial
// CHECK:      {{!.*}} = !DIDerivedType(tag: DW_TAG_member, name: "2", scope: [[VARIANT]],
// CHECK-SAME: file: [[FILE]], line: 17,
// CHECK-SAME: flags: DIFlagArtificial
// CHECK:      {{!.*}} = !DIDerivedType(tag: DW_TAG_member, name: "3", scope: [[VARIANT]],
// CHECK-SAME: file: [[FILE]], line: 14,
// CHECK-SAME: flags: DIFlagArtificial
// CHECK:      {{!.*}} = !DIDerivedType(tag: DW_TAG_member, name: "4", scope: [[VARIANT]],
// CHECK-SAME: file: [[FILE]], line: 16,
// CHECK-SAME: flags: DIFlagArtificial
// CHECK:      [[S1:!.*]] = !DICompositeType(tag: DW_TAG_structure_type, name: "Suspend1", scope: [[GEN]],
// CHECK-SAME: flags: DIFlagArtificial
// CHECK:      {{!.*}} = !DIDerivedType(tag: DW_TAG_member, name: "s", scope: [[S1]]
// CHECK-NOT:  flags: DIFlagArtificial
// CHECK-SAME: )
// CHECK:      [[DISC]] = !DIDerivedType(tag: DW_TAG_member, name: "__state", scope: [[FN]],
// CHECK-SAME: flags: DIFlagArtificial

// CHECK:      [[GEN:!.*]] = !DICompositeType(tag: DW_TAG_structure_type, name: "generator-0", scope: [[FN:![0-9]*]],
// CHECK-SAME: flags: DIFlagArtificial
// CHECK:      [[FN]] = !DINamespace(name: "async_fn_test"
// CHECK:      [[VARIANT:!.*]] = !DICompositeType(tag: DW_TAG_variant_part, scope: [[FN]],
// CHECK-SAME: flags: DIFlagArtificial
// CHECK-SAME: discriminator: [[DISC:![0-9]*]]
// CHECK:      {{!.*}} = !DIDerivedType(tag: DW_TAG_member, name: "0", scope: [[VARIANT]],
// CHECK-SAME: file: [[FILE]], line: 21,
// CHECK-SAME: flags: DIFlagArtificial
// CHECK:      {{!.*}} = !DIDerivedType(tag: DW_TAG_member, name: "1", scope: [[VARIANT]],
// CHECK-SAME: file: [[FILE]], line: 25,
// CHECK-SAME: flags: DIFlagArtificial
// CHECK:      {{!.*}} = !DIDerivedType(tag: DW_TAG_member, name: "2", scope: [[VARIANT]],
// CHECK-SAME: file: [[FILE]], line: 25,
// CHECK-SAME: flags: DIFlagArtificial
// CHECK:      {{!.*}} = !DIDerivedType(tag: DW_TAG_member, name: "3", scope: [[VARIANT]],
// CHECK-SAME: file: [[FILE]], line: 22,
// CHECK-SAME: flags: DIFlagArtificial
// CHECK:      {{!.*}} = !DIDerivedType(tag: DW_TAG_member, name: "4", scope: [[VARIANT]],
// CHECK-SAME: file: [[FILE]], line: 24,
// CHECK-SAME: flags: DIFlagArtificial
// CHECK:      [[S1:!.*]] = !DICompositeType(tag: DW_TAG_structure_type, name: "Suspend1", scope: [[GEN]],
// CHECK-SAME: flags: DIFlagArtificial
// CHECK:      {{!.*}} = !DIDerivedType(tag: DW_TAG_member, name: "s", scope: [[S1]]
// CHECK-NOT:  flags: DIFlagArtificial
// CHECK-SAME: )
// CHECK:      [[DISC]] = !DIDerivedType(tag: DW_TAG_member, name: "__state", scope: [[FN]],
// CHECK-SAME: flags: DIFlagArtificial

fn main() {
    let _dummy = generator_test();
    let _dummy = async_fn_test();
}
