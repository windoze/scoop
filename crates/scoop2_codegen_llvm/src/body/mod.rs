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
    /// 闭包 invoke 函数的 env tuple 类型（首参名为 `$env` 时存在）。
    /// 统一 ABI 下 `$env` 按 GC 指针传入，入口需解包成 env tuple 值。
    pub closure_env_ty: Option<TypeId>,
    /// 第 i 个 LLVM 参数对应的 body local id（来自 `LirParam::local_id`）。
    pub param_local_ids: Vec<u32>,
    /// runtime 函数集合（降低重复声明）。
    pub rt: &'a crate::runtime_abi::RuntimeFns<'ctx>,
    /// GC root frame 状态（None = 无 GC local，不建 frame）。
    pub root_frame: Option<crate::gc::RootFrameState<'ctx>>,
    /// EffectStep 模式：frame 堆化上下文（`sym$step(frame, word)` 编译模式）。
    pub effect: Option<EffectStepCtx<'ctx>>,
    pending_entry_br: Option<(BasicBlock<'ctx>, u32)>,
}

/// EffectStep `sym$step` 函数的 lowering 上下文。
///
/// step 函数签名为 `(ptr frame, i64 word) -> Step`：frame 是堆分配的 GC
/// 对象（布局 = object header + frame tuple payload），word 是 resume 值
/// （初始调用为 0）。body 内对 frame local 的 TupleIndex/StoreTupleIndex
/// 全部走堆 GEP（见 rvalue.rs / stmt.rs 的 frame 特例）。
pub struct EffectStepCtx<'ctx> {
    /// body 内持有 frame 堆指针的 local id（GC root slot 重载协议）。
    pub frame_local: u32,
    /// frame tuple 类型（算槽位字节偏移）。
    pub frame_ty: TypeId,
    /// 参数槽表：`(参数 local id, frame slot 下标)`。
    pub param_slots: Vec<(u32, u64)>,
    /// resume 续点表（块 id → 投递目标）。
    pub resume_points: Vec<scoop2_lir::LirResumePoint>,
    /// resume word 的 alloca（入口存 param1，续点块首读出转换）。
    pub resume_word_alloca: Option<PointerValue<'ctx>>,
    /// `sym$step` 的符号名（MakeContinuation 写 step_fn 字段用）。
    pub step_fn_sym: String,
}

