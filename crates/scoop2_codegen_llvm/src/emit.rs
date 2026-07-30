//! emit 入口：`LirProgram` → LLVM module → IR 文本 / object 文件。

use inkwell::module::Linkage;
use inkwell::types::{BasicMetadataTypeEnum, FunctionType};
use inkwell::values::FunctionValue;

use scoop2_lir::LirProgram;

use crate::body::FunctionLowerer;
use crate::context::CodegenContext;
use crate::error::{CodegenError, CodegenResult};
use crate::target::TargetInfo;

/// codegen 输出选项。
#[derive(Clone, Debug, Default)]
pub struct EmitOptions {
    /// 优化级别（"0".."3"）；当前实现主要支持 "0"。
    pub opt_level: String,
}

/// codegen 产物。
pub struct EmittedModule {
    /// 产物的 LLVM IR 文本。
    pub ir_text: String,
}

impl EmittedModule {
    /// 把 module 编译为 object 文件（relocatable），写入 `path`。
    pub fn write_object(&self, _path: &std::path::Path) -> CodegenResult<()> {
        // 注意：object 输出需要在持有 LLVM module/context 时进行。
        // 当前 EmittedModule 只持有 IR 文本；object 输出在 emit_object_to_file 单独接口。
        Err(CodegenError::TargetOutput {
            message: "write_object 需使用 emit_object_to_file（持有 module）".to_string(),
        })
    }
}

/// 把 `LirProgram` 直接编译为 object 文件（relocatable），写入 `path`。
/// 同时返回 IR 文本（供调试）。
pub fn emit_object_to_file(
    program: &LirProgram,
    path: &std::path::Path,
    _options: &EmitOptions,
) -> CodegenResult<String> {
    use inkwell::targets::{FileType, RelocMode};
    let context = inkwell::context::Context::create();
    let target_info = TargetInfo::host();
    let cg = CodegenContext::new(&context, program, target_info.clone())?;
    *cg.class_itables_data.borrow_mut() = program.class_itables.clone();
    *cg.vtables_data.borrow_mut() = program.vtables.clone();
    let rt = cg.declare_runtime();
    cg.declare_gc_globals()?;
    // 1. 先声明所有 callable（建立函数符号；不含函数体）。
    let mut callable_fns = Vec::new();
    for callable in &program.callables {
        callable_fns.push(declare_callable(&cg, program, callable)?);
    }
    for decl in &program.declarations {
        declare_declaration(&cg, program, decl)?;
    }
    // 2. 生成 type descriptor + itable 全局（此时函数符号已可见，itable 可正确引用）。
    cg.declare_all_globals()?;
    // 3. lowering 函数体。
    for (callable, fv) in program.callables.iter().zip(callable_fns.iter().copied()) {
        if callable.body.is_some() {
            lower_callable_body(&cg, program, &rt, callable, fv)?;
        }
    }
    lower_entry_main(&cg, program, &rt)?;
    // 验证 module。
    if let Err(e) = cg.module.verify() {
        return Err(CodegenError::Verification {
            message: e.to_string(),
        });
    }
    // 输出 object（relocatable）。
    cg.target_machine
        .write_to_file(&cg.module, inkwell::targets::FileType::Object, path)
        .map_err(|e| CodegenError::TargetOutput {
            message: format!("write_to_file(object) 失败：{e}"),
        })?;
    Ok(cg.module.to_string())
}

/// 主入口：把 `LirProgram` 降级为 LLVM module 并返回 IR 文本。
pub fn emit_program(program: &LirProgram, _options: &EmitOptions) -> CodegenResult<EmittedModule> {
    let context = inkwell::context::Context::create();
    let target_info = TargetInfo::host();
    let cg = CodegenContext::new(&context, program, target_info)?;
    let rt = cg.declare_runtime();
    cg.declare_gc_globals()?;
    cg.declare_all_globals()?;
    lower_all_callables(&cg, program, &rt)?;
    lower_entry_main(&cg, program, &rt)?;
    let ir_text = cg.module.to_string();
    Ok(EmittedModule { ir_text })
}

