//! 编译器内置 named intrinsic 的共享声明元数据。
//!
//! 这一层只承载前后端都需要共享的事实：
//! - `@Intrinsic("name")` 的参数解析；
//! - intrinsic 表中允许出现的名字；
//! - 每个 entry 的 lowering 模式与 runtime 审计信息。

use crate::ast;
use crate::source::SourceFile;
use crate::span::Span;
use crate::syntax::string_literal::parse_string_literal_utf8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedIntrinsicLoweringMode {
    IrEmission,
    RuntimeCall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum NamedIntrinsicRuntimeTy {
    Void,
    I32,
    I64,
    WordInt,
    WordUInt,
    Bool,
    Float32,
    Float64,
    StringRef,
    GcRef,
    RawPtr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NamedIntrinsicRuntimeSignature {
    pub params: &'static [NamedIntrinsicRuntimeTy],
    pub return_ty: NamedIntrinsicRuntimeTy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NamedIntrinsicAuditEntry {
    pub name: &'static str,
    pub lowering_mode: NamedIntrinsicLoweringMode,
    pub runtime_symbol: Option<&'static str>,
    pub runtime_signature: Option<NamedIntrinsicRuntimeSignature>,
    pub runtime_reason: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedIntrinsicAnnotationArgs {
    Legacy,
    Named { entry_name: String },
}

impl ParsedIntrinsicAnnotationArgs {
    pub fn entry_name(&self) -> Option<&str> {
        match self {
            Self::Legacy => None,
            Self::Named { entry_name } => Some(entry_name.as_str()),
        }
    }

    pub fn into_entry_name(self) -> Option<String> {
        match self {
            Self::Legacy => None,
            Self::Named { entry_name } => Some(entry_name),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntrinsicAnnotationParseError {
    InvalidShape { span: Span },
    ArgMustBeString { span: Span },
}

const EMPTY_RUNTIME_PARAMS: &[NamedIntrinsicRuntimeTy] = &[];

const DUMMY_RUNTIME_SIGNATURE: NamedIntrinsicRuntimeSignature = NamedIntrinsicRuntimeSignature {
    params: EMPTY_RUNTIME_PARAMS,
    return_ty: NamedIntrinsicRuntimeTy::WordInt,
};
const INT_TO_STRING_PARAMS: &[NamedIntrinsicRuntimeTy] = &[NamedIntrinsicRuntimeTy::I64];
const INT_TO_STRING_SIGNATURE: NamedIntrinsicRuntimeSignature = NamedIntrinsicRuntimeSignature {
    params: INT_TO_STRING_PARAMS,
    return_ty: NamedIntrinsicRuntimeTy::StringRef,
};
const BOOL_TO_STRING_PARAMS: &[NamedIntrinsicRuntimeTy] = &[NamedIntrinsicRuntimeTy::I64];
const BOOL_TO_STRING_SIGNATURE: NamedIntrinsicRuntimeSignature = NamedIntrinsicRuntimeSignature {
    params: BOOL_TO_STRING_PARAMS,
    return_ty: NamedIntrinsicRuntimeTy::StringRef,
};
const CHAR_TO_STRING_PARAMS: &[NamedIntrinsicRuntimeTy] = &[NamedIntrinsicRuntimeTy::I32];
const CHAR_TO_STRING_SIGNATURE: NamedIntrinsicRuntimeSignature = NamedIntrinsicRuntimeSignature {
    params: CHAR_TO_STRING_PARAMS,
    return_ty: NamedIntrinsicRuntimeTy::StringRef,
};
const FLOAT64_TO_STRING_PARAMS: &[NamedIntrinsicRuntimeTy] = &[NamedIntrinsicRuntimeTy::Float64];
const FLOAT64_TO_STRING_SIGNATURE: NamedIntrinsicRuntimeSignature =
    NamedIntrinsicRuntimeSignature {
        params: FLOAT64_TO_STRING_PARAMS,
        return_ty: NamedIntrinsicRuntimeTy::StringRef,
    };
const FLOAT32_TO_STRING_PARAMS: &[NamedIntrinsicRuntimeTy] = &[NamedIntrinsicRuntimeTy::Float32];
const FLOAT32_TO_STRING_SIGNATURE: NamedIntrinsicRuntimeSignature =
    NamedIntrinsicRuntimeSignature {
        params: FLOAT32_TO_STRING_PARAMS,
        return_ty: NamedIntrinsicRuntimeTy::StringRef,
    };

const fn ir_emission_entry(name: &'static str) -> NamedIntrinsicAuditEntry {
    NamedIntrinsicAuditEntry {
        name,
        lowering_mode: NamedIntrinsicLoweringMode::IrEmission,
        runtime_symbol: None,
        runtime_signature: None,
        runtime_reason: None,
    }
}

const NAMED_INTRINSIC_AUDIT_ENTRIES: &[NamedIntrinsicAuditEntry] = &[
    NamedIntrinsicAuditEntry {
        name: "dummy_ir",
        lowering_mode: NamedIntrinsicLoweringMode::IrEmission,
        runtime_symbol: None,
        runtime_signature: None,
        runtime_reason: None,
    },
    NamedIntrinsicAuditEntry {
        name: "array_size_inline",
        lowering_mode: NamedIntrinsicLoweringMode::IrEmission,
        runtime_symbol: None,
        runtime_signature: None,
        runtime_reason: None,
    },
    NamedIntrinsicAuditEntry {
        name: "array_size_outofline",
        lowering_mode: NamedIntrinsicLoweringMode::IrEmission,
        runtime_symbol: None,
        runtime_signature: None,
        runtime_reason: None,
    },
    NamedIntrinsicAuditEntry {
        name: "array_get_inline",
        lowering_mode: NamedIntrinsicLoweringMode::IrEmission,
        runtime_symbol: None,
        runtime_signature: None,
        runtime_reason: None,
    },
    NamedIntrinsicAuditEntry {
        name: "array_get_outofline",
        lowering_mode: NamedIntrinsicLoweringMode::IrEmission,
        runtime_symbol: None,
        runtime_signature: None,
        runtime_reason: None,
    },
    NamedIntrinsicAuditEntry {
        name: "array_set_inline",
        lowering_mode: NamedIntrinsicLoweringMode::IrEmission,
        runtime_symbol: None,
        runtime_signature: None,
        runtime_reason: None,
    },
    NamedIntrinsicAuditEntry {
        name: "array_set_outofline",
        lowering_mode: NamedIntrinsicLoweringMode::IrEmission,
        runtime_symbol: None,
        runtime_signature: None,
        runtime_reason: None,
    },
    NamedIntrinsicAuditEntry {
        name: "array_data_ptr_inline",
        lowering_mode: NamedIntrinsicLoweringMode::IrEmission,
        runtime_symbol: None,
        runtime_signature: None,
        runtime_reason: None,
    },
    NamedIntrinsicAuditEntry {
        name: "array_data_ptr_outofline",
        lowering_mode: NamedIntrinsicLoweringMode::IrEmission,
        runtime_symbol: None,
        runtime_signature: None,
        runtime_reason: None,
    },
    NamedIntrinsicAuditEntry {
        name: "unsafe_mutable_array_cast",
        lowering_mode: NamedIntrinsicLoweringMode::IrEmission,
        runtime_symbol: None,
        runtime_signature: None,
        runtime_reason: None,
    },
    NamedIntrinsicAuditEntry {
        name: "unsafe_mutable_array_erase",
        lowering_mode: NamedIntrinsicLoweringMode::IrEmission,
        runtime_symbol: None,
        runtime_signature: None,
        runtime_reason: None,
    },
    NamedIntrinsicAuditEntry {
        name: "unsafe_array_cast",
        lowering_mode: NamedIntrinsicLoweringMode::IrEmission,
        runtime_symbol: None,
        runtime_signature: None,
        runtime_reason: None,
    },
    NamedIntrinsicAuditEntry {
        name: "unsafe_value_to_word",
        lowering_mode: NamedIntrinsicLoweringMode::IrEmission,
        runtime_symbol: None,
        runtime_signature: None,
        runtime_reason: None,
    },
    NamedIntrinsicAuditEntry {
        name: "unsafe_value_to_any",
        lowering_mode: NamedIntrinsicLoweringMode::IrEmission,
        runtime_symbol: None,
        runtime_signature: None,
        runtime_reason: None,
    },
    NamedIntrinsicAuditEntry {
        name: "unsafe_value_slot",
        lowering_mode: NamedIntrinsicLoweringMode::IrEmission,
        runtime_symbol: None,
        runtime_signature: None,
        runtime_reason: None,
    },
    ir_emission_entry("int_plus"),
    ir_emission_entry("int_minus"),
    ir_emission_entry("int_times"),
    ir_emission_entry("int_div"),
    ir_emission_entry("int_rem"),
    ir_emission_entry("int_unary_minus"),
    ir_emission_entry("int_unary_plus"),
    ir_emission_entry("int_inc"),
    ir_emission_entry("int_dec"),
    ir_emission_entry("int_and"),
    ir_emission_entry("int_or"),
    ir_emission_entry("int_xor"),
    ir_emission_entry("int_inv"),
    ir_emission_entry("int_shl"),
    ir_emission_entry("int_shr"),
    ir_emission_entry("int_ushr"),
    ir_emission_entry("int_lt"),
    ir_emission_entry("int_le"),
    ir_emission_entry("int_gt"),
    ir_emission_entry("int_ge"),
    ir_emission_entry("int_eq"),
    ir_emission_entry("int_ne"),
    ir_emission_entry("int_compare_to"),
    ir_emission_entry("int_hash"),
    ir_emission_entry("float_plus"),
    ir_emission_entry("float_minus"),
    ir_emission_entry("float_times"),
    ir_emission_entry("float_div"),
    ir_emission_entry("float_rem"),
    ir_emission_entry("float_unary_minus"),
    ir_emission_entry("float_unary_plus"),
    ir_emission_entry("float_lt"),
    ir_emission_entry("float_le"),
    ir_emission_entry("float_gt"),
    ir_emission_entry("float_ge"),
    ir_emission_entry("float_eq"),
    ir_emission_entry("float_ne"),
    ir_emission_entry("float_compare_to"),
    ir_emission_entry("float_abs"),
    ir_emission_entry("float_is_nan"),
    ir_emission_entry("float_is_infinite"),
    ir_emission_entry("float_hash"),
    ir_emission_entry("bool_and"),
    ir_emission_entry("bool_or"),
    ir_emission_entry("bool_xor"),
    ir_emission_entry("bool_eq"),
    ir_emission_entry("bool_ne"),
    ir_emission_entry("bool_not"),
    ir_emission_entry("char_to_int"),
    ir_emission_entry("char_hash"),
    ir_emission_entry("char_compare_to"),
    ir_emission_entry("char_equals"),
    ir_emission_entry("char_plus_int"),
    ir_emission_entry("char_minus_int"),
    ir_emission_entry("char_minus_char"),
    NamedIntrinsicAuditEntry {
        name: "dummy_runtime",
        lowering_mode: NamedIntrinsicLoweringMode::RuntimeCall,
        runtime_symbol: Some("scoop_test_named_intrinsic_dummy_runtime"),
        runtime_signature: Some(DUMMY_RUNTIME_SIGNATURE),
        runtime_reason: Some(
            "test-only validation entry: published behavior depends on an external runtime-managed counter, so the runtime boundary itself is part of the contract and must remain a RuntimeCall",
        ),
    },
    NamedIntrinsicAuditEntry {
        name: "int_to_string",
        lowering_mode: NamedIntrinsicLoweringMode::RuntimeCall,
        runtime_symbol: Some("scoop_int_to_string"),
        runtime_signature: Some(INT_TO_STRING_SIGNATURE),
        runtime_reason: Some("integer toString intrinsic"),
    },
    NamedIntrinsicAuditEntry {
        name: "bool_to_string",
        lowering_mode: NamedIntrinsicLoweringMode::RuntimeCall,
        runtime_symbol: Some("scoop_bool_to_string"),
        runtime_signature: Some(BOOL_TO_STRING_SIGNATURE),
        runtime_reason: Some("bool toString intrinsic"),
    },
    NamedIntrinsicAuditEntry {
        name: "char_to_string",
        lowering_mode: NamedIntrinsicLoweringMode::RuntimeCall,
        runtime_symbol: Some("scoop_char_to_string"),
        runtime_signature: Some(CHAR_TO_STRING_SIGNATURE),
        runtime_reason: Some("char toString intrinsic"),
    },
    NamedIntrinsicAuditEntry {
        name: "float64_to_string",
        lowering_mode: NamedIntrinsicLoweringMode::RuntimeCall,
        runtime_symbol: Some("scoop_float64_to_string"),
        runtime_signature: Some(FLOAT64_TO_STRING_SIGNATURE),
        runtime_reason: Some("Float64 toString intrinsic"),
    },
    NamedIntrinsicAuditEntry {
        name: "float32_to_string",
        lowering_mode: NamedIntrinsicLoweringMode::RuntimeCall,
        runtime_symbol: Some("scoop_float32_to_string"),
        runtime_signature: Some(FLOAT32_TO_STRING_SIGNATURE),
        runtime_reason: Some("Float32 toString intrinsic"),
    },
    NamedIntrinsicAuditEntry {
        name: "write_barrier",
        lowering_mode: NamedIntrinsicLoweringMode::RuntimeCall,
        runtime_symbol: Some("scoop_gc_write_barrier"),
        runtime_signature: Some(NamedIntrinsicRuntimeSignature {
            params: &[
                NamedIntrinsicRuntimeTy::RawPtr,
                NamedIntrinsicRuntimeTy::GcRef,
            ],
            return_ty: NamedIntrinsicRuntimeTy::GcRef,
        }),
        runtime_reason: Some(
            "GC-adjacent card marking must stay in runtime substrate because it mutates collector-owned metadata",
        ),
    },
    NamedIntrinsicAuditEntry {
        name: "composite_copy",
        lowering_mode: NamedIntrinsicLoweringMode::RuntimeCall,
        runtime_symbol: Some("scoop_composite_copy"),
        runtime_signature: Some(NamedIntrinsicRuntimeSignature {
            params: &[
                NamedIntrinsicRuntimeTy::RawPtr,
                NamedIntrinsicRuntimeTy::RawPtr,
                NamedIntrinsicRuntimeTy::RawPtr,
            ],
            return_ty: NamedIntrinsicRuntimeTy::Void,
        }),
        runtime_reason: Some(
            "descriptor-driven composite copy remains a runtime call so descriptor-owned copy hooks and GC slot coordination stay centralized",
        ),
    },
];

#[cfg_attr(not(test), allow(dead_code))]
pub fn named_intrinsic_audit_entries() -> &'static [NamedIntrinsicAuditEntry] {
    NAMED_INTRINSIC_AUDIT_ENTRIES
}

pub fn named_intrinsic_audit_entry(name: &str) -> Option<&'static NamedIntrinsicAuditEntry> {
    NAMED_INTRINSIC_AUDIT_ENTRIES
        .iter()
        .find(|entry| entry.name == name)
}

pub fn named_intrinsic_entry_name_for_root(fqn: &str) -> Option<&'static str> {
    let base = fqn
        .split("::<")
        .next()
        .unwrap_or(fqn)
        .split("$overload")
        .next()
        .unwrap_or(fqn);
    match base {
        "scoop.core.Array.size" => Some("array_size_inline"),
        "scoop.core.MutableArray.size" => Some("array_size_outofline"),
        "scoop.core.Array.get" => Some("array_get_inline"),
        "scoop.core.MutableArray.get" => Some("array_get_outofline"),
        "scoop.core.MutableArray.set" => Some("array_set_outofline"),
        "scoop.core.Array.__dataPtr" => Some("array_data_ptr_inline"),
        "scoop.core.MutableArray.__dataPtr" => Some("array_data_ptr_outofline"),
        "scoop.unsafe.__scoop_unsafe_mutable_array_cast" => Some("unsafe_mutable_array_cast"),
        "scoop.unsafe.__scoop_unsafe_mutable_array_erase" => Some("unsafe_mutable_array_erase"),
        "scoop.unsafe.__scoop_unsafe_array_cast" => Some("unsafe_array_cast"),
        "scoop.unsafe.__scoop_unsafe_value_to_word" => Some("unsafe_value_to_word"),
        "scoop.unsafe.__scoop_unsafe_value_to_any" => Some("unsafe_value_to_any"),
        "scoop.unsafe.__scoop_unsafe_value_slot" => Some("unsafe_value_slot"),
        _ => fallback_scalar_method_intrinsic_entry_name(base),
    }
}

pub fn fallback_named_intrinsic_entry_name_for_fqn(fqn: &str) -> Option<&'static str> {
    named_intrinsic_entry_name_for_root(fqn)
}

pub fn legacy_scalar_named_intrinsic_entry_name_for_fqn(fqn: &str) -> Option<&'static str> {
    let base = fqn
        .split("::<")
        .next()
        .unwrap_or(fqn)
        .split("$overload")
        .next()
        .unwrap_or(fqn);
    fallback_scalar_method_intrinsic_entry_name(base)
}

fn fallback_scalar_method_intrinsic_entry_name(base: &str) -> Option<&'static str> {
    let (owner, method) = base.rsplit_once('.')?;
    if scalar_owner_is_integer(owner) {
        return int_method_intrinsic_entry_name(method);
    }
    if matches!(owner, "scoop.core.Float32" | "scoop.core.Float64") {
        return float_method_intrinsic_entry_name(method);
    }
    if owner == "scoop.core.Bool" {
        return bool_method_intrinsic_entry_name(method);
    }
    if owner == "scoop.core.Char" {
        return char_method_intrinsic_entry_name(method);
    }
    None
}

fn scalar_owner_is_integer(owner: &str) -> bool {
    matches!(
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
    )
}

fn int_method_intrinsic_entry_name(method: &str) -> Option<&'static str> {
    match method {
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
        "ushr" => Some("int_ushr"),
        "lt" => Some("int_lt"),
        "le" => Some("int_le"),
        "gt" => Some("int_gt"),
        "ge" => Some("int_ge"),
        "eq" | "equals" => Some("int_eq"),
        "ne" | "notEquals" => Some("int_ne"),
        "compareTo" => Some("int_compare_to"),
        "hash" => Some("int_hash"),
        _ => None,
    }
}

