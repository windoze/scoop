//! 数据驱动的诊断表示与稳定文本渲染。
//!
//! 设计目标：
//!
//! - 每个诊断都有稳定的机器可读诊断码（`scoop::<stage>::<name>`），
//!   fixture 的 `EXPECT-ERROR-CODE` 直接匹配该码；
//! - 诊断是纯数据（非 thiserror 枚举），各阶段用构造辅助函数创建，
//!   避免 300+ 个手写错误枚举变体；
//! - 渲染器手写、输出格式稳定（不依赖 miette fancy 渲染的版本差异），
//!   包含 `--> path:line:col` 行，供 fixture 的 `EXPECT-ERROR-AT` 匹配。

use std::fmt;

use crate::{SourceFile, Span};

/// 诊断严重级别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

/// 一段带消息的源码标注。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    pub span: Span,
    pub message: String,
}

/// 一条诊断。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    /// 稳定诊断码，形如 `scoop::parse::unexpected_token`。
    pub code: &'static str,
    /// 主消息（中文，面向用户）。
    pub message: String,
    /// 主标注：出错位置与说明。
    pub primary: Option<Label>,
    /// 附加标注（如“先前在此处定义”）。
    pub related: Vec<Label>,
    /// 修复建议。
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn error(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code,
            message: message.into(),
            primary: None,
            related: Vec::new(),
            help: None,
        }
    }

    pub fn warning(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            code,
            message: message.into(),
            primary: None,
            related: Vec::new(),
            help: None,
        }
    }

    pub fn with_primary(mut self, span: Span, message: impl Into<String>) -> Self {
        self.primary = Some(Label {
            span,
            message: message.into(),
        });
        self
    }

    pub fn with_related(mut self, span: Span, message: impl Into<String>) -> Self {
        self.related.push(Label {
            span,
            message: message.into(),
        });
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// 主标注的起始偏移（用于 `EXPECT-ERROR-AT` 与排序）；无标注时为 `None`。
    pub fn primary_offset(&self) -> Option<usize> {
        self.primary.as_ref().map(|l| l.span.start)
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}[{}]: {}",
            self.severity.as_str(),
            self.code,
            self.message
        )
    }
}

impl std::error::Error for Diagnostic {}

/// 诊断收集器。各阶段把诊断推入 sink，管线末尾统一检查/渲染。
#[derive(Debug, Default)]
pub struct DiagnosticSink {
    diags: Vec<Diagnostic>,
}

impl DiagnosticSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, diag: Diagnostic) {
        self.diags.push(diag);
    }

    pub fn error(&mut self, code: &'static str, message: impl Into<String>) {
        self.push(Diagnostic::error(code, message));
    }

    pub fn extend(&mut self, diags: impl IntoIterator<Item = Diagnostic>) {
        self.diags.extend(diags);
    }

    pub fn is_empty(&self) -> bool {
        self.diags.is_empty()
    }

    pub fn has_errors(&self) -> bool {
        self.diags.iter().any(Diagnostic::is_error)
    }

    pub fn len(&self) -> usize {
        self.diags.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diags.iter()
    }

    pub fn into_vec(self) -> Vec<Diagnostic> {
        self.diags
    }

    /// 按主标注偏移稳定排序（同一文件内按位置有序输出）。
    pub fn sort_by_offset(&mut self) {
        self.diags
            .sort_by_key(|d| d.primary_offset().unwrap_or(usize::MAX));
    }

    /// 去重冗余诊断：当同一偏移位置同时存在 `redundant_code` 与 `kept_code` 时，
    /// 移除 `redundant_code`（保留更精确的 `kept_code`）。
    /// 例如 resolve 已报 unresolved_type 时，移除 typecheck 的 unresolved_type_ref。
    pub fn dedup_redundant(&mut self, redundant_code: &str, kept_code: &str) {
        use std::collections::HashSet;
        let offsets: HashSet<usize> = self
            .diags
            .iter()
            .filter(|d| d.is_error() && d.code == kept_code)
            .filter_map(|d| d.primary_offset())
            .collect();
        if offsets.is_empty() {
            return;
        }
        self.diags.retain(|d| {
            !(d.is_error()
                && d.code == redundant_code
                && d.primary_offset().is_some_and(|o| offsets.contains(&o)))
        });
    }
}

