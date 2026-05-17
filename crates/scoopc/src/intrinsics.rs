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
pub(crate) enum NamedIntrinsicLoweringMode {
    IrEmission,
    RuntimeCall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum NamedIntrinsicRuntimeTy {
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
pub(crate) struct NamedIntrinsicRuntimeSignature {
    pub(crate) params: &'static [NamedIntrinsicRuntimeTy],
    pub(crate) return_ty: NamedIntrinsicRuntimeTy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NamedIntrinsicAuditEntry {
    pub(crate) name: &'static str,
    pub(crate) lowering_mode: NamedIntrinsicLoweringMode,
    pub(crate) runtime_symbol: Option<&'static str>,
    pub(crate) runtime_signature: Option<NamedIntrinsicRuntimeSignature>,
    pub(crate) runtime_reason: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ParsedIntrinsicAnnotationArgs {
    Legacy,
    Named { entry_name: String },
}

impl ParsedIntrinsicAnnotationArgs {
    pub(crate) fn entry_name(&self) -> Option<&str> {
        match self {
            Self::Legacy => None,
            Self::Named { entry_name } => Some(entry_name.as_str()),
        }
    }

    pub(crate) fn into_entry_name(self) -> Option<String> {
        match self {
            Self::Legacy => None,
            Self::Named { entry_name } => Some(entry_name),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IntrinsicAnnotationParseError {
    InvalidShape { span: Span },
    ArgMustBeString { span: Span },
}

const EMPTY_RUNTIME_PARAMS: &[NamedIntrinsicRuntimeTy] = &[];

const DUMMY_RUNTIME_SIGNATURE: NamedIntrinsicRuntimeSignature = NamedIntrinsicRuntimeSignature {
    params: EMPTY_RUNTIME_PARAMS,
    return_ty: NamedIntrinsicRuntimeTy::WordInt,
};

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
        name: "scalar_char_to_string_bridge",
        lowering_mode: NamedIntrinsicLoweringMode::RuntimeCall,
        runtime_symbol: Some("scoop_char_to_string"),
        runtime_signature: Some(NamedIntrinsicRuntimeSignature {
            params: &[NamedIntrinsicRuntimeTy::I32],
            return_ty: NamedIntrinsicRuntimeTy::StringRef,
        }),
        runtime_reason: Some(
            "scalar Char formatting allocates and returns a managed String in runtime substrate; sysroot helpers must import it through an audited bridge instead of widening native @Extern",
        ),
    },
    NamedIntrinsicAuditEntry {
        name: "scalar_int_to_string_bridge",
        lowering_mode: NamedIntrinsicLoweringMode::RuntimeCall,
        runtime_symbol: Some("scoop_int_to_string"),
        runtime_signature: Some(NamedIntrinsicRuntimeSignature {
            params: &[NamedIntrinsicRuntimeTy::I64],
            return_ty: NamedIntrinsicRuntimeTy::StringRef,
        }),
        runtime_reason: Some(
            "scalar Int formatting allocates and returns a managed String in runtime substrate; sysroot helpers must import it through an audited bridge instead of widening native @Extern",
        ),
    },
    NamedIntrinsicAuditEntry {
        name: "scalar_float32_to_string_bridge",
        lowering_mode: NamedIntrinsicLoweringMode::RuntimeCall,
        runtime_symbol: Some("scoop_float32_to_string"),
        runtime_signature: Some(NamedIntrinsicRuntimeSignature {
            params: &[NamedIntrinsicRuntimeTy::Float32],
            return_ty: NamedIntrinsicRuntimeTy::StringRef,
        }),
        runtime_reason: Some(
            "scalar Float32 formatting allocates and returns a managed String in runtime substrate; sysroot helpers must import it through an audited bridge instead of widening native @Extern",
        ),
    },
    NamedIntrinsicAuditEntry {
        name: "scalar_float64_to_string_bridge",
        lowering_mode: NamedIntrinsicLoweringMode::RuntimeCall,
        runtime_symbol: Some("scoop_float64_to_string"),
        runtime_signature: Some(NamedIntrinsicRuntimeSignature {
            params: &[NamedIntrinsicRuntimeTy::Float64],
            return_ty: NamedIntrinsicRuntimeTy::StringRef,
        }),
        runtime_reason: Some(
            "scalar Float64 formatting allocates and returns a managed String in runtime substrate; sysroot helpers must import it through an audited bridge instead of widening native @Extern",
        ),
    },
    NamedIntrinsicAuditEntry {
        name: "string_concat_bridge",
        lowering_mode: NamedIntrinsicLoweringMode::RuntimeCall,
        runtime_symbol: Some("scoop_string_concat"),
        runtime_signature: Some(NamedIntrinsicRuntimeSignature {
            params: &[
                NamedIntrinsicRuntimeTy::StringRef,
                NamedIntrinsicRuntimeTy::StringRef,
            ],
            return_ty: NamedIntrinsicRuntimeTy::StringRef,
        }),
        runtime_reason: Some(
            "String concatenation allocates a managed String from two byte buffers; the public helper is sysroot code, while this audited bridge keeps the allocation/copy boundary in runtime substrate",
        ),
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
pub(crate) fn named_intrinsic_audit_entries() -> &'static [NamedIntrinsicAuditEntry] {
    NAMED_INTRINSIC_AUDIT_ENTRIES
}

pub(crate) fn named_intrinsic_audit_entry(name: &str) -> Option<&'static NamedIntrinsicAuditEntry> {
    NAMED_INTRINSIC_AUDIT_ENTRIES
        .iter()
        .find(|entry| entry.name == name)
}

pub(crate) fn fallback_named_intrinsic_entry_name_for_fqn(fqn: &str) -> Option<&'static str> {
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
        _ => None,
    }
}

pub(crate) fn parse_intrinsic_annotation_args(
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

pub(crate) fn best_effort_intrinsic_entry_name(
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
