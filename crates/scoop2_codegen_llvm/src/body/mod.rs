//! 函数体 lowering：把 `LirCallable` 的 body 翻译为 LLVM IR。
//!
//! `FunctionLowerer` 持有 per-function 状态。所有 rvalue/terminator 翻译在本模块及子模块。
//! 关键：LIR 的 `LirOperand::Local(id)` 不携带类型，故 FunctionLowerer 维护
//! `local_types: id → TypeId` 映射（来自 `LirLocalDecl.ty`），用于 load。

use std::collections::HashMap;

use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::values::{BasicValueEnum, FunctionValue, PointerValue};

use scoop2_hir::ty::TypeId;
use scoop2_lir::{LirBlock, LirBody, LirCallable, TypeLayoutTable};

use crate::context::CodegenContext;
use crate::error::{CodegenError, CodegenResult};

pub mod call;
pub mod consts;
pub mod control;
pub mod direct;
pub mod operand;
pub mod rvalue;
pub mod stmt;

/// per-function lowering 状态。
pub struct FunctionLowerer<'a, 'ctx> {
    pub cg: &'a CodegenContext<'ctx>,
    pub layouts: &'a TypeLayoutTable,
    pub builder: Builder<'ctx>,
    pub fv: FunctionValue<'ctx>,
    /// local id → alloca 指针（native addrspace 0）。
    pub locals: HashMap<u32, PointerValue<'ctx>>,
    /// local id → TypeId（用于无类型 operand 的 load）。
    pub local_types: HashMap<u32, TypeId>,
    /// block id → BasicBlock。
    pub blocks: HashMap<u32, BasicBlock<'ctx>>,
    /// 当前 callable（用于诊断）。
    pub fqn: String,
    /// 返回类型（用于 Return lowering）。
    pub return_ty: TypeId,
    /// runtime 函数集合（降低重复声明）。
    pub rt: &'a crate::runtime_abi::RuntimeFns<'ctx>,
    /// GC root frame 状态（None = 无 GC local，不建 frame）。
    pub root_frame: Option<crate::gc::RootFrameState<'ctx>>,
    pending_entry_br: Option<(BasicBlock<'ctx>, u32)>,
}

impl<'a, 'ctx> FunctionLowerer<'a, 'ctx> {
    /// 为一个 callable 生成函数定义（含 body）。
    pub fn lower(
        cg: &'a CodegenContext<'ctx>,
        layouts: &'a TypeLayoutTable,
        rt: &'a crate::runtime_abi::RuntimeFns<'ctx>,
        callable: &'a LirCallable,
        fv: FunctionValue<'ctx>,
    ) -> CodegenResult<()> {
        let builder = cg.context.create_builder();
        let mut fl = FunctionLowerer {
            cg,
            layouts,
            builder,
            fv,
            locals: HashMap::new(),
            local_types: HashMap::new(),
            blocks: HashMap::new(),
            fqn: callable.fqn.clone(),
            return_ty: callable.return_ty,
            rt,
            root_frame: None,
            pending_entry_br: None,
        };

        let body = callable.body.as_ref().ok_or_else(|| {
            CodegenError::unsupported("callable 无 body", &callable.fqn, scoop2_base::Span::default())
        })?;

        // 计算 GC root frame 需求。
        let mut rf = crate::gc::RootFrameState::compute(body, layouts);
        fl.alloc_locals(body, &mut rf)?;
        // entry push（在 create_blocks 的 entry→start br 之前；builder 此时位于 entry 末尾）。
        fl.emit_root_frame_push(&mut rf)?;
        fl.root_frame = Some(rf);
        // 在 root frame push 之后重新存储函数参数（GC 参数需镜像到 frame slot；
        // frame push 会清零 slots，所以参数必须在 push 之后存储）。
        fl.store_params_to_locals()?;
        fl.create_blocks(body);
        for blk in &body.blocks {
            fl.lower_block(blk)?;
        }
        Ok(())
    }