/// 声明并 lowering 所有 callable（含 declarations）。
fn lower_all_callables<'ctx>(
    cg: &CodegenContext<'ctx>,
    program: &LirProgram,
    rt: &crate::runtime_abi::RuntimeFns<'ctx>,
) -> CodegenResult<()> {
    // 1. 先声明所有 callable（建立符号可见性）。
    let mut callable_fns: Vec<FunctionValue> = Vec::with_capacity(program.callables.len());
    for callable in &program.callables {
        let fv = declare_callable(cg, program, callable)?;
        callable_fns.push(fv);
    }
    for decl in &program.declarations {
        declare_declaration(cg, program, decl)?;
    }
    // 2. lowering 每个 callable 的函数体。
    for (callable, fv) in program.callables.iter().zip(callable_fns.iter().copied()) {
        if callable.body.is_some() {
            lower_callable_body(cg, program, rt, callable, fv)?;
        }
    }
    Ok(())
}

/// lowering 一个 callable 的函数体。
///
/// EffectStep callable（带 `effect_info`）编译为**两个 LLVM 函数**：
/// - `sym`（原始签名 wrapper）：堆分配 frame（`scoop_alloc_typed` + frame
///   descriptor）、清零 payload、写参数槽，然后调 `sym$step(frame, 0)` 并
///   原样返回其 Step 值。调用方看到的就是普通调用语义。
/// - `sym$step(ptr frame, i64 word) -> Step`：LIR body 的编译目标
///   （`FunctionLowerer::lower_effect_step`）。
fn lower_callable_body<'ctx>(
    cg: &CodegenContext<'ctx>,
    program: &LirProgram,
    rt: &crate::runtime_abi::RuntimeFns<'ctx>,
    callable: &scoop2_lir::LirCallable,
    fv: FunctionValue<'ctx>,
) -> CodegenResult<()> {
    if let Some(ei) = &callable.effect_info {
        let step_sym = format!("{}$step", callable.symbol_name);
        let step_ret = cg.lower_type(ei.step_ty, &program.type_layouts)?;
        let i64_ty = cg.context.i64_type();
        let native_ptr = cg.native_ptr_ty();
        let step_fn_ty = fn_type_from_basic(
            step_ret,
            &[native_ptr.into(), i64_ty.into()],
        );
        let step_fv = cg
            .module
            .add_function(&step_sym, step_fn_ty, Some(Linkage::Internal));
        build_effect_wrapper(cg, program, rt, callable, fv, step_fv, ei)?;
        FunctionLowerer::lower_effect_step(
            cg,
            &program.type_layouts,
            rt,
            callable,
            step_fv,
            step_sym,
        )
    } else {
        FunctionLowerer::lower(cg, &program.type_layouts, rt, callable, fv)
    }
}

