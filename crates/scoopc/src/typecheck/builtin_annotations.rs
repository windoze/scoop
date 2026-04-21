//! 内建注解（built-in annotations）的识别与最小信息提取。
//!
//! 说明：
//! - 这些注解由编译器“硬编码识别”，不依赖用户代码中存在对应的 `annotation class` 声明；
//! - 目前覆盖 `@Unsafe/@Safe/@NoGC/@Extern/@Intrinsic/@AllowIntrinsic/@Deprecated`
//!   的最小语义；
//! - annotation 整体仍是 compile-time marker surface；只有少数 built-in annotation
//!   会在编译器中附带额外语义；
//! - 更完整的 `@Deprecated/@Suppress/...` 规则留给后续任务（见 TODO）。

use crate::ast;
use crate::source::SourceFile;
use crate::span::Span;
use crate::syntax::string_literal::{StringLiteralParseError, parse_string_literal_utf8};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum BuiltinAnnotationKind {
    Unsafe,
    Safe,
    NoGC,
    Extern,
    Intrinsic,
    AllowIntrinsic,
    Deprecated,
    CallingConvention,
}

impl BuiltinAnnotationKind {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            BuiltinAnnotationKind::Unsafe => "Unsafe",
            BuiltinAnnotationKind::Safe => "Safe",
            BuiltinAnnotationKind::NoGC => "NoGC",
            BuiltinAnnotationKind::Extern => "Extern",
            BuiltinAnnotationKind::Intrinsic => "Intrinsic",
            BuiltinAnnotationKind::AllowIntrinsic => "AllowIntrinsic",
            BuiltinAnnotationKind::Deprecated => "Deprecated",
            BuiltinAnnotationKind::CallingConvention => "CallingConvention",
        }
    }

    pub(crate) const fn allowed_targets_hint(self) -> &'static str {
        match self {
            BuiltinAnnotationKind::Unsafe => "函数（以及表达式块；见 TODO T1004）",
            BuiltinAnnotationKind::Safe => "函数（以及表达式块；见 TODO T1021）",
            BuiltinAnnotationKind::NoGC => "函数",
            BuiltinAnnotationKind::Extern => "函数 / 顶层 val/var / object",
            BuiltinAnnotationKind::Intrinsic => "函数或类型",
            BuiltinAnnotationKind::AllowIntrinsic => "文件 / 模块",
            BuiltinAnnotationKind::Deprecated => "函数 / 类型 / 属性",
            BuiltinAnnotationKind::CallingConvention => "函数 / typealias",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeprecatedAnnotationInfo {
    pub(crate) message: String,
    pub(crate) replace_with: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DeprecatedAnnotationParseError {
    TooManyArgs { span: Span },
    PositionalAfterNamed { span: Span },
    OnlyFirstArgMayBePositional { span: Span },
    UnknownParam { name: String, span: Span },
    DuplicateParam { param: &'static str, span: Span },
    ArgMustBeString { param: &'static str, span: Span },
}

/// 判断一个 `@Name(...)` 是否为内建注解。
///
/// 当前阶段的识别规则（尽量保守）：
/// - 允许未限定名：`@Unsafe` / `@NoGC` / `@Extern` / `@Intrinsic`
/// - 允许完整限定名：`@scoop.core.Unsafe` / `@scoop.core.NoGC` / ...
pub(crate) fn builtin_annotation_kind(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
) -> Option<BuiltinAnnotationKind> {
    let segs = ann
        .path
        .iter()
        .map(|id| id.text(source))
        .collect::<Vec<_>>();
    match segs.as_slice() {
        ["Unsafe"] | ["scoop", "core", "Unsafe"] => Some(BuiltinAnnotationKind::Unsafe),
        ["Safe"] | ["scoop", "core", "Safe"] => Some(BuiltinAnnotationKind::Safe),
        ["NoGC"] | ["scoop", "core", "NoGC"] => Some(BuiltinAnnotationKind::NoGC),
        ["Extern"] | ["scoop", "core", "Extern"] => Some(BuiltinAnnotationKind::Extern),
        ["Intrinsic"] | ["scoop", "core", "Intrinsic"] => Some(BuiltinAnnotationKind::Intrinsic),
        ["AllowIntrinsic"] | ["scoop", "core", "AllowIntrinsic"] => {
            Some(BuiltinAnnotationKind::AllowIntrinsic)
        }
        ["Deprecated"] | ["scoop", "core", "Deprecated"] => Some(BuiltinAnnotationKind::Deprecated),
        ["CallingConvention"] | ["scoop", "core", "CallingConvention"] => {
            Some(BuiltinAnnotationKind::CallingConvention)
        }
        _ => None,
    }
}

/// 当前文件是否显式通过 `@file:AllowIntrinsic` 打开用户态 intrinsic 声明 gate。
pub(crate) fn file_allows_intrinsic(source: &SourceFile, anns: &[ast::AnnotationUse]) -> bool {
    anns.iter().any(|ann| {
        builtin_annotation_kind(source, ann) == Some(BuiltinAnnotationKind::AllowIntrinsic)
    })
}

/// 从一组注解使用中提取“内建注解标记位”。
///
/// 说明：
/// - 该结构只表达“出现过与否”，不携带参数（例如 `@Extern("puts")` 的符号名）；
/// - `@Extern` 在语义上隐含 `@NoGC`（spec §15.8.3），因此这里会把 `is_nogc` 置为 `true`。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BuiltinAnnotationFlags {
    pub(crate) is_unsafe: bool,
    pub(crate) is_safe: bool,
    pub(crate) is_nogc: bool,
    pub(crate) is_extern: bool,
    pub(crate) is_intrinsic: bool,
}

impl BuiltinAnnotationFlags {
    pub(crate) fn from_annotations(source: &SourceFile, anns: &[ast::AnnotationUse]) -> Self {
        let mut out = BuiltinAnnotationFlags::default();
        for ann in anns {
            match builtin_annotation_kind(source, ann) {
                Some(BuiltinAnnotationKind::Unsafe) => out.is_unsafe = true,
                Some(BuiltinAnnotationKind::Safe) => out.is_safe = true,
                Some(BuiltinAnnotationKind::NoGC) => out.is_nogc = true,
                Some(BuiltinAnnotationKind::Extern) => out.is_extern = true,
                Some(BuiltinAnnotationKind::Intrinsic) => out.is_intrinsic = true,
                Some(BuiltinAnnotationKind::AllowIntrinsic) => {}
                Some(BuiltinAnnotationKind::Deprecated) => {}
                Some(BuiltinAnnotationKind::CallingConvention) => {}
                None => {}
            }
        }

        // spec §15.8.3：`@Extern` 默认视为 `@NoGC`。
        if out.is_extern {
            out.is_nogc = true;
        }

        out
    }
}

pub(crate) fn parse_deprecated_annotation(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
) -> Result<DeprecatedAnnotationInfo, DeprecatedAnnotationParseError> {
    let mut message: Option<String> = None;
    let mut replace_with: Option<String> = None;
    let mut seen_named = false;
    let mut positional_count = 0usize;

    for arg in &ann.args {
        match &arg.name {
            Some(name_id) => {
                seen_named = true;
                let name = name_id.text(source);
                match name {
                    "message" => {
                        if message.is_some() {
                            return Err(DeprecatedAnnotationParseError::DuplicateParam {
                                param: "message",
                                span: name_id.span,
                            });
                        }
                        message = Some(extract_deprecated_string_arg(
                            source, &arg.value, "message",
                        )?);
                    }
                    "replaceWith" => {
                        if replace_with.is_some() {
                            return Err(DeprecatedAnnotationParseError::DuplicateParam {
                                param: "replaceWith",
                                span: name_id.span,
                            });
                        }
                        replace_with = Some(extract_deprecated_string_arg(
                            source,
                            &arg.value,
                            "replaceWith",
                        )?);
                    }
                    _ => {
                        return Err(DeprecatedAnnotationParseError::UnknownParam {
                            name: name.to_string(),
                            span: name_id.span,
                        });
                    }
                }
            }
            None => {
                if seen_named {
                    return Err(DeprecatedAnnotationParseError::PositionalAfterNamed {
                        span: arg.span,
                    });
                }
                if positional_count > 0 {
                    return Err(
                        DeprecatedAnnotationParseError::OnlyFirstArgMayBePositional {
                            span: arg.span,
                        },
                    );
                }
                positional_count += 1;
                message = Some(extract_deprecated_string_arg(
                    source, &arg.value, "message",
                )?);
            }
        }
    }

    if ann.args.len() > 2 {
        let span = ann.args[2].span;
        return Err(DeprecatedAnnotationParseError::TooManyArgs { span });
    }

    let replace_with = replace_with.filter(|value| !value.is_empty());
    Ok(DeprecatedAnnotationInfo {
        message: message.unwrap_or_default(),
        replace_with,
    })
}

fn extract_deprecated_string_arg(
    source: &SourceFile,
    expr: &ast::Expr,
    param: &'static str,
) -> Result<String, DeprecatedAnnotationParseError> {
    match expr.kind {
        ast::ExprKind::StringLit => {
            let raw = source.slice(expr.span);
            match parse_string_literal_utf8(raw) {
                Ok(text) => Ok(text),
                Err(StringLiteralParseError::Invalid)
                | Err(StringLiteralParseError::InvalidUtf8)
                | Err(StringLiteralParseError::Interpolated) => {
                    Err(DeprecatedAnnotationParseError::ArgMustBeString {
                        param,
                        span: expr.span,
                    })
                }
            }
        }
        _ => Err(DeprecatedAnnotationParseError::ArgMustBeString {
            param,
            span: expr.span,
        }),
    }
}
