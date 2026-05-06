//! 内建注解（built-in annotations）的识别与最小信息提取。
//!
//! 说明：
//! - 这些注解由编译器“硬编码识别”，不依赖用户代码中存在对应的 `annotation class` 声明；
//! - 目前覆盖 `@Unsafe/@Safe/@NoGC/@Extern/@Intrinsic/@Inline/@AllowIntrinsic`
//!   / `@Deprecated` / `@Suppress` / `@Experimental` 的最小语义；
//! - annotation 整体仍是 compile-time marker surface；只有少数 built-in annotation
//!   会在编译器中附带额外语义；
//! - feature gating framework 仍未接入；`@Experimental` 当前只保留 surface 与参数校验。

use crate::ast;
use crate::source::SourceFile;
use crate::span::Span;
use crate::syntax::string_literal::{StringLiteralParseError, parse_string_literal_utf8};
use crate::warnings::{WarningSuppression, is_known_warning_code};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum BuiltinAnnotationKind {
    Unsafe,
    Safe,
    NoGC,
    Extern,
    Intrinsic,
    Inline,
    AllowIntrinsic,
    Deprecated,
    Suppress,
    Experimental,
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
            BuiltinAnnotationKind::Inline => "Inline",
            BuiltinAnnotationKind::AllowIntrinsic => "AllowIntrinsic",
            BuiltinAnnotationKind::Deprecated => "Deprecated",
            BuiltinAnnotationKind::Suppress => "Suppress",
            BuiltinAnnotationKind::Experimental => "Experimental",
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
            BuiltinAnnotationKind::Inline => "函数",
            BuiltinAnnotationKind::AllowIntrinsic => "文件 / 模块",
            BuiltinAnnotationKind::Deprecated => "函数 / 类型 / 属性",
            BuiltinAnnotationKind::Suppress => "表达式 / 声明 / 文件",
            BuiltinAnnotationKind::Experimental => "函数 / 类型 / 属性 / 文件",
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SuppressAnnotationParseError {
    MissingWarningCodes { span: Span },
    NamedArgsNotSupported { span: Span },
    ArgMustBeString { span: Span },
    UnknownWarningCode { code: String, span: Span },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExperimentalAnnotationParseError {
    MissingFeature { span: Span },
    InvalidArgShape { span: Span },
    DuplicateFeature { span: Span },
    ArgMustBeString { span: Span },
}

/// 判断一个 `@Name(...)` 是否为内建注解。
///
/// 当前阶段的识别规则（尽量保守）：
/// - 允许未限定名：`@Unsafe` / `@NoGC` / `@Extern` / `@Intrinsic` / `@Inline`
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
        ["Inline"] | ["scoop", "core", "Inline"] => Some(BuiltinAnnotationKind::Inline),
        ["AllowIntrinsic"] | ["scoop", "core", "AllowIntrinsic"] => {
            Some(BuiltinAnnotationKind::AllowIntrinsic)
        }
        ["Deprecated"] | ["scoop", "core", "Deprecated"] => Some(BuiltinAnnotationKind::Deprecated),
        ["Suppress"] | ["scoop", "core", "Suppress"] => Some(BuiltinAnnotationKind::Suppress),
        ["Experimental"] | ["scoop", "core", "Experimental"] => {
            Some(BuiltinAnnotationKind::Experimental)
        }
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
                Some(BuiltinAnnotationKind::Inline) => {}
                Some(BuiltinAnnotationKind::AllowIntrinsic) => {}
                Some(BuiltinAnnotationKind::Deprecated) => {}
                Some(BuiltinAnnotationKind::Suppress) => {}
                Some(BuiltinAnnotationKind::Experimental) => {}
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

pub(crate) fn parse_experimental_annotation(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
) -> Result<String, ExperimentalAnnotationParseError> {
    let mut feature: Option<String> = None;

    for arg in &ann.args {
        let (key_span, value) = match &arg.name {
            // `@Experimental(feature = "...")`：固定 `name = value` 形态。
            Some(id) if source.slice(id.span) == "feature" => (id.span, &arg.value),
            Some(id) => {
                return Err(ExperimentalAnnotationParseError::InvalidArgShape { span: id.span });
            }
            _ => {
                return Err(ExperimentalAnnotationParseError::InvalidArgShape { span: arg.span });
            }
        };

        if feature.is_some() {
            return Err(ExperimentalAnnotationParseError::DuplicateFeature { span: key_span });
        }

        feature = Some(extract_experimental_string_arg(source, value)?);
    }

    feature.ok_or(ExperimentalAnnotationParseError::MissingFeature { span: ann.span })
}

fn extract_experimental_string_arg(
    source: &SourceFile,
    expr: &ast::Expr,
) -> Result<String, ExperimentalAnnotationParseError> {
    match expr.kind {
        ast::ExprKind::StringLit => {
            let raw = source.slice(expr.span);
            match parse_string_literal_utf8(raw) {
                Ok(text) => Ok(text),
                Err(StringLiteralParseError::Invalid)
                | Err(StringLiteralParseError::InvalidUtf8)
                | Err(StringLiteralParseError::Interpolated) => {
                    Err(ExperimentalAnnotationParseError::ArgMustBeString { span: expr.span })
                }
            }
        }
        _ => Err(ExperimentalAnnotationParseError::ArgMustBeString { span: expr.span }),
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

pub(crate) fn parse_suppress_annotation(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
) -> Result<Vec<String>, SuppressAnnotationParseError> {
    if ann.args.is_empty() {
        return Err(SuppressAnnotationParseError::MissingWarningCodes { span: ann.span });
    }

    let mut codes = Vec::with_capacity(ann.args.len());
    for arg in &ann.args {
        if arg.name.is_some() {
            return Err(SuppressAnnotationParseError::NamedArgsNotSupported { span: arg.span });
        }
        let code = extract_suppress_string_arg(source, &arg.value)?;
        if !is_known_warning_code(code.as_str()) {
            return Err(SuppressAnnotationParseError::UnknownWarningCode {
                code,
                span: arg.value.span,
            });
        }
        codes.push(code);
    }

    codes.sort();
    codes.dedup();
    Ok(codes)
}

fn extract_suppress_string_arg(
    source: &SourceFile,
    expr: &ast::Expr,
) -> Result<String, SuppressAnnotationParseError> {
    match expr.kind {
        ast::ExprKind::StringLit => {
            let raw = source.slice(expr.span);
            match parse_string_literal_utf8(raw) {
                Ok(text) => Ok(text),
                Err(StringLiteralParseError::Invalid)
                | Err(StringLiteralParseError::InvalidUtf8)
                | Err(StringLiteralParseError::Interpolated) => {
                    Err(SuppressAnnotationParseError::ArgMustBeString { span: expr.span })
                }
            }
        }
        _ => Err(SuppressAnnotationParseError::ArgMustBeString { span: expr.span }),
    }
}

pub(crate) fn collect_file_warning_suppressions(
    source: &SourceFile,
    file: &ast::File,
) -> Vec<WarningSuppression> {
    let mut out = Vec::new();
    collect_warning_suppressions_from_annotations(
        source,
        &file.file_annotations,
        source.path(),
        None,
        &mut out,
    );
    for item in &file.items {
        collect_item_warning_suppressions(source, item, source.path(), &mut out);
    }
    out
}

fn collect_item_warning_suppressions(
    source: &SourceFile,
    item: &ast::Item,
    file: &std::path::Path,
    out: &mut Vec<WarningSuppression>,
) {
    match item {
        ast::Item::TypeAlias(decl) => {
            collect_warning_suppressions_from_annotations(
                source,
                &decl.annotations,
                file,
                Some(decl.span),
                out,
            );
        }
        ast::Item::Fun(fun) => {
            collect_warning_suppressions_from_annotations(
                source,
                &fun.annotations,
                file,
                Some(fun.span),
                out,
            );
            collect_fun_body_warning_suppressions(source, &fun.body, file, out);
        }
        ast::Item::ExtensionProperty(prop) => {
            collect_warning_suppressions_from_annotations(
                source,
                &prop.annotations,
                file,
                Some(prop.span),
                out,
            );
            if let Some(init) = &prop.init {
                collect_expr_warning_suppressions(source, init, file, out);
            }
            collect_accessor_warning_suppressions(source, prop.getter.as_ref(), file, out);
            collect_accessor_warning_suppressions(source, prop.setter.as_ref(), file, out);
        }
        ast::Item::Val(val) => {
            collect_val_decl_warning_suppressions(source, val, file, out);
        }
        ast::Item::Type(decl) => collect_type_decl_warning_suppressions(source, decl, file, out),
        ast::Item::Object(obj) => collect_object_decl_warning_suppressions(source, obj, file, out),
        ast::Item::ComptimeIf(ci) => {
            collect_comptime_if_item_warning_suppressions(source, ci, file, out)
        }
    }
}

fn collect_type_decl_warning_suppressions(
    source: &SourceFile,
    decl: &ast::TypeDecl,
    file: &std::path::Path,
    out: &mut Vec<WarningSuppression>,
) {
    collect_warning_suppressions_from_annotations(
        source,
        &decl.annotations,
        file,
        Some(decl.span),
        out,
    );

    if let Some(primary_ctor) = &decl.primary_ctor {
        for param in &primary_ctor.params {
            if let Some(default_value) = &param.default_value {
                collect_expr_warning_suppressions(source, default_value, file, out);
            }
        }
    }

    for supertype in &decl.supertypes {
        for arg in &supertype.ctor_args {
            collect_expr_warning_suppressions(source, arg, file, out);
        }
    }

    if let Some(body) = &decl.body {
        for member in &body.members {
            collect_type_member_warning_suppressions(source, member, file, out);
        }
    }
}

fn collect_object_decl_warning_suppressions(
    source: &SourceFile,
    decl: &ast::ObjectDecl,
    file: &std::path::Path,
    out: &mut Vec<WarningSuppression>,
) {
    collect_warning_suppressions_from_annotations(
        source,
        &decl.annotations,
        file,
        Some(decl.span),
        out,
    );

    for supertype in &decl.supertypes {
        for arg in &supertype.ctor_args {
            collect_expr_warning_suppressions(source, arg, file, out);
        }
    }

    if let Some(body) = &decl.body {
        for member in &body.members {
            collect_type_member_warning_suppressions(source, member, file, out);
        }
    }
}

fn collect_type_member_warning_suppressions(
    source: &SourceFile,
    member: &ast::TypeMember,
    file: &std::path::Path,
    out: &mut Vec<WarningSuppression>,
) {
    match member {
        ast::TypeMember::EnumVariant(variant) => {
            collect_warning_suppressions_from_annotations(
                source,
                &variant.annotations,
                file,
                Some(variant.span),
                out,
            );
            for param in &variant.params {
                if let Some(default_value) = &param.default_value {
                    collect_expr_warning_suppressions(source, default_value, file, out);
                }
            }
            if let Some(discriminant) = &variant.discriminant {
                collect_expr_warning_suppressions(source, discriminant, file, out);
            }
        }
        ast::TypeMember::Property(prop) => {
            collect_warning_suppressions_from_annotations(
                source,
                &prop.annotations,
                file,
                Some(prop.span),
                out,
            );
            if let Some(init) = &prop.init {
                collect_expr_warning_suppressions(source, init, file, out);
            }
            if let Some(delegate) = &prop.delegate {
                collect_expr_warning_suppressions(source, delegate, file, out);
            }
            collect_accessor_warning_suppressions(source, prop.getter.as_ref(), file, out);
            collect_accessor_warning_suppressions(source, prop.setter.as_ref(), file, out);
        }
        ast::TypeMember::InitBlock(init) => {
            collect_block_warning_suppressions(source, &init.body, file, out);
        }
        ast::TypeMember::SecondaryCtor(ctor) => {
            collect_warning_suppressions_from_annotations(
                source,
                &ctor.annotations,
                file,
                Some(ctor.span),
                out,
            );
            if let Some(call) = &ctor.delegation_call {
                for arg in &call.args {
                    collect_expr_warning_suppressions(source, arg, file, out);
                }
            }
            collect_block_warning_suppressions(source, &ctor.body, file, out);
        }
        ast::TypeMember::Fun(fun) => {
            collect_warning_suppressions_from_annotations(
                source,
                &fun.annotations,
                file,
                Some(fun.span),
                out,
            );
            collect_fun_body_warning_suppressions(source, &fun.body, file, out);
        }
        ast::TypeMember::Type(decl) => {
            collect_type_decl_warning_suppressions(source, decl, file, out)
        }
        ast::TypeMember::Object(obj) => {
            collect_object_decl_warning_suppressions(source, obj, file, out)
        }
    }
}

fn collect_fun_body_warning_suppressions(
    source: &SourceFile,
    body: &ast::FunBody,
    file: &std::path::Path,
    out: &mut Vec<WarningSuppression>,
) {
    if let ast::FunBody::Block(block) = body {
        collect_block_warning_suppressions(source, block, file, out);
    }
}

fn collect_accessor_warning_suppressions(
    source: &SourceFile,
    accessor: Option<&ast::AccessorDecl>,
    file: &std::path::Path,
    out: &mut Vec<WarningSuppression>,
) {
    let Some(accessor) = accessor else {
        return;
    };
    match &accessor.body {
        ast::AccessorBody::Block(block) => {
            collect_block_warning_suppressions(source, block, file, out)
        }
        ast::AccessorBody::Expr(expr) => collect_expr_warning_suppressions(source, expr, file, out),
        ast::AccessorBody::Missing => {}
    }
}

fn collect_block_warning_suppressions(
    source: &SourceFile,
    block: &ast::Block,
    file: &std::path::Path,
    out: &mut Vec<WarningSuppression>,
) {
    for stmt in &block.stmts {
        collect_stmt_warning_suppressions(source, stmt, file, out);
    }
}

fn collect_stmt_warning_suppressions(
    source: &SourceFile,
    stmt: &ast::Stmt,
    file: &std::path::Path,
    out: &mut Vec<WarningSuppression>,
) {
    match &stmt.kind {
        ast::StmtKind::Empty
        | ast::StmtKind::Break { .. }
        | ast::StmtKind::Continue { .. }
        | ast::StmtKind::Missing => {}
        ast::StmtKind::Expr(expr) => collect_expr_warning_suppressions(source, expr, file, out),
        ast::StmtKind::Val(decl) => collect_val_decl_warning_suppressions(source, decl, file, out),
        ast::StmtKind::Return { value, .. } => {
            if let Some(value) = value {
                collect_expr_warning_suppressions(source, value, file, out);
            }
        }
        ast::StmtKind::While { cond, body, .. } => {
            collect_expr_warning_suppressions(source, cond, file, out);
            collect_block_warning_suppressions(source, body, file, out);
        }
        ast::StmtKind::For(for_stmt) => {
            collect_for_stmt_warning_suppressions(source, for_stmt, file, out)
        }
        ast::StmtKind::ComptimeBlock { body, .. } => {
            collect_block_warning_suppressions(source, body, file, out)
        }
        ast::StmtKind::ComptimeIf(ci) => {
            collect_comptime_if_warning_suppressions(source, ci, file, out)
        }
        ast::StmtKind::ComptimeFor(cf) => {
            collect_comptime_for_warning_suppressions(source, cf, file, out)
        }
    }
}

fn collect_val_decl_warning_suppressions(
    source: &SourceFile,
    decl: &ast::ValDecl,
    file: &std::path::Path,
    out: &mut Vec<WarningSuppression>,
) {
    collect_warning_suppressions_from_annotations(
        source,
        &decl.annotations,
        file,
        Some(decl.span),
        out,
    );
    if let Some(init) = &decl.init {
        collect_expr_warning_suppressions(source, init, file, out);
    }
}

fn collect_for_stmt_warning_suppressions(
    source: &SourceFile,
    for_stmt: &ast::ForStmt,
    file: &std::path::Path,
    out: &mut Vec<WarningSuppression>,
) {
    collect_expr_warning_suppressions(source, &for_stmt.iter, file, out);
    collect_block_warning_suppressions(source, &for_stmt.body, file, out);
}

fn collect_comptime_if_item_warning_suppressions(
    source: &SourceFile,
    comptime_if: &ast::ComptimeIfItem,
    file: &std::path::Path,
    out: &mut Vec<WarningSuppression>,
) {
    collect_expr_warning_suppressions(source, &comptime_if.cond, file, out);
    for item in &comptime_if.then_branch.items {
        collect_item_warning_suppressions(source, item, file, out);
    }
    if let Some(else_branch) = &comptime_if.else_branch {
        collect_comptime_if_item_else_warning_suppressions(source, else_branch, file, out);
    }
}

fn collect_comptime_if_item_else_warning_suppressions(
    source: &SourceFile,
    else_branch: &ast::ComptimeIfItemElse,
    file: &std::path::Path,
    out: &mut Vec<WarningSuppression>,
) {
    match else_branch {
        ast::ComptimeIfItemElse::Block(block) => {
            for item in &block.items {
                collect_item_warning_suppressions(source, item, file, out);
            }
        }
        ast::ComptimeIfItemElse::If(ci) => {
            collect_comptime_if_item_warning_suppressions(source, ci, file, out)
        }
    }
}

fn collect_comptime_if_warning_suppressions(
    source: &SourceFile,
    comptime_if: &ast::ComptimeIf,
    file: &std::path::Path,
    out: &mut Vec<WarningSuppression>,
) {
    collect_expr_warning_suppressions(source, &comptime_if.cond, file, out);
    collect_block_warning_suppressions(source, &comptime_if.then_branch, file, out);
    if let Some(else_branch) = &comptime_if.else_branch {
        collect_comptime_else_warning_suppressions(source, else_branch, file, out);
    }
}

fn collect_comptime_else_warning_suppressions(
    source: &SourceFile,
    else_branch: &ast::ComptimeIfElse,
    file: &std::path::Path,
    out: &mut Vec<WarningSuppression>,
) {
    match else_branch {
        ast::ComptimeIfElse::Block(block) => {
            collect_block_warning_suppressions(source, block, file, out)
        }
        ast::ComptimeIfElse::If(ci) => {
            collect_comptime_if_warning_suppressions(source, ci, file, out)
        }
    }
}

fn collect_comptime_for_warning_suppressions(
    source: &SourceFile,
    comptime_for: &ast::ComptimeFor,
    file: &std::path::Path,
    out: &mut Vec<WarningSuppression>,
) {
    collect_expr_warning_suppressions(source, &comptime_for.iter, file, out);
    collect_block_warning_suppressions(source, &comptime_for.body, file, out);
}

fn collect_expr_warning_suppressions(
    source: &SourceFile,
    expr: &ast::Expr,
    file: &std::path::Path,
    out: &mut Vec<WarningSuppression>,
) {
    match &expr.kind {
        ast::ExprKind::Missing
        | ast::ExprKind::Ident(_)
        | ast::ExprKind::IntLit
        | ast::ExprKind::FloatLit
        | ast::ExprKind::CharLit
        | ast::ExprKind::StringLit
        | ast::ExprKind::UnitLit
        | ast::ExprKind::ClassLit { .. } => {}
        ast::ExprKind::Annotated {
            annotations,
            expr: inner,
        } => {
            collect_warning_suppressions_from_annotations(
                source,
                annotations,
                file,
                Some(expr.span),
                out,
            );
            collect_expr_warning_suppressions(source, inner, file, out);
        }
        ast::ExprKind::TupleLit { elements } | ast::ExprKind::ArrayLit { elements } => {
            for element in elements {
                collect_expr_warning_suppressions(source, element, file, out);
            }
        }
        ast::ExprKind::InterpolatedString { parts, .. } => {
            for part in parts {
                if let ast::InterpolatedStringPart::Expr { expr: inner } = part {
                    collect_expr_warning_suppressions(source, inner, file, out);
                }
            }
        }
        ast::ExprKind::Block(block)
        | ast::ExprKind::DoBlock { body: block, .. }
        | ast::ExprKind::UnsafeBlock { body: block, .. }
        | ast::ExprKind::SafeBlock { body: block, .. }
        | ast::ExprKind::Async { body: block }
        | ast::ExprKind::Spawn { body: block } => {
            collect_block_warning_suppressions(source, block, file, out)
        }
        ast::ExprKind::Lambda(lambda) => {
            collect_expr_warning_suppressions(source, &lambda.body, file, out)
        }
        ast::ExprKind::StructLit { fields, .. } => {
            for field in fields {
                collect_expr_warning_suppressions(source, &field.value, file, out);
            }
        }
        ast::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_expr_warning_suppressions(source, cond, file, out);
            collect_expr_warning_suppressions(source, then_branch, file, out);
            if let Some(else_branch) = else_branch {
                collect_expr_warning_suppressions(source, else_branch, file, out);
            }
        }
        ast::ExprKind::When { subject, arms } => {
            collect_expr_warning_suppressions(source, subject, file, out);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_expr_warning_suppressions(source, guard, file, out);
                }
                collect_expr_warning_suppressions(source, &arm.body, file, out);
            }
        }
        ast::ExprKind::Handle {
            body,
            arms,
            finally,
        } => {
            collect_block_warning_suppressions(source, body, file, out);
            for arm in arms {
                collect_expr_warning_suppressions(source, &arm.body, file, out);
            }
            if let Some(finally) = finally {
                collect_block_warning_suppressions(source, finally, file, out);
            }
        }
        ast::ExprKind::Await { expr: inner, .. }
        | ast::ExprKind::Join { expr: inner, .. }
        | ast::ExprKind::SpliceField {
            receiver: inner, ..
        }
        | ast::ExprKind::NotNullAssert { expr: inner, .. }
        | ast::ExprKind::Unary { expr: inner, .. }
        | ast::ExprKind::TypeCheck { expr: inner, .. }
        | ast::ExprKind::Cast { expr: inner, .. } => {
            collect_expr_warning_suppressions(source, inner, file, out);
        }
        ast::ExprKind::MemberAccess { receiver, .. }
        | ast::ExprKind::SafeMemberAccess { receiver, .. }
        | ast::ExprKind::TypeApply {
            callee: receiver, ..
        } => {
            collect_expr_warning_suppressions(source, receiver, file, out);
        }
        ast::ExprKind::Call { callee, args } => {
            collect_expr_warning_suppressions(source, callee, file, out);
            for arg in args {
                collect_expr_warning_suppressions(source, arg, file, out);
            }
        }
        ast::ExprKind::SpreadArg { expr: inner, .. }
        | ast::ExprKind::NamedArg { value: inner, .. } => {
            collect_expr_warning_suppressions(source, inner, file, out);
        }
        ast::ExprKind::Binary { lhs, rhs, .. } | ast::ExprKind::Assign { lhs, rhs, .. } => {
            collect_expr_warning_suppressions(source, lhs, file, out);
            collect_expr_warning_suppressions(source, rhs, file, out);
        }
        ast::ExprKind::WithUpdate { base, updates, .. } => {
            collect_expr_warning_suppressions(source, base, file, out);
            for update in updates {
                collect_expr_warning_suppressions(source, &update.value, file, out);
            }
        }
    }
}

fn collect_warning_suppressions_from_annotations(
    source: &SourceFile,
    annotations: &[ast::AnnotationUse],
    file: &std::path::Path,
    span: Option<Span>,
    out: &mut Vec<WarningSuppression>,
) {
    for ann in annotations {
        if builtin_annotation_kind(source, ann) != Some(BuiltinAnnotationKind::Suppress) {
            continue;
        }
        let Ok(codes) = parse_suppress_annotation(source, ann) else {
            continue;
        };
        out.push(match span {
            Some(span) => WarningSuppression::for_span(file, span, codes),
            None => WarningSuppression::for_file(file, codes),
        });
    }
}
