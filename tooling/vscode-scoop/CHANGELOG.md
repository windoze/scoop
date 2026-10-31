# 更新记录

## 0.2.0

接入 `scoop-lsp`：诊断、补全、签名、导航、符号、引用、重命名、选择范围与快速修复。
支持安装目录发现、工作区与分析配置、未保存的 Cone.toml、文件监听、日志和重启。
保留离线 Tree-sitter 高亮、折叠及缩进；完整工具包同时提供服务器和 VSIX。

## 0.1.0

首版 Scoop 编辑器支持：Tree-sitter 高亮、折叠、输入时缩进及文档／选区重新缩进。

## 0.3.0

- Add Run/Debug CodeLens for top-level main functions and individual test cases, with automatic launch configurations and builds.
- Resolve the nearest Cone for source launches and select tests by their compiled directory index, including explicitly selected ignored tests.
- Add native Scoop debugging with the bundled Python-enabled LLVM 22 adapter, published as `scoop-lldb-dap` to avoid PATH conflicts with other LLVM installations.
- Build full debug targets and test executables through the compiler's JSON output.
- Add read-only String, array, enum and object formatters, launch/attach configurations,
  source mapping, terminal support and commands for debugging the current file.