fn float_method_intrinsic_entry_name(method: &str) -> Option<&'static str> {
    match method {
        "plus" => Some("float_plus"),
        "minus" => Some("float_minus"),
        "times" => Some("float_times"),
        "div" => Some("float_div"),
        "rem" => Some("float_rem"),
        "unaryMinus" => Some("float_unary_minus"),
        "unaryPlus" => Some("float_unary_plus"),
        "lt" => Some("float_lt"),
        "le" => Some("float_le"),
        "gt" => Some("float_gt"),
        "ge" => Some("float_ge"),
        "eq" | "equals" => Some("float_eq"),
        "ne" | "notEquals" => Some("float_ne"),
        "compareTo" => Some("float_compare_to"),
        "abs" => Some("float_abs"),
        "isNaN" => Some("float_is_nan"),
        "isInfinite" => Some("float_is_infinite"),
        "hash" => Some("float_hash"),
        _ => None,
    }
}

fn bool_method_intrinsic_entry_name(method: &str) -> Option<&'static str> {
    match method {
        "and" => Some("bool_and"),
        "or" => Some("bool_or"),
        "xor" => Some("bool_xor"),
        "eq" | "equals" => Some("bool_eq"),
        "ne" | "notEquals" => Some("bool_ne"),
        "not" | "negate" => Some("bool_not"),
        _ => None,
    }
}