impl<'ctx> EffectStepCtx<'ctx> {
    /// 该块是否是 resume 续点。
    pub fn resume_point_for(&self, block_id: u32) -> Option<&scoop2_lir::LirResumePoint> {
        self.resume_points.iter().find(|rp| rp.block_id == block_id)
    }
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
            closure_env_ty: callable
                .params
                .first()
                .filter(|p| p.name == "$env")
                .map(|p| p.ty),
            param_local_ids: callable.params.iter().map(|p| p.local_id).collect(),
            rt,
            root_frame: None,
            effect: None,
            pending_entry_br: None,
        };

        let body = callable.body.as_ref().ok_or_else(|| {
            CodegenError::unsupported(
                "callable 无 body",
                &callable.fqn,
                scoop2_base::Span::default(),
            )
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

    /// 为 EffectStep callable 生成 `sym$step(frame, word)` 函数定义。
    ///
    /// step 函数签名固定为 `(ptr frame, i64 word) -> Step`：frame 是堆分配的
    /// GC 对象（wrapper `sym` 负责分配/清零/写参数槽），word 是 resume 值
    /// （初始调用 = 0）。入口把 frame 存入 frame local（GC root slot 协议）、
    /// word 存入 alloca（续点块首取用），并从 frame 参数槽恢复各参数 local。
    pub fn lower_effect_step(
        cg: &'a CodegenContext<'ctx>,
        layouts: &'a TypeLayoutTable,
        rt: &'a crate::runtime_abi::RuntimeFns<'ctx>,
        callable: &'a LirCallable,
        fv: FunctionValue<'ctx>,
        step_fn_sym: String,
    ) -> CodegenResult<()> {
        let ei = callable.effect_info.as_ref().ok_or_else(|| {
            CodegenError::unsupported(
                "EffectStep callable 缺 effect_info",
                &callable.fqn,
                scoop2_base::Span::default(),
            )
        })?;
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
            closure_env_ty: None,
            param_local_ids: Vec::new(),
            rt,
            root_frame: None,
            effect: Some(EffectStepCtx {
                frame_local: ei.frame_local,
                frame_ty: ei.frame_ty,
                param_slots: ei.param_slots.clone(),
                resume_points: ei.resume_points.clone(),
                resume_word_alloca: None,
                step_fn_sym,
            }),
            pending_entry_br: None,
        };

        let body = callable.body.as_ref().ok_or_else(|| {
            CodegenError::unsupported(
                "callable 无 body",
                &callable.fqn,
                scoop2_base::Span::default(),
            )
        })?;

        let mut rf = crate::gc::RootFrameState::compute(body, layouts);
        fl.alloc_locals(body, &mut rf)?;
        fl.emit_root_frame_push(&mut rf)?;
        fl.root_frame = Some(rf);
        // resume word alloca（entry 内、entry→start br 之前）。
        let i64_ty = cg.context.i64_type();
        let word_alloca = fl
            .builder
            .build_alloca(i64_ty, "resume_word")
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "alloca resume_word", scoop2_base::Span::default())
            })?;
        // param0 = frame（native ptr）→ frame local（GC 协议：store_local 镜像
        // 到 root slot；之后所有 frame 访问从 root slot 重载）。
        let frame_param = fv.get_nth_param(0).ok_or_else(|| {
            CodegenError::llvm(
                "step 函数缺 frame 参数".to_string(),
                &callable.fqn,
                scoop2_base::Span::default(),
            )
        })?;
        fl.store_local(ei.frame_local, frame_param)?;
        // param1 = resume word → alloca。
        let word_param = fv.get_nth_param(1).ok_or_else(|| {
            CodegenError::llvm(
                "step 函数缺 word 参数".to_string(),
                &callable.fqn,
                scoop2_base::Span::default(),
            )
        })?;
        fl.builder.build_store(word_alloca, word_param).map_err(|e| {
            CodegenError::llvm(e.to_string(), "store resume_word", scoop2_base::Span::default())
        })?;
        if let Some(effect) = fl.effect.as_mut() {
            effect.resume_word_alloca = Some(word_alloca);
        }
        // 从 frame 参数槽恢复各参数 local（resume 重入时参数只能经 frame 传入）。
        let param_slots = ei.param_slots.clone();
        for (local_id, slot) in param_slots {
            let local_ty = fl.local_types.get(&local_id).copied().ok_or_else(|| {
                CodegenError::unsupported(
                    format!("参数 local {} 类型未知", local_id),
                    &callable.fqn,
                    scoop2_base::Span::default(),
                )
            })?;
            let slot_ptr = fl.frame_slot_ptr_at(slot)?;
            let slot_llvm_ty = fl.cg.lower_type(local_ty, fl.layouts)?;
            let v = fl
                .builder
                .build_load(slot_llvm_ty, slot_ptr, &format!("param_slot{}", local_id))
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "load frame param slot",
                        scoop2_base::Span::default(),
                    )
                })?;
            fl.store_local(local_id, v)?;
        }
        fl.create_blocks(body);
        for blk in &body.blocks {
            fl.lower_block(blk)?;
        }
        Ok(())
    }

    /// frame local 的当前堆指针（root slot 重载 → native ptr）。
    pub fn effect_frame_ptr(&self) -> CodegenResult<PointerValue<'ctx>> {
        let (frame_local, frame_ty) = match &self.effect {
            Some(e) => (e.frame_local, e.frame_ty),
            None => {
                return Err(CodegenError::llvm(
                    "非 EffectStep 函数无 frame".to_string(),
                    &self.fqn,
                    scoop2_base::Span::default(),
                ))
            }
        };
        let v = self.load_local_typed(frame_local, frame_ty)?;
        let p = expect_ptr_val(v, "effect frame local", &self.fqn)?;
        if p.get_type().get_address_space() == crate::context::gc_address_space() {
            let as_int = self
                .builder
                .build_ptr_to_int(p, self.cg.context.i64_type(), "frame_int")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "frame ptrtoint", scoop2_base::Span::default())
                })?;
            Ok(self
                .builder
                .build_int_to_ptr(as_int, self.cg.native_ptr_ty(), "frame_native")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "frame inttoptr", scoop2_base::Span::default())
                })?)
        } else {
            Ok(p)
        }
    }

    /// frame slot 下标 → frame tuple payload 内字节偏移（不含 object header）。
    pub fn frame_slot_byte_offset(&self, slot: u64) -> CodegenResult<u64> {
        let frame_ty = match &self.effect {
            Some(e) => e.frame_ty,
            None => {
                return Err(CodegenError::llvm(
                    "非 EffectStep 函数无 frame".to_string(),
                    &self.fqn,
                    scoop2_base::Span::default(),
                ))
            }
        };
        let layout = self.layouts.get(frame_ty).ok_or_else(|| {
            CodegenError::llvm(
                "frame tuple 布局缺失".to_string(),
                &self.fqn,
                scoop2_base::Span::default(),
            )
        })?;
        let elements = match &layout.kind {
            scoop2_lir::TypeLayoutKind::Tuple { elements } => elements,
            _ => {
                return Err(CodegenError::llvm(
                    "frame 类型不是 tuple".to_string(),
                    &self.fqn,
                    scoop2_base::Span::default(),
                ))
            }
        };
        elements
            .get(slot as usize)
            .map(|f| f.offset)
            .ok_or_else(|| {
                CodegenError::llvm(
                    format!("frame slot {} 越界", slot),
                    &self.fqn,
                    scoop2_base::Span::default(),
                )
            })
    }

    /// frame slot 的堆内地址（`frame_ptr + object_header + slot_offset`）。
    pub fn frame_slot_ptr_at(&self, slot: u64) -> CodegenResult<PointerValue<'ctx>> {
        let base = self.effect_frame_ptr()?;
        let off = self.frame_slot_byte_offset(slot)?;
        let header = self
            .cg
            .target_data
            .get_store_size(&self.cg.object_header_type());
        unsafe {
            self.builder.build_in_bounds_gep(
                self.cg.context.i8_type(),
                base,
                &[self.cg.context.i64_type().const_int(header + off, false)],
                &format!("frame_slot{}", slot),
            )
        }
        .map_err(|e| CodegenError::llvm(e.to_string(), "gep frame slot", scoop2_base::Span::default()))
    }

    /// 该 local 是否是 EffectStep frame local。
    pub fn is_effect_frame_local(&self, id: u32) -> bool {
        self.effect.as_ref().is_some_and(|e| e.frame_local == id)
    }

    fn alloc_locals(
        &mut self,
        body: &LirBody,
        rf: &mut crate::gc::RootFrameState<'ctx>,
    ) -> CodegenResult<()> {
        let entry = self.cg.context.append_basic_block(self.fv, "entry");
        self.builder.position_at_end(entry);
        for d in &body.locals {
            let ty = self.cg.lower_type(d.ty, self.layouts)?;
            let slot = self
                .builder
                .build_alloca(ty, &format!("local{}", d.id))
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "build_alloca", scoop2_base::Span::default())
                })?;
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
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "build_alloca root_frame",
                        scoop2_base::Span::default(),
                    )
                })?;
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
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "load tls_top", scoop2_base::Span::default())
            })?;
        let prev_slot = self
            .builder
            .build_struct_gep(header_ty, frame_ptr, 0, "rf_prev")
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "gep rf_prev", scoop2_base::Span::default())
            })?;
        self.builder.build_store(prev_slot, tls_top).map_err(|e| {
            CodegenError::llvm(e.to_string(), "store rf_prev", scoop2_base::Span::default())
        })?;
        // 3. header.desc = desc（field 1）。
        let desc_slot = self
            .builder
            .build_struct_gep(header_ty, frame_ptr, 1, "rf_desc")
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "gep rf_desc", scoop2_base::Span::default())
            })?;
        self.builder.build_store(desc_slot, desc).map_err(|e| {
            CodegenError::llvm(e.to_string(), "store rf_desc", scoop2_base::Span::default())
        })?;
        // 4. 清零所有 slot。slot 区紧跟 header（字节偏移 HEADER_SIZE）。
        if rf.slot_count > 0 {
            // slots base = (i8*)frame_ptr + HEADER_SIZE，转 ptr*。
            let i8_ptr = self.cg.native_ptr_ty();
            let slots_i8 = unsafe {
                self.builder.build_in_bounds_gep(
                    self.cg.context.i8_type(),
                    frame_ptr,
                    &[self
                        .cg
                        .context
                        .i64_type()
                        .const_int(crate::gc::HEADER_SIZE_BYTES, false)],
                    "rf_slots_i8",
                )
            }
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "gep rf_slots", scoop2_base::Span::default())
            })?;
            let slots_ptr = self
                .builder
                .build_bit_cast(slots_i8, i8_ptr, "rf_slots_ptr")
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "bitcast rf_slots",
                        scoop2_base::Span::default(),
                    )
                })?
                .into_pointer_value();
            let native_null = self.cg.native_ptr_ty().const_null();
            for i in 0..rf.slot_count {
                let slot = unsafe {
                    self.builder.build_in_bounds_gep(
                        self.cg.native_ptr_ty(),
                        slots_ptr,
                        &[self.cg.context.i64_type().const_int(i as u64, false)],
                        &format!("rf_slot{}", i),
                    )
                }
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "gep rf_slot", scoop2_base::Span::default())
                })?;
                self.builder.build_store(slot, native_null).map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "store rf_slot zero",
                        scoop2_base::Span::default(),
                    )
                })?;
            }
        }
        // 5. TLS top = frame_ptr。
        self.builder
            .build_store(tls_global, frame_ptr)
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "store tls_top", scoop2_base::Span::default())
            })?;
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
                self.builder.build_in_bounds_gep(
                    self.cg.context.i8_type(),
                    frame_ptr,
                    &[self
                        .cg
                        .context
                        .i64_type()
                        .const_int(crate::gc::HEADER_SIZE_BYTES, false)],
                    "rf_slots_pop",
                )
            }
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "gep rf_slots_pop",
                    scoop2_base::Span::default(),
                )
            })?;
            let slots_ptr = self
                .builder
                .build_bit_cast(slots_i8, self.cg.native_ptr_ty(), "rf_slots_ptr_pop")
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "bitcast rf_slots_pop",
                        scoop2_base::Span::default(),
                    )
                })?
                .into_pointer_value();
            let native_null = self.cg.native_ptr_ty().const_null();
            for i in 0..rf.slot_count {
                let slot = unsafe {
                    self.builder.build_in_bounds_gep(
                        self.cg.native_ptr_ty(),
                        slots_ptr,
                        &[self.cg.context.i64_type().const_int(i as u64, false)],
                        &format!("rf_slot_pop{}", i),
                    )
                }
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "gep rf_slot_pop",
                        scoop2_base::Span::default(),
                    )
                })?;
                self.builder.build_store(slot, native_null).map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "store rf_slot_pop",
                        scoop2_base::Span::default(),
                    )
                })?;
            }
        }
        // TLS top = header.prev。
        let prev_slot = self
            .builder
            .build_struct_gep(header_ty, frame_ptr, 0, "rf_prev_pop")
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "gep rf_prev_pop",
                    scoop2_base::Span::default(),
                )
            })?;
        let prev = self
            .builder
            .build_load(self.cg.native_ptr_ty(), prev_slot, "rf_prev_val")
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "load rf_prev_pop",
                    scoop2_base::Span::default(),
                )
            })?;
        let tls_global = self.cg.explicit_root_frame_top_global();
        self.builder.build_store(tls_global, prev).map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "store tls_top_pop",
                scoop2_base::Span::default(),
            )
        })?;
        Ok(())
    }

    fn create_blocks(&mut self, body: &LirBody) {
        for blk in &body.blocks {
            let bb = self
                .cg
                .context
                .append_basic_block(self.fv, &format!("bb{}", blk.id));
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
                ));
            }
        };
        self.builder.position_at_end(bb);
        self.emit_resume_delivery(blk.id)?;
        for stmt in &blk.stmts {
            stmt::lower_stmt(self, stmt)?;
        }
        control::lower_terminator(self, &blk.terminator)?;
        Ok(())
    }

    /// resume 续点块首的 resume 值投递：把 step 函数的 word 参数（入口已存
    /// alloca）转换成续点的 resume_ty 后写入 resume_local。
    fn emit_resume_delivery(&mut self, block_id: u32) -> CodegenResult<()> {
        let (rp, word_alloca) = match &self.effect {
            Some(e) => match e.resume_point_for(block_id) {
                Some(rp) => match e.resume_word_alloca {
                    Some(w) => (rp.clone(), w),
                    None => return Ok(()),
                },
                None => return Ok(()),
            },
            None => return Ok(()),
        };
        let i64_ty = self.cg.context.i64_type();
        let word = self
            .builder
            .build_load(i64_ty, word_alloca, "resume_word_ld")
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "load resume_word", scoop2_base::Span::default())
            })?
            .into_int_value();
        let target_llvm = self.cg.lower_type(rp.resume_ty, self.layouts)?;
        let val: BasicValueEnum<'ctx> = match target_llvm {
            inkwell::types::BasicTypeEnum::IntType(it) => {
                if it.get_bit_width() == 64 {
                    word.into()
                } else {
                    self.builder
                        .build_int_truncate(word, it, "resume_trunc")
                        .map_err(|e| {
                            CodegenError::llvm(
                                e.to_string(),
                                "resume word trunc",
                                scoop2_base::Span::default(),
                            )
                        })?
                        .into()
                }
            }
            inkwell::types::BasicTypeEnum::FloatType(ft) => {
                if ft == self.cg.context.f32_type() {
                    let b32 = self
                        .builder
                        .build_int_truncate(word, self.cg.context.i32_type(), "resume_trunc32")
                        .map_err(|e| {
                            CodegenError::llvm(
                                e.to_string(),
                                "resume word trunc32",
                                scoop2_base::Span::default(),
                            )
                        })?;
                    self.builder
                        .build_bit_cast(b32, ft, "resume_f32")
                        .map_err(|e| {
                            CodegenError::llvm(
                                e.to_string(),
                                "resume word bitcast f32",
                                scoop2_base::Span::default(),
                            )
                        })?
                } else {
                    self.builder
                        .build_bit_cast(word, ft, "resume_f64")
                        .map_err(|e| {
                            CodegenError::llvm(
                                e.to_string(),
                                "resume word bitcast f64",
                                scoop2_base::Span::default(),
                            )
                        })?
                }
            }
            inkwell::types::BasicTypeEnum::PointerType(pt) => self
                .builder
                .build_int_to_ptr(word, pt, "resume_inttoptr")
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "resume word inttoptr",
                        scoop2_base::Span::default(),
                    )
                })?
                .into(),
            // 复合值（struct/tuple 等按值类型）：与 `lower_resume` 的 GC box
            // 传递对称——word 是 box 指针，载荷在 object header 之后，按目标
            // 类型直接 load。
            inkwell::types::BasicTypeEnum::StructType(_)
            | inkwell::types::BasicTypeEnum::ArrayType(_) => {
                let box_native = self
                    .builder
                    .build_int_to_ptr(word, self.cg.native_ptr_ty(), "resume_box_native")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "resume box inttoptr",
                            scoop2_base::Span::default(),
                        )
                    })?;
                let payload_ptr = unsafe {
                    self.builder.build_in_bounds_gep(
                        self.cg.context.i8_type(),
                        box_native,
                        &[i64_ty.const_int(
                            scoop2_lir::effect::OBJECT_HEADER_SIZE_BYTES,
                            false,
                        )],
                        "resume_box_payload",
                    )
                }
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "resume box payload gep",
                        scoop2_base::Span::default(),
                    )
                })?;
                self.builder
                    .build_load(target_llvm, payload_ptr, "resume_unbox")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "resume unbox load",
                            scoop2_base::Span::default(),
                        )
                    })?
            }
            other => {
                return Err(CodegenError::unsupported(
                    format!("resume 值类型不支持按 word 投递：{:?}（复合值装箱未实现）", other),
                    &self.fqn,
                    scoop2_base::Span::default(),
                ));
            }
        };
        self.store_local(rp.resume_local, val)
    }

    /// 读取一个 local 的值（按其声明类型 load）。
    pub fn load_local(&self, id: u32) -> CodegenResult<BasicValueEnum<'ctx>> {
        let ty = self.local_types.get(&id).copied().ok_or_else(|| {
            CodegenError::unsupported(
                format!("local {} 类型未知", id),
                &self.fqn,
                scoop2_base::Span::default(),
            )
        })?;
        self.load_local_typed(id, ty)
    }

    /// 读取一个 local 的值（带类型 load）。
    ///
    /// GC-managed local 的 ref 叶子**必须从 root frame slot 同步**（权威源）：
    /// immix 后端是 moving/compacting GC，会在 roots update 阶段原地改写 frame slot 里的指针
    /// 为搬迁后的新地址。alloca 中的旧值在 safepoint 后失效。frame slot 中存的是 native void*，
    /// load 后经 ptrtoint/inttoptr 还原为 `ptr addrspace(1)`。
    /// 普通引用 local（单叶子、偏移 0）直接从 frame slot load 返回；
    /// 内嵌 GC 指针的聚合 local（struct/enum 值）先把各 frame slot 的权威叶子
    /// 回写 alloca，再整体 load（叶子级 slot 镜像，NEW-LLVM-CODEGEN.md §3.2）。
    pub fn load_local_typed(&self, id: u32, ty: TypeId) -> CodegenResult<BasicValueEnum<'ctx>> {
        let slot = self.locals.get(&id).copied().ok_or_else(|| {
            CodegenError::unsupported(
                format!("local {} 未分配", id),
                &self.fqn,
                scoop2_base::Span::default(),
            )
        })?;
        let llvm_ty = self.cg.lower_type(ty, self.layouts)?;
        // GC local：从 frame slot 同步权威叶子。
        let leaf_slots = self
            .root_frame
            .as_ref()
            .and_then(|rf| rf.local_to_slot.get(&id).cloned());
        if let Some(leaves) = leaf_slots {
            let is_single_ptr_leaf = leaves.len() == 1
                && leaves[0].1 == 0
                && matches!(llvm_ty, inkwell::types::BasicTypeEnum::PointerType(_));
            if is_single_ptr_leaf {
                // 整个 local 就是单个 GC 指针：直接从 frame slot load。
                if let Some(frame_slot) = self.mirror_slot_ptr(leaves[0].0)? {
                    let native = self
                        .builder
                        .build_load(self.cg.native_ptr_ty(), frame_slot, &format!("ldf{}", id))
                        .map_err(|e| {
                            CodegenError::llvm(
                                e.to_string(),
                                "build_load frame",
                                scoop2_base::Span::default(),
                            )
                        })?
                        .into_pointer_value();
                    // native void* → GC ptr (addrspace 1)：经 i64 中转。
                    let as_int = self
                        .builder
                        .build_ptr_to_int(
                            native,
                            self.cg.context.i64_type(),
                            &format!("ldf_int{}", id),
                        )
                        .map_err(|e| {
                            CodegenError::llvm(
                                e.to_string(),
                                "ptr_to_int load",
                                scoop2_base::Span::default(),
                            )
                        })?;
                    let gc_ptr = self
                        .builder
                        .build_int_to_ptr(as_int, self.cg.gc_ptr_ty(), &format!("ldf_gc{}", id))
                        .map_err(|e| {
                            CodegenError::llvm(
                                e.to_string(),
                                "int_to_ptr load",
                                scoop2_base::Span::default(),
                            )
                        })?;
                    return Ok(gc_ptr.into());
                }
            } else {
                // 聚合 local：把各 frame slot 的权威叶子回写 alloca，再整体 load。
                for (slot_index, leaf_off) in leaves {
                    let Some(frame_slot) = self.mirror_slot_ptr(slot_index)? else {
                        continue;
                    };
                    let native = self
                        .builder
                        .build_load(
                            self.cg.native_ptr_ty(),
                            frame_slot,
                            &format!("ldf{}_{}", id, slot_index),
                        )
                        .map_err(|e| {
                            CodegenError::llvm(
                                e.to_string(),
                                "build_load frame leaf",
                                scoop2_base::Span::default(),
                            )
                        })?
                        .into_pointer_value();
                    let as_int = self
                        .builder
                        .build_ptr_to_int(
                            native,
                            self.cg.context.i64_type(),
                            &format!("ldf_int{}_{}", id, slot_index),
                        )
                        .map_err(|e| {
                            CodegenError::llvm(
                                e.to_string(),
                                "ptr_to_int load leaf",
                                scoop2_base::Span::default(),
                            )
                        })?;
                    let gc_ptr = self
                        .builder
                        .build_int_to_ptr(
                            as_int,
                            self.cg.gc_ptr_ty(),
                            &format!("ldf_gc{}_{}", id, slot_index),
                        )
                        .map_err(|e| {
                            CodegenError::llvm(
                                e.to_string(),
                                "int_to_ptr load leaf",
                                scoop2_base::Span::default(),
                            )
                        })?;
                    let leaf_addr = unsafe {
                        self.builder.build_gep(
                            self.cg.context.i8_type(),
                            slot,
                            &[self.cg.context.i64_type().const_int(leaf_off, false)],
                            &format!("leaf_addr{}_{}", id, slot_index),
                        )
                    }
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "gep leaf addr",
                            scoop2_base::Span::default(),
                        )
                    })?;
                    self.builder.build_store(leaf_addr, gc_ptr).map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "store leaf back",
                            scoop2_base::Span::default(),
                        )
                    })?;
                }
            }
        }
        self.builder
            .build_load(llvm_ty, slot, &format!("ld{}", id))
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "build_load", scoop2_base::Span::default())
            })
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
            // 第 i 个 LLVM 参数对应的 body local id（参数不一定占据 locals 0..n）。
            let local_id = self
                .param_local_ids
                .get(i as usize)
                .copied()
                .unwrap_or(i as u32);
            if let Some(param) = self.fv.get_nth_param(i) {
                // 闭包 invoke 函数：首参 `$env` 是 GC 指针（统一 ABI），
                // 先解包成 env tuple 值再存入 local（local 声明类型是 tuple struct）。
                if i == 0
                    && let Some(env_ty) = self.closure_env_ty
                {
                    let env_gc = expect_ptr_val(param, "closure $env 参数", &self.fqn)?;
                    let env_val = self.unpack_closure_env(env_ty, env_gc)?;
                    self.store_local(local_id, env_val)?;
                    continue;
                }
                self.store_local(local_id, param)?;
            }
        }
        Ok(())
    }

    /// 把统一 ABI 传入的 env blob 指针解包成 env tuple struct 值。
    /// blob 布局：object header 之后按 tuple 布局的字段 offset 依次存放字段值
    ///（与 lower_make_closure 的打包侧对称）。
    fn unpack_closure_env(
        &self,
        env_ty: TypeId,
        env_gc: PointerValue<'ctx>,
    ) -> CodegenResult<BasicValueEnum<'ctx>> {
        unpack_closure_env_value(
            self.cg,
            self.layouts,
            &self.builder,
            &self.fqn,
            env_ty,
            env_gc,
        )
    }

    /// 把一个值存入 local。
    /// GC-managed local 同时把 GC 指针写入 root frame slot（镜像）。
    /// 把值强制转换到 local 声明类型的 LLVM 表示（标量宽度对齐）。
    ///
    /// 必要性：intrinsic 运算可能把窄整数（UInt8）操作数 zext 到 i64 再计算，
    /// 返回 i64 结果；若直接 store 到 i8 slot 会越界写相邻 local（store i64 到 i8*）。
    /// 这里按 local 类型把整数结果截断/扩展到正确宽度。指针/聚合保持原样。
    fn coerce_to_local_type(
        &self,
        id: u32,
        val: BasicValueEnum<'ctx>,
    ) -> CodegenResult<BasicValueEnum<'ctx>> {
        let Some(&ty) = self.local_types.get(&id) else {
            return Ok(val);
        };
        let target = self.cg.lower_type(ty, self.layouts)?;
        match (val, target) {
            (BasicValueEnum::IntValue(src), inkwell::types::BasicTypeEnum::IntType(dst_ty)) => {
                let src_w = src.get_type().get_bit_width();
                let dst_w = dst_ty.get_bit_width();
                if src_w == dst_w {
                    Ok(src.into())
                } else if src_w > dst_w {
                    self.builder
                        .build_int_truncate(src, dst_ty, &format!("local{}_trunc", id))
                        .map(|v| v.into())
                        .map_err(|e| {
                            CodegenError::llvm(e.to_string(), "coerce trunc", scoop2_base::Span::default())
                        })
                } else {
                    self.builder
                        .build_int_z_extend(src, dst_ty, &format!("local{}_zext", id))
                        .map(|v| v.into())
                        .map_err(|e| {
                            CodegenError::llvm(e.to_string(), "coerce zext", scoop2_base::Span::default())
                        })
                }
            }
            (v, _) => Ok(v),
        }
    }

    pub fn store_local(&self, id: u32, val: BasicValueEnum<'ctx>) -> CodegenResult<()> {
        let slot = self.locals.get(&id).copied().ok_or_else(|| {
            CodegenError::unsupported(
                format!("local {} 未分配", id),
                &self.fqn,
                scoop2_base::Span::default(),
            )
        })?;
        // 值类型与 local 声明类型的 LLVM 表示宽度可能不一致（如 intrinsic 运算把
        // UInt8 操作数 zext 到 i64 再算）。store 到窄 slot 会越界写相邻 local，
        // 必须按 local 类型截断/扩展（标量）或保持（聚合/指针）。
        let val = self.coerce_to_local_type(id, val)?;
        self.builder.build_store(slot, val).map_err(|e| {
            CodegenError::llvm(e.to_string(), "build_store", scoop2_base::Span::default())
        })?;
        // GC local：各 ref 叶子镜像到对应 frame slot（叶子值统一从 alloca 读——
        // val 已写入；普通引用 local 恰好是偏移 0 的单叶子）。
        let leaf_slots = self
            .root_frame
            .as_ref()
            .and_then(|rf| rf.local_to_slot.get(&id).cloned());
        if let Some(leaves) = leaf_slots {
            for (slot_index, leaf_off) in leaves {
                let Some(frame_slot) = self.mirror_slot_ptr(slot_index)? else {
                    continue;
                };
                let leaf_addr = unsafe {
                    self.builder.build_gep(
                        self.cg.context.i8_type(),
                        slot,
                        &[self.cg.context.i64_type().const_int(leaf_off, false)],
                        &format!("sf_leaf{}_{}", id, slot_index),
                    )
                }
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "gep leaf addr", scoop2_base::Span::default())
                })?;
                let leaf_ptr = self
                    .builder
                    .build_load(
                        self.cg.gc_ptr_ty(),
                        leaf_addr,
                        &format!("sf_leaf_ld{}_{}", id, slot_index),
                    )
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "load leaf for mirror",
                            scoop2_base::Span::default(),
                        )
                    })?
                    .into_pointer_value();
                // GC 指针（addrspace 1）cast 到 native ptr 后存入 frame slot。
                let native = self
                    .builder
                    .build_ptr_to_int(
                        leaf_ptr,
                        self.cg.context.i64_type(),
                        &format!("sf_cast{}_{}", id, slot_index),
                    )
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "ptr_to_int frame mirror",
                            scoop2_base::Span::default(),
                        )
                    })?;
                let native_ptr = self
                    .builder
                    .build_int_to_ptr(native, self.cg.native_ptr_ty(), &format!("sf_ptr{}", id))
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "int_to_ptr frame mirror",
                            scoop2_base::Span::default(),
                        )
                    })?;
                self.builder
                    .build_store(frame_slot, native_ptr)
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "store frame mirror",
                            scoop2_base::Span::default(),
                        )
                    })?;
            }
        }
        Ok(())
    }

    /// 取 root frame 第 `slot_index` 个镜像 slot 的指针（None = 无 frame）。
    fn mirror_slot_ptr(
        &self,
        slot_index: u32,
    ) -> CodegenResult<Option<inkwell::values::PointerValue<'ctx>>> {
        let rf = match &self.root_frame {
            Some(rf) => rf,
            None => return Ok(None),
        };
        let frame_ptr = match rf.frame_ptr {
            Some(p) => p,
            None => return Ok(None),
        };
        // slot 区在 frame 偏移 HEADER_SIZE；slot_index 个 ptr。
        let slots_i8 = unsafe {
            self.builder.build_in_bounds_gep(
                self.cg.context.i8_type(),
                frame_ptr,
                &[self
                    .cg
                    .context
                    .i64_type()
                    .const_int(crate::gc::HEADER_SIZE_BYTES, false)],
                "mirror_slots",
            )
        }
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "gep mirror_slots",
                scoop2_base::Span::default(),
            )
        })?;
        let slots_ptr = self
            .builder
            .build_bit_cast(slots_i8, self.cg.native_ptr_ty(), "mirror_slots_ptr")
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "bitcast mirror_slots",
                    scoop2_base::Span::default(),
                )
            })?
            .into_pointer_value();
        let slot = unsafe {
            self.builder.build_in_bounds_gep(
                self.cg.native_ptr_ty(),
                slots_ptr,
                &[self
                    .cg
                    .context
                    .i64_type()
                    .const_int(slot_index as u64, false)],
                &format!("mirror_slot{}", slot_index),
            )
        }
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "gep mirror_slot",
                scoop2_base::Span::default(),
            )
        })?;
        Ok(Some(slot))
    }
}

