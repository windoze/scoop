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
            closure_env_ty: callable
                .params
                .first()
                .filter(|p| p.name == "$env")
                .map(|p| p.ty),
            param_local_ids: callable.params.iter().map(|p| p.local_id).collect(),
            rt,
            root_frame: None,
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
        for stmt in &blk.stmts {
            stmt::lower_stmt(self, stmt)?;
        }
        control::lower_terminator(self, &blk.terminator)?;
        Ok(())
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
    /// GC-managed local **必须从 root frame slot load**（权威源）：
    /// immix 后端是 moving/compacting GC，会在 roots update 阶段原地改写 frame slot 里的指针
    /// 为搬迁后的新地址。alloca 中的旧值在 safepoint 后失效。frame slot 中存的是 native void*，
    /// load 后经 ptrtoint/inttoptr 还原为 `ptr addrspace(1)`。
    pub fn load_local_typed(&self, id: u32, ty: TypeId) -> CodegenResult<BasicValueEnum<'ctx>> {
        let slot = self.locals.get(&id).copied().ok_or_else(|| {
            CodegenError::unsupported(
                format!("local {} 未分配", id),
                &self.fqn,
                scoop2_base::Span::default(),
            )
        })?;
        let llvm_ty = self.cg.lower_type(ty, self.layouts)?;
        // GC local：从 frame slot 取权威值。
        if self.is_gc_local(id) {
            if let Some(frame_slot) = self.frame_slot_ptr(id)? {
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
        let struct_llvm = match self.cg.lower_type(env_ty, self.layouts)? {
            inkwell::types::BasicTypeEnum::StructType(st) => st,
            // env 不是 record（理论不发生）→ 无法解包。
            _ => {
                return Err(CodegenError::unsupported(
                    "closure $env 类型不是 struct/tuple",
                    &self.fqn,
                    scoop2_base::Span::default(),
                ));
            }
        };
        let fields: Vec<scoop2_lir::FieldLayout> = match self.layouts.get(env_ty) {
            Some(layout) => match &layout.kind {
                scoop2_lir::TypeLayoutKind::Struct { fields }
                | scoop2_lir::TypeLayoutKind::Tuple { elements: fields } => fields.clone(),
                _ => Vec::new(),
            },
            None => Vec::new(),
        };
        let header_size = self
            .cg
            .target_data
            .get_store_size(&self.cg.object_header_type());
        let mut val = struct_llvm.get_undef();
        for (i, f) in fields.iter().enumerate() {
            let field_llvm = self.cg.lower_type(f.ty, self.layouts)?;
            let slot = unsafe {
                self.builder.build_in_bounds_gep(
                    self.cg.context.i8_type(),
                    env_gc,
                    &[self
                        .cg
                        .context
                        .i64_type()
                        .const_int(header_size + f.offset, false)],
                    "env_field_i8",
                )
            }
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "gep env field", scoop2_base::Span::default())
            })?;
            let field_val = self
                .builder
                .build_load(field_llvm, slot, "env_field")
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "load env field",
                        scoop2_base::Span::default(),
                    )
                })?;
            val = self
                .builder
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

    /// 把一个值存入 local。
    /// GC-managed local 同时把 GC 指针写入 root frame slot（镜像）。
    pub fn store_local(&self, id: u32, val: BasicValueEnum<'ctx>) -> CodegenResult<()> {
        let slot = self.locals.get(&id).copied().ok_or_else(|| {
            CodegenError::unsupported(
                format!("local {} 未分配", id),
                &self.fqn,
                scoop2_base::Span::default(),
            )
        })?;
        self.builder.build_store(slot, val).map_err(|e| {
            CodegenError::llvm(e.to_string(), "build_store", scoop2_base::Span::default())
        })?;
        // GC local：镜像到 frame slot（仅对指针类型值）。
        if let Some(frame_slot) = self.frame_slot_ptr(id)? {
            match val {
                BasicValueEnum::PointerValue(ptr_val) => {
                    // GC 指针（addrspace 1）cast 到 native ptr 后存入 frame slot。
                    let native = self
                        .builder
                        .build_ptr_to_int(
                            ptr_val,
                            self.cg.context.i64_type(),
                            &format!("sf_cast{}", id),
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
                // 非指针值（Int/Float/Struct）的 GC local：当前不镜像到 frame slot
                //（immix 非移动式 GC 下 alloca 值在 safepoint 后仍有效）。
                _ => {}
            }
        }
        Ok(())
    }

    /// 取一个 GC local 在 root frame 中的 slot 指针（None = 非 GC local 或无 frame）。
    fn frame_slot_ptr(
        &self,
        id: u32,
    ) -> CodegenResult<Option<inkwell::values::PointerValue<'ctx>>> {
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
                &format!("mirror_slot{}", id),
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
