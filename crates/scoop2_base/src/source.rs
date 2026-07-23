//! 源文件身份、行表与行列映射。

use std::fmt;
use std::path::{Path, PathBuf};

use crate::Span;

/// 编译会话中某个源文件的稳定标识（在其所属文件列表中的下标）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileId(pub u32);

impl FileId {
    pub fn as_u32(self) -> u32 {
        self.0
    }

    pub fn as_usize(self) -> usize {
        self.0 as usize
    }
}

impl fmt::Display for FileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "file#{}", self.0)
    }
}

/// 源文件的物理来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceOrigin {
    User,
    Sysroot,
}

/// 源文件的信任级别。
///
/// 语义边界：[`SourceOrigin`] 只描述物理来源；语言特权（如 `@Intrinsic`
/// 内建实现）必须由 `TrustedSyslib` 授予，不能由 `Sysroot` 来源直接授予。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceTrust {
    Untrusted,
    TrustedSyslib,
}

/// 单个源文件：路径、全文、来源/信任级别与行表。
#[derive(Debug, Clone)]
pub struct SourceFile {
    path: PathBuf,
    text: String,
    origin: SourceOrigin,
    trust: SourceTrust,
    /// 每一行起始的字节偏移（包含第 0 行的 0）。
    line_starts: Vec<usize>,
}

/// 加载源文件失败。
#[derive(Debug)]
pub struct SourceLoadError {
    pub path: PathBuf,
    pub io: std::io::Error,
}

impl fmt::Display for SourceLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "读取源文件失败：{}: {}", self.path.display(), self.io)
    }
}

impl std::error::Error for SourceLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.io)
    }
}

impl SourceFile {
    /// 从磁盘读取用户源文件。
    pub fn load(path: impl AsRef<Path>) -> Result<Self, SourceLoadError> {
        Self::load_with(path, SourceOrigin::User, SourceTrust::Untrusted)
    }

    /// 从磁盘读取 sysroot 源文件。
    pub fn load_sysroot(path: impl AsRef<Path>) -> Result<Self, SourceLoadError> {
        Self::load_with(path, SourceOrigin::Sysroot, SourceTrust::Untrusted)
    }

    /// 从磁盘读取受信任的标准库源文件。
    pub fn load_trusted_syslib(path: impl AsRef<Path>) -> Result<Self, SourceLoadError> {
        Self::load_with(path, SourceOrigin::Sysroot, SourceTrust::TrustedSyslib)
    }

    pub fn load_with(
        path: impl AsRef<Path>,
        origin: SourceOrigin,
        trust: SourceTrust,
    ) -> Result<Self, SourceLoadError> {
        let path = path.as_ref().to_path_buf();
        let text = std::fs::read_to_string(&path).map_err(|io| SourceLoadError {
            path: path.clone(),
            io,
        })?;
        Ok(Self::with_text(path, text, origin, trust))
    }

    /// 创建一个内存中的源文件（单元测试与合成代码使用）。
    pub fn new_virtual(path: impl Into<PathBuf>, text: impl Into<String>) -> Self {
        Self::with_text(
            path.into(),
            text.into(),
            SourceOrigin::User,
            SourceTrust::Untrusted,
        )
    }

    pub fn new_virtual_with(
        path: impl Into<PathBuf>,
        text: impl Into<String>,
        origin: SourceOrigin,
        trust: SourceTrust,
    ) -> Self {
        Self::with_text(path.into(), text.into(), origin, trust)
    }

    fn with_text(path: PathBuf, text: String, origin: SourceOrigin, trust: SourceTrust) -> Self {
        let line_starts = compute_line_starts(&text);
        Self {
            path,
            text,
            origin,
            trust,
            line_starts,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn origin(&self) -> SourceOrigin {
        self.origin
    }

    pub fn trust(&self) -> SourceTrust {
        self.trust
    }

    pub fn is_sysroot(&self) -> bool {
        self.origin == SourceOrigin::Sysroot
    }

    pub fn is_trusted_syslib(&self) -> bool {
        self.trust == SourceTrust::TrustedSyslib
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// 根据 span 切片源文本。越界返回 `None`（调用方负责保证 span 来自本文件）。
    pub fn get_slice(&self, span: Span) -> Option<&str> {
        self.text.get(span.start..span.end)
    }

    /// 将字节偏移映射为 (line, column)（均为 1-based，column 以 Unicode scalar 计数）。
    ///
    /// 越界或非字符边界的 offset 会被钳制到最近合法位置，以保证诊断渲染永不失败。
    pub fn offset_to_line_col(&self, offset: usize) -> (usize, usize) {
        let mut offset = offset.min(self.text.len());
        while offset > 0 && !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        let line_index = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        let line_start = self.line_starts[line_index];
        let column = self.text[line_start..offset].chars().count() + 1;
        (line_index + 1, column)
    }

    /// 返回 1-based 行号对应的整行文本（不含换行符）；越界返回空串。
    pub fn line_text(&self, line: usize) -> &str {
        if line == 0 || line > self.line_starts.len() {
            return "";
        }
        let start = self.line_starts[line - 1];
        let end = if line < self.line_starts.len() {
            self.line_starts[line]
        } else {
            self.text.len()
        };
        self.text[start..end].trim_end_matches(['\n', '\r'])
    }

    /// 返回 1-based 行号对应的行起始字节偏移；越界返回 `None`。
    pub fn line_start_offset(&self, line: usize) -> Option<usize> {
        if line == 0 || line > self.line_starts.len() {
            return None;
        }
        Some(self.line_starts[line - 1])
    }
}

fn compute_line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (idx, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(idx + 1);
        }
    }
    starts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_col_mapping_basic() {
        let file = SourceFile::new_virtual("<mem>", "a\nbcd\nef");
        assert_eq!(file.offset_to_line_col(0), (1, 1));
        assert_eq!(file.offset_to_line_col(1), (1, 2));
        assert_eq!(file.offset_to_line_col(2), (2, 1));
        assert_eq!(file.offset_to_line_col(4), (2, 3));
        assert_eq!(file.offset_to_line_col(6), (3, 1));
    }

    #[test]
    fn line_col_clamps_out_of_range_offsets() {
        let file = SourceFile::new_virtual("<mem>", "a中b");
        // 非字符边界 offset 被钳制到最近合法位置。
        assert_eq!(file.offset_to_line_col(2), (1, 2));
        // 超出文件末尾的 offset 被钳制到 EOF（col 为最后一个字符之后）。
        assert_eq!(file.offset_to_line_col(100), (1, 4));
    }

    #[test]
    fn line_text_round_trip() {
        let file = SourceFile::new_virtual("<mem>", "one\ntwo\n");
        assert_eq!(file.line_text(1), "one");
        assert_eq!(file.line_text(2), "two");
        assert_eq!(file.line_text(3), "");
    }
}
