//! 内建注解（built-in annotations）的识别与最小信息提取。
//!
//! 说明：
//! - 这些注解由编译器“硬编码识别”，不依赖用户代码中存在对应的 `annotation class` 声明；
//! - 目前覆盖 `@Unsafe/@Safe/@NoGC/@Extern/@Intrinsic` 的最小语义（更多规则见 TODO）；
//! - 更完整的 `@Target/@Retention/@AllowIntrinsic/...` 规则留给后续任务（见 TODO）。

use crate::ast;
use crate::source::SourceFile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum BuiltinAnnotationKind {
    Unsafe,
    Safe,
    NoGC,
    Extern,
    Intrinsic,
}

impl BuiltinAnnotationKind {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            BuiltinAnnotationKind::Unsafe => "Unsafe",
            BuiltinAnnotationKind::Safe => "Safe",
            BuiltinAnnotationKind::NoGC => "NoGC",
            BuiltinAnnotationKind::Extern => "Extern",
            BuiltinAnnotationKind::Intrinsic => "Intrinsic",
        }
    }

    pub(crate) const fn allowed_targets_hint(self) -> &'static str {
        match self {
            BuiltinAnnotationKind::Unsafe => "函数（以及表达式块；见 TODO T1004）",
            BuiltinAnnotationKind::Safe => "函数（以及表达式块；见 TODO T1021）",
            BuiltinAnnotationKind::NoGC => "函数",
            BuiltinAnnotationKind::Extern => "函数 / 顶层 val/var / object",
            BuiltinAnnotationKind::Intrinsic => "函数或类型",
        }
    }
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
        _ => None,
    }
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
