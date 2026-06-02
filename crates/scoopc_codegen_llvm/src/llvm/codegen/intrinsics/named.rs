//! Named intrinsic table lowering.

use inkwell::AddressSpace;
use inkwell::FloatPredicate;
use inkwell::IntPredicate;
use inkwell::types::BasicMetadataTypeEnum;
use inkwell::types::BasicTypeEnum;
use inkwell::values::BasicMetadataValueEnum;
use inkwell::values::BasicValueEnum;
use inkwell::values::FloatValue;
use inkwell::values::FunctionValue;
use inkwell::values::IntValue;
use inkwell::values::PointerValue;

use super::super::mir_body::MirLocalSlot;
use super::super::*;
use crate::effect_lowered::mir_source as mir;
use crate::intrinsics::{
    NamedIntrinsicAuditEntry, NamedIntrinsicLoweringMode, NamedIntrinsicRuntimeTy,
    named_intrinsic_audit_entry,
};
use crate::ty::{RefTypeKind, TypeId, TypeKind};

#[derive(Clone)]
struct LoweredNamedIntrinsicOperand<'ctx> {
    span: crate::span::Span,
    source_ty: Option<TypeId>,
    value: CgValue<'ctx>,
}

struct LoweredNamedIntrinsicCall<'ctx> {
    span: crate::span::Span,
    callee_span: crate::span::Span,
    operands: Vec<LoweredNamedIntrinsicOperand<'ctx>>,
    array_element_source_ty: Option<TypeId>,
}

type NamedIntrinsicIrEmissionLowerer = for<'a, 'ctx> fn(
    &mut MainCodegen<'a, 'ctx>,
    LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError>;

struct NamedIntrinsicIrRuleEntry {
    name: &'static str,
    lower: NamedIntrinsicIrEmissionLowerer,
}

#[derive(Clone, Copy)]
enum NamedIntrinsicArrayLayout {
    Inline,
    OutOfLine,
}

#[derive(Clone, Copy)]
enum NamedIntrinsicIntBinaryOp {
    Add,
    Sub,
    Mul,
    SignednessDiv,
    SignednessRem,
    And,
    Or,
    Xor,
}

#[derive(Clone, Copy)]
enum NamedIntrinsicIntUnaryOp {
    Neg,
    Pos,
    Inc,
    Dec,
    Inv,
}

#[derive(Clone, Copy)]
enum NamedIntrinsicIntShiftOp {
    Shl,
    Shr,
    UShr,
}

#[derive(Clone, Copy)]
enum NamedIntrinsicIntCompareOp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

#[derive(Clone, Copy)]
enum NamedIntrinsicFloatBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

#[derive(Clone, Copy)]
enum NamedIntrinsicFloatCompareOp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

#[derive(Clone, Copy)]
enum NamedIntrinsicBoolBinaryOp {
    And,
    Or,
    Xor,
}

const NAMED_INTRINSIC_IR_RULES: &[NamedIntrinsicIrRuleEntry] = &[
    NamedIntrinsicIrRuleEntry {
        name: "dummy_ir",
        lower: lower_dummy_ir,
    },
    NamedIntrinsicIrRuleEntry {
        name: "array_size_inline",
        lower: lower_array_size_inline,
    },
    NamedIntrinsicIrRuleEntry {
        name: "array_size_outofline",
        lower: lower_array_size_outofline,
    },
    NamedIntrinsicIrRuleEntry {
        name: "array_get_inline",
        lower: lower_array_get_inline,
    },
    NamedIntrinsicIrRuleEntry {
        name: "array_get_outofline",
        lower: lower_array_get_outofline,
    },
    NamedIntrinsicIrRuleEntry {
        name: "array_set_inline",
        lower: lower_array_set_inline,
    },
    NamedIntrinsicIrRuleEntry {
        name: "array_set_outofline",
        lower: lower_array_set_outofline,
    },
    NamedIntrinsicIrRuleEntry {
        name: "array_data_ptr_inline",
        lower: lower_array_data_ptr_inline,
    },
    NamedIntrinsicIrRuleEntry {
        name: "array_data_ptr_outofline",
        lower: lower_array_data_ptr_outofline,
    },
    NamedIntrinsicIrRuleEntry {
        name: "unsafe_mutable_array_cast",
        lower: lower_unsafe_ref_passthrough,
    },
    NamedIntrinsicIrRuleEntry {
        name: "unsafe_mutable_array_erase",
        lower: lower_unsafe_ref_passthrough,
    },
    NamedIntrinsicIrRuleEntry {
        name: "unsafe_array_cast",
        lower: lower_unsafe_ref_passthrough,
    },
    NamedIntrinsicIrRuleEntry {
        name: "unsafe_value_to_word",
        lower: lower_unsafe_value_to_word,
    },
    NamedIntrinsicIrRuleEntry {
        name: "unsafe_value_to_any",
        lower: lower_unsafe_value_to_any,
    },
    NamedIntrinsicIrRuleEntry {
        name: "unsafe_value_slot",
        lower: lower_unsafe_value_slot,
    },
    NamedIntrinsicIrRuleEntry {
        name: "int_plus",
        lower: lower_int_plus,
    },
    NamedIntrinsicIrRuleEntry {
        name: "int_minus",
        lower: lower_int_minus,
    },
    NamedIntrinsicIrRuleEntry {
        name: "int_times",
        lower: lower_int_times,
    },
    NamedIntrinsicIrRuleEntry {
        name: "int_div",
        lower: lower_int_div,
    },
    NamedIntrinsicIrRuleEntry {
        name: "int_rem",
        lower: lower_int_rem,
    },
    NamedIntrinsicIrRuleEntry {
        name: "int_unary_minus",
        lower: lower_int_unary_minus,
    },
    NamedIntrinsicIrRuleEntry {
        name: "int_unary_plus",
        lower: lower_int_unary_plus,
    },
    NamedIntrinsicIrRuleEntry {
        name: "int_inc",
        lower: lower_int_inc,
    },
    NamedIntrinsicIrRuleEntry {
        name: "int_dec",
        lower: lower_int_dec,
    },
    NamedIntrinsicIrRuleEntry {
        name: "int_and",
        lower: lower_int_and,
    },
    NamedIntrinsicIrRuleEntry {
        name: "int_or",
        lower: lower_int_or,
    },
    NamedIntrinsicIrRuleEntry {
        name: "int_xor",
        lower: lower_int_xor,
    },
    NamedIntrinsicIrRuleEntry {
        name: "int_inv",
        lower: lower_int_inv,
    },
    NamedIntrinsicIrRuleEntry {
        name: "int_shl",
        lower: lower_int_shl,
    },
    NamedIntrinsicIrRuleEntry {
        name: "int_shr",
        lower: lower_int_shr,
    },
    NamedIntrinsicIrRuleEntry {
        name: "int_ushr",
        lower: lower_int_ushr,
    },
    NamedIntrinsicIrRuleEntry {
        name: "int_lt",
        lower: lower_int_lt,
    },
    NamedIntrinsicIrRuleEntry {
        name: "int_le",
        lower: lower_int_le,
    },
    NamedIntrinsicIrRuleEntry {
        name: "int_gt",
        lower: lower_int_gt,
    },
    NamedIntrinsicIrRuleEntry {
        name: "int_ge",
        lower: lower_int_ge,
    },
    NamedIntrinsicIrRuleEntry {
        name: "int_eq",
        lower: lower_int_eq,
    },
    NamedIntrinsicIrRuleEntry {
        name: "int_ne",
        lower: lower_int_ne,
    },
    NamedIntrinsicIrRuleEntry {
        name: "int_compare_to",
        lower: lower_int_compare_to,
    },
    NamedIntrinsicIrRuleEntry {
        name: "int_hash",
        lower: lower_int_hash,
    },
    NamedIntrinsicIrRuleEntry {
        name: "float_plus",
        lower: lower_float_plus,
    },
    NamedIntrinsicIrRuleEntry {
        name: "float_minus",
        lower: lower_float_minus,
    },
    NamedIntrinsicIrRuleEntry {
        name: "float_times",
        lower: lower_float_times,
    },
    NamedIntrinsicIrRuleEntry {
        name: "float_div",
        lower: lower_float_div,
    },
    NamedIntrinsicIrRuleEntry {
        name: "float_rem",
        lower: lower_float_rem,
    },
    NamedIntrinsicIrRuleEntry {
        name: "float_unary_minus",
        lower: lower_float_unary_minus,
    },
    NamedIntrinsicIrRuleEntry {
        name: "float_unary_plus",
        lower: lower_float_unary_plus,
    },
    NamedIntrinsicIrRuleEntry {
        name: "float_lt",
        lower: lower_float_lt,
    },
    NamedIntrinsicIrRuleEntry {
        name: "float_le",
        lower: lower_float_le,
    },
    NamedIntrinsicIrRuleEntry {
        name: "float_gt",
        lower: lower_float_gt,
    },
    NamedIntrinsicIrRuleEntry {
        name: "float_ge",
        lower: lower_float_ge,
    },
    NamedIntrinsicIrRuleEntry {
        name: "float_eq",
        lower: lower_float_eq,
    },
    NamedIntrinsicIrRuleEntry {
        name: "float_ne",
        lower: lower_float_ne,
    },
    NamedIntrinsicIrRuleEntry {
        name: "float_compare_to",
        lower: lower_float_compare_to,
    },
    NamedIntrinsicIrRuleEntry {
        name: "float_abs",
        lower: lower_float_abs,
    },
    NamedIntrinsicIrRuleEntry {
        name: "float_is_nan",
        lower: lower_float_is_nan,
    },
    NamedIntrinsicIrRuleEntry {
        name: "float_is_infinite",
        lower: lower_float_is_infinite,
    },
    NamedIntrinsicIrRuleEntry {
        name: "float_hash",
        lower: lower_float_hash,
    },
    NamedIntrinsicIrRuleEntry {
        name: "bool_and",
        lower: lower_bool_and,
    },
    NamedIntrinsicIrRuleEntry {
        name: "bool_or",
        lower: lower_bool_or,
    },
    NamedIntrinsicIrRuleEntry {
        name: "bool_xor",
        lower: lower_bool_xor,
    },
    NamedIntrinsicIrRuleEntry {
        name: "bool_eq",
        lower: lower_bool_eq,
    },
    NamedIntrinsicIrRuleEntry {
        name: "bool_ne",
        lower: lower_bool_ne,
    },
    NamedIntrinsicIrRuleEntry {
        name: "bool_not",
        lower: lower_bool_not,
    },
    NamedIntrinsicIrRuleEntry {
        name: "char_to_int",
        lower: lower_char_to_int,
    },
    NamedIntrinsicIrRuleEntry {
        name: "char_hash",
        lower: lower_char_hash,
    },
    NamedIntrinsicIrRuleEntry {
        name: "char_compare_to",
        lower: lower_char_compare_to,
    },
    NamedIntrinsicIrRuleEntry {
        name: "char_equals",
        lower: lower_char_equals,
    },
    NamedIntrinsicIrRuleEntry {
        name: "char_plus_int",
        lower: lower_char_plus_int,
    },
    NamedIntrinsicIrRuleEntry {
        name: "char_minus_int",
        lower: lower_char_minus_int,
    },
    NamedIntrinsicIrRuleEntry {
        name: "char_minus_char",
        lower: lower_char_minus_char,
    },
];

fn lookup_named_intrinsic_ir_rule(name: &str) -> Option<NamedIntrinsicIrEmissionLowerer> {
    NAMED_INTRINSIC_IR_RULES
        .iter()
        .find(|entry| entry.name == name)
        .map(|entry| entry.lower)
}

fn lower_dummy_ir<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    if call.operands.len() != 1 {
        cg.panic_verified_intrinsic_contract("named intrinsic dummy_ir", "operand arity drift");
    }
    let word_ty = cg.context.custom_width_int_type(cg.host.word_bit_width());
    let value = word_ty.const_int(41, false);
    Ok(CgValue::int(
        value,
        IntTy {
            bits: cg.host.word_bit_width(),
            signed: true,
        },
    ))
}

