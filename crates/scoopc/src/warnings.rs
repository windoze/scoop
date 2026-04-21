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
}

pub struct WarningCaptureGuard {
    previous: Option<WarningBuffer>,
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

pub fn emit(warning: CompileWarning) {
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
}
