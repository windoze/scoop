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
    let rt = cg.declare_runtime();
    cg.declare_gc_globals()?;
    cg.declare_all_globals()?;
    lower_all_callables(&cg, program, &rt)?;
    // entry main（若存在用户 main）。
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
            FunctionLowerer::lower(cg, &program.type_layouts, rt, callable, fv)?;
        }
    }
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
    // 查找用户 main：fqn == "main"。
    let user_main = program.callables.iter().find(|c| c.fqn == "main");
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
        .map_err(|e| CodegenError::llvm(e.to_string(), "call runtime_init", scoop2_base::Span::default()))?;
    // 2. 调用用户 main。
    let user_call = builder
        .build_call(user_main_fv, &[], "user_main")
        .map_err(|e| CodegenError::llvm(e.to_string(), "call user main", scoop2_base::Span::default()))?;
    // 3. 退出码：Unit main → 0；Int main → 用户返回值。
    let exit_code = if returns_int {
        match user_call.try_as_basic_value() {
            inkwell::values::ValueKind::Basic(v) => {
                // 用户 main 返回 i64；trunc 到 i32 作为进程退出码。
                let i64_val = v.into_int_value();
                builder
                    .build_int_truncate(i64_val, i32_ty, "exit_i32")
                    .map_err(|e| CodegenError::llvm(e.to_string(), "trunc exit", scoop2_base::Span::default()))?
            }
            inkwell::values::ValueKind::Instruction(_) => i32_ty.const_zero(),
        }
    } else {
        i32_ty.const_zero()
    };
    let _ = builder
        .build_return(Some(&exit_code))
        .map_err(|e| CodegenError::llvm(e.to_string(), "build_return exit", scoop2_base::Span::default()))?;
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
    Ok(fv)
}

/// 声明一个 extern declaration。
fn declare_declaration<'ctx>(
    cg: &CodegenContext<'ctx>,
    program: &LirProgram,
    decl: &scoop2_lir::LirDeclaration,
) -> CodegenResult<FunctionValue<'ctx>> {
    if cg.module.get_function(&decl.symbol_name).is_some() {
        return Ok(cg.module.get_function(&decl.symbol_name).unwrap());
    }
    // extern 用 native 指针参数布局（C ABI）。
    let return_llvm = cg.lower_type(decl.return_ty, &program.type_layouts)?;
    let params: Vec<BasicMetadataTypeEnum<'ctx>> = decl
        .params
        .iter()
        .map(|p| Ok::<_, CodegenError>(cg.lower_type(p.ty, &program.type_layouts)?.into()))
        .collect::<CodegenResult<_>>()?;
    let fn_ty = fn_type_from_basic(return_llvm, &params);
    let fv = cg
        .module
        .add_function(&decl.symbol_name, fn_ty, Some(Linkage::External));
    Ok(fv)
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
        .map(|p| Ok::<_, CodegenError>(cg.lower_type(p.ty, &program.type_layouts)?.into()))
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