fn lower_array_size_inline<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_array_size(call, NamedIntrinsicArrayLayout::Inline)
}

fn lower_array_size_outofline<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_array_size(call, NamedIntrinsicArrayLayout::OutOfLine)
}

fn lower_array_get_inline<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_array_get(call, NamedIntrinsicArrayLayout::Inline)
}

fn lower_array_get_outofline<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_array_get(call, NamedIntrinsicArrayLayout::OutOfLine)
}

fn lower_array_set_inline<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_array_set(call, NamedIntrinsicArrayLayout::Inline)
}

fn lower_array_set_outofline<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_array_set(call, NamedIntrinsicArrayLayout::OutOfLine)
}

fn lower_array_data_ptr_inline<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_array_data_ptr(call, NamedIntrinsicArrayLayout::Inline)
}

fn lower_array_data_ptr_outofline<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_array_data_ptr(call, NamedIntrinsicArrayLayout::OutOfLine)
}

fn lower_unsafe_ref_passthrough<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_unsafe_ref_passthrough(call)
}

fn lower_unsafe_value_to_word<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_unsafe_value_to_word(call)
}

fn lower_unsafe_value_to_any<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_unsafe_value_to_any(call)
}

fn lower_unsafe_value_slot<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_unsafe_value_slot(call)
}

fn lower_int_plus<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_int_binary(call, NamedIntrinsicIntBinaryOp::Add)
}

fn lower_int_minus<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_int_binary(call, NamedIntrinsicIntBinaryOp::Sub)
}

fn lower_int_times<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_int_binary(call, NamedIntrinsicIntBinaryOp::Mul)
}

fn lower_int_div<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_int_binary(call, NamedIntrinsicIntBinaryOp::SignednessDiv)
}

fn lower_int_rem<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_int_binary(call, NamedIntrinsicIntBinaryOp::SignednessRem)
}

fn lower_int_unary_minus<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_int_unary(call, NamedIntrinsicIntUnaryOp::Neg)
}

fn lower_int_unary_plus<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_int_unary(call, NamedIntrinsicIntUnaryOp::Pos)
}

fn lower_int_inc<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_int_unary(call, NamedIntrinsicIntUnaryOp::Inc)
}

fn lower_int_dec<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_int_unary(call, NamedIntrinsicIntUnaryOp::Dec)
}

fn lower_int_and<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_int_binary(call, NamedIntrinsicIntBinaryOp::And)
}

fn lower_int_or<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_int_binary(call, NamedIntrinsicIntBinaryOp::Or)
}

fn lower_int_xor<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_int_binary(call, NamedIntrinsicIntBinaryOp::Xor)
}

fn lower_int_inv<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_int_unary(call, NamedIntrinsicIntUnaryOp::Inv)
}

fn lower_int_shl<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_int_shift(call, NamedIntrinsicIntShiftOp::Shl)
}

fn lower_int_shr<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_int_shift(call, NamedIntrinsicIntShiftOp::Shr)
}

fn lower_int_ushr<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_int_shift(call, NamedIntrinsicIntShiftOp::UShr)
}

fn lower_int_lt<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_int_compare(call, NamedIntrinsicIntCompareOp::Lt)
}

fn lower_int_le<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_int_compare(call, NamedIntrinsicIntCompareOp::Le)
}

fn lower_int_gt<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_int_compare(call, NamedIntrinsicIntCompareOp::Gt)
}

fn lower_int_ge<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_int_compare(call, NamedIntrinsicIntCompareOp::Ge)
}

fn lower_int_eq<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_int_compare(call, NamedIntrinsicIntCompareOp::Eq)
}

fn lower_int_ne<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_int_compare(call, NamedIntrinsicIntCompareOp::Ne)
}

fn lower_int_compare_to<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_int_compare_to(call)
}

fn lower_int_hash<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_int_hash(call)
}

fn lower_float_plus<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_float_binary(call, NamedIntrinsicFloatBinaryOp::Add)
}

fn lower_float_minus<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_float_binary(call, NamedIntrinsicFloatBinaryOp::Sub)
}

fn lower_float_times<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_float_binary(call, NamedIntrinsicFloatBinaryOp::Mul)
}

fn lower_float_div<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_float_binary(call, NamedIntrinsicFloatBinaryOp::Div)
}

fn lower_float_rem<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_float_binary(call, NamedIntrinsicFloatBinaryOp::Rem)
}

fn lower_float_unary_minus<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_float_unary_minus(call)
}

fn lower_float_unary_plus<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_float_unary_plus(call)
}

fn lower_float_lt<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_float_compare(call, NamedIntrinsicFloatCompareOp::Lt)
}

fn lower_float_le<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_float_compare(call, NamedIntrinsicFloatCompareOp::Le)
}

fn lower_float_gt<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_float_compare(call, NamedIntrinsicFloatCompareOp::Gt)
}

fn lower_float_ge<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_float_compare(call, NamedIntrinsicFloatCompareOp::Ge)
}

fn lower_float_eq<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_float_compare(call, NamedIntrinsicFloatCompareOp::Eq)
}

fn lower_float_ne<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_float_compare(call, NamedIntrinsicFloatCompareOp::Ne)
}

fn lower_float_compare_to<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_float_compare_to(call)
}

fn lower_float_abs<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_float_abs(call)
}

fn lower_float_is_nan<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_float_is_nan(call)
}

fn lower_float_is_infinite<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_float_is_infinite(call)
}

fn lower_float_hash<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_float_hash(call)
}

fn lower_bool_and<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_bool_binary(call, NamedIntrinsicBoolBinaryOp::And)
}

fn lower_bool_or<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_bool_binary(call, NamedIntrinsicBoolBinaryOp::Or)
}

fn lower_bool_xor<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_bool_binary(call, NamedIntrinsicBoolBinaryOp::Xor)
}

fn lower_bool_eq<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_bool_compare(call, IntPredicate::EQ)
}

fn lower_bool_ne<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_bool_compare(call, IntPredicate::NE)
}

fn lower_bool_not<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_bool_not(call)
}

fn lower_char_to_int<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_char_to_int(call)
}

fn lower_char_hash<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_char_hash(call)
}

fn lower_char_compare_to<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_char_compare_to(call)
}

fn lower_char_equals<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_char_equals(call)
}

fn lower_char_plus_int<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_char_plus_int(call)
}

fn lower_char_minus_int<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_char_minus_int(call)
}

fn lower_char_minus_char<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_char_minus_char(call)
}

fn normalize_array_like_fqn(fqn: &str) -> Option<&'static str> {
    match fqn {
        "scoop.core.Array"
        | "scoop.core.List"
        | "scoop.collections.Set"
        | "scoop.collections.MapView" => Some("scoop.core.Array"),
        "scoop.core.MutableArray"
        | "scoop.core.MutableList"
        | "scoop.collections.MutableSet"
        | "scoop.collections.MutableMap" => Some("scoop.core.MutableArray"),
        _ => None,
    }
}

fn named_intrinsic_char_ty() -> IntTy {
    IntTy {
        bits: 32,
        signed: false,
    }
}

fn named_intrinsic_signed_i32_ty() -> IntTy {
    IntTy {
        bits: 32,
        signed: true,
    }
}

fn named_intrinsic_int_compare_predicate(
    ty: IntTy,
    op: NamedIntrinsicIntCompareOp,
) -> IntPredicate {
    match op {
        NamedIntrinsicIntCompareOp::Lt if ty.signed => IntPredicate::SLT,
        NamedIntrinsicIntCompareOp::Lt => IntPredicate::ULT,
        NamedIntrinsicIntCompareOp::Le if ty.signed => IntPredicate::SLE,
        NamedIntrinsicIntCompareOp::Le => IntPredicate::ULE,
        NamedIntrinsicIntCompareOp::Gt if ty.signed => IntPredicate::SGT,
        NamedIntrinsicIntCompareOp::Gt => IntPredicate::UGT,
        NamedIntrinsicIntCompareOp::Ge if ty.signed => IntPredicate::SGE,
        NamedIntrinsicIntCompareOp::Ge => IntPredicate::UGE,
        NamedIntrinsicIntCompareOp::Eq => IntPredicate::EQ,
        NamedIntrinsicIntCompareOp::Ne => IntPredicate::NE,
    }
}

fn named_intrinsic_float_compare_predicate(op: NamedIntrinsicFloatCompareOp) -> FloatPredicate {
    match op {
        NamedIntrinsicFloatCompareOp::Lt => FloatPredicate::OLT,
        NamedIntrinsicFloatCompareOp::Le => FloatPredicate::OLE,
        NamedIntrinsicFloatCompareOp::Gt => FloatPredicate::OGT,
        NamedIntrinsicFloatCompareOp::Ge => FloatPredicate::OGE,
        NamedIntrinsicFloatCompareOp::Eq => FloatPredicate::OEQ,
        NamedIntrinsicFloatCompareOp::Ne => FloatPredicate::UNE,
    }
}

fn named_intrinsic_float_binary_target_ty(lhs: CgTy, rhs: CgTy) -> CgTy {
    if lhs == CgTy::Float64 || rhs == CgTy::Float64 {
        CgTy::Float64
    } else {
        CgTy::Float32
    }
}

