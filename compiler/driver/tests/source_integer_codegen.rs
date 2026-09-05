//! Source-to-LLVM guard for M22's fixed-width integer backend contract.

#[path = "source_integer_codegen/llvm_contract.rs"]
mod llvm_contract;
#[path = "source_integer_codegen/mir_contract.rs"]
mod mir_contract;
#[path = "source_integer_codegen/pipeline.rs"]
mod pipeline;

#[derive(Clone, Copy)]
struct IntegerCase {
    function: &'static str,
    mir_kind: scoop_mir::IntegerKind,
    lir_kind: scoop_lir::IntegerKind,
    llvm_type: &'static str,
}

const INTEGER_CASES: [IntegerCase; 8] = [
    IntegerCase {
        function: "signed8Ops",
        mir_kind: scoop_mir::IntegerKind::SIGNED_8,
        lir_kind: scoop_lir::IntegerKind::SIGNED_8,
        llvm_type: "i8",
    },
    IntegerCase {
        function: "signed16Ops",
        mir_kind: scoop_mir::IntegerKind::SIGNED_16,
        lir_kind: scoop_lir::IntegerKind::SIGNED_16,
        llvm_type: "i16",
    },
    IntegerCase {
        function: "signed32Ops",
        mir_kind: scoop_mir::IntegerKind::SIGNED_32,
        lir_kind: scoop_lir::IntegerKind::SIGNED_32,
        llvm_type: "i32",
    },
    IntegerCase {
        function: "signed64Ops",
        mir_kind: scoop_mir::IntegerKind::SIGNED_64,
        lir_kind: scoop_lir::IntegerKind::SIGNED_64,
        llvm_type: "i64",
    },
    IntegerCase {
        function: "unsigned8Ops",
        mir_kind: scoop_mir::IntegerKind::UNSIGNED_8,
        lir_kind: scoop_lir::IntegerKind::UNSIGNED_8,
        llvm_type: "i8",
    },
    IntegerCase {
        function: "unsigned16Ops",
        mir_kind: scoop_mir::IntegerKind::UNSIGNED_16,
        lir_kind: scoop_lir::IntegerKind::UNSIGNED_16,
        llvm_type: "i16",
    },
    IntegerCase {
        function: "unsigned32Ops",
        mir_kind: scoop_mir::IntegerKind::UNSIGNED_32,
        lir_kind: scoop_lir::IntegerKind::UNSIGNED_32,
        llvm_type: "i32",
    },
    IntegerCase {
        function: "unsigned64Ops",
        mir_kind: scoop_mir::IntegerKind::UNSIGNED_64,
        lir_kind: scoop_lir::IntegerKind::UNSIGNED_64,
        llvm_type: "i64",
    },
];

fn integer_program() -> String {
    let mut source = String::new();
    for case in INTEGER_CASES {
        let function = case.function;
        let ty = case.mir_kind.canonical_name();
        source.push_str(&format!(
            "\
fun {function}(lhs: {ty}, rhs: {ty}, count: Long): {ty} {{
    val quotient: {ty} = lhs / rhs
    val remainder: {ty} = lhs % rhs
    val wrapped: {ty} = (quotient + remainder) * lhs - rhs
    val negated: {ty} = -wrapped
    return negated shl count
}}

"
        ));
    }
    source.push_str("fun main() {}\n");
    source
}

#[test]
fn source_integer_division_reaches_only_guarded_llvm_blocks() {
    let (mir, lir, llvm) = pipeline::lower_program(&integer_program());
    for case in INTEGER_CASES {
        let function = mir
            .functions
            .iter()
            .map(|(_, function)| function)
            .find(|function| function.name == case.function)
            .unwrap_or_else(|| panic!("{} reaches MIR", case.function));
        assert_eq!(function.return_ty, scoop_mir::Type::Integer(case.mir_kind));
        mir_contract::assert_contract(function, case);
        llvm_contract::assert_contract(&lir, &llvm, function, case);
    }
}