/// 生成 EffectStep wrapper（`sym`）的函数体：
/// `frame = scoop_alloc_typed(frame_desc, header + payload); memset(payload, 0);
/// 写参数槽; return sym$step(frame, 0)`。
fn build_effect_wrapper<'ctx>(
    cg: &CodegenContext<'ctx>,
    program: &LirProgram,
    rt: &crate::runtime_abi::RuntimeFns<'ctx>,
    callable: &scoop2_lir::LirCallable,
    wrapper_fv: FunctionValue<'ctx>,
    step_fv: FunctionValue<'ctx>,
    ei: &scoop2_lir::LirEffectInfo,
) -> CodegenResult<()> {
    let llvm = |e: inkwell::builder::BuilderError, what: &str| {
        CodegenError::llvm(e.to_string(), what, scoop2_base::Span::default())
    };
    let builder = cg.context.create_builder();
    let entry = cg.context.append_basic_block(wrapper_fv, "entry");
    builder.position_at_end(entry);
    let i64_ty = cg.context.i64_type();
    let i8_ty = cg.context.i8_type();
    let native_ptr = cg.native_ptr_ty();
    // frame 尺寸：object header + frame tuple payload。
    let frame_layout = program.type_layouts.get(ei.frame_ty).ok_or_else(|| {
        CodegenError::llvm(
            "frame tuple 布局缺失".to_string(),
            &callable.fqn,
            scoop2_base::Span::default(),
        )
    })?;
    let tuple_elements: Vec<scoop2_lir::FieldLayout> = match &frame_layout.kind {
        scoop2_lir::TypeLayoutKind::Tuple { elements } => elements.clone(),
        _ => {
            return Err(CodegenError::llvm(
                "frame 类型不是 tuple".to_string(),
                &callable.fqn,
                scoop2_base::Span::default(),
            ))
        }
    };
    let payload_size = frame_layout.size;
    let header_size = cg.target_data.get_store_size(&cg.object_header_type());
    let total_size = header_size + payload_size;
    // 1. 堆分配 frame（frame descriptor：trace bitmap 覆盖 tuple 内 GC 指针叶子）。
    let desc =
        cg.get_or_create_frame_type_descriptor(&callable.symbol_name, ei.frame_ty, payload_size);
    let alloc_call = builder
        .build_call(
            rt.alloc_typed,
            &[desc.into(), i64_ty.const_int(total_size, false).into()],
            "frame_alloc",
        )
        .map_err(|e| llvm(e, "alloc effect frame"))?;
    let frame_gc = match alloc_call.try_as_basic_value() {
        inkwell::values::ValueKind::Basic(v) => v.into_pointer_value(),
        inkwell::values::ValueKind::Instruction(_) => {
            return Err(CodegenError::llvm(
                "scoop_alloc_typed 未返回值".to_string(),
                &callable.fqn,
                scoop2_base::Span::default(),
            ))
        }
    };
    let frame_int = builder
        .build_ptr_to_int(frame_gc, i64_ty, "frame_int")
        .map_err(|e| llvm(e, "frame ptrtoint"))?;
    let frame_native = builder
        .build_int_to_ptr(frame_int, native_ptr, "frame_native")
        .map_err(|e| llvm(e, "frame inttoptr"))?;
    // 2. 清零 payload（GC 安全：descriptor 会 trace 指针字段；state 也归零）。
    let payload_ptr = unsafe {
        builder.build_in_bounds_gep(
            i8_ty,
            frame_native,
            &[i64_ty.const_int(header_size, false)],
            "frame_payload",
        )
    }
    .map_err(|e| llvm(e, "gep frame payload"))?;
    if payload_size > 0 {
        builder
            .build_memset(
                payload_ptr,
                frame_layout.align.max(1) as u32,
                i8_ty.const_zero(),
                i64_ty.const_int(payload_size, false),
            )
            .map_err(|e| llvm(e, "memset frame payload"))?;
    }
    // 3. 写参数槽：第 i 个 LLVM 参数 → frame slot（param_slots 给出槽下标）。
    //    注：wrapper 内参数只在 memset/store 间使用，无额外 safepoint，
    //    不需要 root frame（与 immix 非移动 GC 的既有假设一致）。
    //    闭包 invoke 函数的首参 `$env` 按统一 ABI 是 env blob 的 GC 指针，
    //    而 frame 槽声明类型是 env tuple struct——先解包成值再写槽，否则
    //    step 恢复参数时把 blob 指针位当作 captured 字段值（与普通 invoke
    //    函数入口的 unpack_closure_env 处理一致）。
    let closure_env_ty = callable
        .params
        .first()
        .filter(|p| p.name == "$env")
        .map(|p| p.ty);
    for (i, (param_local, slot)) in ei.param_slots.iter().enumerate() {
        let _ = param_local;
        let param = wrapper_fv.get_nth_param(i as u32).ok_or_else(|| {
            CodegenError::llvm(
                format!("wrapper 缺第 {} 个参数", i),
                &callable.fqn,
                scoop2_base::Span::default(),
            )
        })?;
        let param = if i == 0
            && let Some(env_ty) = closure_env_ty
        {
            let env_gc = match param {
                inkwell::values::BasicValueEnum::PointerValue(p) => p,
                _ => {
                    return Err(CodegenError::llvm(
                        "closure $env 参数不是指针".to_string(),
                        &callable.fqn,
                        scoop2_base::Span::default(),
                    ));
                }
            };
            crate::body::unpack_closure_env_value(
                cg,
                &program.type_layouts,
                &builder,
                &callable.fqn,
                env_ty,
                env_gc,
            )?
        } else {
            param
        };
        let off = tuple_elements
            .get(*slot as usize)
            .map(|f| f.offset)
            .ok_or_else(|| {
                CodegenError::llvm(
                    format!("frame 参数槽 {} 越界", slot),
                    &callable.fqn,
                    scoop2_base::Span::default(),
                )
            })?;
        let slot_ptr = unsafe {
            builder.build_in_bounds_gep(
                i8_ty,
                frame_native,
                &[i64_ty.const_int(header_size + off, false)],
                "frame_param_slot",
            )
        }
        .map_err(|e| llvm(e, "gep frame param slot"))?;
        builder
            .build_store(slot_ptr, param)
            .map_err(|e| llvm(e, "store frame param slot"))?;
    }
    // 4. 调 step 并原样返回 Step。
    let step_call = builder
        .build_call(
            step_fv,
            &[frame_native.into(), i64_ty.const_zero().into()],
            "step_call",
        )
        .map_err(|e| llvm(e, "call step fn"))?;
    let step_val = match step_call.try_as_basic_value() {
        inkwell::values::ValueKind::Basic(v) => v,
        inkwell::values::ValueKind::Instruction(_) => {
            return Err(CodegenError::llvm(
                "step 函数未返回值".to_string(),
                &callable.fqn,
                scoop2_base::Span::default(),
            ))
        }
    };
    builder
        .build_return(Some(&step_val))
        .map_err(|e| llvm(e, "wrapper return"))?;
    Ok(())
}

