//! 源文件与位置信息。
//!
//! 早期阶段我们先把“可靠的文件读取 + 行列号映射”做扎实，
//! 这样后续的诊断（diagnostics）与 fixtures 才能稳定。

use std::ops::Range;
use std::path::{Path, PathBuf};

use miette::{Result, miette};

use crate::span::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceOrigin {
    User,
    Sysroot,
}

/// 单个源文件。
#[derive(Debug, Clone)]
pub struct SourceFile {
    path: PathBuf,
    text: String,
    origin: SourceOrigin,
    /// 每一行起始的字节偏移（包含第 0 行的 0）。
    line_starts: Vec<usize>,
}

impl SourceFile {
    /// 从磁盘读取源文件。
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        Self::load_with_origin(path, SourceOrigin::User)
    }

    pub fn load_sysroot(path: impl AsRef<Path>) -> Result<Self> {
        Self::load_with_origin(path, SourceOrigin::Sysroot)
    }

    pub fn load_with_origin(path: impl AsRef<Path>, origin: SourceOrigin) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let text = std::fs::read_to_string(&path)
            .map_err(|e| miette!("读取源文件失败：{}: {}", path.display(), e))?;

        let line_starts = compute_line_starts(&text);

        Ok(Self {
            path,
            text,
            origin,
            line_starts,
        })
    }

    /// 创建一个“虚拟源文件”（常用于单元测试）。
    pub fn new_virtual(path: impl Into<PathBuf>, text: impl Into<String>) -> Self {
        Self::new_virtual_with_origin(path, text, SourceOrigin::User)
    }

    pub fn new_virtual_with_origin(
        path: impl Into<PathBuf>,
        text: impl Into<String>,
        origin: SourceOrigin,
    ) -> Self {
        let text = text.into();
        let line_starts = compute_line_starts(&text);
        Self {
            path: path.into(),
            text,
            origin,
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

    pub fn is_sysroot(&self) -> bool {
        self.origin == SourceOrigin::Sysroot
    }

    /// 根据 span 切片源文本。
    ///
    /// # Panics
    /// 如果 span 越界将 panic。编译器内部 span 应该始终来自 lexer/parser，
    /// 因此越界代表内部 bug。
    pub fn slice(&self, span: Span) -> &str {
        &self.text[span.start..span.end]
    }

    /// 将字节偏移映射为 (line, column)（均为 1-based）。
    ///
    /// 注意：本函数目前按 UTF-8 字节偏移处理，column 以 Unicode scalar
    /// 计数（通过 `chars()` 计算）。后续若需要 LSP 精确对齐，可改为 UTF-16。
    pub fn offset_to_line_col(&self, offset: usize) -> Result<(usize, usize)> {
        if offset > self.text.len() {
            return Err(miette!("offset 越界：{} > {}", offset, self.text.len()));
        }
        if !self.text.is_char_boundary(offset) {
            return Err(miette!(
                "offset 非 UTF-8 字符边界：{}（文件：{}）",
                offset,
                self.path.display()
            ));
        }

        let line_index = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };

        let line_start = self.line_starts[line_index];
        let column = self.text[line_start..offset].chars().count() + 1;
        Ok((line_index + 1, column))
    }
}

/// `SourceMap` 中某个源文件的稳定标识。
///
/// 说明：
/// - `SourceId` 只在当前 `SourceMap` 实例内有效；
/// - 后续多文件 lowering / codegen / diagnostics 可以用它把“文件身份”和本地 `Span`
///   绑定在一起，而不需要立刻把整个编译器的 `Span` 结构改成全局偏移。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceId(usize);

impl SourceId {
    pub fn as_usize(self) -> usize {
        self.0
    }
}

/// 一个绑定到具体源文件的本地 span。
///
/// 说明：
/// - `span` 仍然是该文件内的本地 UTF-8 字节偏移；
/// - `SourceMap` 负责把它转换为源文本、位置，以及未来可能需要的“全局 offset 空间”。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceMapSpan {
    pub source_id: SourceId,
    pub span: Span,
}

impl SourceMapSpan {
    pub fn new(source_id: SourceId, span: Span) -> Self {
        Self { source_id, span }
    }
}

/// 一个源位置（文件 + 1-based 行列号）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    pub path: PathBuf,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
struct SourceMapEntry {
    source: SourceFile,
    global_range: Range<usize>,
}

