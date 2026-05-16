//! Unit / int / enum boxing, boxed-enum type descriptor.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn codegen_box_unit_to_ref(
        &mut self,
        at: crate::span::Span,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        // 约定（early stage）：
        // - box 对象布局：`{ header: ScoopGcObjectHeader }`（无 payload）
        // - 对象头字段由 runtime 的 `scoop_alloc` 初始化（与 `codegen_box_int_to_ref` 一致）。
        let boxed_ty = self.llvm_boxed_unit_type();
        let obj_size_bytes = self.target_data.get_store_size(&boxed_ty);

        let size_v = self.context.i64_type().const_int(obj_size_bytes, false);

        let desc = self.get_or_create_boxed_unit_type_desc_global(at)?;
        let desc_i8 = self.builder.build_pointer_cast(
            desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "boxed_unit_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            at,
            rt_alloc,
            &[desc_i8.into(), size_v.into()],
            "rt_alloc_box_unit",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return value",
                at: at.into(),
            })?;

        let BasicValueEnum::PointerValue(raw_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return type",
                at: at.into(),
            });
        };

        Ok(raw_ptr)
    }

    pub(in crate::llvm::codegen) fn codegen_box_int_to_ref(
        &mut self,
        at: crate::span::Span,
        value: IntValue<'ctx>,
        value_ty: IntTy,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        // 约定（early stage）：
        // - box 对象布局：`{ header: ScoopGcObjectHeader, payload: <int> }`（TODO T0908）
        // - 当前阶段由 runtime 的 `scoop_alloc_typed` 初始化对象头字段：
        //   - `next = NULL`
        //   - `type_desc = <boxed-int type desc>`
        //   - `size_bytes = alloc_size`
        //   - `flags/mark = 0`
        //
        // 注意：这里不尝试做"复用 box 类型"或 cache；LLVM named struct 会在 module 内复用。
        let target = self.target_layout();
        let payload_size = u64::from(value_ty.bits).div_ceil(8);
        let payload_align = payload_size.clamp(1, target.pointer_align.max(1));

        // 对象头布局与 C runtime 对齐（见 `runtime/c/scoop_gc.h` 的 static asserts）。
        //
        // `ScoopGcObjectHeader` 字段：
        // - next: void*
        // - type_desc: void*
        // - size_bytes: u64
        // - flags: u32
        // - mark: u32
        let header_size = 2 * target.pointer_size + 16;
        let header_align = target.pointer_align.max(8).max(1);
        let payload_offset = align_to(header_size, payload_align);
        let obj_align = header_align.max(payload_align);
        let total_size = align_to(payload_offset.saturating_add(payload_size), obj_align);

        let size_v = self.context.i64_type().const_int(total_size, false);

        let desc = self.get_or_create_boxed_int_type_desc_global(at, value_ty)?;
        let desc_i8 = self.builder.build_pointer_cast(
            desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "boxed_int_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            at,
            rt_alloc,
            &[desc_i8.into(), size_v.into()],
            "rt_alloc_box",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return value",
                at: at.into(),
            })?;

        let BasicValueEnum::PointerValue(raw_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return type",
                at: at.into(),
            });
        };

        // 写入 payload（对象头由 runtime 初始化）。
        let boxed_ty = self.llvm_boxed_int_type(value_ty);
        let boxed_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let boxed_ptr = self
            .builder
            .build_pointer_cast(raw_ptr, boxed_ptr_ty, "boxed_int_ptr")?;

        let payload_ptr =
            self.builder
                .build_struct_gep(boxed_ty, boxed_ptr, 1, "boxed_payload_gep")?;
        let _ = self.builder.build_store(payload_ptr, value)?;

        Ok(raw_ptr)
    }

    pub(in crate::llvm::codegen) fn codegen_box_enum_to_ref(
        &mut self,
        at: crate::span::Span,
        enum_ty: TypeId,
        value: BasicValueEnum<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let payload_ty = self.llvm_basic_type_of(at, CgTy::Enum(enum_ty))?;
        let object_ty = self.llvm_boxed_enum_type(enum_ty, payload_ty)?;
        let object_size = self.target_data.get_store_size(&object_ty);
        let size_v = self.context.i64_type().const_int(object_size, false);
        let desc = self.get_or_create_boxed_enum_type_desc_global(at, enum_ty, object_ty)?;
        let desc_i8 = self.builder.build_pointer_cast(
            desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "boxed_enum_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            at,
            rt_alloc,
            &[desc_i8.into(), size_v.into()],
            "rt_alloc_box_enum",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed enum box return value",
                at: at.into(),
            })?;
        let BasicValueEnum::PointerValue(raw_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed enum box return type",
                at: at.into(),
            });
        };
        let obj_ptr = self.builder.build_pointer_cast(
            raw_ptr,
            self.llvm_ptr_type(self.gc_address_space()),
            "boxed_enum_ptr",
        )?;
        let payload_ptr =
            self.builder
                .build_struct_gep(object_ty, obj_ptr, 1, "boxed_enum_payload_gep")?;
        let _ = self.builder.build_store(payload_ptr, value)?;
        Ok(raw_ptr)
    }

    pub(in crate::llvm::codegen) fn llvm_boxed_enum_type(
        &self,
        enum_ty: TypeId,
        payload_ty: BasicTypeEnum<'ctx>,
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let key = CanonicalTextKey::new(
            self.canonical_type_key_text_for_codegen(enum_ty, "boxed enum LLVM type")?,
        );
        let name = PrivateSymbolMangler.type_name("BoxedEnum", "boxed_enum_type", &key);
        if let Some(existing) = self.context.get_struct_type(&name) {
            return Ok(existing);
        }
        let ty = self.context.opaque_struct_type(&name);
        let header_ty = self.llvm_gc_object_header_type();
        ty.set_body(&[header_ty.into(), payload_ty], false);
        Ok(ty)
    }

    pub(in crate::llvm::codegen) fn get_or_create_boxed_enum_type_desc_global(
        &mut self,
        at: crate::span::Span,
        enum_ty: TypeId,
        object_ty: StructType<'ctx>,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let base_type_key =
            self.canonical_type_key_text_for_codegen(enum_ty, "boxed enum type descriptor")?;
        let key = CanonicalTextKey::new(base_type_key.clone());
        let global_name = PrivateSymbolMangler.mangle("boxed_enum_type_desc", &key);
        if let Some(existing) = self.module.get_global(&global_name) {
            return Ok(existing);
        }
        let trace_start_offset_bytes = self
            .target_data
            .offset_of_element(&object_ty, 1)
            .unwrap_or(0);
        let type_id_key = stable_rtti_derived_type_key("boxed_enum_type_desc", &base_type_key);
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: &global_name,
            type_id_key: type_id_key.as_str(),
            obj_ty: object_ty,
            trace_start_offset_bytes,
            parent: None,
            itable: None,
            vtable: None,
        })
    }
}
