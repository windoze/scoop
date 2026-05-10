//! LLVM 后端（inkwell）——可回归的最小 codegen 落点（T0802～T0810）。
//!
//! 当前阶段目标：
//! 1) 初始化 host target（target triple + data layout）。
//! 2) 生成一个 LLVM module，包含入口 `i32 @main(i32 argc, i8** argv)`（C ABI）：
//!    - 若源文件中存在顶层 `fun main`，则对其 body 做早期子集 codegen，并将返回值作为进程退出码；
//!    - 同时生成/声明 `main` 调用到的顶层函数（T0810：先按简单 C ABI）。
//!
//! 说明：
//! - 目前仍只支持“表达式/语句最小子集”；复杂控制流需要 MIR/CFG codegen（后续任务）。
//! - 目前仍只生成单个 LLVM module，但该 module 已可覆盖整个 compilation unit；
//!   多文件顶层值读取与跨文件泛型实例化走同一模块内的 codegen 主线。
//! - 尚未做多模块拆分与真正的链接管理（后续任务）。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use inkwell::values::InstructionValueError;
use miette::{Diagnostic, NamedSource};
use thiserror::Error;

use crate::hir;
use crate::parser::ParseError;
use crate::source::SourceFile;
use crate::span::Span;

mod codegen;
pub(crate) mod codegen_gap_inventory;
mod emit;
mod frontend;
mod pipeline;
mod reachability;
mod stackmap;
mod target;
#[cfg(test)]
mod tests;

#[cfg(feature = "llvm")]
pub(crate) use emit::emit_single_file_llvm_artifact_to_file_with_opt_level;
pub use emit::{
    StageEmitInput, emit_main_asm_to_file_from_stage_output,
    emit_main_ir_to_file_from_stage_output, emit_main_obj_to_file_from_stage_output,
    emit_minimal_main_asm_to_file, emit_minimal_main_asm_to_file_with_opt_level,
    emit_minimal_main_ir, emit_minimal_main_ir_to_file, emit_minimal_main_obj_to_file,
    emit_minimal_main_obj_to_file_with_opt_level,
};
pub use target::{HostTargetInfo, LlvmTargetError};

#[cfg(test)]
pub(crate) use emit::{
    build_main_module_from_stage_output, build_minimal_main_module,
    build_minimal_main_module_with_opt_level,
};
#[cfg(test)]
pub(crate) use pipeline::run_pass_pipeline;

fn configure_llvm_global_options_once() {
    // 说明：
    // - 默认 explicit mode 已不再给托管函数打 `gc "statepoint-example"`，因此默认产物不会进入
    //   statepoint/stackmap 路径；
    // - 这里仍保留 LLVM 全局选项初始化，供未来按显式开关重新启用 stackmap/statepoint 模式时复用。
    #[cfg(feature = "llvm")]
    {
        use std::sync::Once;

        static ONCE: Once = Once::new();
        ONCE.call_once(|| unsafe {
            use std::ffi::c_char;

            let arg0 = b"scoopc\0";
            let arg1 = b"-fixup-max-csr-statepoints=0\0";
            let args: [*const c_char; 2] = [arg0.as_ptr().cast(), arg1.as_ptr().cast()];

            // Safety: LLVM global CLI options; must be called before codegen.
            llvm_sys::support::LLVMParseCommandLineOptions(
                args.len() as i32,
                args.as_ptr(),
                std::ptr::null(),
            );
        });
    }
}

/// Backend gate diagnostic emitted before an unsupported MIR shape enters LLVM body emission.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "LLVM backend gate 拒绝 `{body_fqn}` 进入 {route}：{detail}（gap {gap_id}, owner {owner_task}, suggested owner {suggested_owner}, source_span={source_span:?}）"
)]
#[diagnostic(code(scoop::llvm::backend_gate))]
pub struct BackendGateError {
    pub(crate) body_fqn: String,
    pub(crate) source_span: Span,
    pub(crate) gap_id: &'static str,
    pub(crate) owner_task: &'static str,
    pub(crate) suggested_owner: &'static str,
    pub(crate) route: &'static str,
    pub(crate) detail: &'static str,
    #[label("这里")]
    pub(crate) at: miette::SourceSpan,
}