/// 多文件 source lookup 基础设施。
///
/// 设计要点：
/// - 统一持有一个编译单元内的全部 `SourceFile`；
/// - 每个文件分配一个互不重叠的“全局偏移区间”，便于后续把 `(SourceId, local Span)`
///   映射到统一的 span 空间；
/// - 当前阶段先提供文本切片、line/column 和 global span 查询，不立即改变现有 parser/HIR
///   对本地 `Span` 的使用方式。
#[derive(Debug, Clone, Default)]
pub struct SourceMap {
    entries: Vec<SourceMapEntry>,
    next_global_start: usize,
}

impl SourceMap {
    /// 创建一个空的 `SourceMap`。
    pub fn new() -> Self {
        Self::default()
    }

    /// 用一组源文件构建 `SourceMap`（按给定顺序分配 `SourceId`）。
    pub fn from_sources<I>(sources: I) -> Self
    where
        I: IntoIterator<Item = SourceFile>,
    {
        let mut map = Self::new();
        for source in sources {
            let _ = map.add_source(source);
        }
        map
    }

    /// 用一组 `SourceFile` 引用构建 `SourceMap`（内部克隆，便于与现有调用方对接）。
    pub fn from_source_refs<'a, I>(sources: I) -> Self
    where
        I: IntoIterator<Item = &'a SourceFile>,
    {
        Self::from_sources(sources.into_iter().cloned())
    }

    /// 追加一个源文件，并返回其 `SourceId`。
    pub fn add_source(&mut self, source: SourceFile) -> SourceId {
        let source_id = SourceId(self.entries.len());
        let global_start = self.next_global_start;
        let global_end = global_start + source.text.len();

        self.entries.push(SourceMapEntry {
            source,
            global_range: global_start..global_end,
        });

        // 文件之间预留一个字节空隙，避免相邻文件的边界在全局 span 空间里看起来连续重叠。
        self.next_global_start = global_end.saturating_add(1);
        source_id
    }

    /// 追加一个现有源文件的克隆。
    pub fn add_source_clone(&mut self, source: &SourceFile) -> SourceId {
        self.add_source(source.clone())
    }

    /// 返回 `SourceMap` 中源文件数量。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 遍历当前 source map 持有的全部源文件。
    pub fn sources(&self) -> impl Iterator<Item = &SourceFile> {
        self.entries.iter().map(|entry| &entry.source)
    }

    /// 通过 `SourceId` 取回源文件。
    pub fn source(&self, source_id: SourceId) -> Option<&SourceFile> {
        self.entries.get(source_id.0).map(|entry| &entry.source)
    }

    /// 通过源文件路径反查 `SourceId`。
    pub fn source_id_of_path(&self, path: &Path) -> Option<SourceId> {
        self.entries
            .iter()
            .position(|entry| entry.source.path() == path)
            .map(SourceId)
    }

    /// 返回某个源文件在全局偏移空间中的区间。
    pub fn global_range(&self, source_id: SourceId) -> Option<Range<usize>> {
        self.entries
            .get(source_id.0)
            .map(|entry| entry.global_range.clone())
    }

    /// 绑定一个本地 span，返回可被 `SourceMap` 查询的句柄。
    pub fn bind_span(&self, source_id: SourceId, span: Span) -> Result<SourceMapSpan> {
        self.validate_local_span(source_id, span)?;
        Ok(SourceMapSpan::new(source_id, span))
    }

    /// 切出某个已绑定 span 对应的源文本。
    pub fn slice(&self, span: SourceMapSpan) -> Result<&str> {
        let entry = self.entry(span.source_id)?;
        self.validate_span_against_source(&entry.source, span.span)?;
        Ok(entry.source.slice(span.span))
    }

    /// 返回某个已绑定 span 起点对应的文件/行/列信息。
    pub fn span_location(&self, span: SourceMapSpan) -> Result<SourceLocation> {
        self.location(span.source_id, span.span.start)
    }

    /// 返回某个源文件内 offset 对应的文件/行/列信息。
    pub fn location(&self, source_id: SourceId, offset: usize) -> Result<SourceLocation> {
        let entry = self.entry(source_id)?;
        let (line, column) = entry.source.offset_to_line_col(offset)?;
        Ok(SourceLocation {
            path: entry.source.path.clone(),
            line,
            column,
        })
    }

    /// 把一个本地 `(source_id, span)` 映射为全局偏移空间里的 `Span`。
    pub fn global_span(&self, span: SourceMapSpan) -> Result<Span> {
        let entry = self.entry(span.source_id)?;
        self.validate_span_against_source(&entry.source, span.span)?;
        let start = entry.global_range.start + span.span.start;
        let end = entry.global_range.start + span.span.end;
        Ok(Span::new(start, end))
    }

    fn entry(&self, source_id: SourceId) -> Result<&SourceMapEntry> {
        self.entries
            .get(source_id.0)
            .ok_or_else(|| miette!("未知 SourceId：{}", source_id.0))
    }