// =========================================================================
// inkwell 类型断言辅助
// =========================================================================
//
// inkwell 的 `into_struct_type()` / `into_pointer_value()` / `build_struct_gep`
// 等在类型种类不匹配时直接 panic（panic 发生在 inkwell 内部，无法 catch），
// 会把可诊断的编译器 bug 变成进程崩溃。所有"类型/值种类源自程序内容"的断言点
// 必须走下面的校验辅助：不符时返回 CodegenError。

/// 断言 BasicTypeEnum 是 StructType。
pub fn expect_struct_type<'ctx>(
    ty: inkwell::types::BasicTypeEnum<'ctx>,
    what: &str,
    fqn: &str,
) -> CodegenResult<inkwell::types::StructType<'ctx>> {
    match ty {
        inkwell::types::BasicTypeEnum::StructType(s) => Ok(s),
        other => Err(CodegenError::llvm(
            format!("{what}: 期望 struct 类型，实际 {:?}", other),
            fqn,
            scoop2_base::Span::default(),
        )),
    }
}

/// 断言 BasicValueEnum 是 StructValue。
pub fn expect_struct_val<'ctx>(
    v: BasicValueEnum<'ctx>,
    what: &str,
    fqn: &str,
) -> CodegenResult<inkwell::values::StructValue<'ctx>> {
    match v {
        BasicValueEnum::StructValue(s) => Ok(s),
        other => Err(CodegenError::llvm(
            format!("{what}: 期望 struct 值，实际 {:?}", other),
            fqn,
            scoop2_base::Span::default(),
        )),
    }
}

