# Tools

该目录用于放置 Scoop 仓库的辅助工具/脚本（与编译器实现解耦）。

当前包含：

- `tools/scoop_tools/`：Rust 工具箱（建议通过 `cargo run -p scoop_tools -- ...` 运行）
  - `spec-fixtures sync`：从 `SCOOP_FULL_SPEC.md` 抽取带 `// FIXTURE:` 的代码块，更新 `tests/fixtures/spec_doctest/`
  - `spec-fixtures check`：检查生成结果是否与规范一致（CI 会执行）