    fn alloc_locals(&mut self, body: &LirBody, rf: &mut crate::gc::RootFrameState<'ctx>) -> CodegenResult<()> {
        let entry = self.cg.context.append_basic_block(self.fv, "entry");
        self.builder.position_at_end(entry);
        for d in &body.locals {
            let ty = self.cg.lower_type(d.ty, self.layouts)?;
            let slot = self
                .builder
                .build_alloca(ty, &format!("local{}", d.id))
                .map_err(|e| CodegenError::llvm(e.to_string(), "build_alloca", scoop2_base::Span::default()))?;
            self.locals.insert(d.id, slot);
            self.local_types.insert(d.id, d.ty);
        }
        // 若需要 root frame，在此分配（entry 中）。
        if rf.needs_frame() {
            let header_ty = self.cg.root_frame_header_type();
            let ptr_ty = self.cg.native_ptr_ty();
            // frame = { header; [slot_count x ptr] }
            let frame_ty: inkwell::types::StructType<'ctx> = if rf.slot_count == 0 {
                // 0 slot：仅 header，但要保持 struct 形态以便 GEP。用 { header }。
                self.cg.context.struct_type(&[header_ty.into()], false)
            } else {
                let arr = ptr_ty.array_type(rf.slot_count);
                self.cg
                    .context
                    .struct_type(&[header_ty.into(), arr.into()], false)
            };
            let frame_ptr = self
                .builder
                .build_alloca(frame_ty, "root_frame")
                .map_err(|e| CodegenError::llvm(e.to_string(), "build_alloca root_frame", scoop2_base::Span::default()))?;
            rf.frame_ptr = Some(frame_ptr);
            rf.frame_ty = Some(frame_ty);
        }
        self.pending_entry_br = Some((entry, body.start_block));
        Ok(())
    }

    /// 在 entry（start_block br 之前）emit root frame push。
    fn emit_root_frame_push(&mut self, rf: &crate::gc::RootFrameState<'ctx>) -> CodegenResult<()> {
        let frame_ptr = match rf.frame_ptr {
            Some(p) => p,
            None => return Ok(()),
        };
        let header_ty = self.cg.root_frame_header_type();
        // 1. desc 全局。
        let desc = self
            .cg
            .create_root_frame_desc_global(&self.fqn, rf.slot_count);
        // 2. header.prev = TLS top。frame_ptr bitcast 到 header* 后 struct_gep field 0。
        let tls_global = self.cg.explicit_root_frame_top_global();
        let tls_top = self
            .builder
            .build_load(self.cg.native_ptr_ty(), tls_global, "tls_top")
            .map_err(|e| CodegenError::llvm(e.to_string(), "load tls_top", scoop2_base::Span::default()))?;
        let prev_slot = self
            .builder
            .build_struct_gep(header_ty, frame_ptr, 0, "rf_prev")
            .map_err(|e| CodegenError::llvm(e.to_string(), "gep rf_prev", scoop2_base::Span::default()))?;
        self.builder
            .build_store(prev_slot, tls_top)
            .map_err(|e| CodegenError::llvm(e.to_string(), "store rf_prev", scoop2_base::Span::default()))?;
        // 3. header.desc = desc（field 1）。
        let desc_slot = self
            .builder
            .build_struct_gep(header_ty, frame_ptr, 1, "rf_desc")
            .map_err(|e| CodegenError::llvm(e.to_string(), "gep rf_desc", scoop2_base::Span::default()))?;
        self.builder
            .build_store(desc_slot, desc)
            .map_err(|e| CodegenError::llvm(e.to_string(), "store rf_desc", scoop2_base::Span::default()))?;
        // 4. 清零所有 slot。slot 区紧跟 header（字节偏移 HEADER_SIZE）。
        if rf.slot_count > 0 {
            // slots base = (i8*)frame_ptr + HEADER_SIZE，转 ptr*。
            let i8_ptr = self.cg.native_ptr_ty();
            let slots_i8 = unsafe {
                self.builder
                    .build_in_bounds_gep(
                        self.cg.context.i8_type(),
                        frame_ptr,
                        &[self.cg.context.i64_type().const_int(crate::gc::HEADER_SIZE_BYTES, false)],
                        "rf_slots_i8",
                    )
            }
                .map_err(|e| CodegenError::llvm(e.to_string(), "gep rf_slots", scoop2_base::Span::default()))?;
            let slots_ptr = self
                .builder
                .build_bit_cast(slots_i8, i8_ptr, "rf_slots_ptr")
                .map_err(|e| CodegenError::llvm(e.to_string(), "bitcast rf_slots", scoop2_base::Span::default()))?
                .into_pointer_value();
            let native_null = self.cg.native_ptr_ty().const_null();
            for i in 0..rf.slot_count {
                let slot = unsafe {
                    self.builder
                        .build_in_bounds_gep(
                            self.cg.native_ptr_ty(),
                            slots_ptr,
                            &[self.cg.context.i64_type().const_int(i as u64, false)],
                            &format!("rf_slot{}", i),
                        )
                }
                    .map_err(|e| CodegenError::llvm(e.to_string(), "gep rf_slot", scoop2_base::Span::default()))?;
                self.builder
                    .build_store(slot, native_null)
                    .map_err(|e| CodegenError::llvm(e.to_string(), "store rf_slot zero", scoop2_base::Span::default()))?;
            }
        }
        // 5. TLS top = frame_ptr。
        self.builder
            .build_store(tls_global, frame_ptr)
            .map_err(|e| CodegenError::llvm(e.to_string(), "store tls_top", scoop2_base::Span::default()))?;
        Ok(())
    }

