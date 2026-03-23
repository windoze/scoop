//! LLVM 后端（inkwell）——最小可回归落点（T0802～T0804）。
//!
//! 当前阶段只做两件事：
//! 1) 初始化 host target；
//! 2) 生成一个最小 LLVM module：只包含 `i32 @main()`，返回 0，并可打印/写出 `.ll`/`.o`。
//!
//! 并在 T0803 里补齐：
//! - module target triple + data layout（由 host target machine 提供）。
//!
//! 说明：
//! - 这里暂不从 HIR/MIR 生成真实用户函数 body；那属于后续任务（T0808+）。
//! - 但我们仍会对输入做最小前端检查：必须能 parse，且包含顶层 `fun main`。

use std::path::{Path, PathBuf};

use inkwell::context::Context;
use inkwell::targets::FileType;
use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::parser::ParseError;
use crate::session::Session;
use crate::source::SourceFile;

mod target;
pub use target::{HostTargetInfo, LlvmTargetError};

/// LLVM codegen（早期阶段）的错误集合。
#[derive(Debug, Error, Diagnostic)]
pub enum LlvmEmitError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Parse(#[from] ParseError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Target(#[from] LlvmTargetError),

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

    #[error("输出路径不是有效 UTF-8：{path}")]
    #[diagnostic(code(scoop::llvm::invalid_output_path))]
    InvalidOutputPath { path: PathBuf },

    #[error("写入 object 文件失败：{path}: {message}")]
    #[diagnostic(code(scoop::llvm::write_obj_failed))]
    WriteObjFailed { path: PathBuf, message: String },
}

/// 为一个最小 Scoop 程序生成 LLVM IR（`.ll` 文本）。
///
/// 当前阶段（T0802）的输出固定为：
/// - 一个 LLVM module（module name 取决于输入文件名）；
/// - module target triple 设为 host default triple；
/// - `i32 @main()` 返回 `0`。
pub fn emit_minimal_main_ir(session: &Session, source: &SourceFile) -> Result<String, LlvmEmitError> {
    let context = Context::create();
    let module = build_minimal_main_module(session, source, &context)?;
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

/// 生成最小 LLVM object，并写入到指定路径（通常为 `.o`）。
pub fn emit_minimal_main_obj_to_file(
    session: &Session,
    source: &SourceFile,
    output: &Path,
) -> Result<(), LlvmEmitError> {
    // `TargetMachine::write_to_file` 内部会 `path.to_str().expect(...)`，为了避免 panic，
    // 这里提前做 UTF-8 校验并返回结构化诊断。
    if output.to_str().is_none() {
        return Err(LlvmEmitError::InvalidOutputPath {
            path: output.to_path_buf(),
        });
    }

    let context = Context::create();
    let module = build_minimal_main_module(session, source, &context)?;

    let (target_machine, _target_info) = target::host_target_machine()?;
    target_machine
        .write_to_file(&module, FileType::Object, output)
        .map_err(|e| LlvmEmitError::WriteObjFailed {
            path: output.to_path_buf(),
            message: e.to_string(),
        })?;

    Ok(())
}

fn build_minimal_main_module<'ctx>(
    session: &Session,
    source: &SourceFile,
    context: &'ctx Context,
) -> Result<inkwell::module::Module<'ctx>, LlvmEmitError> {
    // 先让前端能“读懂”输入：如果连 parse 都过不了，直接返回结构化诊断。
    let ast = session.parse(source)?;
    if !file_has_top_level_main(source, &ast) {
        return Err(LlvmEmitError::MissingEntryMain);
    }

    let module_name = module_name_from_path(source.path());
    let module = context.create_module(&module_name);

    // T0803：用 host target machine 配置 module（triple + data layout），并暴露 target 信息。
    let _target_info = target::configure_module_for_host(&module)?;

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

    Ok(module)
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

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn make_temp_dir(prefix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "scoopc_{prefix}_{}_{}",
            std::process::id(),
            nanos
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn minimal_main_ir_contains_main_and_ret0() {
        let source = SourceFile::new_virtual("<mem>", "package a\nfun main() {}");
        let session = Session::new().unwrap();
        let ir = emit_minimal_main_ir(&session, &source).unwrap();

        assert!(ir.contains("define i32 @main()"));
        assert!(ir.contains("ret i32 0"));
        assert!(ir.contains("target datalayout ="));
        assert!(ir.contains("target triple ="));
    }

    #[test]
    fn missing_main_is_reported() {
        let source = SourceFile::new_virtual("<mem>", "package a\nfun not_main() {}");
        let session = Session::new().unwrap();
        let err = emit_minimal_main_ir(&session, &source).unwrap_err();

        assert!(matches!(err, LlvmEmitError::MissingEntryMain));
    }

    #[test]
    fn minimal_main_obj_written_is_non_empty() {
        let dir = make_temp_dir("minimal_main_obj_written_is_non_empty");
        let output = dir.join("main.o");

        let source = SourceFile::new_virtual("<mem>", "package a\nfun main() {}");
        let session = Session::new().unwrap();
        emit_minimal_main_obj_to_file(&session, &source, &output).unwrap();

        let size = std::fs::metadata(&output).unwrap().len();
        assert!(size > 0, "object 文件不应为空");

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
