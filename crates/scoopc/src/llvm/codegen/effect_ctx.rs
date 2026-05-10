//! Backend-owned explicit `EffectCtx` / handler node layout helpers.
//!
//! 当前阶段先固定 target-shape 下的显式数据模型：
//! - `ScoopEffectCtx`：持有当前 handler 链顶；
//! - `ScoopEffectHandlerNode`：持有单个 handled-arm registration。
//!
//! 这里故意只定义 object layout / field helper / stable dispatch identity，
//! 不重新引入任何 TLS bridge 或 runtime policy。

use inkwell::types::StructType;
use inkwell::values::{IntValue, PointerValue};

use super::MainCodegen;
use crate::llvm::LlvmEmitError;
use crate::mir::SiteId;

const EFFECT_CTX_FIELD_HANDLER_TOP: u32 = 1;

const EFFECT_HANDLER_NODE_FIELD_PREV_REF: u32 = 1;
const EFFECT_HANDLER_NODE_FIELD_OP_TAG: u32 = 2;
const EFFECT_HANDLER_NODE_FIELD_FLAGS: u32 = 3;
const EFFECT_HANDLER_NODE_FIELD_OWNER_FRAME_REF: u32 = 4;
const EFFECT_HANDLER_NODE_FIELD_DISPATCH_IDENTITY: u32 = 5;

