# Scoop language server

构建与启动：

```sh
cargo build --release -p scoop-lsp
target/release/scoop-lsp --stdio
```

将编辑器的 Scoop language server 命令设置为该可执行文件的绝对路径，参数为 `--stdio`。标准输入／输出仅承载 LSP Content-Length 消息，启动错误写到 stderr。此命令不依赖 LLVM；语法服务也不要求 sysroot。

当前已实现 initialize／shutdown／exit、完整或增量文档同步、UTF-8／UTF-16／UTF-32 位置协商（默认 UTF-16）、语法诊断、documentSymbol、foldingRange 和 selectionRange。大纲按客户端能力返回分层或平面结构。URI 为未保存文档时同样提供语法服务；本地文件关闭后恢复普通磁盘输入，文件变更通知不会覆盖打开的 buffer。

每个请求读取固定分析版本，未变化文件复用语法树；输入更新使旧计算失效并重新安排尚需响应的请求。传输循环在解析期间继续处理修改和取消，取消请求仍收到响应。诊断携带当前客户端版本，修复后发布空列表清除原诊断。

工作区复用普通 Cone.toml、源码条件与依赖选择。workspaceFolders 或 rootUri 可指向 Cone 根；较大的仓库目录在打开文件时选择其最近的 Cone 根，同时加载声明的源码依赖。未归属项目的文件继续提供语法服务。打开的 Cone.toml 也覆盖磁盘；错误 manifest、缺失依赖与无效目标以 `SCOOP_PROJECT` 诊断发布。服务器响应 workspace/didChangeConfiguration、workspace/didChangeWorkspaceFolders 和文件变更通知；客户端支持动态文件监听注册时，服务器请求监听 manifest、Scoop 源码与 slib。

初始化选项支持如下配置；同样的对象可放在配置变更通知的 `settings.scoop` 中：

```json
{
  "target": "aarch64-apple-darwin",
  "sysroot": "/opt/scoop",
  "purpose": "production",
  "artifactSearchRoots": ["/work/artifacts"]
}
```

target 默认使用宿主的现有目标选择；purpose 可为 production 或 test，默认 production。Test 只将根 Cone 的测试源码和测试依赖加入输入，源码依赖仍按 production 处理。sysroot 沿用 `SCOOP_SYSROOT` 或安装布局，编辑器不自动准备系统库、运行 native 构建或启动生成器。相对 sysroot／搜索路径相对于所属 Cone 根解析。

同一文件需使用不同目标或用途时，可以在 `configurations` 中显式列出配置，并用 `activeConfiguration` 选取语义上下文：

```json
{
  "sysroot": "/opt/scoop",
  "configurations": [
    {"name": "app", "rootUri": "file:///work/app", "purpose": "production"},
    {"name": "app-tests", "rootUri": "file:///work/app", "purpose": "test"}
  ],
  "activeConfiguration": "app-tests"
}
```

每个配置还可覆盖 target、sysroot 和 artifactSearchRoots。配置名称须非空且唯一；未指定 activeConfiguration 时按配置顺序选择当前文件所属的第一个项目。同一文件共享文本和语法，项目归属与依赖保持分开。

hover／定义／补全／签名帮助及工作区编辑继续按[完整设计](../../docs/language-service/DESIGN.md)实施，实际进度见[实施记录](../../docs/language-service/PROGRESS.md)。这些尚未交付的能力不出现在初始化能力表中。

开发验证使用 `cargo test -p scoop-analysis -p scoop-lsp`；LSP 集成测试启动真实 `scoop-lsp` 进程，检查传输、状态变化和查询结果。运行时将 `TMPDIR` 指向仓库外工作目录，测试工作文件遵守同一设置。

预编译 core／标准库的正式进程覆盖位于 `tests/fixtures/lsp`，使用显式准备的 sysroot，按 fixture runner 的 `--scoop-lsp` 参数选择服务器。