/// 生成 C-ABI 入口 `main`：runtime_init → 调用用户 main → exit code。
/// 当前最小实现：若存在用户 `main`（symbol "main"），生成 wrapper。
/// 注意：用户 main 与 C main 同名会冲突；这里用 `__scoop_user_main` 作为用户 main 的导出符号约定，
/// 由 driver 在 LIR 阶段重命名。若程序无 main（库），跳过 entry 生成。
fn lower_entry_main<'ctx>(
    cg: &CodegenContext<'ctx>,
    program: &LirProgram,
    rt: &crate::runtime_abi::RuntimeFns<'ctx>,
) -> CodegenResult<()> {
    // 查找用户 main：无 package 时 fqn == "main"；有 package 时为 "<pkg>.main"。
    let user_main = program
        .callables
        .iter()
        .find(|c| c.fqn == "main" || c.fqn.ends_with(".main"));
    let user_main = match user_main {
        Some(c) => c,
        None => return Ok(()), // 无 main（库模式）：不生成 entry。
    };
    // C main 签名：i32 main(i32 argc, ptr argv)。
    let i32_ty = cg.context.i32_type();
    let ptr_ty = cg.native_ptr_ty();
    let main_fn_ty = i32_ty.fn_type(&[i32_ty.into(), ptr_ty.into()], false);
    let _ = main_fn_ty;
    // 用户 main 的 symbol（LIR mangled）。
    let user_main_fv = match cg.module.get_function(&user_main.symbol_name) {
        Some(fv) => fv,
        None => return Ok(()),
    };
    // 查用户 main 的返回类型：Unit → exit 0；Int → 用户返回值。
    let ret_layout = program.type_layouts.get(user_main.return_ty);
    let returns_int = ret_layout.is_some_and(|l| {
        matches!(
            l.kind,
            scoop2_lir::TypeLayoutKind::Scalar {
                scalar_kind: scoop2_lir::ScalarKind::Int { .. }
            }
        )
    });
    // 生成 C main：i32 main(i32 argc, ptr argv)。
    let i32_ty = cg.context.i32_type();
    let ptr_ty = cg.native_ptr_ty();
    let c_main_ty = i32_ty.fn_type(&[i32_ty.into(), ptr_ty.into()], false);
    let c_main = cg
        .module
        .add_function("main", c_main_ty, Some(Linkage::External));
    let entry_bb = cg.context.append_basic_block(c_main, "entry");
    let builder = cg.context.create_builder();
    builder.position_at_end(entry_bb);
    // 1. scoop_runtime_init()。
    let _ = builder
        .build_call(rt.runtime_init, &[], "rt_init")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "call runtime_init",
                scoop2_base::Span::default(),
            )
        })?;
    // 2. 调用用户 main。
    let user_call = builder
        .build_call(user_main_fv, &[], "user_main")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "call user main",
                scoop2_base::Span::default(),
            )
        })?;
    // 3. 退出码：Unit main → 0；Int main → 用户返回值。
    // EffectStep main：wrapper 返回 Step——tag != 0 表示有未捕获的 effect
    // 传播到顶层（panic "unhandled effect"）；tag == 0 提取 Complete payload
    // （原始返回类型）作为退出码。
    if user_main.effect_info.is_some() {
        let step_val = match user_call.try_as_basic_value() {
            inkwell::values::ValueKind::Basic(v) => v,
            inkwell::values::ValueKind::Instruction(_) => {
                return Err(CodegenError::llvm(
                    "EffectStep main 未返回 Step".to_string(),
                    "lower_entry_main",
                    scoop2_base::Span::default(),
                ))
            }
        };
        let step_llvm_ty = cg.lower_type(user_main.return_ty, &program.type_layouts)?;
        let step_struct = crate::body::expect_struct_val(
            step_val,
            "EffectStep main Step 值",
            "lower_entry_main",
        )?;
        let tag = builder
            .build_extract_value(step_struct, 0, "main_step_tag")
            .map_err(|e| CodegenError::llvm(e.to_string(), "main step tag", scoop2_base::Span::default()))?
            .into_int_value();
        let is_complete = builder
            .build_int_compare(
                inkwell::IntPredicate::EQ,
                tag,
                tag.get_type().const_zero(),
                "main_step_complete",
            )
            .map_err(|e| CodegenError::llvm(e.to_string(), "main step cmp", scoop2_base::Span::default()))?;
        let complete_bb = cg.context.append_basic_block(c_main, "main_step_complete");
        let unhandled_bb = cg.context.append_basic_block(c_main, "main_step_unhandled");
        builder
            .build_conditional_branch(is_complete, complete_bb, unhandled_bb)
            .map_err(|e| CodegenError::llvm(e.to_string(), "main step br", scoop2_base::Span::default()))?;
        // 未捕获 effect → panic。
        builder.position_at_end(unhandled_bb);
        let msg = cg.get_or_create_string_literal("unhandled effect")?;
        let msg_int = builder
            .build_ptr_to_int(msg, cg.context.i64_type(), "main_panic_msg_int")
            .map_err(|e| CodegenError::llvm(e.to_string(), "main panic msg int", scoop2_base::Span::default()))?;
        let msg_native = builder
            .build_int_to_ptr(msg_int, cg.native_ptr_ty(), "main_panic_msg")
            .map_err(|e| CodegenError::llvm(e.to_string(), "main panic msg", scoop2_base::Span::default()))?;
        builder
            .build_call(rt.panic, &[msg_native.into()], "main_unhandled_panic")
            .map_err(|e| CodegenError::llvm(e.to_string(), "main panic call", scoop2_base::Span::default()))?;
        builder
            .build_unreachable()
            .map_err(|e| CodegenError::llvm(e.to_string(), "main panic unreachable", scoop2_base::Span::default()))?;
        // Complete → 提取 payload（内存 round-trip）作为退出码。
        builder.position_at_end(complete_bb);
        let complete_payload_ty = user_main
            .step_layout
            .as_ref()
            .and_then(|sl| sl.complete_variant.payload);
        let mut exit_code: inkwell::values::IntValue = i32_ty.const_zero();
        if let Some(pty) = complete_payload_ty {
            let payload_is_int = program.type_layouts.get(pty).is_some_and(|l| {
                matches!(
                    l.kind,
                    scoop2_lir::TypeLayoutKind::Scalar {
                        scalar_kind: scoop2_lir::ScalarKind::Int { .. }
                    }
                )
            });
            if payload_is_int {
                let step_layout = program.type_layouts.get(user_main.return_ty).ok_or_else(|| {
                    CodegenError::llvm(
                        "Step 布局缺失".to_string(),
                        "lower_entry_main",
                        scoop2_base::Span::default(),
                    )
                })?;
                let offset =
                    crate::body::rvalue::enum_payload_offset(step_layout, &program.type_layouts);
                let scratch = builder
                    .build_alloca(step_llvm_ty, "main_step_scratch")
                    .map_err(|e| CodegenError::llvm(e.to_string(), "main step alloca", scoop2_base::Span::default()))?;
                builder
                    .build_store(scratch, step_val)
                    .map_err(|e| CodegenError::llvm(e.to_string(), "main step store", scoop2_base::Span::default()))?;
                let payload_ptr = unsafe {
                    builder.build_gep(
                        cg.context.i8_type(),
                        scratch,
                        &[cg.context.i64_type().const_int(offset, false)],
                        "main_step_payload_ptr",
                    )
                }
                .map_err(|e| CodegenError::llvm(e.to_string(), "main step gep", scoop2_base::Span::default()))?;
                let payload = builder
                    .build_load(cg.context.i64_type(), payload_ptr, "main_step_payload")
                    .map_err(|e| CodegenError::llvm(e.to_string(), "main step load", scoop2_base::Span::default()))?
                    .into_int_value();
                exit_code = builder
                    .build_int_truncate(payload, i32_ty, "main_exit_i32")
                    .map_err(|e| CodegenError::llvm(e.to_string(), "main exit trunc", scoop2_base::Span::default()))?;
            }
        }
        let _ = builder.build_return(Some(&exit_code)).map_err(|e| {
            CodegenError::llvm(e.to_string(), "build_return exit", scoop2_base::Span::default())
        })?;
        return Ok(());
    }
    let exit_code = if returns_int {
        match user_call.try_as_basic_value() {
            inkwell::values::ValueKind::Basic(v) => {
                // 用户 main 返回 i64；trunc 到 i32 作为进程退出码。
                let i64_val = v.into_int_value();
                builder
                    .build_int_truncate(i64_val, i32_ty, "exit_i32")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "trunc exit",
                            scoop2_base::Span::default(),
                        )
                    })?
            }
            inkwell::values::ValueKind::Instruction(_) => i32_ty.const_zero(),
        }
    } else {
        i32_ty.const_zero()
    };
    let _ = builder.build_return(Some(&exit_code)).map_err(|e| {
        CodegenError::llvm(
            e.to_string(),
            "build_return exit",
            scoop2_base::Span::default(),
        )
    })?;
    Ok(())
}