    fn validate_local_span(&self, source_id: SourceId, span: Span) -> Result<()> {
        let entry = self.entry(source_id)?;
        self.validate_span_against_source(&entry.source, span)
    }

    fn validate_span_against_source(&self, source: &SourceFile, span: Span) -> Result<()> {
        if span.start > span.end {
            return Err(miette!(
                "span 非法：start {} > end {}（文件：{}）",
                span.start,
                span.end,
                source.path.display()
            ));
        }
        if span.end > source.text.len() {
            return Err(miette!(
                "span 越界：{}..{} 超出文件范围 0..{}（文件：{}）",
                span.start,
                span.end,
                source.text.len(),
                source.path.display()
            ));
        }
        if !source.text.is_char_boundary(span.start) || !source.text.is_char_boundary(span.end) {
            return Err(miette!(
                "span 非 UTF-8 字符边界：{}..{}（文件：{}）",
                span.start,
                span.end,
                source.path.display()
            ));
        }
        Ok(())
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
        let file = SourceFile {
            path: PathBuf::from("<mem>"),
            text: "a\nbcd\nef".to_string(),
            origin: SourceOrigin::User,
            line_starts: compute_line_starts("a\nbcd\nef"),
        };

        assert_eq!(file.offset_to_line_col(0).unwrap(), (1, 1));
        assert_eq!(file.offset_to_line_col(1).unwrap(), (1, 2)); // 'a'
        assert_eq!(file.offset_to_line_col(2).unwrap(), (2, 1)); // 'b'
        assert_eq!(file.offset_to_line_col(4).unwrap(), (2, 3)); // 'd'
        assert_eq!(file.offset_to_line_col(6).unwrap(), (3, 1)); // 'e'
    }

    #[test]
    fn offset_to_line_col_rejects_non_char_boundary() {
        let file = SourceFile {
            path: PathBuf::from("<mem>"),
            text: "a中b".to_string(),
            origin: SourceOrigin::User,
            line_starts: compute_line_starts("a中b"),
        };

        let err = file.offset_to_line_col(2).unwrap_err();
        assert!(
            err.to_string().contains("UTF-8 字符边界"),
            "expected non-char-boundary offset to be rejected"
        );
    }

    #[test]
    fn source_map_slice_and_location_across_multiple_files() {
        let mut map = SourceMap::new();
        let alpha_id = map.add_source(SourceFile::new_virtual("alpha.scoop", "one\nalpha\n"));
        let beta_id = map.add_source(SourceFile::new_virtual("beta.scoop", "zero\nbeta\n"));

        let alpha_span = map.bind_span(alpha_id, Span::new(4, 9)).unwrap();
        let beta_span = map.bind_span(beta_id, Span::new(5, 9)).unwrap();

        assert_eq!(map.slice(alpha_span).unwrap(), "alpha");
        assert_eq!(map.slice(beta_span).unwrap(), "beta");

        let alpha_loc = map.span_location(alpha_span).unwrap();
        let beta_loc = map.span_location(beta_span).unwrap();

        assert_eq!(alpha_loc.path, PathBuf::from("alpha.scoop"));
        assert_eq!((alpha_loc.line, alpha_loc.column), (2, 1));
        assert_eq!(beta_loc.path, PathBuf::from("beta.scoop"));
        assert_eq!((beta_loc.line, beta_loc.column), (2, 1));
    }

    #[test]
    fn source_map_slice_rejects_non_char_boundary_span() {
        let mut map = SourceMap::new();
        let source_id = map.add_source(SourceFile::new_virtual("utf8.scoop", "a中b"));

        let err = map
            .slice(SourceMapSpan::new(source_id, Span::new(1, 2)))
            .unwrap_err();
        assert!(
            err.to_string().contains("UTF-8 字符边界"),
            "expected non-char-boundary span to be rejected"
        );
    }

    #[test]
    fn source_map_global_spans_are_non_overlapping() {
        let mut map = SourceMap::new();
        let first_id = map.add_source(SourceFile::new_virtual("first.scoop", "abc"));
        let second_id = map.add_source(SourceFile::new_virtual("second.scoop", "xyz"));

        let first_span = map
            .global_span(SourceMapSpan::new(first_id, Span::new(0, 3)))
            .unwrap();
        let second_span = map
            .global_span(SourceMapSpan::new(second_id, Span::new(0, 3)))
            .unwrap();

        assert!(first_span.end < second_span.start);
        assert_eq!(map.global_range(first_id).unwrap(), 0..3);
        assert_eq!(map.global_range(second_id).unwrap(), 4..7);
    }
}
