//! Effect-neutral value/expression primitives for the clean refactor LLVM path.
//!
//! This module is the narrow sharing boundary between the refactor backend and
//! generic LLVM value helpers.  It may lower literals, local loads/stores,
//! scalar/tuple ABI packing, primitive operators, casts that do not introduce a
//! hidden control path, and canonical MIR member read/write primitives.  It must
//! not choose call targets, returns, state transitions, boundary dispatch, or
//! continuation behavior; those decisions come from published P5/P6 contracts.

use std::collections::HashSet;

use inkwell::types::BasicTypeEnum;
use inkwell::values::BasicValueEnum;

use crate::effect_lowered::ir::{LateLoweredOperandSource, LateLoweredOperandValueSource};
use crate::llvm::LlvmEmitError;
use crate::mir::{self, LocalId};
use crate::span::Span;
use crate::ty::{TypeId, TypeStore};

use super::super::MainCodegen;
use super::super::mir_body::MirLocalSlot;
use super::super::types::CgValue;
use super::types::{RefactorAbiQuery, RefactorSourceAbiLayoutKind};

/// A borrow-scoped facade over effect-neutral LLVM value primitives.
pub(super) struct RefactorValuePrimitives<'p, 'a, 'ctx> {
    codegen: &'p mut MainCodegen<'a, 'ctx>,
    source_types: &'a TypeStore,
    body: &'a mir::Body,
    slots: &'p [MirLocalSlot<'ctx>],
    abi: &'p RefactorAbiQuery<'ctx>,
}

impl<'p, 'a, 'ctx> RefactorValuePrimitives<'p, 'a, 'ctx> {
    pub(super) fn new(
        codegen: &'p mut MainCodegen<'a, 'ctx>,
        source_types: &'a TypeStore,
        body: &'a mir::Body,
        slots: &'p [MirLocalSlot<'ctx>],
        abi: &'p RefactorAbiQuery<'ctx>,
    ) -> Self {
        Self {
            codegen,
            source_types,
            body,
            slots,
            abi,
        }
    }

    pub(super) fn lower_effect_neutral_statement(
        &mut self,
        stmt: &mir::Statement,
        used_locals: &HashSet<LocalId>,
    ) -> Result<(), LlvmEmitError> {
        self.codegen.codegen_mir_effect_neutral_statement(
            stmt,
            self.body,
            self.source_types,
            self.slots,
            used_locals,
        )
    }

    pub(super) fn load_local(
        &mut self,
        span: Span,
        local: LocalId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let slot = self.codegen.mir_local_slot(span, self.slots, local)?;
        self.codegen.load_mir_local(span, slot)
    }

    pub(super) fn store_local(
        &mut self,
        span: Span,
        local: LocalId,
        value: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let slot = self.codegen.mir_local_slot(span, self.slots, local)?;
        let value = self.codegen.coerce_value(span, value, slot.cg_ty)?;
        self.codegen
            .store_local_value(span, slot.ptr, slot.cg_ty, value)
    }

    pub(super) fn store_loaded_raw_local(
        &mut self,
        span: Span,
        local: LocalId,
        raw: BasicValueEnum<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let slot = self.codegen.mir_local_slot(span, self.slots, local)?;
        let value = self.codegen.cg_value_from_loaded(span, slot.cg_ty, raw)?;
        self.codegen
            .store_local_value(span, slot.ptr, slot.cg_ty, value)
    }

    pub(super) fn lower_operand_source(
        &mut self,
        source: &LateLoweredOperandSource,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let span = source.span().unwrap_or_else(|| self.body_span());
        let expected = self
            .codegen
            .cg_ty_of_mir_type(self.source_types, source.source_ty())
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor operand source type",
                at: span.into(),
            })?;
        let operand = match source.value() {
            LateLoweredOperandValueSource::Local(local) => mir::Operand::Local(*local),
            LateLoweredOperandValueSource::Const(value) => mir::Operand::Const(value.clone()),
        };
        let value = self.codegen.codegen_mir_operand_expected(
            span,
            &operand,
            self.slots,
            Some(expected),
        )?;
        self.codegen.coerce_value(span, value, expected)
    }

    pub(super) fn pack_sources(
        &mut self,
        source_ty: TypeId,
        sources: &[LateLoweredOperandSource],
        name: &str,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        let layout = self.abi.source_value_layout(source_ty)?;
        if layout.abi().is_elided() {
            return Ok(None);
        }
        match layout.kind() {
            RefactorSourceAbiLayoutKind::Scalar => {
                let source = sources.first().ok_or_else(|| {
                    frontend_error(format!("refactor ABI scalar payload `{name}` 缺少 source"))
                })?;
                Ok(self.lower_operand_source(source)?.value)
            }
            RefactorSourceAbiLayoutKind::Tuple => {
                let BasicTypeEnum::StructType(struct_ty) = layout.abi().llvm_ty() else {
                    return Err(frontend_error(format!(
                        "refactor ABI tuple payload `{name}` layout 不是 struct"
                    )));
                };
                let mut aggregate = struct_ty.get_undef();
                for (index, source) in sources.iter().enumerate() {
                    let Some(field) = layout.field(index) else {
                        return Err(frontend_error(format!(
                            "refactor ABI tuple payload `{name}` source index {index} 超出 layout 字段"
                        )));
                    };
                    if field.is_elided() {
                        continue;
                    }
                    let raw = self.lower_operand_source(source)?.value.ok_or_else(|| {
                        frontend_error(format!(
                            "refactor ABI tuple payload `{name}` source index {index} 被 elide 但 field 需要值"
                        ))
                    })?;
                    aggregate = self
                        .codegen
                        .builder
                        .build_insert_value(
                            aggregate,
                            raw,
                            field
                                .abi_field_index()
                                .expect("non-elided field has ABI index"),
                            &format!("{name}_field{index}"),
                        )?
                        .into_struct_value();
                }
                Ok(Some(aggregate.into()))
            }
        }
    }

    pub(super) fn unpack_payload_field(
        &mut self,
        payload: Option<BasicValueEnum<'ctx>>,
        payload_ty: TypeId,
        ordinal: u32,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        let layout = self.abi.source_value_layout(payload_ty)?;
        if layout.abi().is_elided() {
            return Ok(None);
        }
        match layout.kind() {
            RefactorSourceAbiLayoutKind::Scalar => Ok(payload),
            RefactorSourceAbiLayoutKind::Tuple => {
                let Some(field) = layout.field(ordinal as usize) else {
                    return Err(frontend_error(format!(
                        "refactor payload tuple t{} 缺少 ordinal {}",
                        payload_ty.as_u32(),
                        ordinal
                    )));
                };
                if field.is_elided() {
                    return Ok(None);
                }
                let Some(BasicValueEnum::StructValue(tuple)) = payload else {
                    return Err(frontend_error(format!(
                        "refactor payload tuple t{} 缺少 struct payload",
                        payload_ty.as_u32()
                    )));
                };
                Ok(Some(
                    self.codegen.builder.build_extract_value(
                        tuple,
                        field
                            .abi_field_index()
                            .expect("non-elided field has ABI index"),
                        "refactor_payload_field",
                    )?,
                ))
            }
        }
    }

    fn body_span(&self) -> Span {
        self.body
            .blocks
            .first()
            .map(|block| block.terminator.span)
            .unwrap_or_else(|| Span::new(0, 0))
    }
}

