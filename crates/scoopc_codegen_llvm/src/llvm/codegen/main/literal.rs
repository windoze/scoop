//! Return / literal / string / interpolated-string lowering.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn emit_return(
        &mut self,
        _span: crate::span::Span,
        declared_return_ty: CgTy,
        value: CgValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        match declared_return_ty {
            CgTy::Unit => {
                self.builder.build_return(None)?;
                Ok(())
            }
            // T1612: A function declared as returning Nothing never returns normally.
            // Emit `unreachable` instead of a return instruction.
            CgTy::Never => {
                self.builder.build_unreachable()?;
                Ok(())
            }
            CgTy::Bool
            | CgTy::Float64
            | CgTy::Float32
            | CgTy::Int(_)
            | CgTy::String
            | CgTy::Ref
            | CgTy::Tuple(_)
            | CgTy::Struct(_)
            | CgTy::Enum(_) => {
                let raw = value.value.unwrap_or_else(|| {
                    if self.function_cx.current_sret_return_ptr.is_some() {
                        panic!(
                            "emit_return: MIR return contract accepted missing sret return value"
                        )
                    }
                    panic!("emit_return: MIR return contract accepted missing return value")
                });
                if let Some(sret_ptr) = self.function_cx.current_sret_return_ptr
                    && matches!(
                        declared_return_ty,
                        CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_)
                    )
                {
                    let _ = self.builder.build_store(sret_ptr, raw)?;
                    self.builder.build_return(None)?;
                } else {
                    self.builder.build_return(Some(&raw))?;
                }
                Ok(())
            }
        }
    }

    pub(in crate::llvm::codegen) fn codegen_literal(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
        lit: &hir::LiteralKind,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match lit {
            hir::LiteralKind::Unit => Ok(CgValue::unit()),
            hir::LiteralKind::Bool(v) => Ok(CgValue::bool(
                self.context.bool_type().const_int(*v as u64, false),
            )),
            hir::LiteralKind::Char(value) => Ok(CgValue::int(
                self.context.i32_type().const_int(*value as u64, false),
                IntTy {
                    bits: 32,
                    signed: false,
                },
            )),
            hir::LiteralKind::Int => {
                let CgTy::Int(int_ty) = self.cg_ty_of_type_id(ty, "integer literal target type")
                else {
                    panic!(
                        "codegen_literal: typecheck gate accepted non-integer target for int literal"
                    )
                };
                let value = self.int_literal_bits_for_ty(span, int_ty)?;
                Ok(CgValue::int(
                    self.int_type(int_ty).const_int(value, false),
                    int_ty,
                ))
            }
            hir::LiteralKind::Float64(value) => Ok(CgValue::float(
                self.context.f64_type().const_float(*value),
                CgTy::Float64,
            )),
            hir::LiteralKind::Float32(value) => Ok(CgValue::float(
                self.context.f32_type().const_float(f64::from(*value)),
                CgTy::Float32,
            )),
            hir::LiteralKind::String => self.codegen_string_literal(span),
            hir::LiteralKind::SynthString(value) => {
                self.codegen_string_literal_from_text(span, value)
            }
            hir::LiteralKind::SynthInt(value) => {
                // Synthesized integer literal from compiler desugaring (T0110).
                let int_ty = IntTy {
                    bits: 64,
                    signed: true,
                };
                Ok(CgValue::int(
                    self.int_type(int_ty).const_int(*value as u64, false),
                    int_ty,
                ))
            }
        }
    }

    /// Emit LLVM IR for a string literal by parsing the current source text on demand.
    pub(in crate::llvm::codegen) fn codegen_string_literal(
        &mut self,
        span: crate::span::Span,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let bytes = self.parse_current_string_literal_bytes(span)?;
        self.codegen_string_literal_from_bytes(span, &bytes)
    }

    pub(in crate::llvm::codegen) fn codegen_string_literal_from_text(
        &mut self,
        span: crate::span::Span,
        text: &str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_string_literal_from_bytes(span, text.as_bytes())
    }

    /// Emit LLVM IR for a string literal from already parsed bytes.
    pub(in crate::llvm::codegen) fn codegen_string_literal_from_bytes(
        &mut self,
        span: crate::span::Span,
        bytes: &[u8],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let str_global = self.get_or_create_immortal_string_global(span, bytes)?;

        Ok(CgValue {
            ty: CgTy::String,
            value: Some(str_global.as_pointer_value().into()),
        })
    }
}