/// 声明一个 callable 的函数签名（External/Internal 取决于可见性；当前统一 Internal 便于内部调用，
/// 导出由 entry main 决定）。
fn declare_callable<'ctx>(
    cg: &CodegenContext<'ctx>,
    program: &LirProgram,
    callable: &scoop2_lir::LirCallable,
) -> CodegenResult<FunctionValue<'ctx>> {
    if let Some(cached) = cg.lookup_callable_fn(&callable.symbol_name) {
        return Ok(cached);
    }
    let fn_ty = build_callable_fn_type(cg, program, callable)?;
    let fv = cg
        .module
        .add_function(&callable.symbol_name, fn_ty, Some(Linkage::External));
    cg.cache_callable_fn(callable.symbol_name.clone(), fv);
    // 同时缓存 FQN → FunctionValue（供 itable 方法解析按 FQN 查找正确重载）。
    cg.cache_callable_fn(callable.fqn.clone(), fv);
    Ok(fv)
}

/// 声明一个 extern declaration。
fn declare_declaration<'ctx>(
    cg: &CodegenContext<'ctx>,
    program: &LirProgram,
    decl: &scoop2_lir::LirDeclaration,
) -> CodegenResult<FunctionValue<'ctx>> {
    // extern 符号名：`@Extern(name=...)` 的真实符号。
    // LIR 用 FQN 作为 symbol_name；对 `__scoop_*` 前缀的 extern，映射到运行时符号。
    let actual_symbol = if decl.is_extern {
        resolve_extern_runtime_symbol(&decl.symbol_name)
    } else {
        decl.symbol_name.clone()
    };
    // 若 actual_symbol 已存在（如 runtime 已声明），直接复用并缓存 FQN 别名。
    if let Some(existing) = cg.module.get_function(&actual_symbol) {
        if actual_symbol != decl.symbol_name {
            cg.cache_callable_fn(decl.symbol_name.clone(), existing);
        }
        return Ok(existing);
    }
    if let Some(existing) = cg.module.get_function(&decl.symbol_name) {
        return Ok(existing);
    }
    // extern 函数使用 C ABI：标量参数提升到 i64（与 runtime int64_t 对齐）；
    // 引用参数用 GC ptr（addrspace 1）—— runtime 的 extern(abi="scoop") 接受 GC ptr。
    let return_llvm = lower_extern_type(cg, decl.return_ty, &program.type_layouts)?;
    let params: Vec<BasicMetadataTypeEnum<'ctx>> = decl
        .params
        .iter()
        .map(|p| Ok::<_, CodegenError>(lower_extern_type(cg, p.ty, &program.type_layouts)?.into()))
        .collect::<CodegenResult<_>>()?;
    let fn_ty = fn_type_from_basic(return_llvm, &params);
    let fv = cg
        .module
        .add_function(&actual_symbol, fn_ty, Some(Linkage::External));
    // 缓存 FQN → 声明（供 Direct 调用按 FQN 解析到正确函数）。
    if actual_symbol != decl.symbol_name {
        cg.cache_callable_fn(decl.symbol_name.clone(), fv);
    }
    Ok(fv)
}

