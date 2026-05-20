use super::*;

fn emit_named_intrinsic_test_ir(source_name: &str, body: &str) -> String {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual_trusted_syslib(source_name, body);
    emit_minimal_main_ir(&session, &source).unwrap()
}

#[test]
pub(super) fn named_intrinsic_int_plus_emits_add_inst() {
    let ir = emit_named_intrinsic_test_ir(
        "<mem>/named_intrinsic_int_plus.scoop",
        r#"
@file:AllowIntrinsic

package fixtures.named_intrinsic

import scoop.core.*

@Intrinsic("int_plus")
fun intrinsicIntPlus(a: Int, b: Int): Int

fun intPlus(a: Int, b: Int): Int {
    return intrinsicIntPlus(a, b)
}

fun main(): Int {
    return intPlus(5, 3)
}
"#,
    );
    let body = function_ir_matching(&ir, "named intrinsic int plus", |_, function| {
        stable_id_symbol_mentions_fqn(
            llvm_function_symbol_name(function),
            "fixtures.named_intrinsic.intPlus",
        )
    });
    assert!(
        body.contains(" add i64 "),
        "int_plus should emit add:\n{body}"
    );
    assert!(
        !body.contains("intrinsicIntPlus"),
        "int_plus should not leave a direct call to the declaration:\n{body}"
    );
}

#[test]
pub(super) fn named_intrinsic_int_div_signed_vs_unsigned_diverges() {
    let ir = emit_named_intrinsic_test_ir(
        "<mem>/named_intrinsic_int_div.scoop",
        r#"
@file:AllowIntrinsic

package fixtures.named_intrinsic

import scoop.core.*

@Intrinsic("int_div")
fun intrinsicIntDiv(a: Int, b: Int): Int

@Intrinsic("int_div")
fun intrinsicUIntDiv(a: UInt, b: UInt): UInt

fun signedDiv(a: Int, b: Int): Int {
    return intrinsicIntDiv(a, b)
}

fun unsignedDiv(a: UInt, b: UInt): UInt {
    return intrinsicUIntDiv(a, b)
}

fun main(): Int {
    if (unsignedDiv(6, 3) != 2) {
        return 1
    }
    return signedDiv(6, 3)
}
"#,
    );
    let signed_body = function_ir_matching(&ir, "named intrinsic signed div", |_, function| {
        stable_id_symbol_mentions_fqn(
            llvm_function_symbol_name(function),
            "fixtures.named_intrinsic.signedDiv",
        )
    });
    let unsigned_body = function_ir_matching(&ir, "named intrinsic unsigned div", |_, function| {
        stable_id_symbol_mentions_fqn(
            llvm_function_symbol_name(function),
            "fixtures.named_intrinsic.unsignedDiv",
        )
    });
    assert!(
        signed_body.contains(" sdiv i64 "),
        "Int.div should emit signed division:\n{signed_body}"
    );
    assert!(
        unsigned_body.contains(" udiv i64 "),
        "UInt.div should emit unsigned division:\n{unsigned_body}"
    );
}

#[test]
pub(super) fn named_intrinsic_float_eq_emits_oeq_predicate() {
    let ir = emit_named_intrinsic_test_ir(
        "<mem>/named_intrinsic_float_eq.scoop",
        r#"
@file:AllowIntrinsic

package fixtures.named_intrinsic

import scoop.core.*

@Intrinsic("float_eq")
fun intrinsicFloatEq(a: Float64, b: Float64): Bool

fun floatEq(a: Float64, b: Float64): Bool {
    return intrinsicFloatEq(a, b)
}

fun main(): Int {
    if (floatEq(1.0, 1.0)) {
        return 0
    }
    return 1
}
"#,
    );
    let body = function_ir_matching(&ir, "named intrinsic float eq", |_, function| {
        stable_id_symbol_mentions_fqn(
            llvm_function_symbol_name(function),
            "fixtures.named_intrinsic.floatEq",
        )
    });
    assert!(
        body.contains("fcmp oeq double"),
        "float_eq should emit ordered equality:\n{body}"
    );
}

#[test]
pub(super) fn named_intrinsic_int_compare_to_three_way_select() {
    let ir = emit_named_intrinsic_test_ir(
        "<mem>/named_intrinsic_int_compare_to.scoop",
        r#"
@file:AllowIntrinsic

package fixtures.named_intrinsic

import scoop.core.*

@Intrinsic("int_compare_to")
fun intrinsicIntCompareTo(a: Int, b: Int): Int

fun intCompareTo(a: Int, b: Int): Int {
    return intrinsicIntCompareTo(a, b)
}

fun main(): Int {
    return intCompareTo(5, 3)
}
"#,
    );
    let body = function_ir_matching(&ir, "named intrinsic int compareTo", |_, function| {
        stable_id_symbol_mentions_fqn(
            llvm_function_symbol_name(function),
            "fixtures.named_intrinsic.intCompareTo",
        )
    });
    assert!(
        body.contains("icmp slt i64"),
        "compareTo should emit signed less-than predicate:\n{body}"
    );
    assert!(
        body.contains("icmp eq i64"),
        "compareTo should emit equality predicate:\n{body}"
    );
    assert!(
        body.contains("select i1"),
        "compareTo should lower to select chain:\n{body}"
    );
}

#[test]
pub(super) fn named_intrinsic_bool_and_emits_and_i1_not_select() {
    let ir = emit_named_intrinsic_test_ir(
        "<mem>/named_intrinsic_bool_and.scoop",
        r#"
@file:AllowIntrinsic

package fixtures.named_intrinsic

import scoop.core.*

@Intrinsic("bool_and")
fun intrinsicBoolAnd(a: Bool, b: Bool): Bool

fun boolAnd(a: Bool, b: Bool): Bool {
    return intrinsicBoolAnd(a, b)
}

fun main(): Int {
    if (boolAnd(true, false)) {
        return 1
    }
    return 0
}
"#,
    );
    let body = function_ir_matching(&ir, "named intrinsic bool and", |_, function| {
        stable_id_symbol_mentions_fqn(
            llvm_function_symbol_name(function),
            "fixtures.named_intrinsic.boolAnd",
        )
    });
    assert!(
        body.contains(" and i1 "),
        "bool_and should emit and i1:\n{body}"
    );
    assert!(
        !body.contains("select"),
        "bool_and should not lower to select/control-flow sugar:\n{body}"
    );
}
