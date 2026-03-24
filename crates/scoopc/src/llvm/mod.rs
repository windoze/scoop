//! LLVM 后端（inkwell）——可回归的最小 codegen 落点（T0802～T0808）。
//!
//! 当前阶段目标：
//! 1) 初始化 host target（target triple + data layout）。
//! 2) 生成一个 LLVM module，包含入口 `i32 @main()`：
//!    - 若源文件中存在顶层 `fun main`，则对其 body 做 **v1 子集** codegen（见 T0808）；
//!    - 该子集覆盖整数/布尔字面量、算术/比较、位运算与移位（含 shift count mask）、`val` 局部绑定、`return`/隐式返回。
//!
//! 说明：
//! - 当前仍只 codegen `main`；其它顶层函数/类型声明会被忽略（但仍会参与 parse/resolve 以保持前端一致）。
//! - `var` 与赋值更新、函数调用 ABI、控制流 CFG 等能力留给后续任务逐步补齐。

use std::path::{Path, PathBuf};

use inkwell::context::Context;
use inkwell::targets::FileType;
use miette::Diagnostic;
use thiserror::Error;

use crate::hir;
use crate::parser::ParseError;
use crate::session::Session;
use crate::source::SourceFile;

mod codegen;
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
    HirLower(#[from] hir::HirLowerError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Target(#[from] LlvmTargetError),

    #[error("LLVM IR 构造失败：{0}")]
    #[diagnostic(code(scoop::llvm::builder_error))]
    Builder(#[from] inkwell::builder::BuilderError),

    #[error("找不到入口函数 `main`（当前阶段仅支持顶层 `fun main() {{ ... }}`）")]
    #[diagnostic(code(scoop::llvm::missing_entry_main))]
    MissingEntryMain,

    #[error("暂不支持的 main 代码生成节点：{kind}")]
    #[diagnostic(code(scoop::llvm::unsupported_main_body))]
    UnsupportedMainBody {
        kind: &'static str,
        #[label("这里")]
        at: miette::SourceSpan,
    },

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

/// 为一个 Scoop 程序生成 LLVM IR（`.ll` 文本）。
///
/// 当前阶段（T0808）的输出形态：
/// - 一个 LLVM module（module name 取决于输入文件名）；
/// - module target triple / data layout 设为 host；
/// - `i32 @main()` 的 body 来自 `fun main` 的 v1 子集 codegen；若 `main` 为空则返回 0。
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
    let module_name = module_name_from_path(source.path());
    let module = context.create_module(&module_name);

    // T0803：用 host target machine 配置 module（triple + data layout），并暴露 target 信息。
    let target_info = target::configure_module_for_host(&module)?;

    let builder = context.create_builder();
    let i32_type = context.i32_type();
    let fn_type = i32_type.fn_type(&[], false);

    let main = module.add_function("main", fn_type, None);
    let entry = context.append_basic_block(main, "entry");
    builder.position_at_end(entry);

    // 当前阶段只 codegen 顶层 `fun main` 的有限子集；其它顶层声明会被忽略。
    let lowered = hir::lower_for_dump(session, source)?;
    let hir_main = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            hir::Item::Fun(fun) if fun.name == "main" => Some(fun),
            _ => None,
        })
        .ok_or(LlvmEmitError::MissingEntryMain)?;

    let exit_code = codegen::MainCodegen::new(context, &builder, &target_info, source, &lowered.types)
        .codegen_main_exit_code(hir_main)?;
    builder.build_return(Some(&exit_code))?;

    module
        .verify()
        .map_err(|e| LlvmEmitError::ModuleVerificationFailed {
            message: e.to_string(),
        })?;

    Ok(module)
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
