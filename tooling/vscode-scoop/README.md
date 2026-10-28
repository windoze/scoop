# Scoop Language Support

为 `.scoop` 文件提供 `scoop-lsp` 语言服务和 Tree-sitter 编辑功能。
- LSP：实时诊断、成员补全、悬停文档、签名帮助、定义／类型定义／实现跳转、文档与
  工作区符号、引用、重命名、选择范围和缺失标点的快速修复。
- Tree-sitter：语法高亮、代码折叠、输入时缩进以及文档／选区重新缩进。重新缩进
  只调整行首空白，保留 raw 字符串和块注释内部文本，遵守空格／tab 设置。

高亮通过 VS Code semantic token API 接入主题，分类依据为语法树。扩展默认启用
Scoop 文件的 semantic highlighting 和输入时格式化；显式关闭
`editor.semanticHighlighting.enabled` 或 `editor.formatOnType` 会禁用对应功能。

支持本地及远程工作区中的桌面 VS Code 1.100 及更新版本。扩展内含 parser、runtime
WASM 和语言客户端；语法功能可独立运行。LSP 使用工作区所在机器上的 `scoop-lsp`，
完整语义分析还需要与服务器匹配的 sysroot。扩展不会自动编译项目、准备 sysroot
或运行生成器；不属于 Cone 的文件及未保存的新文件仍有 LSP 语法诊断和结构查询。

## 安装和配置

将完整工具包解压到 `~/.scoop` 后安装随包 VSIX：

```sh
code --install-extension "$HOME"/.scoop/tooling/scoop-language-support-*.vsix
```

也可在 VS Code 的扩展菜单选择“从 VSIX 安装”。打开 `.scoop` 文件即可激活。
扩展依次使用 `scoop.server.path`、`~/.scoop/bin/scoop-lsp`、PATH 中的 `scoop-lsp`，
以 `--stdio` 启动；路径可包含空格，支持 `~/`，不填写额外命令行参数。
默认 sysroot 沿服务端规则使用 `SCOOP_SYSROOT` 或服务器安装根。
服务器在受信任工作区运行，受限工作区保留 Tree-sitter 功能。

开发构建或安装到其他目录时，在 VS Code 设置中指定：

```json
{
  "scoop.server.path": "/absolute/path/to/scoop/target/release/scoop-lsp",
  "scoop.sysroot": "/absolute/path/to/scoop/sysroot"
}
```

| 设置                        | 默认值与用途                                                                                                                 |
| --------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `scoop.server.enabled`      | `true`；关闭后仅保留 Tree-sitter 功能                                                                                        |
| `scoop.server.path`         | 空；按上述顺序发现服务器，修改后自动重启                                                                                     |
| `scoop.sysroot`             | 空；沿服务端默认规则选取，显式相对路径基于 Cone 根                                                                           |
| `scoop.target`              | 空；服务端宿主 target triple                                                                                                 |
| `scoop.purpose`             | `production`；`test` 同时分析根 Cone 的测试输入                                                                              |
| `scoop.artifactSearchRoots` | `[]`；额外预编译依赖搜索目录                                                                                                 |
| `scoop.packageManager` | 空；使用 scoop-lsp 同目录的 scoop-pm |
| `scoop.packageCacheDir` | 空；沿 PM 的共享源码缓存规则 |
| `scoop.lockfile` | 空；使用 workspace 的 Cone.lock |
| `scoop.configurations`      | `[]`；命名配置，每项必须有唯一 `name` 和 Cone 根 `rootUri`（file URI），可覆盖 target、sysroot、purpose、artifactSearchRoots 和上述 PM 设置 |
| `scoop.activeConfiguration` | 空；按文档选择配置，也可指定配置名称                                                                                         |
| `scoop.trace.server`        | `messages`；可选 `verbose` 增加细节；先通过 `Developer: Set Log Level` 将 Scoop LSP 日志等级设为 Trace                       |

分析配置改变后通知现有服务器。空 configurations 使用工作区目录与打开文件最近的
`Cone.toml` 发现项目；支持多根工作区、未保存的 Scoop／Cone.toml 文本，以及磁盘上
源码、manifest、Cone.lock 和 slib 的变更。依赖由用户显式 resolve/fetch 或 build/run 准备；LSP 离线消费锁定图，不自动下载或改写 lock。workspace 根加载全部成员，缺少锁定输入的成员仍保留语法服务。使用测试或多目标配置的完整例子见
[服务器说明](../../compiler/lsp/README.md)。

命令面板提供 `Scoop: Restart Language Server` 和 `Scoop: Show Language Server Output`。
启动失败时可在输出中检查路径和配置；修正后重启即可，高亮、折叠和缩进仍可使用。

## 构建、调试和测试

从仓库根目录构建服务器，并按[准备脚本说明](../../scripts/README.md)显式准备开发
sysroot。普通单元测试不需要服务器；真实扩展宿主测试需要 release 服务器和预编译库：

```sh
cargo build --release -p scoop-lsp -p scoop-pm
cd tooling
npm ci
npm run format
npm run lint
npm run build
npm test
npm run test:vscode
npm run package
code --install-extension vscode-scoop/scoop-language-support-0.2.0.vsix
```

`npm test` 包含 Tree-sitter grammar 与扩展单元测试；只运行扩展单元测试可使用
`npm test --workspace scoop-language-support`。构建后用 VS Code 打开
`tooling/vscode-scoop/`，按 F5 启动开发宿主；开发宿主沿正常设置选择服务器与 sysroot。

`test:vscode` 默认使用仓库的 `target/release/scoop-lsp` 和 `sysroot/`，可通过
`SCOOP_TEST_LSP`、`SCOOP_SYSROOT` 覆盖。测试使用独立工作区、用户配置和扩展目录，
验证离线语法功能及真实 LSP 的补全、文档、签名、导航、引用、重命名、诊断、修复、
配置更新、文件监听、重启与启动失败后的恢复，不隐式构建服务器或准备系统库。
默认下载稳定版 VS Code；可设置 `VSCODE_EXECUTABLE_PATH` 使用已有可执行文件。

VSIX 包含语言客户端和运行依赖及许可，无需用户安装 npm 依赖。打包只生成 VSIX，
不发布 Marketplace。完整工具包的单一入口仍为 `python3 scripts/package_toolchain.py`，
会一并构建 `scoop-lsp` 和扩展，分别放入 `bin/` 与 `tooling/`。
