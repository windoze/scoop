//! LLVM 后端（inkwell）——最小可回归落点（T0802）。
//!
//! 当前阶段只做两件事：
//! 1) 初始化 host target；
//! 2) 生成一个最小 LLVM module：只包含 `i32 @main()`，返回 0，并可打印/写出 `.ll`。
//!
//! 说明：
//! - 这里暂不从 HIR/MIR 生成真实用户函数 body；那属于后续任务（T0808+）。
//! - 但我们仍会对输入做最小前端检查：必须能 parse，且包含顶层 `fun main`。

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use inkwell::context::Context;
use inkwell::targets::{InitializationConfig, Target, TargetMachine};
use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::parser::ParseError;
use crate::session::Session;
use crate::source::SourceFile;

/// LLVM codegen（早期阶段）的错误集合。
#[derive(Debug, Error, Diagnostic)]
pub enum LlvmEmitError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Parse(#[from] ParseError),

    #[error("LLVM target 初始化失败：{message}")]
    #[diagnostic(code(scoop::llvm::target_init_failed))]
    TargetInitFailed { message: String },

    #[error("LLVM IR 构造失败：{0}")]
    #[diagnostic(code(scoop::llvm::builder_error))]
    Builder(#[from] inkwell::builder::BuilderError),

    #[error("找不到入口函数 `main`（当前阶段仅支持顶层 `fun main() {{ ... }}`）")]
    #[diagnostic(code(scoop::llvm::missing_entry_main))]
    MissingEntryMain,

    #[error("LLVM module 校验失败：{message}")]
    #[diagnostic(code(scoop::llvm::module_verification_failed))]
    ModuleVerificationFailed { message: String },

    #[error("写入 LLVM IR 失败：{path}: {source}")]
    #[diagnostic(code(scoop::llvm::write_ll_failed))]
    WriteLlFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// 为一个最小 Scoop 程序生成 LLVM IR（`.ll` 文本）。
///
/// 当前阶段（T0802）的输出固定为：
/// - 一个 LLVM module（module name 取决于输入文件名）；
/// - module target triple 设为 host default triple；
/// - `i32 @main()` 返回 `0`。
pub fn emit_minimal_main_ir(session: &Session, source: &SourceFile) -> Result<String, LlvmEmitError> {
    // 先让前端能“读懂”输入：如果连 parse 都过不了，直接返回结构化诊断。
    let ast = session.parse(source)?;
    if !file_has_top_level_main(source, &ast) {
        return Err(LlvmEmitError::MissingEntryMain);
    }

    init_native_target()?;

    let context = Context::create();
    let module_name = module_name_from_path(source.path());
    let module = context.create_module(&module_name);

    // T0802：先把 target triple 跑通；data layout/target machine 在 T0803 再补齐。
    let triple = TargetMachine::get_default_triple();
    module.set_triple(&triple);

    let builder = context.create_builder();
    let i32_type = context.i32_type();
    let fn_type = i32_type.fn_type(&[], false);

    let main = module.add_function("main", fn_type, None);
    let entry = context.append_basic_block(main, "entry");
    builder.position_at_end(entry);
    builder.build_return(Some(&i32_type.const_int(0, false)))?;

    module
        .verify()
        .map_err(|e| LlvmEmitError::ModuleVerificationFailed {
            message: e.to_string(),
        })?;

    Ok(module.print_to_string().to_string())
}

/// 生成最小 LLVM IR，并写入到指定路径（通常为 `.ll`）。
pub fn emit_minimal_main_ir_to_file(
    session: &Session,
    source: &SourceFile,
    output: &Path,
) -> Result<(), LlvmEmitError> {
    let ir = emit_minimal_main_ir(session, source)?;

    std::fs::write(output, ir).map_err(|e| LlvmEmitError::WriteLlFailed {
        path: output.to_path_buf(),
        source: e,
    })?;

    Ok(())
}

fn file_has_top_level_main(source: &SourceFile, file: &ast::File) -> bool {
    file.items.iter().any(|item| match item {
        ast::Item::Fun(fun) => source.slice(fun.name.span) == "main",
        _ => false,
    })
}

fn module_name_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("scoop_module")
        .to_string()
}

fn init_native_target() -> Result<(), LlvmEmitError> {
    static INIT: OnceLock<Result<(), String>> = OnceLock::new();
    let result = INIT.get_or_init(|| Target::initialize_native(&InitializationConfig::default()));

    match result {
        Ok(()) => Ok(()),
        Err(message) => Err(LlvmEmitError::TargetInitFailed {
            message: message.clone(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_main_ir_contains_main_and_ret0() {
        let source = SourceFile::new_virtual("<mem>", "package a\nfun main() {}");
        let session = Session::new().unwrap();
        let ir = emit_minimal_main_ir(&session, &source).unwrap();

        assert!(ir.contains("define i32 @main()"));
        assert!(ir.contains("ret i32 0"));
        assert!(ir.contains("target triple ="));
    }

    #[test]
    fn missing_main_is_reported() {
        let source = SourceFile::new_virtual("<mem>", "package a\nfun not_main() {}");
        let session = Session::new().unwrap();
        let err = emit_minimal_main_ir(&session, &source).unwrap_err();

        assert!(matches!(err, LlvmEmitError::MissingEntryMain));
    }
}
