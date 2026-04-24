//! 编译期 warning 的轻量捕获与去重。
//!
//! 说明：
//! - 当前工程长期以“出错即返回 `miette::Diagnostic`”为主，尚无统一的非致命诊断通道；
//! - `T4012b2` 需要为 `@Deprecated` 建立最小可测的 warning-on-use 合同，因此先补一层
//!   线程内的 warning capture；
//! - 默认情况下（未安装 capture）warning 会被静默丢弃，避免污染直接调用编译库的单测；
//! - `scoop build/run` 会显式安装 capture，并在前端成功后统一打印 warning。

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::source::SourceFile;
use crate::span::Span;

pub const DEPRECATED_WARNING_CODE: &str = "deprecated";
pub const ENUM_SIZE_DISPARITY_WARNING_CODE: &str = "enum-size-disparity";
pub const REDUNDANT_WHEN_ELSE_WARNING_CODE: &str = "redundant-when-else";

pub fn is_known_warning_code(code: &str) -> bool {
    matches!(
        code,
        DEPRECATED_WARNING_CODE
            | ENUM_SIZE_DISPARITY_WARNING_CODE
            | REDUNDANT_WHEN_ELSE_WARNING_CODE
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarningSuppression {
    file: PathBuf,
    span: Option<Span>,
    codes: Vec<String>,
}

impl WarningSuppression {
    pub fn for_file(file: &Path, codes: Vec<String>) -> Self {
        Self::new(file, None, codes)
    }

    pub fn for_span(file: &Path, span: Span, codes: Vec<String>) -> Self {
        Self::new(file, Some(span), codes)
    }

    fn new(file: &Path, span: Option<Span>, mut codes: Vec<String>) -> Self {
        codes.sort();
        codes.dedup();
        Self {
            file: file.to_path_buf(),
            span,
            codes,
        }
    }

    fn suppresses(&self, warning: &CompileWarning) -> bool {
        if self.file != warning.file {
            return false;
        }
        if !self.codes.iter().any(|code| code == warning.code) {
            return false;
        }
        match self.span {
            None => true,
            Some(scope) => {
                scope.start <= warning.span.start
                    && warning.span.end <= scope.end.max(warning.span.start)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CompileWarning {
    code: &'static str,
    file: PathBuf,
    span: Span,
    rendered: String,
}

impl CompileWarning {
    pub fn deprecated_use(
        source: &SourceFile,
        span: Span,
        subject_kind: &'static str,
        subject_name: &str,
        message: &str,
        replace_with: Option<&str>,
    ) -> Self {
        let mut rendered =
            format!("warn[{DEPRECATED_WARNING_CODE}]: {subject_kind} `{subject_name}` 已弃用");
        if !message.is_empty() {
            rendered.push('：');
            rendered.push_str(message);
        }
        if let Some(replacement) = replace_with.filter(|value| !value.is_empty()) {
            rendered.push_str("；建议改用 `");
            rendered.push_str(replacement);
            rendered.push('`');
        }

        Self {
            code: DEPRECATED_WARNING_CODE,
            file: source.path().to_path_buf(),
            span,
            rendered,
        }
    }

    pub fn enum_size_disparity(
        source: &SourceFile,
        span: Span,
        enum_fqn: &str,
        boxed_variants: &[String],
        max_size: u64,
        second_size: u64,
    ) -> Self {
        let rendered = format!(
            "warn[{ENUM_SIZE_DISPARITY_WARNING_CODE}]: enum `{enum_fqn}` 的 variant payload 尺寸差异显著；已对 oversized variant 做 boxing（boxed={}; max_size={max_size}; second_size={second_size}）",
            boxed_variants.join(", ")
        );

        Self {
            code: ENUM_SIZE_DISPARITY_WARNING_CODE,
            file: source.path().to_path_buf(),
            span,
            rendered,
        }
    }

    pub fn redundant_when_else(source: &SourceFile, span: Span) -> Self {
        Self {
            code: REDUNDANT_WHEN_ELSE_WARNING_CODE,
            file: source.path().to_path_buf(),
            span,
            rendered: format!(
                "warn[{REDUNDANT_WHEN_ELSE_WARNING_CODE}]: `when` 已经穷尽；`else` 分支是冗余的"
            ),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn file(&self) -> &Path {
        self.file.as_path()
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn render(&self) -> &str {
        self.rendered.as_str()
    }
}

#[derive(Debug, Default)]
struct WarningBuffer {
    seen: HashSet<CompileWarning>,
    warnings: Vec<CompileWarning>,
}

thread_local! {
    static ACTIVE_WARNINGS: RefCell<Option<WarningBuffer>> = const { RefCell::new(None) };
    static ACTIVE_SUPPRESSIONS: RefCell<Vec<WarningSuppression>> = const { RefCell::new(Vec::new()) };
}

pub struct WarningCaptureGuard {
    previous: Option<WarningBuffer>,
    finished: bool,
}

pub struct WarningSuppressionGuard {
    previous: Vec<WarningSuppression>,
    finished: bool,
}

pub fn begin_capture() -> WarningCaptureGuard {
    let previous = ACTIVE_WARNINGS.with(|slot| {
        let mut slot = slot.borrow_mut();
        slot.replace(WarningBuffer::default())
    });
    WarningCaptureGuard {
        previous,
        finished: false,
    }
}

impl WarningCaptureGuard {
    pub fn finish(mut self) -> Vec<CompileWarning> {
        self.finished = true;
        ACTIVE_WARNINGS.with(|slot| {
            let mut slot = slot.borrow_mut();
            let current = slot
                .take()
                .expect("warning capture should be active before finish");
            *slot = self.previous.take();
            current.warnings
        })
    }
}

impl Drop for WarningCaptureGuard {
    fn drop(&mut self) {
        if self.finished {
            return;
        }

        ACTIVE_WARNINGS.with(|slot| {
            let mut slot = slot.borrow_mut();
            let _ = slot.take();
            *slot = self.previous.take();
        });
    }
}

pub fn install_suppressions(suppressions: Vec<WarningSuppression>) -> WarningSuppressionGuard {
    let previous = ACTIVE_SUPPRESSIONS.with(|slot| {
        let mut slot = slot.borrow_mut();
        let previous = slot.clone();
        if !suppressions.is_empty() {
            slot.extend(suppressions);
        }
        previous
    });

    WarningSuppressionGuard {
        previous,
        finished: false,
    }
}

impl WarningSuppressionGuard {
    pub fn finish(mut self) {
        self.finished = true;
        ACTIVE_SUPPRESSIONS.with(|slot| {
            let mut slot = slot.borrow_mut();
            *slot = self.previous.clone();
        });
    }
}

impl Drop for WarningSuppressionGuard {
    fn drop(&mut self) {
        if self.finished {
            return;
        }

        ACTIVE_SUPPRESSIONS.with(|slot| {
            let mut slot = slot.borrow_mut();
            *slot = self.previous.clone();
        });
    }
}

pub fn emit(warning: CompileWarning) {
    let suppressed = ACTIVE_SUPPRESSIONS.with(|slot| {
        slot.borrow()
            .iter()
            .any(|suppression| suppression.suppresses(&warning))
    });
    if suppressed {
        return;
    }

    ACTIVE_WARNINGS.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(active) = slot.as_mut() else {
            return;
        };
        if active.seen.insert(warning.clone()) {
            active.warnings.push(warning);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_deduplicates_identical_warnings() {
        let source = SourceFile::new_virtual("<mem>", "fun main() {}\n");
        let span = Span::new(0, 4);

        let guard = begin_capture();
        emit(CompileWarning::deprecated_use(
            &source,
            span,
            "函数",
            "a.old",
            "use a.new",
            Some("a.new"),
        ));
        emit(CompileWarning::deprecated_use(
            &source,
            span,
            "函数",
            "a.old",
            "use a.new",
            Some("a.new"),
        ));

        let warnings = guard.finish();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code(), DEPRECATED_WARNING_CODE);
    }

    #[test]
    fn suppression_filters_matching_warning_by_span() {
        let source = SourceFile::new_virtual("<mem>", "fun main() {}\n");
        let capture = begin_capture();
        let suppressions = install_suppressions(vec![WarningSuppression::for_span(
            source.path(),
            Span::new(0, 12),
            vec![DEPRECATED_WARNING_CODE.to_string()],
        )]);

        emit(CompileWarning::deprecated_use(
            &source,
            Span::new(4, 8),
            "函数",
            "a.old",
            "",
            None,
        ));
        emit(CompileWarning::redundant_when_else(
            &source,
            Span::new(4, 8),
        ));

        suppressions.finish();
        let warnings = capture.finish();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code(), REDUNDANT_WHEN_ELSE_WARNING_CODE);
    }

    #[test]
    fn file_suppression_filters_matching_warning_code() {
        let source = SourceFile::new_virtual("<mem>", "fun main() {}\n");
        let capture = begin_capture();
        let suppressions = install_suppressions(vec![WarningSuppression::for_file(
            source.path(),
            vec![ENUM_SIZE_DISPARITY_WARNING_CODE.to_string()],
        )]);

        emit(CompileWarning::enum_size_disparity(
            &source,
            Span::new(0, 4),
            "demo.Big",
            &["Huge".to_string()],
            128,
            8,
        ));
        emit(CompileWarning::redundant_when_else(
            &source,
            Span::new(0, 4),
        ));

        suppressions.finish();
        let warnings = capture.finish();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code(), REDUNDANT_WHEN_ELSE_WARNING_CODE);
    }
}