    /// 在当前 block 末尾 emit root frame pop（用于 Return/Unreachable 前）。
    pub fn emit_root_frame_pop(&self) -> CodegenResult<()> {
        let rf = match &self.root_frame {
            Some(rf) => rf,
            None => return Ok(()),
        };
        let frame_ptr = match rf.frame_ptr {
            Some(p) => p,
            None => return Ok(()),
        };
        let header_ty = self.cg.root_frame_header_type();
        // 清零 slot（防御性，避免 GC 在 pop 后误读）。
        if rf.slot_count > 0 {
            let slots_i8 = unsafe {
                self.builder
                    .build_in_bounds_gep(
                        self.cg.context.i8_type(),
                        frame_ptr,
                        &[self.cg.context.i64_type().const_int(crate::gc::HEADER_SIZE_BYTES, false)],
                        "rf_slots_pop",
                    )
            }
                .map_err(|e| CodegenError::llvm(e.to_string(), "gep rf_slots_pop", scoop2_base::Span::default()))?;
            let slots_ptr = self
                .builder
                .build_bit_cast(slots_i8, self.cg.native_ptr_ty(), "rf_slots_ptr_pop")
                .map_err(|e| CodegenError::llvm(e.to_string(), "bitcast rf_slots_pop", scoop2_base::Span::default()))?
                .into_pointer_value();
            let native_null = self.cg.native_ptr_ty().const_null();
            for i in 0..rf.slot_count {
                let slot = unsafe {
                    self.builder
                        .build_in_bounds_gep(
                            self.cg.native_ptr_ty(),
                            slots_ptr,
                            &[self.cg.context.i64_type().const_int(i as u64, false)],
                            &format!("rf_slot_pop{}", i),
                        )
                }
                    .map_err(|e| CodegenError::llvm(e.to_string(), "gep rf_slot_pop", scoop2_base::Span::default()))?;
                self.builder
                    .build_store(slot, native_null)
                    .map_err(|e| CodegenError::llvm(e.to_string(), "store rf_slot_pop", scoop2_base::Span::default()))?;
            }
        }
        // TLS top = header.prev。
        let prev_slot = self
            .builder
            .build_struct_gep(header_ty, frame_ptr, 0, "rf_prev_pop")
            .map_err(|e| CodegenError::llvm(e.to_string(), "gep rf_prev_pop", scoop2_base::Span::default()))?;
        let prev = self
            .builder
            .build_load(self.cg.native_ptr_ty(), prev_slot, "rf_prev_val")
            .map_err(|e| CodegenError::llvm(e.to_string(), "load rf_prev_pop", scoop2_base::Span::default()))?;
        let tls_global = self.cg.explicit_root_frame_top_global();
        self.builder
            .build_store(tls_global, prev)
            .map_err(|e| CodegenError::llvm(e.to_string(), "store tls_top_pop", scoop2_base::Span::default()))?;
        Ok(())
    }