pub(in crate::llvm::codegen) fn scalar_bodyless_intrinsic_entry_name(
    fqn: &str,
) -> Option<&'static str> {
    let base = fqn
        .split("::<")
        .next()
        .unwrap_or(fqn)
        .split("$overload")
        .next()
        .unwrap_or(fqn);
    let (owner, method) = base.rsplit_once('.')?;
    if matches!(
        owner,
        "scoop.core.Int"
            | "scoop.core.UInt"
            | "scoop.core.Int8"
            | "scoop.core.Int16"
            | "scoop.core.Int32"
            | "scoop.core.Int64"
            | "scoop.core.UInt8"
            | "scoop.core.UInt16"
            | "scoop.core.UInt32"
            | "scoop.core.UInt64"
    ) {
        return match method {
            "plus" => Some("int_plus"),
            "minus" => Some("int_minus"),
            "times" => Some("int_times"),
            "div" => Some("int_div"),
            "rem" => Some("int_rem"),
            "unaryMinus" => Some("int_unary_minus"),
            "unaryPlus" => Some("int_unary_plus"),
            "inc" => Some("int_inc"),
            "dec" => Some("int_dec"),
            "and" => Some("int_and"),
            "or" => Some("int_or"),
            "xor" => Some("int_xor"),
            "inv" => Some("int_inv"),
            "shl" => Some("int_shl"),
            "shr" => Some("int_shr"),
            "compareTo" => Some("int_compare_to"),
            "lt" => Some("int_lt"),
            "le" => Some("int_le"),
            "gt" => Some("int_gt"),
            "ge" => Some("int_ge"),
            "equals" => Some("int_eq"),
            "notEquals" => Some("int_ne"),
            _ => None,
        };
    }
    if matches!(owner, "scoop.core.Float32" | "scoop.core.Float64") {
        return match method {
            "plus" => Some("float_plus"),
            "minus" => Some("float_minus"),
            "times" => Some("float_times"),
            "div" => Some("float_div"),
            "unaryMinus" => Some("float_unary_minus"),
            "compareTo" => Some("float_compare_to"),
            "lt" => Some("float_lt"),
            "le" => Some("float_le"),
            "gt" => Some("float_gt"),
            "ge" => Some("float_ge"),
            "equals" => Some("float_eq"),
            "notEquals" => Some("float_ne"),
            _ => None,
        };
    }
    if owner == "scoop.core.Bool" {
        return match method {
            "not" | "negate" => Some("bool_not"),
            "and" => Some("bool_and"),
            "or" => Some("bool_or"),
            "xor" => Some("bool_xor"),
            "equals" => Some("bool_eq"),
            "notEquals" => Some("bool_ne"),
            _ => None,
        };
    }
    if owner == "scoop.core.Char" {
        return match method {
            "toInt" => Some("char_to_int"),
            "equals" => Some("char_eq"),
            "notEquals" => Some("char_ne"),
            _ => None,
        };
    }
    None
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn try_codegen_named_intrinsic_hir_top_level_call(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
        entry_name: &str,
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let entry = self.published_named_intrinsic_entry(entry_name)?;
        let mut operands = Vec::with_capacity(args.len());
        for arg in args {
            let value = match arg {
                hir::CallArg::Positional(value) | hir::CallArg::Named { value, .. } => value,
            };
            operands.push(self.lower_named_intrinsic_hir_operand(value)?);
        }
        let call = LoweredNamedIntrinsicCall {
            span,
            callee_span,
            operands,
            array_element_source_ty: None,
        };
        self.codegen_named_intrinsic_call(entry, call).map(Some)
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn try_codegen_named_intrinsic_mir_direct_call(
        &mut self,
        span: crate::span::Span,
        entry_name: &str,
        args: &[mir::CallArg],
        body: &mir::Body,
        mir_types: &TypeStore,
        array_transport: Option<&mir::ArrayElementTransportMetadata>,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let entry = self.published_named_intrinsic_entry(entry_name)?;
        let call = self.lower_named_intrinsic_mir_call(
            span,
            args,
            body,
            mir_types,
            array_transport,
            slots,
        )?;
        self.codegen_named_intrinsic_call(entry, call).map(Some)
    }

    fn codegen_named_intrinsic_call(
        &mut self,
        entry: &NamedIntrinsicAuditEntry,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match entry.lowering_mode {
            NamedIntrinsicLoweringMode::IrEmission => {
                let lower = lookup_named_intrinsic_ir_rule(entry.name).unwrap_or_else(|| {
                    self.panic_verified_intrinsic_contract(
                        "codegen_named_intrinsic_call",
                        "missing IR emission rule",
                    )
                });
                lower(self, call)
            }
            NamedIntrinsicLoweringMode::RuntimeCall => {
                self.codegen_named_runtime_intrinsic_call(entry, call)
            }
        }
    }

    fn published_named_intrinsic_entry(
        &self,
        entry_name: &str,
    ) -> Result<&'static NamedIntrinsicAuditEntry, LlvmEmitError> {
        named_intrinsic_audit_entry(entry_name).ok_or_else(|| {
            LlvmEmitError::Frontend {
                message: format!(
                    "published named intrinsic entry `{entry_name}` is not present in the backend audit table"
                ),
            }
        })
    }

    fn lower_named_intrinsic_hir_operand(
        &mut self,
        expr: &hir::Expr,
    ) -> Result<LoweredNamedIntrinsicOperand<'ctx>, LlvmEmitError> {
        let value = self.codegen_expr(expr)?;
        let value = if let Some(cg_ty) = self.resolve_expr_cg_ty(expr) {
            self.coerce_value(expr.span, value, cg_ty)?
        } else {
            value
        };
        Ok(LoweredNamedIntrinsicOperand {
            span: expr.span,
            source_ty: Some(expr.ty),
            value,
        })
    }

    fn lower_named_intrinsic_mir_call(
        &mut self,
        span: crate::span::Span,
        args: &[mir::CallArg],
        body: &mir::Body,
        mir_types: &TypeStore,
        array_transport: Option<&mir::ArrayElementTransportMetadata>,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<LoweredNamedIntrinsicCall<'ctx>, LlvmEmitError> {
        let mut operands = Vec::with_capacity(args.len());
        for arg in args {
            operands.push(self.lower_named_intrinsic_mir_operand(arg, body, mir_types, slots)?);
        }
        Ok(LoweredNamedIntrinsicCall {
            span,
            callee_span: span,
            operands,
            array_element_source_ty: array_transport
                .and_then(|metadata| {
                    self.equivalent_codegen_type_id(mir_types, metadata.element_ty)
                })
                .or_else(|| {
                    array_transport.and_then(|metadata| {
                        self.equivalent_codegen_type_id(mir_types, metadata.element.source_ty)
                    })
                }),
        })
    }

    fn lower_named_intrinsic_mir_operand(
        &mut self,
        arg: &mir::CallArg,
        body: &mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<LoweredNamedIntrinsicOperand<'ctx>, LlvmEmitError> {
        let source_ty = self
            .mir_operand_type_id(body, &arg.value)
            .unwrap_or_else(|| {
                self.panic_verified_intrinsic_contract(
                    "lower_named_intrinsic_mir_operand",
                    "missing MIR operand type",
                )
            });
        let operand_cg = self
            .cg_ty_of_mir_type(mir_types, source_ty)
            .or_else(|| {
                self.equivalent_codegen_type_id(mir_types, source_ty)
                    .and_then(|ty| self.try_cg_ty_of_type_id(ty))
            })
            .unwrap_or_else(|| {
                panic!("lower_named_intrinsic_mir_operand: TypeStore equivalence verifier accepted unsupported named intrinsic operand codegen type")
            });
        let value =
            self.codegen_mir_operand_expected(arg.span, &arg.value, slots, Some(operand_cg))?;
        let value = if value.ty == operand_cg {
            value
        } else {
            self.coerce_value(arg.span, value, operand_cg)?
        };
        Ok(LoweredNamedIntrinsicOperand {
            span: arg.span,
            source_ty: self.equivalent_codegen_type_id(mir_types, source_ty),
            value,
        })
    }

    fn codegen_named_intrinsic_array_size(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
        layout: NamedIntrinsicArrayLayout,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(&call, 1, "named intrinsic array_size operand arity")?;
        let receiver = &call.operands[0];
        let arr_ptr = self.named_intrinsic_array_receiver_ptr(receiver, "array_size receiver")?;
        let len_i64 = self.named_intrinsic_array_len_value(call.span, arr_ptr, layout)?;
        let len_word = self.cast_int(
            len_i64,
            IntTy {
                bits: 64,
                signed: false,
            },
            IntTy {
                bits: self.host.word_bit_width(),
                signed: true,
            },
        )?;
        Ok(CgValue::int(
            len_word,
            IntTy {
                bits: self.host.word_bit_width(),
                signed: true,
            },
        ))
    }

    fn codegen_named_intrinsic_array_get(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
        layout: NamedIntrinsicArrayLayout,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(&call, 2, "named intrinsic array_get operand arity")?;
        let receiver = &call.operands[0];
        let index = &call.operands[1];
        let arr_ptr = self.named_intrinsic_array_receiver_ptr(receiver, "array_get receiver")?;
        let index_i64 = self.named_intrinsic_array_index_i64(index, "array_get index")?;
        let (_elem_ty, elem_cg) = self.named_intrinsic_array_element_cg_ty(
            call.callee_span,
            receiver,
            call.array_element_source_ty,
            "array_get element type",
        )?;
        if elem_cg == CgTy::Unit {
            return Ok(CgValue::unit());
        }

        let len_i64 = self.named_intrinsic_array_len_value(call.span, arr_ptr, layout)?;
        let current_fn = self.expect_current_function("named intrinsic array_get bounds check");
        let in_bounds_bb = self
            .context
            .append_basic_block(current_fn, "array_get_in_bounds");
        let out_of_bounds_bb = self
            .context
            .append_basic_block(current_fn, "array_get_out_of_bounds");
        let not_negative_bb = self
            .context
            .append_basic_block(current_fn, "array_get_not_negative");
        let merge_bb = self
            .context
            .append_basic_block(current_fn, "array_get_merge");

        let is_negative = self.builder.build_int_compare(
            IntPredicate::SLT,
            index_i64,
            self.context.i64_type().const_zero(),
            "array_get_negative",
        )?;
        self.builder
            .build_conditional_branch(is_negative, out_of_bounds_bb, not_negative_bb)?;

        self.builder.position_at_end(not_negative_bb);
        let is_ge_len = self.builder.build_int_compare(
            IntPredicate::SGE,
            index_i64,
            len_i64,
            "array_get_ge_len",
        )?;
        self.builder
            .build_conditional_branch(is_ge_len, out_of_bounds_bb, in_bounds_bb)?;

        let llvm_elem_ty = self.llvm_basic_type_of(call.span, elem_cg)?;
        let oob_value = match elem_cg {
            CgTy::Ref => self.llvm_gc_i8_ptr_type().const_null().into(),
            CgTy::String => self.llvm_scoop_string_ptr_type().const_null().into(),
            _ => llvm_elem_ty.const_zero(),
        };

        self.builder.position_at_end(out_of_bounds_bb);
        self.builder.build_unconditional_branch(merge_bb)?;

        self.builder.position_at_end(in_bounds_bb);
        let slot_ptr = self.named_intrinsic_array_slot_ptr(
            call.span, arr_ptr, receiver, elem_cg, index_i64, layout,
        )?;
        let loaded = self
            .builder
            .build_load(llvm_elem_ty, slot_ptr, "array_get_load")?;
        self.builder.build_unconditional_branch(merge_bb)?;

        self.builder.position_at_end(merge_bb);
        let phi = self.builder.build_phi(llvm_elem_ty, "array_get_result")?;
        phi.add_incoming(&[(&oob_value, out_of_bounds_bb), (&loaded, in_bounds_bb)]);
        let loaded = phi.as_basic_value();
        match elem_cg {
            CgTy::String => {
                let ptr = loaded.into_pointer_value();
                let str_ptr = self.builder.build_pointer_cast(
                    ptr,
                    self.llvm_scoop_string_ptr_type(),
                    "array_get_string",
                )?;
                Ok(CgValue {
                    ty: CgTy::String,
                    value: Some(str_ptr.into()),
                })
            }
            _ => self.cg_value_from_loaded(call.span, elem_cg, loaded),
        }
    }

    fn codegen_named_intrinsic_array_set(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
        layout: NamedIntrinsicArrayLayout,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(&call, 3, "named intrinsic array_set operand arity")?;
        let receiver = &call.operands[0];
        let index = &call.operands[1];
        let value_operand = &call.operands[2];
        let arr_ptr = self.named_intrinsic_array_receiver_ptr(receiver, "array_set receiver")?;
        let index_i64 = self.named_intrinsic_array_index_i64(index, "array_set index")?;
        let (elem_ty, elem_cg) = self.named_intrinsic_array_element_cg_ty(
            call.callee_span,
            receiver,
            call.array_element_source_ty,
            "array_set element type",
        )?;
        let len_i64 = self.named_intrinsic_array_len_value(call.span, arr_ptr, layout)?;
        let current_fn = self.expect_current_function("named intrinsic array_set bounds check");
        let in_bounds_bb = self
            .context
            .append_basic_block(current_fn, "array_set_in_bounds");
        let out_of_bounds_bb = self
            .context
            .append_basic_block(current_fn, "array_set_out_of_bounds");
        let not_negative_bb = self
            .context
            .append_basic_block(current_fn, "array_set_not_negative");
        let merge_bb = self
            .context
            .append_basic_block(current_fn, "array_set_merge");

        let is_negative = self.builder.build_int_compare(
            IntPredicate::SLT,
            index_i64,
            self.context.i64_type().const_zero(),
            "array_set_negative",
        )?;
        self.builder
            .build_conditional_branch(is_negative, out_of_bounds_bb, not_negative_bb)?;

        self.builder.position_at_end(not_negative_bb);
        let is_ge_len = self.builder.build_int_compare(
            IntPredicate::SGE,
            index_i64,
            len_i64,
            "array_set_ge_len",
        )?;
        self.builder
            .build_conditional_branch(is_ge_len, out_of_bounds_bb, in_bounds_bb)?;

        self.builder.position_at_end(out_of_bounds_bb);
        self.builder.build_unconditional_branch(merge_bb)?;

        self.builder.position_at_end(in_bounds_bb);
        if elem_cg != CgTy::Unit {
            let slot_ptr = self.named_intrinsic_array_slot_ptr(
                call.span, arr_ptr, receiver, elem_cg, index_i64, layout,
            )?;
            if matches!(elem_cg, CgTy::Ref | CgTy::String) {
                let value = self.coerce_value(value_operand.span, value_operand.value, elem_cg)?;
                let value = self.coerce_value(value_operand.span, value, CgTy::Ref)?;
                let value_ptr =
                    self.expect_cg_pointer(value, "named intrinsic array_set ref value");
                match layout {
                    NamedIntrinsicArrayLayout::Inline => {
                        self.store_gc_pointer_slot_with_write_barrier(
                            call.span, slot_ptr, value_ptr,
                        )?;
                    }
                    NamedIntrinsicArrayLayout::OutOfLine => {
                        self.store_out_of_line_gc_pointer_slot_with_promotion_barrier(
                            call.span, slot_ptr, value_ptr,
                        )?;
                    }
                }
            } else if matches!(elem_cg, CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_)) {
                let value = self.coerce_value(value_operand.span, value_operand.value, elem_cg)?;
                let value_ptr = self.named_intrinsic_materialize_value_ptr(
                    value_operand.span,
                    "array_set_composite_value",
                    elem_cg,
                    value,
                )?;
                let descriptor =
                    self.named_intrinsic_array_composite_descriptor(value_operand.span, elem_ty)?;
                let slot_i8 = self.named_intrinsic_array_slot_i8_ptr(
                    call.span, arr_ptr, receiver, elem_cg, index_i64, layout,
                )?;
                let slot_i8_native =
                    self.named_intrinsic_native_i8_ptr(slot_i8, "array_set_composite_dst")?;
                let src_i8 = self.builder.build_pointer_cast(
                    value_ptr,
                    self.llvm_i8_ptr_type(),
                    "array_set_composite_src",
                )?;
                let drop = self.declare_runtime_composite_drop();
                let copy = self.declare_runtime_composite_copy();
                let _ = self.build_call_preserving_gc_local_roots(
                    value_operand.span,
                    drop,
                    &[descriptor.into(), slot_i8_native.into()],
                    "array_set_composite_drop",
                )?;
                let _ = self.build_call_preserving_gc_local_roots(
                    value_operand.span,
                    copy,
                    &[descriptor.into(), slot_i8_native.into(), src_i8.into()],
                    "array_set_composite_copy",
                )?;
                for offset in self
                    .named_intrinsic_array_composite_gc_slot_offsets(value_operand.span, elem_cg)?
                {
                    let slot_gc_i8 = unsafe {
                        self.builder.build_in_bounds_gep(
                            self.context.i8_type(),
                            slot_i8,
                            &[self.context.i64_type().const_int(offset, false)],
                            "array_set_composite_gc_slot_i8",
                        )?
                    };
                    let slot_gc_ptr = self.named_intrinsic_array_slot_storage_ptr(
                        slot_gc_i8,
                        "array_set_composite_gc_slot_ptr",
                    )?;
                    let loaded = self
                        .builder
                        .build_load(
                            self.llvm_gc_i8_ptr_type(),
                            slot_gc_ptr,
                            "array_set_composite_gc_slot_load",
                        )?
                        .into_pointer_value();
                    match layout {
                        NamedIntrinsicArrayLayout::Inline => {
                            self.store_gc_pointer_slot_with_write_barrier(
                                value_operand.span,
                                slot_gc_ptr,
                                loaded,
                            )?;
                        }
                        NamedIntrinsicArrayLayout::OutOfLine => {
                            self.call_gc_promotion_barrier_for_out_of_line_value(
                                value_operand.span,
                                loaded,
                            )?;
                        }
                    }
                }
            } else {
                let value = self.coerce_value(value_operand.span, value_operand.value, elem_cg)?;
                let raw = self.expect_cg_value(value, "named intrinsic array_set scalar value");
                let _ = self.builder.build_store(slot_ptr, raw)?;
            }
        }
        self.builder.build_unconditional_branch(merge_bb)?;

        self.builder.position_at_end(merge_bb);
        Ok(CgValue::unit())
    }

    fn codegen_named_intrinsic_array_data_ptr(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
        layout: NamedIntrinsicArrayLayout,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(
            &call,
            1,
            "named intrinsic array_data_ptr operand arity",
        )?;
        let receiver = &call.operands[0];
        let arr_ptr =
            self.named_intrinsic_array_receiver_ptr(receiver, "array_data_ptr receiver")?;
        let data_base = self.named_intrinsic_array_data_base_ptr(call.span, arr_ptr, layout)?;
        let data_ptr = self.named_intrinsic_native_i8_ptr(data_base, "array_data_ptr_native")?;
        let ptr_int_ty = self.llvm_ptr_sized_int_type(Some(AddressSpace::default()));
        let raw = self
            .builder
            .build_ptr_to_int(data_ptr, ptr_int_ty, "array_data_ptr_word")?;
        Ok(CgValue::int(
            raw,
            IntTy {
                bits: self.host.word_bit_width(),
                signed: false,
            },
        ))
    }

    fn codegen_named_intrinsic_unsafe_ref_passthrough(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if call.operands.len() != 1 {
            self.panic_verified_intrinsic_contract(
                "unsafe ref passthrough named intrinsic",
                "operand arity drift",
            );
        }
        self.coerce_value(call.operands[0].span, call.operands[0].value, CgTy::Ref)
    }

    fn codegen_named_intrinsic_unsafe_value_to_any(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if call.operands.len() != 1 {
            self.panic_verified_intrinsic_contract(
                "unsafe value-to-any named intrinsic",
                "operand arity drift",
            );
        }
        let operand = &call.operands[0];
        match operand.value.ty {
            CgTy::Unit | CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref | CgTy::Enum(_) => {
                self.coerce_value(operand.span, operand.value, CgTy::Ref)
            }
            _ => Ok(CgValue {
                ty: CgTy::Ref,
                value: Some(self.llvm_gc_i8_ptr_type().const_null().into()),
            }),
        }
    }

    fn codegen_named_intrinsic_unsafe_value_to_word(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if call.operands.len() != 1 {
            self.panic_verified_intrinsic_contract(
                "unsafe value-to-word named intrinsic",
                "operand arity drift",
            );
        }
        let operand = &call.operands[0];
        let word_ty = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };
        let word_llvm_ty = self.int_type(word_ty);
        let raw = match operand.value.ty {
            CgTy::Unit | CgTy::Never => word_llvm_ty.const_zero(),
            CgTy::Bool => {
                let value = operand.value.as_bool().unwrap_or_else(|| {
                    self.panic_verified_intrinsic_contract(
                        "unsafe value-to-word named intrinsic",
                        "Bool operand payload drift",
                    )
                });
                self.builder
                    .build_int_z_extend(value, word_llvm_ty, "unsafe_bool_to_word")?
            }
            CgTy::Int(from_ty) => {
                let (value, _) = operand.value.as_int().unwrap_or_else(|| {
                    self.panic_verified_intrinsic_contract(
                        "unsafe value-to-word named intrinsic",
                        "Int operand payload drift",
                    )
                });
                self.cast_int(value, from_ty, word_ty)?
            }
            CgTy::Float32 => {
                let (value, _) = operand.value.as_float().unwrap_or_else(|| {
                    self.panic_verified_intrinsic_contract(
                        "unsafe value-to-word named intrinsic",
                        "Float32 operand payload drift",
                    )
                });
                let bits = self
                    .builder
                    .build_bit_cast(value, self.context.i32_type(), "unsafe_f32_bits")?
                    .into_int_value();
                self.cast_int(
                    bits,
                    IntTy {
                        bits: 32,
                        signed: false,
                    },
                    word_ty,
                )?
            }
            CgTy::Float64 => {
                let (value, _) = operand.value.as_float().unwrap_or_else(|| {
                    self.panic_verified_intrinsic_contract(
                        "unsafe value-to-word named intrinsic",
                        "Float64 operand payload drift",
                    )
                });
                let bits = self
                    .builder
                    .build_bit_cast(value, self.context.i64_type(), "unsafe_f64_bits")?
                    .into_int_value();
                self.cast_int(
                    bits,
                    IntTy {
                        bits: 64,
                        signed: false,
                    },
                    word_ty,
                )?
            }
            CgTy::String | CgTy::Ref => {
                let value = self.coerce_value(operand.span, operand.value, CgTy::Ref)?;
                let Some(BasicValueEnum::PointerValue(ptr)) = value.value else {
                    self.panic_verified_intrinsic_contract(
                        "unsafe value-to-word named intrinsic",
                        "Ref operand payload drift",
                    );
                };
                self.unsafe_ptr_to_word(ptr, "unsafe_ref_to_word")?
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                let ptr = self.named_intrinsic_materialize_value_ptr(
                    operand.span,
                    "unsafe_value_to_word_slot",
                    operand.value.ty,
                    operand.value,
                )?;
                self.unsafe_ptr_to_word(ptr, "unsafe_composite_to_word")?
            }
        };
        Ok(CgValue::int(raw, word_ty))
    }

    fn codegen_named_intrinsic_unsafe_value_slot(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if call.operands.len() != 1 {
            self.panic_verified_intrinsic_contract(
                "unsafe value-slot named intrinsic",
                "operand arity drift",
            );
        }
        let operand = &call.operands[0];
        let word_ty = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };
        if matches!(operand.value.ty, CgTy::Unit | CgTy::Never) {
            return Ok(CgValue::int(self.int_type(word_ty).const_zero(), word_ty));
        }
        let ptr = self.named_intrinsic_materialize_value_ptr(
            operand.span,
            "unsafe_value_slot",
            operand.value.ty,
            operand.value,
        )?;
        let raw = self.unsafe_ptr_to_word(ptr, "unsafe_value_slot_word")?;
        Ok(CgValue::int(raw, word_ty))
    }

    fn codegen_named_intrinsic_int_binary(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
        op: NamedIntrinsicIntBinaryOp,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(&call, 2, "named intrinsic int binary operand arity")?;
        let (lhs_raw, lhs_ty) =
            self.named_intrinsic_int_operand(&call.operands[0], "named intrinsic int binary lhs")?;
        let (rhs_raw, rhs_ty) =
            self.named_intrinsic_int_operand(&call.operands[1], "named intrinsic int binary rhs")?;
        let target_ty = self.named_intrinsic_int_binary_target_ty(lhs_ty, rhs_ty);
        let lhs = self.cast_int(lhs_raw, lhs_ty, target_ty)?;
        let rhs = self.cast_int(rhs_raw, rhs_ty, target_ty)?;
        let value = match op {
            NamedIntrinsicIntBinaryOp::Add => {
                self.builder.build_int_add(lhs, rhs, "intrinsic_iadd")?
            }
            NamedIntrinsicIntBinaryOp::Sub => {
                self.builder.build_int_sub(lhs, rhs, "intrinsic_isub")?
            }
            NamedIntrinsicIntBinaryOp::Mul => {
                self.builder.build_int_mul(lhs, rhs, "intrinsic_imul")?
            }
            NamedIntrinsicIntBinaryOp::SignednessDiv if target_ty.signed => self
                .builder
                .build_int_signed_div(lhs, rhs, "intrinsic_sdiv")?,
            NamedIntrinsicIntBinaryOp::SignednessDiv => {
                self.builder
                    .build_int_unsigned_div(lhs, rhs, "intrinsic_udiv")?
            }
            NamedIntrinsicIntBinaryOp::SignednessRem if target_ty.signed => self
                .builder
                .build_int_signed_rem(lhs, rhs, "intrinsic_srem")?,
            NamedIntrinsicIntBinaryOp::SignednessRem => {
                self.builder
                    .build_int_unsigned_rem(lhs, rhs, "intrinsic_urem")?
            }
            NamedIntrinsicIntBinaryOp::And => self.builder.build_and(lhs, rhs, "intrinsic_iand")?,
            NamedIntrinsicIntBinaryOp::Or => self.builder.build_or(lhs, rhs, "intrinsic_ior")?,
            NamedIntrinsicIntBinaryOp::Xor => self.builder.build_xor(lhs, rhs, "intrinsic_ixor")?,
        };
        Ok(CgValue::int(value, target_ty))
    }

    fn codegen_named_intrinsic_int_unary(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
        op: NamedIntrinsicIntUnaryOp,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(&call, 1, "named intrinsic int unary operand arity")?;
        let (value, ty) = self
            .named_intrinsic_int_operand(&call.operands[0], "named intrinsic int unary operand")?;
        let one = self.int_type(ty).const_int(1, false);
        let value = match op {
            NamedIntrinsicIntUnaryOp::Neg => self.builder.build_int_neg(value, "intrinsic_ineg")?,
            NamedIntrinsicIntUnaryOp::Pos => value,
            NamedIntrinsicIntUnaryOp::Inc => {
                self.builder.build_int_add(value, one, "intrinsic_iinc")?
            }
            NamedIntrinsicIntUnaryOp::Dec => {
                self.builder.build_int_sub(value, one, "intrinsic_idec")?
            }
            NamedIntrinsicIntUnaryOp::Inv => self.builder.build_not(value, "intrinsic_iinv")?,
        };
        Ok(CgValue::int(value, ty))
    }

    fn codegen_named_intrinsic_int_shift(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
        op: NamedIntrinsicIntShiftOp,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(&call, 2, "named intrinsic int shift operand arity")?;
        let (lhs, lhs_ty) =
            self.named_intrinsic_int_operand(&call.operands[0], "named intrinsic int shift lhs")?;
        let (rhs_raw, rhs_ty) =
            self.named_intrinsic_int_operand(&call.operands[1], "named intrinsic int shift rhs")?;
        let rhs = self.cast_int(rhs_raw, rhs_ty, lhs_ty)?;
        let amount = self.mask_shift_count(lhs_ty, rhs)?;
        let value = match op {
            NamedIntrinsicIntShiftOp::Shl => {
                self.builder
                    .build_left_shift(lhs, amount, "intrinsic_shl")?
            }
            NamedIntrinsicIntShiftOp::Shr => {
                self.builder
                    .build_right_shift(lhs, amount, lhs_ty.signed, "intrinsic_shr")?
            }
            NamedIntrinsicIntShiftOp::UShr => {
                self.builder
                    .build_right_shift(lhs, amount, false, "intrinsic_ushr")?
            }
        };
        Ok(CgValue::int(value, lhs_ty))
    }

    fn codegen_named_intrinsic_int_compare(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
        op: NamedIntrinsicIntCompareOp,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(&call, 2, "named intrinsic int compare operand arity")?;
        let (lhs_raw, lhs_ty) =
            self.named_intrinsic_int_operand(&call.operands[0], "named intrinsic int compare lhs")?;
        let (rhs_raw, rhs_ty) =
            self.named_intrinsic_int_operand(&call.operands[1], "named intrinsic int compare rhs")?;
        let target_ty = self.named_intrinsic_int_binary_target_ty(lhs_ty, rhs_ty);
        let lhs = self.cast_int(lhs_raw, lhs_ty, target_ty)?;
        let rhs = self.cast_int(rhs_raw, rhs_ty, target_ty)?;
        let value = self.builder.build_int_compare(
            named_intrinsic_int_compare_predicate(target_ty, op),
            lhs,
            rhs,
            "intrinsic_icmp",
        )?;
        Ok(CgValue::bool(value))
    }

    fn codegen_named_intrinsic_int_compare_to(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(
            &call,
            2,
            "named intrinsic int compareTo operand arity",
        )?;
        let (lhs_raw, lhs_ty) = self
            .named_intrinsic_int_operand(&call.operands[0], "named intrinsic int compareTo lhs")?;
        let (rhs_raw, rhs_ty) = self
            .named_intrinsic_int_operand(&call.operands[1], "named intrinsic int compareTo rhs")?;
        let target_ty = self.named_intrinsic_int_binary_target_ty(lhs_ty, rhs_ty);
        let lhs = self.cast_int(lhs_raw, lhs_ty, target_ty)?;
        let rhs = self.cast_int(rhs_raw, rhs_ty, target_ty)?;
        let is_lt = self.builder.build_int_compare(
            named_intrinsic_int_compare_predicate(target_ty, NamedIntrinsicIntCompareOp::Lt),
            lhs,
            rhs,
            "intrinsic_compare_to_lt",
        )?;
        let is_eq = self.builder.build_int_compare(
            IntPredicate::EQ,
            lhs,
            rhs,
            "intrinsic_compare_to_eq",
        )?;
        self.named_intrinsic_three_way_result(is_lt, is_eq)
    }

    fn codegen_named_intrinsic_int_hash(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(&call, 1, "named intrinsic int hash operand arity")?;
        let (value, ty) = self
            .named_intrinsic_int_operand(&call.operands[0], "named intrinsic int hash operand")?;
        let widened = self.cast_int(
            value,
            ty,
            IntTy {
                bits: 64,
                signed: ty.signed,
            },
        )?;
        self.codegen_i64_hash_value(widened)
    }

    fn codegen_named_intrinsic_float_binary(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
        op: NamedIntrinsicFloatBinaryOp,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(&call, 2, "named intrinsic float binary operand arity")?;
        let (lhs_raw, lhs_ty) = self
            .named_intrinsic_float_operand(&call.operands[0], "named intrinsic float binary lhs")?;
        let (rhs_raw, rhs_ty) = self
            .named_intrinsic_float_operand(&call.operands[1], "named intrinsic float binary rhs")?;
        let target_ty = named_intrinsic_float_binary_target_ty(lhs_ty, rhs_ty);
        let lhs = self.cast_float(lhs_raw, lhs_ty, target_ty)?;
        let rhs = self.cast_float(rhs_raw, rhs_ty, target_ty)?;
        let value = match op {
            NamedIntrinsicFloatBinaryOp::Add => {
                self.builder.build_float_add(lhs, rhs, "intrinsic_fadd")?
            }
            NamedIntrinsicFloatBinaryOp::Sub => {
                self.builder.build_float_sub(lhs, rhs, "intrinsic_fsub")?
            }
            NamedIntrinsicFloatBinaryOp::Mul => {
                self.builder.build_float_mul(lhs, rhs, "intrinsic_fmul")?
            }
            NamedIntrinsicFloatBinaryOp::Div => {
                self.builder.build_float_div(lhs, rhs, "intrinsic_fdiv")?
            }
            NamedIntrinsicFloatBinaryOp::Rem => {
                self.builder.build_float_rem(lhs, rhs, "intrinsic_frem")?
            }
        };
        Ok(CgValue::float(value, target_ty))
    }

    fn codegen_named_intrinsic_float_unary_minus(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(&call, 1, "named intrinsic float unary operand arity")?;
        let (value, ty) = self.named_intrinsic_float_operand(
            &call.operands[0],
            "named intrinsic float unary operand",
        )?;
        Ok(CgValue::float(
            self.builder.build_float_neg(value, "intrinsic_fneg")?,
            ty,
        ))
    }

    fn codegen_named_intrinsic_float_unary_plus(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(&call, 1, "named intrinsic float unary operand arity")?;
        let (value, ty) = self.named_intrinsic_float_operand(
            &call.operands[0],
            "named intrinsic float unary operand",
        )?;
        Ok(CgValue::float(value, ty))
    }

    fn codegen_named_intrinsic_float_compare(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
        op: NamedIntrinsicFloatCompareOp,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(
            &call,
            2,
            "named intrinsic float compare operand arity",
        )?;
        let (lhs_raw, lhs_ty) = self.named_intrinsic_float_operand(
            &call.operands[0],
            "named intrinsic float compare lhs",
        )?;
        let (rhs_raw, rhs_ty) = self.named_intrinsic_float_operand(
            &call.operands[1],
            "named intrinsic float compare rhs",
        )?;
        let target_ty = named_intrinsic_float_binary_target_ty(lhs_ty, rhs_ty);
        let lhs = self.cast_float(lhs_raw, lhs_ty, target_ty)?;
        let rhs = self.cast_float(rhs_raw, rhs_ty, target_ty)?;
        let value = self.builder.build_float_compare(
            named_intrinsic_float_compare_predicate(op),
            lhs,
            rhs,
            "intrinsic_fcmp",
        )?;
        Ok(CgValue::bool(value))
    }

    fn codegen_named_intrinsic_float_compare_to(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(
            &call,
            2,
            "named intrinsic float compareTo operand arity",
        )?;
        let (lhs_raw, lhs_ty) = self.named_intrinsic_float_operand(
            &call.operands[0],
            "named intrinsic float compareTo lhs",
        )?;
        let (rhs_raw, rhs_ty) = self.named_intrinsic_float_operand(
            &call.operands[1],
            "named intrinsic float compareTo rhs",
        )?;
        let target_ty = named_intrinsic_float_binary_target_ty(lhs_ty, rhs_ty);
        let lhs = self.cast_float(lhs_raw, lhs_ty, target_ty)?;
        let rhs = self.cast_float(rhs_raw, rhs_ty, target_ty)?;
        let is_lt = self.builder.build_float_compare(
            FloatPredicate::OLT,
            lhs,
            rhs,
            "intrinsic_fcompare_to_lt",
        )?;
        let is_eq = self.builder.build_float_compare(
            FloatPredicate::OEQ,
            lhs,
            rhs,
            "intrinsic_fcompare_to_eq",
        )?;
        self.named_intrinsic_three_way_result(is_lt, is_eq)
    }

    fn codegen_named_intrinsic_float_abs(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(&call, 1, "named intrinsic float abs operand arity")?;
        let operand = call.operands[0].clone();
        self.codegen_float_abs_value(operand.span, operand.value)
    }

    fn codegen_named_intrinsic_float_is_nan(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(&call, 1, "named intrinsic float isNaN operand arity")?;
        let operand = call.operands[0].clone();
        self.codegen_float_is_nan_value(operand.span, operand.value)
    }

    fn codegen_named_intrinsic_float_is_infinite(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(
            &call,
            1,
            "named intrinsic float isInfinite operand arity",
        )?;
        let operand = call.operands[0].clone();
        self.codegen_float_is_infinite_value(operand.span, operand.value)
    }

    fn codegen_named_intrinsic_float_hash(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(&call, 1, "named intrinsic float hash operand arity")?;
        let operand = call.operands[0].clone();
        self.codegen_float_hash_value(operand.span, operand.value)
    }

    fn codegen_named_intrinsic_bool_binary(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
        op: NamedIntrinsicBoolBinaryOp,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(&call, 2, "named intrinsic bool binary operand arity")?;
        let lhs = self
            .named_intrinsic_bool_operand(&call.operands[0], "named intrinsic bool binary lhs")?;
        let rhs = self
            .named_intrinsic_bool_operand(&call.operands[1], "named intrinsic bool binary rhs")?;
        let value = match op {
            NamedIntrinsicBoolBinaryOp::And => {
                self.builder.build_and(lhs, rhs, "intrinsic_band")?
            }
            NamedIntrinsicBoolBinaryOp::Or => self.builder.build_or(lhs, rhs, "intrinsic_bor")?,
            NamedIntrinsicBoolBinaryOp::Xor => {
                self.builder.build_xor(lhs, rhs, "intrinsic_bxor")?
            }
        };
        Ok(CgValue::bool(value))
    }

    fn codegen_named_intrinsic_bool_compare(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
        pred: IntPredicate,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(&call, 2, "named intrinsic bool compare operand arity")?;
        let lhs = self
            .named_intrinsic_bool_operand(&call.operands[0], "named intrinsic bool compare lhs")?;
        let rhs = self
            .named_intrinsic_bool_operand(&call.operands[1], "named intrinsic bool compare rhs")?;
        Ok(CgValue::bool(self.builder.build_int_compare(
            pred,
            lhs,
            rhs,
            "intrinsic_bcmp",
        )?))
    }

    fn codegen_named_intrinsic_bool_not(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(&call, 1, "named intrinsic bool not operand arity")?;
        let value = self
            .named_intrinsic_bool_operand(&call.operands[0], "named intrinsic bool not operand")?;
        Ok(CgValue::bool(
            self.builder.build_not(value, "intrinsic_bnot")?,
        ))
    }

    fn codegen_named_intrinsic_char_to_int(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(&call, 1, "named intrinsic char toInt operand arity")?;
        let value = self.named_intrinsic_char_operand(
            &call.operands[0],
            "named intrinsic char toInt operand",
        )?;
        let int_ty = self.named_intrinsic_word_int_ty();
        let value = self.cast_int(value, named_intrinsic_char_ty(), int_ty)?;
        Ok(CgValue::int(value, int_ty))
    }

    fn codegen_named_intrinsic_char_hash(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(&call, 1, "named intrinsic char hash operand arity")?;
        let value = self
            .named_intrinsic_char_operand(&call.operands[0], "named intrinsic char hash operand")?;
        let widened = self.cast_int(
            value,
            named_intrinsic_char_ty(),
            IntTy {
                bits: 64,
                signed: false,
            },
        )?;
        self.codegen_i64_hash_value(widened)
    }

    fn codegen_named_intrinsic_char_compare_to(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(
            &call,
            2,
            "named intrinsic char compareTo operand arity",
        )?;
        let lhs = self.named_intrinsic_char_operand(
            &call.operands[0],
            "named intrinsic char compareTo lhs",
        )?;
        let rhs = self.named_intrinsic_char_operand(
            &call.operands[1],
            "named intrinsic char compareTo rhs",
        )?;
        let is_lt = self.builder.build_int_compare(
            IntPredicate::ULT,
            lhs,
            rhs,
            "intrinsic_char_compare_to_lt",
        )?;
        let is_eq = self.builder.build_int_compare(
            IntPredicate::EQ,
            lhs,
            rhs,
            "intrinsic_char_compare_to_eq",
        )?;
        self.named_intrinsic_three_way_result(is_lt, is_eq)
    }

    fn codegen_named_intrinsic_char_equals(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(&call, 2, "named intrinsic char equals operand arity")?;
        let lhs = self
            .named_intrinsic_char_operand(&call.operands[0], "named intrinsic char equals lhs")?;
        let rhs = self
            .named_intrinsic_char_operand(&call.operands[1], "named intrinsic char equals rhs")?;
        Ok(CgValue::bool(self.builder.build_int_compare(
            IntPredicate::EQ,
            lhs,
            rhs,
            "intrinsic_char_eq",
        )?))
    }

    fn codegen_named_intrinsic_char_plus_int(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_named_intrinsic_char_int_arithmetic(call, NamedIntrinsicIntBinaryOp::Add)
    }

    fn codegen_named_intrinsic_char_minus_int(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_named_intrinsic_char_int_arithmetic(call, NamedIntrinsicIntBinaryOp::Sub)
    }

    fn codegen_named_intrinsic_char_minus_char(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(
            &call,
            2,
            "named intrinsic char minus char operand arity",
        )?;
        let lhs = self.named_intrinsic_char_operand(
            &call.operands[0],
            "named intrinsic char minus char lhs",
        )?;
        let rhs = self.named_intrinsic_char_operand(
            &call.operands[1],
            "named intrinsic char minus char rhs",
        )?;
        let diff = self
            .builder
            .build_int_sub(lhs, rhs, "intrinsic_char_minus_char")?;
        let int_ty = self.named_intrinsic_word_int_ty();
        let widened = self.cast_int(diff, named_intrinsic_signed_i32_ty(), int_ty)?;
        Ok(CgValue::int(widened, int_ty))
    }

    fn codegen_named_intrinsic_char_int_arithmetic(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
        op: NamedIntrinsicIntBinaryOp,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.named_intrinsic_require_arity(&call, 2, "named intrinsic char int operand arity")?;
        let char_ty = named_intrinsic_char_ty();
        let lhs =
            self.named_intrinsic_char_operand(&call.operands[0], "named intrinsic char int lhs")?;
        let (rhs_raw, rhs_ty) =
            self.named_intrinsic_int_operand(&call.operands[1], "named intrinsic char int rhs")?;
        let rhs = self.cast_int(rhs_raw, rhs_ty, char_ty)?;
        let value = match op {
            NamedIntrinsicIntBinaryOp::Add => {
                self.builder.build_int_add(lhs, rhs, "intrinsic_char_add")?
            }
            NamedIntrinsicIntBinaryOp::Sub => {
                self.builder.build_int_sub(lhs, rhs, "intrinsic_char_sub")?
            }
            _ => unreachable!("char arithmetic only uses add/sub"),
        };
        Ok(CgValue::int(value, char_ty))
    }

    fn named_intrinsic_require_arity(
        &self,
        call: &LoweredNamedIntrinsicCall<'ctx>,
        expected: usize,
        kind: &'static str,
    ) -> Result<(), LlvmEmitError> {
        if call.operands.len() == expected {
            return Ok(());
        }
        self.panic_verified_intrinsic_contract("named intrinsic operand arity", kind)
    }

    fn named_intrinsic_int_operand(
        &self,
        operand: &LoweredNamedIntrinsicOperand<'ctx>,
        kind: &'static str,
    ) -> Result<(IntValue<'ctx>, IntTy), LlvmEmitError> {
        operand.value.as_int().ok_or_else(|| {
            self.panic_verified_intrinsic_contract("named intrinsic int operand", kind)
        })
    }

    fn named_intrinsic_bool_operand(
        &self,
        operand: &LoweredNamedIntrinsicOperand<'ctx>,
        kind: &'static str,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        operand.value.as_bool().ok_or_else(|| {
            self.panic_verified_intrinsic_contract("named intrinsic bool operand", kind)
        })
    }

    fn named_intrinsic_float_operand(
        &self,
        operand: &LoweredNamedIntrinsicOperand<'ctx>,
        kind: &'static str,
    ) -> Result<(FloatValue<'ctx>, CgTy), LlvmEmitError> {
        operand.value.as_float().ok_or_else(|| {
            self.panic_verified_intrinsic_contract("named intrinsic float operand", kind)
        })
    }

    fn named_intrinsic_char_operand(
        &mut self,
        operand: &LoweredNamedIntrinsicOperand<'ctx>,
        kind: &'static str,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let (value, ty) = self.named_intrinsic_int_operand(operand, kind)?;
        self.cast_int(value, ty, named_intrinsic_char_ty())
    }

    fn named_intrinsic_word_int_ty(&self) -> IntTy {
        IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        }
    }

    fn named_intrinsic_int_binary_target_ty(&self, lhs: IntTy, rhs: IntTy) -> IntTy {
        let word_bits = self.host.word_bit_width();
        if lhs.bits == word_bits && rhs.bits != word_bits {
            rhs
        } else {
            lhs
        }
    }

    fn named_intrinsic_three_way_result(
        &mut self,
        is_lt: IntValue<'ctx>,
        is_eq: IntValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let int_ty = self.named_intrinsic_word_int_ty();
        let llvm_ty = self.int_type(int_ty);
        let neg_one = llvm_ty.const_all_ones();
        let zero = llvm_ty.const_zero();
        let one = llvm_ty.const_int(1, false);
        let eq_or_gt = self
            .builder
            .build_select(is_eq, zero, one, "intrinsic_compare_to_eq_or_gt")?
            .into_int_value();
        let result = self
            .builder
            .build_select(is_lt, neg_one, eq_or_gt, "intrinsic_compare_to_result")?
            .into_int_value();
        Ok(CgValue::int(result, int_ty))
    }

    fn unsafe_ptr_to_word(
        &mut self,
        ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<inkwell::values::IntValue<'ctx>, LlvmEmitError> {
        let ptr_int_ty = self.llvm_ptr_sized_int_type(Some(ptr.get_type().get_address_space()));
        let from_ty = IntTy {
            bits: ptr_int_ty.get_bit_width(),
            signed: false,
        };
        let to_ty = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };
        let raw = self.builder.build_ptr_to_int(ptr, ptr_int_ty, name)?;
        self.cast_int(raw, from_ty, to_ty)
    }

    fn named_intrinsic_array_receiver_ptr(
        &mut self,
        operand: &LoweredNamedIntrinsicOperand<'ctx>,
        kind: &'static str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let value = self.coerce_value(operand.span, operand.value, CgTy::Ref)?;
        let Some(BasicValueEnum::PointerValue(ptr)) = value.value else {
            self.panic_verified_intrinsic_contract("named intrinsic array receiver", kind);
        };
        Ok(ptr)
    }

    fn named_intrinsic_array_index_i64(
        &mut self,
        operand: &LoweredNamedIntrinsicOperand<'ctx>,
        kind: &'static str,
    ) -> Result<inkwell::values::IntValue<'ctx>, LlvmEmitError> {
        let value = self.coerce_value(
            operand.span,
            operand.value,
            CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: true,
            }),
        )?;
        let Some((raw, from)) = value.as_int() else {
            self.panic_verified_intrinsic_contract("named intrinsic array index", kind);
        };
        self.cast_int(
            raw,
            from,
            IntTy {
                bits: 64,
                signed: true,
            },
        )
    }

    fn named_intrinsic_array_element_cg_ty(
        &self,
        _span: crate::span::Span,
        receiver: &LoweredNamedIntrinsicOperand<'ctx>,
        fallback_elem_ty: Option<TypeId>,
        kind: &'static str,
    ) -> Result<(TypeId, CgTy), LlvmEmitError> {
        let elem_ty = receiver
            .source_ty
            .and_then(|receiver_ty| match self.types.kind(receiver_ty) {
                TypeKind::Ref(RefTypeKind::Nominal(nominal))
                    if normalize_array_like_fqn(nominal.fqn.as_str()).is_some() =>
                {
                    nominal.args.first().copied()
                }
                _ => None,
            })
            .or(fallback_elem_ty)
            .unwrap_or_else(|| {
                self.panic_verified_intrinsic_contract("named intrinsic array element type", kind)
            });
        let elem_cg = self.try_cg_ty_of_type_id(elem_ty).unwrap_or_else(|| {
            self.panic_verified_intrinsic_contract(
                "named intrinsic array element codegen type",
                kind,
            )
        });
        Ok((elem_ty, elem_cg))
    }

    fn named_intrinsic_array_len_value(
        &mut self,
        _span: crate::span::Span,
        arr_ptr: PointerValue<'ctx>,
        layout: NamedIntrinsicArrayLayout,
    ) -> Result<inkwell::values::IntValue<'ctx>, LlvmEmitError> {
        let (array_ty, gep_name, load_name) = match layout {
            NamedIntrinsicArrayLayout::Inline => {
                (self.llvm_scoop_array_type(), "array_len_gep", "array_len")
            }
            NamedIntrinsicArrayLayout::OutOfLine => (
                self.llvm_scoop_mutable_array_type(),
                "mutable_array_len_gep",
                "mutable_array_len",
            ),
        };
        let len_ptr = self
            .builder
            .build_struct_gep(array_ty, arr_ptr, 1, gep_name)?;
        Ok(self
            .builder
            .build_load(self.context.i64_type(), len_ptr, load_name)?
            .into_int_value())
    }

    fn named_intrinsic_array_data_base_ptr(
        &mut self,
        _span: crate::span::Span,
        arr_ptr: PointerValue<'ctx>,
        layout: NamedIntrinsicArrayLayout,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        match layout {
            NamedIntrinsicArrayLayout::Inline => {
                let array_ty = self.llvm_scoop_array_type();
                let data_offset_ptr =
                    self.builder
                        .build_struct_gep(array_ty, arr_ptr, 3, "array_data_offset_gep")?;
                let data_offset = self
                    .builder
                    .build_load(
                        self.context.i64_type(),
                        data_offset_ptr,
                        "array_data_offset",
                    )?
                    .into_int_value();
                let array_i8_gc = self.builder.build_pointer_cast(
                    arr_ptr,
                    self.llvm_gc_i8_ptr_type(),
                    "array_i8_gc",
                )?;
                Ok(unsafe {
                    self.builder.build_in_bounds_gep(
                        self.context.i8_type(),
                        array_i8_gc,
                        &[data_offset],
                        "array_data_base_gc",
                    )?
                })
            }
            NamedIntrinsicArrayLayout::OutOfLine => {
                let array_ty = self.llvm_scoop_mutable_array_type();
                let data_ptr = self.builder.build_struct_gep(
                    array_ty,
                    arr_ptr,
                    6,
                    "mutable_array_data_gep",
                )?;
                Ok(self
                    .builder
                    .build_load(self.llvm_i8_ptr_type(), data_ptr, "mutable_array_data")?
                    .into_pointer_value())
            }
        }
    }

    fn named_intrinsic_array_stride_bytes(
        &mut self,
        span: crate::span::Span,
        elem_cg: CgTy,
    ) -> Result<u64, LlvmEmitError> {
        match elem_cg {
            CgTy::Ref | CgTy::String => Ok(self.target_layout().pointer_size.max(1)),
            CgTy::Unit => Ok(1),
            CgTy::Bool | CgTy::Float64 | CgTy::Float32 | CgTy::Int(_) => {
                let llvm_ty = self.llvm_basic_type_of(span, elem_cg)?;
                Ok(self.store_size_bytes_of_basic_type(llvm_ty).max(1))
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                let llvm_ty = self.llvm_basic_type_of(span, elem_cg)?;
                Ok(self.store_size_bytes_of_basic_type(llvm_ty))
            }
            CgTy::Never => self.panic_verified_intrinsic_contract(
                "named intrinsic array element stride",
                "array element type is Never",
            ),
        }
    }

    fn named_intrinsic_array_slot_i8_ptr(
        &mut self,
        span: crate::span::Span,
        arr_ptr: PointerValue<'ctx>,
        receiver: &LoweredNamedIntrinsicOperand<'ctx>,
        elem_cg: CgTy,
        index_i64: inkwell::values::IntValue<'ctx>,
        layout: NamedIntrinsicArrayLayout,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let data_base = self.named_intrinsic_array_data_base_ptr(span, arr_ptr, layout)?;
        let stride = self.named_intrinsic_array_stride_bytes(span, elem_cg)?;
        let byte_offset = if stride == 1 {
            index_i64
        } else {
            self.builder.build_int_mul(
                index_i64,
                self.context.i64_type().const_int(stride, false),
                "array_elem_byte_offset",
            )?
        };
        let _ = receiver;
        let name = match layout {
            NamedIntrinsicArrayLayout::Inline => "array_elem_i8_gc",
            NamedIntrinsicArrayLayout::OutOfLine => "mutable_array_elem_i8_native",
        };
        Ok(unsafe {
            self.builder.build_in_bounds_gep(
                self.context.i8_type(),
                data_base,
                &[byte_offset],
                name,
            )?
        })
    }

    fn named_intrinsic_array_slot_ptr(
        &mut self,
        span: crate::span::Span,
        arr_ptr: PointerValue<'ctx>,
        receiver: &LoweredNamedIntrinsicOperand<'ctx>,
        elem_cg: CgTy,
        index_i64: inkwell::values::IntValue<'ctx>,
        layout: NamedIntrinsicArrayLayout,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let slot_i8 = self.named_intrinsic_array_slot_i8_ptr(
            span, arr_ptr, receiver, elem_cg, index_i64, layout,
        )?;
        self.named_intrinsic_array_slot_storage_ptr(
            slot_i8,
            match layout {
                NamedIntrinsicArrayLayout::Inline => "array_elem_ptr_gc",
                NamedIntrinsicArrayLayout::OutOfLine => "mutable_array_elem_ptr_native",
            },
        )
    }

    fn named_intrinsic_array_slot_storage_ptr(
        &mut self,
        slot_i8: PointerValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        Ok(self.builder.build_pointer_cast(
            slot_i8,
            self.llvm_ptr_type(slot_i8.get_type().get_address_space()),
            name,
        )?)
    }

    fn named_intrinsic_native_i8_ptr(
        &mut self,
        ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        if ptr.get_type().get_address_space() == AddressSpace::default() {
            return Ok(ptr);
        }
        Ok(self
            .builder
            .build_address_space_cast(ptr, self.llvm_i8_ptr_type(), name)?)
    }

    fn store_out_of_line_gc_pointer_slot_with_promotion_barrier(
        &mut self,
        at: crate::span::Span,
        slot_ptr: PointerValue<'ctx>,
        value_ptr: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let value_i8 = self.builder.build_pointer_cast(
            value_ptr,
            self.llvm_gc_i8_ptr_type(),
            "mutable_array_set_ref_value_i8",
        )?;
        let _ = self.builder.build_store(slot_ptr, value_i8)?;
        self.call_gc_promotion_barrier_for_out_of_line_value(at, value_i8)
    }

    fn call_gc_promotion_barrier_for_out_of_line_value(
        &mut self,
        at: crate::span::Span,
        value_ptr: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let wb = self.declare_runtime_gc_write_barrier();
        let value_i8 = self.builder.build_pointer_cast(
            value_ptr,
            self.llvm_gc_i8_ptr_type(),
            "gc_promotion_value_i8",
        )?;
        let null_slot = self.llvm_i8_ptr_type().const_null();
        let _ = self.build_call_preserving_gc_local_roots(
            at,
            wb,
            &[null_slot.into(), value_i8.into()],
            "gc_promotion_barrier",
        )?;
        Ok(())
    }

    fn named_intrinsic_materialize_value_ptr(
        &mut self,
        span: crate::span::Span,
        name: &str,
        cg_ty: CgTy,
        value: CgValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let slot = self.create_entry_alloca(span, name, cg_ty)?;
        let _ = self.store_local_value(span, slot, cg_ty, value)?;
        Ok(slot)
    }

    fn named_intrinsic_array_composite_descriptor(
        &mut self,
        span: crate::span::Span,
        elem_ty: TypeId,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let body_fqn = self
            .function_cx
            .current_callable_fqn
            .clone()
            .unwrap_or_else(|| "<named-intrinsic-array>".to_string());
        let metadata =
            mir::ValueTransportMetadata::plain(elem_ty, mir::MirTransportKind::ArrayElement);
        let descriptor = self.get_or_create_value_composite_transport_descriptor_global(
            &body_fqn, span, self.types, &metadata,
        )?;
        Ok(descriptor.as_pointer_value())
    }

    fn named_intrinsic_array_composite_gc_slot_offsets(
        &mut self,
        span: crate::span::Span,
        elem_cg: CgTy,
    ) -> Result<Vec<u64>, LlvmEmitError> {
        let llvm_ty = self.llvm_basic_type_of(span, elem_cg)?;
        let mut offsets = Vec::new();
        self.collect_gc_ptr_offsets_in_basic_type(span, llvm_ty, 0, &mut offsets)?;
        offsets.sort_unstable();
        offsets.dedup();
        Ok(offsets)
    }

    fn codegen_named_runtime_intrinsic_call(
        &mut self,
        entry: &NamedIntrinsicAuditEntry,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let symbol = entry.runtime_symbol.unwrap_or_else(|| {
            self.panic_verified_intrinsic_contract(
                "named runtime intrinsic",
                "missing runtime symbol metadata",
            )
        });
        let signature = entry.runtime_signature.unwrap_or_else(|| {
            self.panic_verified_intrinsic_contract(
                "named runtime intrinsic",
                "missing runtime signature metadata",
            )
        });
        let _reason = entry.runtime_reason.unwrap_or_else(|| {
            self.panic_verified_intrinsic_contract(
                "named runtime intrinsic",
                "missing runtime reason metadata",
            )
        });
        if call.operands.len() != signature.params.len() {
            self.panic_verified_intrinsic_contract(
                "named runtime intrinsic",
                "operand arity drift",
            );
        }

        let runtime = self.declare_named_intrinsic_runtime_symbol(symbol, signature)?;
        let mut llvm_args = Vec::with_capacity(call.operands.len());
        for (operand, &param_ty) in call.operands.iter().zip(signature.params.iter()) {
            llvm_args.push(self.named_intrinsic_runtime_arg(operand, param_ty)?);
        }
        let call_site = self.build_call_preserving_gc_local_roots(
            call.span,
            runtime,
            &llvm_args,
            "named_intrinsic_runtime_call",
        )?;
        self.named_intrinsic_runtime_result(call.span, call_site, signature.return_ty)
    }

    fn declare_named_intrinsic_runtime_symbol(
        &mut self,
        symbol: &str,
        signature: crate::intrinsics::NamedIntrinsicRuntimeSignature,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        // P4-T01j：named intrinsic runtime symbol 也属于 runtime/native import surface，
        // 必须经过 [`declare_runtime_or_native_import_function`] 的分类入口完成 surface
        // assertions（External linkage、未来一旦改写不会绕开 classification 检查）；
        // wrapper 内部已包含 "已存在则复用" 语义，所以这里不再重复 `module.get_function` early-return。
        let param_tys = signature
            .params
            .iter()
            .copied()
            .map(|ty| self.named_intrinsic_runtime_metadata_ty(ty))
            .collect::<Result<Vec<_>, _>>()?;
        let fn_ty = match self.named_intrinsic_runtime_basic_ty(signature.return_ty)? {
            Some(ret) => ret.fn_type(&param_tys, false),
            None => self.context.void_type().fn_type(&param_tys, false),
        };
        Ok(self.declare_runtime_or_native_import_function(symbol, fn_ty))
    }

    fn named_intrinsic_runtime_metadata_ty(
        &self,
        ty: NamedIntrinsicRuntimeTy,
    ) -> Result<BasicMetadataTypeEnum<'ctx>, LlvmEmitError> {
        Ok(self
            .named_intrinsic_runtime_basic_ty(ty)?
            .unwrap_or_else(|| {
                self.panic_verified_intrinsic_contract(
                    "named runtime intrinsic metadata",
                    "void parameter type",
                )
            })
            .into())
    }

    fn named_intrinsic_runtime_basic_ty(
        &self,
        ty: NamedIntrinsicRuntimeTy,
    ) -> Result<Option<BasicTypeEnum<'ctx>>, LlvmEmitError> {
        Ok(match ty {
            NamedIntrinsicRuntimeTy::Void => None,
            NamedIntrinsicRuntimeTy::I32 => Some(self.context.i32_type().into()),
            NamedIntrinsicRuntimeTy::I64 => Some(self.context.i64_type().into()),
            NamedIntrinsicRuntimeTy::WordInt | NamedIntrinsicRuntimeTy::WordUInt => Some(
                self.context
                    .custom_width_int_type(self.host.word_bit_width())
                    .into(),
            ),
            NamedIntrinsicRuntimeTy::Bool => Some(self.context.bool_type().into()),
            NamedIntrinsicRuntimeTy::Float32 => Some(self.context.f32_type().into()),
            NamedIntrinsicRuntimeTy::Float64 => Some(self.context.f64_type().into()),
            NamedIntrinsicRuntimeTy::StringRef => Some(self.llvm_scoop_string_ptr_type().into()),
            NamedIntrinsicRuntimeTy::GcRef => Some(self.llvm_gc_i8_ptr_type().into()),
            NamedIntrinsicRuntimeTy::RawPtr => Some(self.llvm_i8_ptr_type().into()),
        })
    }

    fn named_intrinsic_runtime_arg(
        &mut self,
        operand: &LoweredNamedIntrinsicOperand<'ctx>,
        target_ty: NamedIntrinsicRuntimeTy,
    ) -> Result<BasicMetadataValueEnum<'ctx>, LlvmEmitError> {
        let value = match target_ty {
            NamedIntrinsicRuntimeTy::Void => {
                self.panic_verified_intrinsic_contract(
                    "named runtime intrinsic argument",
                    "void operand",
                );
            }
            NamedIntrinsicRuntimeTy::I32 => {
                let target = CgTy::Int(IntTy {
                    bits: 32,
                    signed: true,
                });
                let coerced = self.coerce_value(operand.span, operand.value, target)?;
                self.expect_cg_value(coerced, "named runtime intrinsic i32 operand")
            }
            NamedIntrinsicRuntimeTy::I64 => {
                let target = CgTy::Int(IntTy {
                    bits: 64,
                    signed: true,
                });
                let coerced = self.coerce_value(operand.span, operand.value, target)?;
                self.expect_cg_value(coerced, "named runtime intrinsic i64 operand")
            }
            NamedIntrinsicRuntimeTy::WordInt | NamedIntrinsicRuntimeTy::WordUInt => {
                let target = CgTy::Int(IntTy {
                    bits: self.host.word_bit_width(),
                    signed: matches!(target_ty, NamedIntrinsicRuntimeTy::WordInt),
                });
                let coerced = self.coerce_value(operand.span, operand.value, target)?;
                self.expect_cg_value(coerced, "named runtime intrinsic word operand")
            }
            NamedIntrinsicRuntimeTy::Bool => {
                let coerced = self.coerce_value(operand.span, operand.value, CgTy::Bool)?;
                self.expect_cg_value(coerced, "named runtime intrinsic bool operand")
            }
            NamedIntrinsicRuntimeTy::Float32 => {
                let coerced = self.coerce_value(operand.span, operand.value, CgTy::Float32)?;
                self.expect_cg_value(coerced, "named runtime intrinsic f32 operand")
            }
            NamedIntrinsicRuntimeTy::Float64 => {
                let coerced = self.coerce_value(operand.span, operand.value, CgTy::Float64)?;
                self.expect_cg_value(coerced, "named runtime intrinsic f64 operand")
            }
            NamedIntrinsicRuntimeTy::StringRef => {
                let coerced = self.coerce_value(operand.span, operand.value, CgTy::String)?;
                self.expect_cg_value(coerced, "named runtime intrinsic string operand")
            }
            NamedIntrinsicRuntimeTy::GcRef => {
                let coerced = self.coerce_value(operand.span, operand.value, CgTy::Ref)?;
                self.expect_cg_value(coerced, "named runtime intrinsic GC ref operand")
            }
            NamedIntrinsicRuntimeTy::RawPtr => {
                let raw = self
                    .expect_cg_value(operand.value, "named runtime intrinsic raw pointer operand");
                match raw {
                    inkwell::values::BasicValueEnum::PointerValue(ptr) => self
                        .builder
                        .build_pointer_cast(
                            ptr,
                            self.llvm_i8_ptr_type(),
                            "named_intrinsic_raw_ptr",
                        )?
                        .into(),
                    _ => {
                        self.panic_verified_intrinsic_contract(
                            "named runtime intrinsic argument",
                            "raw pointer operand type drift",
                        );
                    }
                }
            }
        };
        Ok(value.into())
    }

    fn named_intrinsic_runtime_result(
        &self,
        _span: crate::span::Span,
        call_site: inkwell::values::CallSiteValue<'ctx>,
        result_ty: NamedIntrinsicRuntimeTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match result_ty {
            NamedIntrinsicRuntimeTy::Void => Ok(CgValue::unit()),
            NamedIntrinsicRuntimeTy::I32 => {
                let raw = self.expect_basic_value(call_site, "named runtime intrinsic i32 return");
                let value = self.expect_int_value(raw, "named runtime intrinsic i32 return");
                Ok(CgValue::int(
                    value,
                    IntTy {
                        bits: 32,
                        signed: true,
                    },
                ))
            }
            NamedIntrinsicRuntimeTy::I64 => {
                let raw = self.expect_basic_value(call_site, "named runtime intrinsic i64 return");
                let value = self.expect_int_value(raw, "named runtime intrinsic i64 return");
                Ok(CgValue::int(
                    value,
                    IntTy {
                        bits: 64,
                        signed: true,
                    },
                ))
            }
            NamedIntrinsicRuntimeTy::WordInt | NamedIntrinsicRuntimeTy::WordUInt => {
                let raw = self.expect_basic_value(call_site, "named runtime intrinsic word return");
                let value = self.expect_int_value(raw, "named runtime intrinsic word return");
                Ok(CgValue::int(
                    value,
                    IntTy {
                        bits: self.host.word_bit_width(),
                        signed: matches!(result_ty, NamedIntrinsicRuntimeTy::WordInt),
                    },
                ))
            }
            NamedIntrinsicRuntimeTy::Bool => {
                let raw = self.expect_basic_value(call_site, "named runtime intrinsic bool return");
                let value = self.expect_int_value(raw, "named runtime intrinsic bool return");
                Ok(CgValue::bool(value))
            }
            NamedIntrinsicRuntimeTy::Float32 => {
                let raw = self.expect_basic_value(call_site, "named runtime intrinsic f32 return");
                let value = self.expect_float_value(raw, "named runtime intrinsic f32 return");
                Ok(CgValue::float(value, CgTy::Float32))
            }
            NamedIntrinsicRuntimeTy::Float64 => {
                let raw = self.expect_basic_value(call_site, "named runtime intrinsic f64 return");
                let value = self.expect_float_value(raw, "named runtime intrinsic f64 return");
                Ok(CgValue::float(value, CgTy::Float64))
            }
            NamedIntrinsicRuntimeTy::StringRef => {
                let raw =
                    self.expect_basic_value(call_site, "named runtime intrinsic string return");
                let value = self.expect_pointer_value(raw, "named runtime intrinsic string return");
                Ok(CgValue {
                    ty: CgTy::String,
                    value: Some(value.into()),
                })
            }
            NamedIntrinsicRuntimeTy::GcRef => {
                let raw =
                    self.expect_basic_value(call_site, "named runtime intrinsic GC ref return");
                let value = self.expect_pointer_value(raw, "named runtime intrinsic GC ref return");
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(value.into()),
                })
            }
            NamedIntrinsicRuntimeTy::RawPtr => {
                let raw = self
                    .expect_basic_value(call_site, "named runtime intrinsic raw pointer return");
                let value =
                    self.expect_pointer_value(raw, "named runtime intrinsic raw pointer return");
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(
                        self.builder
                            .build_pointer_cast(
                                value,
                                self.llvm_gc_i8_ptr_type(),
                                "named_intrinsic_raw_result",
                            )?
                            .into(),
                    ),
                })
            }
        }
    }
}

#[cfg(all(test, not(feature = "standalone-codegen-crate")))]
mod tests {
    use super::*;
    use crate::intrinsics::named_intrinsic_audit_entries;

    #[test]
    fn ir_rule_table_covers_shared_ir_entries() {
        for entry in named_intrinsic_audit_entries() {
            if entry.lowering_mode != NamedIntrinsicLoweringMode::IrEmission {
                continue;
            }
            assert!(
                lookup_named_intrinsic_ir_rule(entry.name).is_some(),
                "missing IR rule for shared named intrinsic entry {:?}",
                entry.name
            );
        }
    }
}
