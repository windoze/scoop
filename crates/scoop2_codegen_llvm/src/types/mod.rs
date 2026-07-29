//! 类型 lowering：`TypeId` / `TypeLayout` → LLVM `BasicTypeEnum`。
//!
//! 这是 codegen 的核心基础。所有类型决策取自 LIR 的 `TypeLayoutTable`。
//!
//! 注意：本模块当前为**初始骨架实现**，覆盖标量/引用/简单 struct 的正确 lowering，
//! 复合类型（带 padding 的 struct、enum tagged union、Option niche、class 对象头）
//! 的完整布局在后续迭代中逐步精确化（见 NEW-LLVM-CODEGEN.md §3.1）。

use inkwell::types::BasicTypeEnum;

use scoop2_hir::ty::TypeId;
use scoop2_lir::{NicheStorage, ScalarKind, TypeLayout, TypeLayoutKind, TypeLayoutTable};

use crate::context::{CodegenContext, gc_address_space};
use crate::error::{CodegenError, CodegenResult};

impl<'ctx> CodegenContext<'ctx> {
    /// 把一个 TypeId 降级为 LLVM BasicType。
    /// 查缓存；未命中则按 `TypeLayoutTable` 计算。
    pub fn lower_type(
        &self,
        ty: TypeId,
        layouts: &TypeLayoutTable,
    ) -> CodegenResult<BasicTypeEnum<'ctx>> {
        if let Some(cached) = self.lookup_type(ty) {
            return Ok(cached);
        }
        let layout = layouts.get(ty).ok_or_else(|| {
            CodegenError::missing_layout(ty.0, "lower_type", scoop2_base::Span::default())
        })?;
        let llvm_ty = self.lower_type_layout(layout, layouts, ty)?;
        self.cache_type(ty, llvm_ty);
        Ok(llvm_ty)
    }

    /// 降级一个 TypeLayout（递归类型会按 named struct 处理）。
    fn lower_type_layout(
        &self,
        layout: &TypeLayout,
        layouts: &TypeLayoutTable,
        ty: TypeId,
    ) -> CodegenResult<BasicTypeEnum<'ctx>> {
        let ctx = self.context;
        match &layout.kind {
            TypeLayoutKind::Scalar { scalar_kind } => Ok(match scalar_kind {
                ScalarKind::Unit => ctx.i8_type().into(), // Unit 用 i8 占位（0 字节语义）
                ScalarKind::Bool => ctx.i8_type().into(),
                ScalarKind::Char => ctx.i32_type().into(),
                ScalarKind::Int { bits, .. } => ctx.custom_width_int_type(*bits as u32).into(),
                ScalarKind::Float { bits } => match *bits {
                    32 => ctx.f32_type().into(),
                    64 => ctx.f64_type().into(),
                    other => {
                        return Err(CodegenError::unsupported(
                            format!("Float bits={other}"),
                            "lower_type_layout",
                            scoop2_base::Span::default(),
                        ));
                    }
                },
            }),
            TypeLayoutKind::Reference { .. } | TypeLayoutKind::Function => {
                Ok(ctx.ptr_type(gc_address_space()).into())
            }
            TypeLayoutKind::Nothing => Ok(ctx.i8_type().into()),
            TypeLayoutKind::Struct { fields } | TypeLayoutKind::Tuple { elements: fields } => {
                self.lower_record(fields, layouts)
            }
            TypeLayoutKind::Option {
                storage,
                payload_ty,
                ..
            } => self.lower_option(storage, *payload_ty, layouts),
            TypeLayoutKind::Enum { tag_size, .. } => self.lower_enum(*tag_size, layout),
        }
        .map(|t| {
            let _ = (layouts, ty);
            t
        })
    }

    fn lower_record(
        &self,
        fields: &[scoop2_lir::FieldLayout],
        layouts: &TypeLayoutTable,
    ) -> CodegenResult<BasicTypeEnum<'ctx>> {
        // 简单实现：按字段顺序构建 struct（padding 由 LLVM struct 自然对齐处理）。
        // 完整实现需按 offset 插 padding；当前先按顺序（大多数情况下正确）。
        let mut field_tys: Vec<BasicTypeEnum<'ctx>> = Vec::with_capacity(fields.len());
        for f in fields {
            field_tys.push(self.lower_type(f.ty, layouts)?);
        }
        // 零字段 record 用空 struct `{}`（不是 `[0 x i8]`）——下游统一按
        // struct 处理（build_struct_gep / into_struct_type 对 array 会 panic）。
        let st = self.context.struct_type(&field_tys, false);
        Ok(st.into())
    }

    fn lower_option(
        &self,
        storage: &NicheStorage,
        payload_ty: TypeId,
        layouts: &TypeLayoutTable,
    ) -> CodegenResult<BasicTypeEnum<'ctx>> {
        match storage {
            NicheStorage::Pointer => {
                // None = null；Some(r) = r 本身。
                Ok(self.context.ptr_type(gc_address_space()).into())
            }
            NicheStorage::U8 { .. } => {
                // Option<Bool> 等：payload 整型 + none_value 编码 None。
                // 表示 = payload 的 LLVM 整型（None 用 none_value 填充）。
                let payload_llvm = self.lower_type(payload_ty, layouts)?;
                match payload_llvm {
                    BasicTypeEnum::IntType(_) => Ok(payload_llvm),
                    // payload 非整型时 U8 niche 不可用；上游布局不会这么选，
                    // 防御：回退 i8（编码空间足够）。
                    _ => Ok(self.context.i8_type().into()),
                }
            }
            NicheStorage::Tagged => {
                // { i8 tag; payload }（tag: 0 = Some, 1 = None——
                // 即 sysroot Option 的 Some/None 声明序）。
                let payload_llvm = self.lower_type(payload_ty, layouts)?;
                let st = self
                    .context
                    .struct_type(&[self.context.i8_type().into(), payload_llvm], false);
                Ok(st.into())
            }
        }
    }

    fn lower_enum(&self, tag_size: u64, layout: &TypeLayout) -> CodegenResult<BasicTypeEnum<'ctx>> {
        // 简化：tag + 字节数组 payload。完整实现需对齐 + GC 指针单列字段。
        let tag_bits = (tag_size.max(1) as u32) * 8;
        let tag_ty = self.context.custom_width_int_type(tag_bits);
        let payload_bytes = layout.size.saturating_sub(tag_size.max(1));
        let payload_ty = self.context.i8_type().array_type(payload_bytes as u32);
        let st = self
            .context
            .struct_type(&[tag_ty.into(), payload_ty.into()], false);
        Ok(st.into())
    }

    /// GC 引用指针类型（addrspace 1）。
    pub fn gc_ptr_ty(&self) -> inkwell::types::PointerType<'ctx> {
        self.context.ptr_type(gc_address_space())
    }
    /// native 指针类型（addrspace 0）。
    pub fn native_ptr_ty(&self) -> inkwell::types::PointerType<'ctx> {
        self.context
            .ptr_type(crate::context::native_address_space())
    }
}
