# Scoop language server

构建与启动：

```sh
cargo build --release -p scoop-lsp
target/release/scoop-lsp --stdio
```

将编辑器的 Scoop language server 命令设置为该可执行文件的绝对路径，参数为 `--stdio`。标准输入／输出仅承载 LSP Content-Length 消息，启动错误写到 stderr。此命令不依赖 LLVM；语法服务也不要求 sysroot。

当前已实现 initialize／shutdown／exit、完整或增量文档同步、UTF-8／UTF-16／UTF-32 位置协商（默认 UTF-16）、语法诊断、documentSymbol、foldingRange 和 selectionRange。大纲按客户端能力返回分层或平面结构。URI 为未保存文档时同样提供语法服务；本地文件关闭后恢复普通磁盘输入，文件变更通知不会覆盖打开的 buffer。

每个请求读取固定分析版本，未变化文件复用语法树；输入更新使旧计算失效并重新安排尚需响应的请求。传输循环在解析期间继续处理修改和取消，取消请求仍收到响应。诊断携带当前客户端版本，修复后发布空列表清除原诊断。

共享语义查询、项目加载、hover／定义／补全／签名帮助及工作区编辑继续按[完整设计](../../docs/language-service/DESIGN.md)实施，实际进度见[实施记录](../../docs/language-service/PROGRESS.md)。这些尚未交付的能力不出现在初始化能力表中。

开发验证使用 `cargo test -p scoop-analysis -p scoop-lsp`；LSP 集成测试启动真实 `scoop-lsp` 进程，检查传输、状态变化和查询结果。运行时将 `TMPDIR` 指向仓库外工作目录，测试工作文件遵守同一设置。