/// 把 extern FQN 映射到运行时符号名。
///
/// sysroot 的 `@Extern(name="scoop_X")` 声明遵循 `scoop_<simple_name>` 命名约定：
/// - `scoop.core.__scoop_println` → `scoop_println`（`__scoop_` 前缀替换）
/// - `scoop.core.panic` → `scoop_panic`（无前缀时按 `scoop_<simple>` 推断）
///
/// 完整实现应从 `@Extern(name=...)` 注解透传符号名；此处按 sysroot 约定推断。
fn resolve_extern_runtime_symbol(fqn: &str) -> String {
    // 取最后一段（simple name），把 `__scoop_` 前缀替换为 `scoop_`。
    let simple = fqn.rsplit('.').next().unwrap_or(fqn);
    if let Some(stripped) = simple.strip_prefix("__scoop_") {
        format!("scoop_{stripped}")
    } else {
        // 无 `__scoop_` 前缀的 extern（如 panic/print）：按 `scoop_<simple>` 推断符号。
        format!("scoop_{simple}")
    }
}

/// 构造 callable 的 LLVM 函数类型（参数 + 返回值，GC 引用按 addrspace）。
fn build_callable_fn_type<'ctx>(
    cg: &CodegenContext<'ctx>,
    program: &LirProgram,
    callable: &scoop2_lir::LirCallable,
) -> CodegenResult<FunctionType<'ctx>> {
    let return_llvm = cg.lower_type(callable.return_ty, &program.type_layouts)?;
    let params: Vec<BasicMetadataTypeEnum<'ctx>> = callable
        .params
        .iter()
        .enumerate()
        .map(|(i, p)| {
            // 闭包 invoke 函数的统一 ABI：首参 `$env` 一律按 GC 指针传递
            //（指向堆上的 env blob；间接调用点不知道 env 的具体 tuple 类型）。
            // 函数入口再把 blob 解包成 env tuple 值（见 unpack_closure_env）。
            if i == 0 && p.name == "$env" {
                return Ok::<_, CodegenError>(cg.gc_ptr_ty().into());
            }
            Ok(cg.lower_type(p.ty, &program.type_layouts)?.into())
        })
        .collect::<CodegenResult<_>>()?;
    Ok(fn_type_from_basic(return_llvm, &params))
}