fn char_method_intrinsic_entry_name(method: &str) -> Option<&'static str> {
    match method {
        "toInt" => Some("char_to_int"),
        "hash" => Some("char_hash"),
        "compareTo" => Some("char_compare_to"),
        "eq" | "equals" => Some("char_equals"),
        "plus" | "plusInt" => Some("char_plus_int"),
        "minus" | "minusInt" => Some("char_minus_int"),
        "minusChar" => Some("char_minus_char"),
        _ => None,
    }
}

pub fn parse_intrinsic_annotation_args(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
) -> Result<ParsedIntrinsicAnnotationArgs, IntrinsicAnnotationParseError> {
    if ann.args.is_empty() {
        return Ok(ParsedIntrinsicAnnotationArgs::Legacy);
    }

    if ann.args.len() != 1 {
        return Err(IntrinsicAnnotationParseError::InvalidShape { span: ann.span });
    }

    let arg = &ann.args[0];
    if arg.name.is_some() {
        return Err(IntrinsicAnnotationParseError::InvalidShape { span: arg.span });
    }
    if !matches!(arg.value.kind, ast::ExprKind::StringLit) {
        return Err(IntrinsicAnnotationParseError::ArgMustBeString {
            span: arg.value.span,
        });
    }

    let entry_name = parse_string_literal_utf8(source.slice(arg.value.span)).map_err(|_| {
        IntrinsicAnnotationParseError::ArgMustBeString {
            span: arg.value.span,
        }
    })?;
    Ok(ParsedIntrinsicAnnotationArgs::Named { entry_name })
}

