//! 源文件与位置信息。
//!
//! 早期阶段我们先把“可靠的文件读取 + 行列号映射”做扎实，
//! 这样后续的诊断（diagnostics）与 fixtures 才能稳定。

use std::path::{Path, PathBuf};

use miette::{miette, Result};

/// 单个源文件。
#[derive(Debug, Clone)]
pub struct SourceFile {
    path: PathBuf,
    text: String,
    /// 每一行起始的字节偏移（包含第 0 行的 0）。
    line_starts: Vec<usize>,
}

impl SourceFile {
    /// 从磁盘读取源文件。
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let text = std::fs::read_to_string(&path)
            .map_err(|e| miette!("读取源文件失败：{}: {}", path.display(), e))?;

        let line_starts = compute_line_starts(&text);

        Ok(Self {
            path,
            text,
            line_starts,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// 将字节偏移映射为 (line, column)（均为 1-based）。
    ///
    /// 注意：本函数目前按 UTF-8 字节偏移处理，column 以 Unicode scalar
    /// 计数（通过 `chars()` 计算）。后续若需要 LSP 精确对齐，可改为 UTF-16。
    pub fn offset_to_line_col(&self, offset: usize) -> Result<(usize, usize)> {
        if offset > self.text.len() {
            return Err(miette!(
                "offset 越界：{} > {}",
                offset,
                self.text.len()
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
            line_starts: compute_line_starts("a\nbcd\nef"),
        };

        assert_eq!(file.offset_to_line_col(0).unwrap(), (1, 1));
        assert_eq!(file.offset_to_line_col(1).unwrap(), (1, 2)); // 'a'
        assert_eq!(file.offset_to_line_col(2).unwrap(), (2, 1)); // 'b'
        assert_eq!(file.offset_to_line_col(4).unwrap(), (2, 3)); // 'd'
        assert_eq!(file.offset_to_line_col(6).unwrap(), (3, 1)); // 'e'
    }
}

