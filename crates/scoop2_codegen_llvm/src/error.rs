//! LLVM codegen 错误类型：codegen 层的唯一错误出口。
//!
//! 设计原则：codegen 遇到任何"不该发生"的输入都返回 `CodegenError`，
//! 绝不 `panic!` / `unwrap` / `todo!`（`no_placeholder` 守卫强制）。
//! 绝大多数非法程序在 parse/typecheck/MIR/LIR 阶段已被拒绝；
//! codegen 层的错误主要覆盖：(a) codegen 自身能力边界，(b) 符号未定义，
//! (c) 目标平台不支持，(d) LLVM 内部错误。

use scoop2_base::Span;
use thiserror::Error;

/// codegen 错误。
#[derive(Debug, Error)]
pub enum CodegenError {
    /// 遇到 codegen 尚未覆盖（或不该出现）的 LIR 构造。
    #[error("不支持的 codegen 构造：{kind}（{context}）")]
    UnsupportedConstruct {
        kind: String,
        context: String,
        span: Span,
    },

    /// 直接调用的 callee 符号在 declarations / definitions 中均找不到。
    #[error("未定义的调用符号：`{symbol}`（{context}）")]
    UndefinedSymbol {
        symbol: String,
        context: String,
        span: Span,
    },

    /// 类型布局缺失：LIR 引用了一个未在 type_layouts 中注册的 TypeId。
    #[error("缺少类型布局：TypeId({ty_id})（{context}）")]
    MissingTypeLayout {
        ty_id: u32,
        context: String,
        span: Span,
    },

    /// 内置（intrinsic）名称未在 codegen 的 intrinsic 表中注册。
    #[error("未实现的内置 intrinsic：`{name}`（{context}）")]
    UnknownIntrinsic {
        name: String,
        context: String,
        span: Span,
    },

    /// LLVM 层面的错误（类型不兼容、builder 失败等）。
    #[error("LLVM 错误：{message}（{context}）")]
    Llvm {
        message: String,
        context: String,
        span: Span,
    },

    /// 目标机器 / 对象输出错误。
    #[error("目标输出错误：{message}")]
    TargetOutput { message: String },

    /// 字符串化 LLVM IR / 验证 module 失败。
    #[error("LLVM module 验证失败：{message}")]
    Verification { message: String },

    /// 字符串不是合法 UTF-8（极少见；防御性）。
    #[error("字符串包含非法 UTF-8 字节：{context}")]
    InvalidUtf8 { context: String },
}

/// 所有 codegen 公共函数的统一 Result。
pub type CodegenResult<T> = Result<T, CodegenError>;

impl CodegenError {
    /// 构造 `UnsupportedConstruct`。
    pub fn unsupported(kind: impl Into<String>, context: impl Into<String>, span: Span) -> Self {
        CodegenError::UnsupportedConstruct {
            kind: kind.into(),
            context: context.into(),
            span,
        }
    }

    /// 构造 `UndefinedSymbol`。
    pub fn undefined_symbol(
        symbol: impl Into<String>,
        context: impl Into<String>,
        span: Span,
    ) -> Self {
        CodegenError::UndefinedSymbol {
            symbol: symbol.into(),
            context: context.into(),
            span,
        }
    }

    /// 构造 `MissingTypeLayout`。
    pub fn missing_layout(ty_id: u32, context: impl Into<String>, span: Span) -> Self {
        CodegenError::MissingTypeLayout {
            ty_id,
            context: context.into(),
            span,
        }
    }

    /// 构造 `UnknownIntrinsic`。
    pub fn unknown_intrinsic(
        name: impl Into<String>,
        context: impl Into<String>,
        span: Span,
    ) -> Self {
        CodegenError::UnknownIntrinsic {
            name: name.into(),
            context: context.into(),
            span,
        }
    }

    /// 构造 `Llvm`。
    pub fn llvm(message: impl Into<String>, context: impl Into<String>, span: Span) -> Self {
        CodegenError::Llvm {
            message: message.into(),
            context: context.into(),
            span,
        }
    }

    /// 转换为 `scoop2_base::Diagnostic`（供驱动 push 进 DiagnosticSink）。
    pub fn to_diagnostic(&self) -> scoop2_base::diag::Diagnostic {
        let (span, code) = self.span_and_code();
        let mut d = scoop2_base::diag::Diagnostic::error(code, self.message_string());
        if !span.is_empty() {
            d = d.with_primary(span, "");
        }
        d
    }

    fn span_and_code(&self) -> (Span, &'static str) {
        match self {
            CodegenError::UnsupportedConstruct { span, .. } => {
                (*span, "scoop::codegen::unsupported_construct")
            }
            CodegenError::UndefinedSymbol { span, .. } => {
                (*span, "scoop::codegen::undefined_symbol")
            }
            CodegenError::MissingTypeLayout { span, .. } => {
                (*span, "scoop::codegen::missing_type_layout")
            }
            CodegenError::UnknownIntrinsic { span, .. } => {
                (*span, "scoop::codegen::unknown_intrinsic")
            }
            CodegenError::Llvm { span, .. } => (*span, "scoop::codegen::llvm_error"),
            CodegenError::TargetOutput { .. } => (Span::default(), "scoop::codegen::target_output"),
            CodegenError::Verification { .. } => (Span::default(), "scoop::codegen::verification"),
            CodegenError::InvalidUtf8 { .. } => (Span::default(), "scoop::codegen::invalid_utf8"),
        }
    }

    fn message_string(&self) -> String {
        format!("{self}")
    }
}