/// 断言 BasicValueEnum 是 PointerValue。
pub fn expect_ptr_val<'ctx>(
    v: BasicValueEnum<'ctx>,
    what: &str,
    fqn: &str,
) -> CodegenResult<PointerValue<'ctx>> {
    match v {
        BasicValueEnum::PointerValue(p) => Ok(p),
        other => Err(CodegenError::llvm(
            format!("{what}: 期望指针值，实际 {:?}", other),
            fqn,
            scoop2_base::Span::default(),
        )),
    }
}

/// 断言 BasicValueEnum 是 IntValue。
pub fn expect_int_val<'ctx>(
    v: BasicValueEnum<'ctx>,
    what: &str,
    fqn: &str,
) -> CodegenResult<inkwell::values::IntValue<'ctx>> {
    match v {
        BasicValueEnum::IntValue(i) => Ok(i),
        other => Err(CodegenError::llvm(
            format!("{what}: 期望整型值，实际 {:?}", other),
            fqn,
            scoop2_base::Span::default(),
        )),
    }
}

/// 断言 BasicValueEnum 是 FloatValue。
pub fn expect_float_val<'ctx>(
    v: BasicValueEnum<'ctx>,
    what: &str,
    fqn: &str,
) -> CodegenResult<inkwell::values::FloatValue<'ctx>> {
    match v {
        BasicValueEnum::FloatValue(f) => Ok(f),
        other => Err(CodegenError::llvm(
            format!("{what}: 期望浮点值，实际 {:?}", other),
            fqn,
            scoop2_base::Span::default(),
        )),
    }
}