impl IntoIterator for DiagnosticSink {
    type Item = Diagnostic;
    type IntoIter = std::vec::IntoIter<Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.diags.into_iter()
    }
}

/// 将单条诊断渲染为稳定的多行文本（不含尾随空行）。
///
/// 输出格式示例：
///
/// ```text
/// error[scoop::parse::unexpected_token]: 期望表达式，但遇到了 `}`
///   --> main.scoop:3:9
///    |
///  3 |     val x = }
///    |             ^ 期望表达式
///    = help: 检查是否漏写了右操作数
/// ```
pub fn render_diagnostic(source: &SourceFile, diag: &Diagnostic) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{}[{}]: {}\n",
        diag.severity.as_str(),
        diag.code,
        diag.message
    ));

    let mut labels: Vec<&Label> = Vec::new();
    if let Some(primary) = &diag.primary {
        labels.push(primary);
    }
    labels.extend(diag.related.iter());

    //  gutter 宽度由最大行号决定。
    let max_line = labels
        .iter()
        .map(|l| source.offset_to_line_col(l.span.start).0)
        .max()
        .unwrap_or(0);
    let gutter = max_line.max(1).to_string().len();

    for label in labels {
        let (line, col) = source.offset_to_line_col(label.span.start);
        out.push_str(&format!(
            "{:>width$} --> {}:{}:{}\n",
            "",
            source.path().display(),
            line,
            col,
            width = gutter
        ));
        out.push_str(&format!("{:>width$} |\n", "", width = gutter));
        let line_text = source.line_text(line);
        out.push_str(&format!(
            "{:>width$} | {}\n",
            line,
            line_text,
            width = gutter
        ));
        // 标注行：列以字符计，下划线长度至少为 1。
        let line_start = source.line_start_offset(line).unwrap_or(0);
        let span_len = if label.span.is_empty() {
            1
        } else {
            let clamped_end = label.span.end.min(line_start + line_text.len());
            let start = label.span.start.max(line_start);
            source
                .text()
                .get(start..clamped_end)
                .map(|s| s.chars().count())
                .unwrap_or(1)
                .max(1)
        };
        let underline: String = "^".repeat(span_len);
        let padding: String = " ".repeat(col.saturating_sub(1));
        if label.message.is_empty() {
            out.push_str(&format!(
                "{:>width$} | {}{}\n",
                "",
                padding,
                underline,
                width = gutter
            ));
        } else {
            out.push_str(&format!(
                "{:>width$} | {}{} {}\n",
                "",
                padding,
                underline,
                label.message,
                width = gutter
            ));
        }
    }

    if let Some(help) = &diag.help {
        out.push_str(&format!(
            "{:>width$} = help: {}\n",
            "",
            help,
            width = gutter
        ));
    }
    out
}

/// 渲染一组诊断，诊断之间以空行分隔。
pub fn render_diagnostics(source: &SourceFile, diags: &[Diagnostic]) -> String {
    diags
        .iter()
        .map(|d| render_diagnostic(source, d))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_includes_code_path_and_line_col() {
        let file = SourceFile::new_virtual("main.scoop", "fun main() {\n    val x = }\n}\n");
        let diag = Diagnostic::error(
            "scoop::parse::expected_expression",
            "期望表达式，但遇到了 `}`",
        )
        .with_primary(Span::new(25, 26), "期望表达式")
        .with_help("检查是否漏写了右操作数");
        let text = render_diagnostic(&file, &diag);
        assert!(
            text.contains("error[scoop::parse::expected_expression]"),
            "{text}"
        );
        assert!(text.contains("--> main.scoop:2:13"), "{text}");
        assert!(text.contains("^ 期望表达式"), "{text}");
        assert!(text.contains("= help: 检查是否漏写了右操作数"), "{text}");
    }

    #[test]
    fn sink_collects_and_sorts() {
        let mut sink = DiagnosticSink::new();
        assert!(sink.is_empty());
        sink.push(Diagnostic::error("scoop::test::b", "第二条").with_primary(Span::new(9, 10), ""));
        sink.push(Diagnostic::error("scoop::test::a", "第一条").with_primary(Span::new(1, 2), ""));
        assert!(sink.has_errors());
        sink.sort_by_offset();
        let codes: Vec<_> = sink.iter().map(|d| d.code).collect();
        assert_eq!(codes, ["scoop::test::a", "scoop::test::b"]);
    }
}