    fn create_blocks(&mut self, body: &LirBody) {
        for blk in &body.blocks {
            let bb = self.cg.context.append_basic_block(self.fv, &format!("bb{}", blk.id));
            self.blocks.insert(blk.id, bb);
        }
        if let Some((entry, start_id)) = self.pending_entry_br.take() {
            self.builder.position_at_end(entry);
            if let Some(&start) = self.blocks.get(&start_id) {
                let _ = self.builder.build_unconditional_branch(start);
            }
        }
    }

    fn lower_block(&mut self, blk: &LirBlock) -> CodegenResult<()> {
        let bb = match self.blocks.get(&blk.id).copied() {
            Some(b) => b,
            None => {
                return Err(CodegenError::unsupported(
                    format!("block id {} 缺失", blk.id),
                    &self.fqn,
                    scoop2_base::Span::default(),
                ))
            }
        };
        self.builder.position_at_end(bb);
        for stmt in &blk.stmts {
            stmt::lower_stmt(self, stmt)?;
        }
        control::lower_terminator(self, &blk.terminator)?;
        Ok(())
    }

    /// 读取一个 local 的值（按其声明类型 load）。
    pub fn load_local(&self, id: u32) -> CodegenResult<BasicValueEnum<'ctx>> {
        let ty = self.local_types.get(&id).copied().ok_or_else(|| {
            CodegenError::unsupported(format!("local {} 类型未知", id), &self.fqn, scoop2_base::Span::default())
        })?;
        self.load_local_typed(id, ty)
    }

    /// 读取一个 local 的值（带类型 load）。
    ///
    /// GC-managed local **必须从 root frame slot load**（权威源）：
    /// immix 后端是 moving/compacting GC，会在 roots update 阶段原地改写 frame slot 里的指针
    /// 为搬迁后的新地址。alloca 中的旧值在 safepoint 后失效。frame slot 中存的是 native void*，
    /// load 后经 ptrtoint/inttoptr 还原为 `ptr addrspace(1)`。
    pub fn load_local_typed(&self, id: u32, ty: TypeId) -> CodegenResult<BasicValueEnum<'ctx>> {
        let slot = self.locals.get(&id).copied().ok_or_else(|| {
            CodegenError::unsupported(format!("local {} 未分配", id), &self.fqn, scoop2_base::Span::default())
        })?;
        let llvm_ty = self.cg.lower_type(ty, self.layouts)?;
        // GC local：从 frame slot 取权威值。
        if self.is_gc_local(id) {
            if let Some(frame_slot) = self.frame_slot_ptr(id)? {
                let native = self
                    .builder
                    .build_load(self.cg.native_ptr_ty(), frame_slot, &format!("ldf{}", id))
                    .map_err(|e| CodegenError::llvm(e.to_string(), "build_load frame", scoop2_base::Span::default()))?
                    .into_pointer_value();
                // native void* → GC ptr (addrspace 1)：经 i64 中转。
                let as_int = self
                    .builder
                    .build_ptr_to_int(native, self.cg.context.i64_type(), &format!("ldf_int{}", id))
                    .map_err(|e| CodegenError::llvm(e.to_string(), "ptr_to_int load", scoop2_base::Span::default()))?;
                let gc_ptr = self
                    .builder
                    .build_int_to_ptr(as_int, self.cg.gc_ptr_ty(), &format!("ldf_gc{}", id))
                    .map_err(|e| CodegenError::llvm(e.to_string(), "int_to_ptr load", scoop2_base::Span::default()))?;
                return Ok(gc_ptr.into());
            }
        }
        self.builder
            .build_load(llvm_ty, slot, &format!("ld{}", id))
            .map_err(|e| CodegenError::llvm(e.to_string(), "build_load", scoop2_base::Span::default()))
    }

    /// 该 local 是否 GC-managed（在 root frame 的 local_to_slot 中）。
    fn is_gc_local(&self, id: u32) -> bool {
        self.root_frame
            .as_ref()
            .and_then(|rf| rf.local_to_slot.get(&id))
            .is_some()
    }

    /// 将函数参数值存储到对应的 local slot（在 root frame push 之后调用，
    /// 以确保 GC 参数被正确镜像到 frame slot）。
    pub fn store_params_to_locals(&self) -> CodegenResult<()> {
        let param_count = self.fv.count_params();
        for i in 0..param_count {
            let local_id = i as u32;
            if let Some(param) = self.fv.get_nth_param(i) {
                self.store_local(local_id, param)?;
            }
        }
        Ok(())
    }

    /// 把一个值存入 local。
    /// GC-managed local 同时把 GC 指针写入 root frame slot（镜像）。
    pub fn store_local(&self, id: u32, val: BasicValueEnum<'ctx>) -> CodegenResult<()> {
        let slot = self.locals.get(&id).copied().ok_or_else(|| {
            CodegenError::unsupported(format!("local {} 未分配", id), &self.fqn, scoop2_base::Span::default())
        })?;
        self.builder
            .build_store(slot, val)
            .map_err(|e| CodegenError::llvm(e.to_string(), "build_store", scoop2_base::Span::default()))?;
        // GC local：镜像到 frame slot。
        if let Some(frame_slot) = self.frame_slot_ptr(id)? {
            // GC 指针（addrspace 1）cast 到 native ptr 后存入 frame slot。
            let native = self
                .builder
                .build_ptr_to_int(val.into_pointer_value(), self.cg.context.i64_type(), &format!("sf_cast{}", id))
                .map_err(|e| CodegenError::llvm(e.to_string(), "ptr_to_int frame mirror", scoop2_base::Span::default()))?;
            let native_ptr = self
                .builder
                .build_int_to_ptr(native, self.cg.native_ptr_ty(), &format!("sf_ptr{}", id))
                .map_err(|e| CodegenError::llvm(e.to_string(), "int_to_ptr frame mirror", scoop2_base::Span::default()))?;
            self.builder
                .build_store(frame_slot, native_ptr)
                .map_err(|e| CodegenError::llvm(e.to_string(), "store frame mirror", scoop2_base::Span::default()))?;
        }
        Ok(())
    }

    /// 取一个 GC local 在 root frame 中的 slot 指针（None = 非 GC local 或无 frame）。
    fn frame_slot_ptr(&self, id: u32) -> CodegenResult<Option<inkwell::values::PointerValue<'ctx>>> {
        let rf = match &self.root_frame {
            Some(rf) => rf,
            None => return Ok(None),
        };
        let frame_ptr = match rf.frame_ptr {
            Some(p) => p,
            None => return Ok(None),
        };
        let slot_index = match rf.local_to_slot.get(&id) {
            Some(&i) => i,
            None => return Ok(None),
        };
        // slot 区在 frame 偏移 HEADER_SIZE；slot_index 个 ptr。
        let slots_i8 = unsafe {
            self.builder.build_in_bounds_gep(
                self.cg.context.i8_type(),
                frame_ptr,
                &[self.cg.context.i64_type().const_int(crate::gc::HEADER_SIZE_BYTES, false)],
                "mirror_slots",
            )
        }
        .map_err(|e| CodegenError::llvm(e.to_string(), "gep mirror_slots", scoop2_base::Span::default()))?;
        let slots_ptr = self
            .builder
            .build_bit_cast(slots_i8, self.cg.native_ptr_ty(), "mirror_slots_ptr")
            .map_err(|e| CodegenError::llvm(e.to_string(), "bitcast mirror_slots", scoop2_base::Span::default()))?
            .into_pointer_value();
        let slot = unsafe {
            self.builder.build_in_bounds_gep(
                self.cg.native_ptr_ty(),
                slots_ptr,
                &[self.cg.context.i64_type().const_int(slot_index as u64, false)],
                &format!("mirror_slot{}", id),
            )
        }
        .map_err(|e| CodegenError::llvm(e.to_string(), "gep mirror_slot", scoop2_base::Span::default()))?;
        Ok(Some(slot))
    }
}