/// 把统一 ABI 的 env blob 指针解包成 env tuple struct 值（自由函数版）。
/// blob 布局：object header 之后按 tuple 布局的字段 offset 依次存放字段值
/// （与 lower_make_closure 的打包侧对称）。供 FunctionLowerer 参数入口与
/// EffectStep wrapper（emit.rs 写 frame 参数槽）共用。
pub(crate) fn unpack_closure_env_value<'ctx>(
    cg: &CodegenContext<'ctx>,
    layouts: &TypeLayoutTable,
    builder: &Builder<'ctx>,
    fqn: &str,
    env_ty: TypeId,
    env_gc: PointerValue<'ctx>,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let struct_llvm = match cg.lower_type(env_ty, layouts)? {
        inkwell::types::BasicTypeEnum::StructType(st) => st,
        // env 不是 record（理论不发生）→ 无法解包。
        _ => {
            return Err(CodegenError::unsupported(
                "closure $env 类型不是 struct/tuple",
                fqn,
                scoop2_base::Span::default(),
            ));
        }
    };
    let fields: Vec<scoop2_lir::FieldLayout> = match layouts.get(env_ty) {
        Some(layout) => match &layout.kind {
            scoop2_lir::TypeLayoutKind::Struct { fields }
            | scoop2_lir::TypeLayoutKind::Tuple { elements: fields } => fields.clone(),
            _ => Vec::new(),
        },
        None => Vec::new(),
    };
    let header_size = cg.target_data.get_store_size(&cg.object_header_type());
    let mut val = struct_llvm.get_undef();
    for (i, f) in fields.iter().enumerate() {
        let field_llvm = cg.lower_type(f.ty, layouts)?;
        let slot = unsafe {
            builder.build_in_bounds_gep(
                cg.context.i8_type(),
                env_gc,
                &[cg
                    .context
                    .i64_type()
                    .const_int(header_size + f.offset, false)],
                "env_field_i8",
            )
        }
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "gep env field", scoop2_base::Span::default())
        })?;
        let field_val = builder
            .build_load(field_llvm, slot, "env_field")
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "load env field",
                    scoop2_base::Span::default(),
                )
            })?;
        val = builder
            .build_insert_value(val, field_val, i as u32, "env_unpack")
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "insertvalue env",
                    scoop2_base::Span::default(),
                )
            })?
            .into_struct_value();
    }
    Ok(val.into())
}
