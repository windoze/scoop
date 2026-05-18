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
                let Some(CgTy::Int(int_ty)) = self.cg_ty_of(ty) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "int literal type",
                        at: span.into(),
                    });
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
        // 1) 分配一个 GC-managed `ScoopString` 对象：
        //    - LLVM 侧类型为 `ScoopString addrspace(1)*`
        //    - 分配通过 `scoop_alloc_typed(desc, sizeof(ScoopString))`（runtime 写入对象头 type_desc）
        let scoop_str_ty = self.llvm_scoop_string_type();
        let obj_size = self.target_data.get_store_size(&scoop_str_ty);
        let size_v = self.context.i64_type().const_int(obj_size, false);

        let str_desc = self.get_or_create_string_type_desc_global(span)?;
        let str_desc_i8 = self.builder.build_pointer_cast(
            str_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "str_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            span,
            rt_alloc,
            &[str_desc_i8.into(), size_v.into()],
            "rt_alloc_string_lit",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return value",
                at: span.into(),
            })?;
        let raw_ptr = self.expect_pointer_value(raw, "scoop_alloc_typed string allocation");

        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let str_ptr = self
            .builder
            .build_pointer_cast(raw_ptr, str_ptr_ty, "str_obj_ptr")?;

        // 2) 写入 `{ len, data }`。
        let len_ptr = self
            .builder
            .build_struct_gep(scoop_str_ty, str_ptr, 1, "str_len_gep")?;
        let data_ptr = self
            .builder
            .build_struct_gep(scoop_str_ty, str_ptr, 2, "str_data_gep")?;

        let len = self.context.i64_type().const_int(bytes.len() as u64, false);
        let _ = self.builder.build_store(len_ptr, len)?;

        // 空串：保持 `data = NULL`（与 runtime 侧空串约定一致）。
        if bytes.is_empty() {
            let i8_ptr_ty = self.llvm_i8_ptr_type();
            let _ = self.builder.build_store(data_ptr, i8_ptr_ty.const_null())?;
        } else {
            // 把字节序列落到一个只读全局常量：`[N x i8] @__scoop_str_data_*`
            let data_gv = self.get_or_create_global_bytes(span, bytes);
            let i8_ptr_ty = self.llvm_i8_ptr_type();
            let data_i8_ptr = self.builder.build_pointer_cast(
                data_gv.as_pointer_value(),
                i8_ptr_ty,
                "str_data_ptr",
            )?;
            let _ = self.builder.build_store(data_ptr, data_i8_ptr)?;
        }

        Ok(CgValue {
            ty: CgTy::String,
            value: Some(str_ptr.into()),
        })
    }
}