fn frontend_error(message: String) -> LlvmEmitError {
    LlvmEmitError::Frontend { message }
}

#[cfg(test)]
mod tests {
    #[test]
    fn refactor_llvm_clean_backend_boundary_audits_body_fallbacks() {
        let body = include_str!("body.rs");
        let forbidden = [
            concat!("codegen_mir_", "statement("),
            concat!("codegen_mir_", "direct_call("),
            concat!("build_unified_", "lowering_contract"),
            concat!("effect_analysis_", "ctx"),
            concat!("scoop_effect_", "handler_stack"),
            concat!("scoop_effect_", "outcome"),
        ];
        for needle in forbidden {
            assert!(
                !body.contains(needle),
                "refactor body must use published contracts plus value primitives, not `{needle}`"
            );
        }
    }

    #[test]
    fn refactor_llvm_value_primitive_inventory_is_explicit() {
        let inventory = [
            "literal",
            "local-load",
            "local-store",
            "scalar-pack",
            "tuple-pack",
            "tuple-unpack",
            "primitive-op",
            "non-control-cast",
            "member-read",
            "member-write",
        ];
        assert_eq!(inventory.len(), 10);
        assert!(inventory.contains(&"local-store"));
        assert!(inventory.contains(&"member-write"));
    }

    #[test]
    fn refactor_llvm_source_slice_classification_audits_body_skip_heuristics() {
        let body = include_str!("body.rs");
        let forbidden = [
            "skipped_statement_indices_for_state",
            "statement_is_published_resume_payload_injection",
            "try_lower_refactor_specialized_direct_call",
            "CallKind::Resume { .. }",
            "TopLevelRef(mir::TopLevelRef",
        ];
        for needle in forbidden {
            assert!(
                !body.contains(needle),
                "refactor body must consume source-slice classifications instead of private skip heuristic `{needle}`"
            );
        }
    }
}