pub fn best_effort_intrinsic_entry_name(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
) -> Option<String> {
    parse_intrinsic_annotation_args(source, ann)
        .ok()
        .and_then(ParsedIntrinsicAnnotationArgs::into_entry_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_file;

    #[test]
    fn runtime_entries_always_publish_symbol_signature_and_reason() {
        for entry in named_intrinsic_audit_entries() {
            if entry.lowering_mode != NamedIntrinsicLoweringMode::RuntimeCall {
                continue;
            }
            assert!(
                entry.runtime_symbol.is_some(),
                "runtime entry {:?} missing symbol",
                entry.name
            );
            assert!(
                entry.runtime_signature.is_some(),
                "runtime entry {:?} missing signature",
                entry.name
            );
            assert!(
                entry
                    .runtime_reason
                    .is_some_and(|reason| !reason.is_empty()),
                "runtime entry {:?} missing reason",
                entry.name
            );
        }
    }

    #[test]
    fn parse_named_intrinsic_annotation_positional_string() {
        let source = SourceFile::new_virtual(
            "<mem>/intrinsic_named_arg.scoop",
            "package fixtures\n@Intrinsic(\"dummy_ir\") fun foo(): Int\n",
        );
        let file = parse_file(&source).expect("parse should succeed");
        let ast::Item::Fun(fun) = &file.items[0] else {
            panic!("expected function item");
        };
        let ann = &fun.annotations[0];
        assert_eq!(
            parse_intrinsic_annotation_args(&source, ann),
            Ok(ParsedIntrinsicAnnotationArgs::Named {
                entry_name: "dummy_ir".to_string(),
            })
        );
    }
}