const EFFECT_HANDLER_FLAG_ACTIVE: u32 = 1;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn effect_ctx_layout_anchor_name(&self) -> &'static str {
        "scoop.runtime.ScoopEffectCtx"
    }

    pub(in crate::llvm::codegen) fn effect_handler_node_layout_anchor_name(&self) -> &'static str {
        "scoop.runtime.ScoopEffectHandlerNode"
    }

    pub(in crate::llvm::codegen) fn llvm_effect_ctx_object_type(&self) -> StructType<'ctx> {
        const TY_NAME: &str = "scoop.runtime.ScoopEffectCtx";
        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(TY_NAME);
        ty.set_body(
            &[
                self.llvm_gc_object_header_type().into(),
                self.llvm_gc_i8_ptr_type().into(),
            ],
            false,
        );
        ty
    }

    pub(in crate::llvm::codegen) fn llvm_effect_handler_node_object_type(
        &self,
    ) -> StructType<'ctx> {
        const TY_NAME: &str = "scoop.runtime.ScoopEffectHandlerNode";
        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(TY_NAME);
        ty.set_body(
            &[
                self.llvm_gc_object_header_type().into(),
                self.llvm_gc_i8_ptr_type().into(),
                self.context.i32_type().into(),
                self.context.i32_type().into(),
                self.llvm_gc_i8_ptr_type().into(),
                self.context.i64_type().into(),
            ],
            false,
        );
        ty
    }

    pub(in crate::llvm::codegen) fn effect_handler_active_flag(&self) -> u32 {
        EFFECT_HANDLER_FLAG_ACTIVE
    }

    pub(in crate::llvm::codegen) fn effect_handler_dispatch_identity(
        &self,
        site_id: SiteId,
        arm_ordinal: u32,
    ) -> u64 {
        (u64::from(site_id.as_u32()) << 32) | u64::from(arm_ordinal)
    }

    pub(in crate::llvm::codegen) fn effect_handler_dispatch_identity_const(
        &self,
        site_id: SiteId,
        arm_ordinal: u32,
    ) -> IntValue<'ctx> {
        self.context.i64_type().const_int(
            self.effect_handler_dispatch_identity(site_id, arm_ordinal),
            false,
        )
    }

    pub(in crate::llvm::codegen) fn load_effect_ctx_handler_top(
        &mut self,
        ctx_ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let field_ptr = self.builder.build_struct_gep(
            self.llvm_effect_ctx_object_type(),
            ctx_ptr,
            EFFECT_CTX_FIELD_HANDLER_TOP,
            &format!("{name}_effect_ctx_handler_top_gep"),
        )?;
        Ok(self
            .builder
            .build_load(
                self.llvm_gc_i8_ptr_type(),
                field_ptr,
                &format!("{name}_effect_ctx_handler_top"),
            )?
            .into_pointer_value())
    }

    pub(in crate::llvm::codegen) fn store_effect_ctx_handler_top(
        &mut self,
        at: crate::span::Span,
        ctx_ptr: PointerValue<'ctx>,
        handler_top: PointerValue<'ctx>,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        let field_ptr = self.builder.build_struct_gep(
            self.llvm_effect_ctx_object_type(),
            ctx_ptr,
            EFFECT_CTX_FIELD_HANDLER_TOP,
            &format!("{name}_effect_ctx_handler_top_gep"),
        )?;
        self.store_gc_pointer_slot_with_write_barrier(at, field_ptr, handler_top)
    }

    fn effect_handler_node_field_ptr(
        &mut self,
        node_ptr: PointerValue<'ctx>,
        field_index: u32,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        self.builder
            .build_struct_gep(
                self.llvm_effect_handler_node_object_type(),
                node_ptr,
                field_index,
                name,
            )
            .map_err(Into::into)
    }

    pub(in crate::llvm::codegen) fn load_effect_handler_prev_ref(
        &mut self,
        node_ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let field_ptr = self.effect_handler_node_field_ptr(
            node_ptr,
            EFFECT_HANDLER_NODE_FIELD_PREV_REF,
            &format!("{name}_effect_handler_prev_gep"),
        )?;
        Ok(self
            .builder
            .build_load(
                self.llvm_gc_i8_ptr_type(),
                field_ptr,
                &format!("{name}_effect_handler_prev"),
            )?
            .into_pointer_value())
    }

    pub(in crate::llvm::codegen) fn store_effect_handler_prev_ref(
        &mut self,
        at: crate::span::Span,
        node_ptr: PointerValue<'ctx>,
        prev_ref: PointerValue<'ctx>,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        let field_ptr = self.effect_handler_node_field_ptr(
            node_ptr,
            EFFECT_HANDLER_NODE_FIELD_PREV_REF,
            &format!("{name}_effect_handler_prev_gep"),
        )?;
        self.store_gc_pointer_slot_with_write_barrier(at, field_ptr, prev_ref)
    }

    pub(in crate::llvm::codegen) fn load_effect_handler_op_tag(
        &mut self,
        node_ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let field_ptr = self.effect_handler_node_field_ptr(
            node_ptr,
            EFFECT_HANDLER_NODE_FIELD_OP_TAG,
            &format!("{name}_effect_handler_op_tag_gep"),
        )?;
        Ok(self
            .builder
            .build_load(
                self.context.i32_type(),
                field_ptr,
                &format!("{name}_effect_handler_op_tag"),
            )?
            .into_int_value())
    }

    pub(in crate::llvm::codegen) fn store_effect_handler_op_tag(
        &mut self,
        node_ptr: PointerValue<'ctx>,
        op_tag: IntValue<'ctx>,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        let field_ptr = self.effect_handler_node_field_ptr(
            node_ptr,
            EFFECT_HANDLER_NODE_FIELD_OP_TAG,
            &format!("{name}_effect_handler_op_tag_gep"),
        )?;
        self.builder.build_store(field_ptr, op_tag)?;
        Ok(())
    }

    pub(in crate::llvm::codegen) fn load_effect_handler_flags(
        &mut self,
        node_ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let field_ptr = self.effect_handler_node_field_ptr(
            node_ptr,
            EFFECT_HANDLER_NODE_FIELD_FLAGS,
            &format!("{name}_effect_handler_flags_gep"),
        )?;
        Ok(self
            .builder
            .build_load(
                self.context.i32_type(),
                field_ptr,
                &format!("{name}_effect_handler_flags"),
            )?
            .into_int_value())
    }

    pub(in crate::llvm::codegen) fn store_effect_handler_flags(
        &mut self,
        node_ptr: PointerValue<'ctx>,
        flags: IntValue<'ctx>,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        let field_ptr = self.effect_handler_node_field_ptr(
            node_ptr,
            EFFECT_HANDLER_NODE_FIELD_FLAGS,
            &format!("{name}_effect_handler_flags_gep"),
        )?;
        self.builder.build_store(field_ptr, flags)?;
        Ok(())
    }

    pub(in crate::llvm::codegen) fn load_effect_handler_owner_frame_ref(
        &mut self,
        node_ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let field_ptr = self.effect_handler_node_field_ptr(
            node_ptr,
            EFFECT_HANDLER_NODE_FIELD_OWNER_FRAME_REF,
            &format!("{name}_effect_handler_owner_frame_gep"),
        )?;
        Ok(self
            .builder
            .build_load(
                self.llvm_gc_i8_ptr_type(),
                field_ptr,
                &format!("{name}_effect_handler_owner_frame"),
            )?
            .into_pointer_value())
    }

    pub(in crate::llvm::codegen) fn store_effect_handler_owner_frame_ref(
        &mut self,
        at: crate::span::Span,
        node_ptr: PointerValue<'ctx>,
        owner_frame_ref: PointerValue<'ctx>,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        let field_ptr = self.effect_handler_node_field_ptr(
            node_ptr,
            EFFECT_HANDLER_NODE_FIELD_OWNER_FRAME_REF,
            &format!("{name}_effect_handler_owner_frame_gep"),
        )?;
        self.store_gc_pointer_slot_with_write_barrier(at, field_ptr, owner_frame_ref)
    }

    pub(in crate::llvm::codegen) fn load_effect_handler_dispatch_identity(
        &mut self,
        node_ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let field_ptr = self.effect_handler_node_field_ptr(
            node_ptr,
            EFFECT_HANDLER_NODE_FIELD_DISPATCH_IDENTITY,
            &format!("{name}_effect_handler_dispatch_identity_gep"),
        )?;
        Ok(self
            .builder
            .build_load(
                self.context.i64_type(),
                field_ptr,
                &format!("{name}_effect_handler_dispatch_identity"),
            )?
            .into_int_value())
    }

    pub(in crate::llvm::codegen) fn store_effect_handler_dispatch_identity(
        &mut self,
        node_ptr: PointerValue<'ctx>,
        dispatch_identity: IntValue<'ctx>,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        let field_ptr = self.effect_handler_node_field_ptr(
            node_ptr,
            EFFECT_HANDLER_NODE_FIELD_DISPATCH_IDENTITY,
            &format!("{name}_effect_handler_dispatch_identity_gep"),
        )?;
        self.builder.build_store(field_ptr, dispatch_identity)?;
        Ok(())
    }
}