/// 从 BasicTypeEnum 构造 FunctionType。
fn fn_type_from_basic<'ctx>(
    ret: inkwell::types::BasicTypeEnum<'ctx>,
    params: &[BasicMetadataTypeEnum<'ctx>],
) -> FunctionType<'ctx> {
    match ret {
        inkwell::types::BasicTypeEnum::IntType(t) => t.fn_type(params, false),
        inkwell::types::BasicTypeEnum::FloatType(t) => t.fn_type(params, false),
        inkwell::types::BasicTypeEnum::PointerType(t) => t.fn_type(params, false),
        inkwell::types::BasicTypeEnum::StructType(t) => t.fn_type(params, false),
        inkwell::types::BasicTypeEnum::ArrayType(t) => t.fn_type(params, false),
        inkwell::types::BasicTypeEnum::VectorType(t) => t.fn_type(params, false),
        inkwell::types::BasicTypeEnum::ScalableVectorType(_) => {
            ret.into_int_type().fn_type(params, false)
        }
    }
}

/// extern/runtime 函数的类型降级：标量 → i64（C ABI int64_t），引用 → GC ptr（addrspace 1）。
/// 这与 runtime C 函数签名对齐（scoop_bool_to_string(int64_t) 等）。
fn lower_extern_type<'ctx>(
    cg: &CodegenContext<'ctx>,
    ty: scoop2_hir::ty::TypeId,
    layouts: &scoop2_lir::TypeLayoutTable,
) -> CodegenResult<inkwell::types::BasicTypeEnum<'ctx>> {
    let layout = layouts.get(ty);
    if let Some(l) = layout {
        match &l.kind {
            scoop2_lir::TypeLayoutKind::Scalar { .. } => {
                // 所有标量（Bool/Char/Int/Float）在 C ABI 中用 i64（runtime 的 int64_t 参数）。
                // Float 除外：Float 用 f64/f32。但当前 runtime 函数都用 int64_t，故标量 → i64。
                return Ok(cg.context.i64_type().into());
            }
            scoop2_lir::TypeLayoutKind::Reference { .. } | scoop2_lir::TypeLayoutKind::Function => {
                return Ok(cg.gc_ptr_ty().into());
            }
            _ => {}
        }
    }
    // 回退：正常降级。
    cg.lower_type(ty, layouts)
}