/// LLVM codegen（早期阶段）的错误集合。
#[derive(Debug, Error, Diagnostic)]
pub enum LlvmEmitError {
    #[error("LLVM codegen 前端准备失败：{message}")]
    #[diagnostic(code(scoop::llvm::frontend_prepare_failed))]
    Frontend { message: String },

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

    #[error("LLVM 指令构造失败：{0}")]
    #[diagnostic(code(scoop::llvm::instruction_error))]
    Instruction(#[from] InstructionValueError),

    #[error(
        "找不到合法入口函数 `main`（仅支持 `fun main(): Unit / Pure!`、`fun main(): Int / Pure!`、`fun main(args: Array<String>): Unit / Pure!`、`fun main(args: Array<String>): Int / Pure!`）"
    )]
    #[diagnostic(code(scoop::llvm::missing_entry_main))]
    MissingEntryMain,

    #[error(
        "当前 LLVM production codegen 入口要求 canonical materialized MIR/pass 视图，但 lowering 产物未携带它"
    )]
    #[diagnostic(code(scoop::llvm::missing_materialized_pass_view))]
    MissingMaterializedPassView,

    #[error(
        "LLVM backend 尚未迁移入口 `{entry}` 的 reachable callable `{callable}` 所需的 lowering 路径（{unsupported_paths}）；已显式禁止回落到已删除的 handler-stack / EffectOutcome backend"
    )]
    #[diagnostic(code(scoop::llvm::effect_lowering_unsupported))]
    EffectLoweringUnsupported {
        entry: String,
        callable: String,
        unsupported_paths: String,
    },

    #[error(transparent)]
    #[diagnostic(transparent)]
    BackendGate(#[from] Box<BackendGateError>),

    #[error(
        "入口函数 `{entry}` 存在多个合法候选（{count} 个）；可执行程序必须且只能有一个 entry main"
    )]
    #[diagnostic(code(scoop::llvm::ambiguous_entry_main))]
    AmbiguousEntryMain { entry: String, count: usize },

    #[error("暂不支持的 main 代码生成节点：{kind}")]
    #[diagnostic(code(scoop::llvm::unsupported_main_body))]
    UnsupportedMainBody {
        kind: &'static str,
        #[label("这里")]
        at: miette::SourceSpan,
    },

    #[error("字面量解析失败：{kind}（{file}:{line}:{column}，原文：{text}，原因：{reason}）")]
    #[diagnostic(code(scoop::llvm::invalid_literal))]
    InvalidLiteral {
        kind: &'static str,
        reason: &'static str,
        file: String,
        line: usize,
        column: usize,
        text: String,
        #[source_code]
        src: Arc<NamedSource<String>>,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("LLVM module 校验失败：{message}")]
    #[diagnostic(code(scoop::llvm::module_verification_failed))]
    ModuleVerificationFailed { message: String },

    #[error("运行 LLVM pass 失败（passes={passes}）：{message}")]
    #[diagnostic(code(scoop::llvm::run_passes_failed))]
    RunPassesFailed { passes: String, message: String },

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

    #[error("写入 assembly 文件失败：{path}: {message}")]
    #[diagnostic(code(scoop::llvm::write_asm_failed))]
    WriteAsmFailed { path: PathBuf, message: String },
}

impl LlvmEmitError {
    pub(crate) fn invalid_literal(
        source: &SourceFile,
        span: Span,
        kind: &'static str,
        reason: &'static str,
        text: &str,
    ) -> Self {
        let file = diagnostic_source_name(source.path());
        let (line, column) = source.offset_to_line_col(span.start).unwrap_or((1, 1));
        Self::InvalidLiteral {
            kind,
            reason,
            file: file.clone(),
            line,
            column,
            text: literal_text_preview(text),
            src: Arc::new(NamedSource::new(file, source.text().to_string())),
            span: span.into(),
        }
    }
}

fn diagnostic_source_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path.to_str().unwrap_or("<source>"))
        .to_string()
}

fn literal_text_preview(text: &str) -> String {
    const LIMIT: usize = 80;

    let mut out = String::new();
    let mut truncated = false;

    for (count, ch) in text.chars().enumerate() {
        if count == LIMIT {
            truncated = true;
            break;
        }
        match ch {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }

    if truncated {
        out.push_str("...");
    }

    out
}
