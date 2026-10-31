# Scoop Language Support

为 `.scoop` 文件提供 `scoop-lsp` 语言服务和 Tree-sitter 编辑功能。
- LSP：实时诊断、成员补全、悬停文档、签名帮助、定义／类型定义／实现跳转、文档与
  工作区符号、引用、重命名、选择范围和缺失标点的快速修复。
- Tree-sitter：语法高亮、代码折叠、输入时缩进以及文档／选区重新缩进。重新缩进
  只调整行首空白，保留 raw 字符串和块注释内部文本，遵守空格／tab 设置。
- CodeLens：在顶层 `main` 和测试容器的 `@Test` 方法上显示 Run／Debug，支持限定注解名、
  import alias 和被忽略用例的显式执行。

高亮通过 VS Code semantic token API 接入主题，分类依据为语法树。扩展默认启用
Scoop 文件的 semantic highlighting 和输入时格式化；显式关闭
`editor.semanticHighlighting.enabled` 或 `editor.formatOnType` 会禁用对应功能。

支持本地及远程工作区中的桌面 VS Code 1.100 及更新版本。扩展内含 parser、runtime
WASM 和语言客户端；语法功能可独立运行。LSP 使用工作区所在机器上的 `scoop-lsp`，
完整语义分析还需要与服务器匹配的 sysroot。编辑和显示 CodeLens 不触发编译；点击
Run／Debug 后才调用普通构建入口。扩展不自动准备 sysroot；不属于 Cone 的文件及
未保存的新文件仍有 LSP 语法诊断和结构查询。

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

| 设置                        | 默认值与用途                                                                                                                                |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `scoop.server.enabled`      | `true`；关闭后仅保留 Tree-sitter 功能                                                                                                       |
| `scoop.server.path`         | 空；按上述顺序发现服务器，修改后自动重启                                                                                                    |
| `scoop.sysroot`             | 空；沿服务端默认规则选取，显式相对路径基于 Cone 根                                                                                          |
| `scoop.target`              | 空；服务端宿主 target triple                                                                                                                |
| `scoop.purpose`             | `production`；`test` 同时分析根 Cone 的测试输入                                                                                             |
| `scoop.artifactSearchRoots` | `[]`；额外预编译依赖搜索目录                                                                                                                |
| `scoop.packageManager`      | 空；使用 scoop-lsp 同目录的 scoop-pm                                                                                                        |
| `scoop.packageCacheDir`     | 空；沿 PM 的共享源码缓存规则                                                                                                                |
| `scoop.lockfile`            | 空；使用 workspace 的 Cone.lock                                                                                                             |
| `scoop.configurations`      | `[]`；命名配置，每项必须有唯一 `name` 和 Cone 根 `rootUri`（file URI），可覆盖 target、sysroot、purpose、artifactSearchRoots 和上述 PM 设置 |
| `scoop.activeConfiguration` | 空；按文档选择配置，也可指定配置名称                                                                                                        |
| `scoop.trace.server`        | `messages`；可选 `verbose` 增加细节；先通过 `Developer: Set Log Level` 将 Scoop LSP 日志等级设为 Trace                                      |

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
cargo build --release -p scoop -p scoopc -p scoop-linker -p scoop-lsp -p scoop-pm --bins
cd tooling
npm ci
npm run format
npm run lint
npm run build
npm test
npm run test:vscode
npm run package
code --install-extension vscode-scoop/scoop-language-support-0.3.0.vsix
```

`npm test` 包含 Tree-sitter grammar 与扩展单元测试；只运行扩展单元测试可使用
`npm test --workspace scoop-language-support`。构建后用 VS Code 打开
`tooling/vscode-scoop/`，按 F5 启动开发宿主；开发宿主沿正常设置选择服务器与 sysroot。

`test:vscode` 默认使用仓库的 `target/release/scoop-lsp` 和 `sysroot/`，可通过
`SCOOP_TEST_LSP`、`SCOOP_SYSROOT` 覆盖；Run／Debug 验证使用同目录的 `scoop`，可通过
`SCOOP_TEST_COMPILER` 指定。测试使用独立工作区、用户配置和扩展目录，
验证离线语法功能及真实 LSP 的补全、文档、签名、导航、引用、重命名、诊断、修复、
配置更新、文件监听、重启与启动失败后的恢复；CodeLens 验证实际调用编译器并运行
本次生成的程序，检查自动启动配置、用例精确选择及构建失败处理。测试不隐式构建
编译器／服务器或准备系统库，也不要求宿主授予原生调试权限。
默认下载稳定版 VS Code；可设置 `VSCODE_EXECUTABLE_PATH` 使用已有可执行文件。

VSIX 包含语言客户端和运行依赖及许可，无需用户安装 npm 依赖。打包只生成 VSIX，
不发布 Marketplace。完整工具包的单一入口仍为 `python3 scripts/package_toolchain.py`，
会一并构建 `scoop-lsp` 和扩展，分别放入 `bin/` 与 `tooling/`。

## 原生调试

打开源码，在 `main` 或测试用例上方点击 **Run** 或 **Debug** 即可启动。扩展自动生成
本次 launch profile，无需先创建 `.vscode/launch.json`。Run 忽略断点，Debug 启用断点、
单步和变量查看；两者均先保存并以 debug profile、full 调试信息编译，再启动本次程序。
构建失败或取消时不启动程序，错误及构建输出可通过 `Scoop: Show Debug Output` 查看。

构建输入和默认工作目录取源文件最近的父级 `Cone.toml`，因此多 Cone 工作区会选择
当前文件所属的成员。没有 manifest 的 `main` 按单文件编译；测试需要 manifest Cone。
测试 CodeLens 根据本次编译返回的完整测试目录精确选择所点方法，包含其构造与
BeforeTest／AfterTest 生命周期，不会连带运行同名前缀的其他用例。显式点击被
`@Ignore` 标记的用例仍会执行该用例。CodeLens 使用本地语法树，LSP 关闭时也可显示；
最终入口和测试声明仍由编译器验证。`editor.codeLens` 控制这些源码标记的显示。

安装本发行包的 `scoop-lldb-dap`（LLVM 22，启用 Python），按 F5 选择 Scoop，或运行
`Scoop: Debug Current File`。默认发现 `~/.scoop/bin/scoop` 和
`~/.scoop/bin/scoop-lldb-dap`，随后查询 PATH；可通过 `scoop.debug.compilerPath` 和
`scoop.debug.adapterPath` 指定路径。调试器在 workspace host 上运行，支持 Remote SSH。
Windows 默认发现 `~/.scoop/bin/sco.exe` 与 `~/.scoop/bin/scoop-lldb-dap.exe`；其他平台的主驱动名仍为 `scoop`。所有平台的发行 LLVM 命令均带 `scoop-` 前缀，显式配置的命令路径按原值使用。

Cone 的最小 `.vscode/launch.json` 配置：

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "type": "scoop",
      "request": "launch",
      "name": "Scoop: Build and debug",
      "build": { "input": "${workspaceFolder}" },
      "console": "integratedTerminal"
    }
  ]
}
```

启动前保存文件，调用 `scoop build --profile debug --debug-info full`，从 JSON 结果读取
实际程序路径；编译仍使用 Scoop 的现有缓存。`build.profile: "release"` 可调试优化程序。
单文件可用 `build.input: "${file}"`；已有程序设置 `program` 和 `build: false`。
附加进程使用 `request: "attach"` 和 `pid`，按操作系统的普通调试权限运行。

`build.tests: true` 使用 `scoop test --no-run`，直接启动生成的测试 executable；
可用 `build.filter` 按完整测试名称的子串选择用例。未指定过滤器时运行非 ignored 用例。
CodeLens 自动填写的 `build.testCase` 使用源码相对路径、容器名及方法名精确选择，
优先于 `build.filter`。
额外构建选项放入 `build.args`，程序选项放入顶层 `args`。`scoop.sysroot` 同时用于调试构建。
多 Cone workspace 应通过 `build.input` 选择一个具体 executable Cone。

支持源码断点、单步、物理调用栈、局部值、参数、全局变量以及 String、数组、enum 展示。
String 按 UTF-8 byte length 读取，保留内嵌 NUL，长文本截断为 4096 字节预览；数组按
DAP 请求的范围读取元素。enum 仅展示有效变体；`[raw]` 可查看原始字段。
预编译库应自行带有调试信息；源码路径不同可设置
`"sourceMap": [["/build/source", "${workspaceFolder}"]]`，邻接的 `.dSYM`／`.debug` 自动加载。
`env`、`cwd`、`stdio`、`stopOnEntry` 和 `initCommands` 直接传给 LLVM lldb-dap。

调试显示不调用 getter、`toString()` 或任何 inferior 函数，不修改 managed 对象。
GC 后无法保证的旧变量位置显示 unavailable，程序再次读取后可恢复；在 GC 内部暂停时
不保证堆可读。闭包捕获值在实际读取区间可见。协程当前使用物理栈，逻辑 completion 链
及完整 Scoop 表达式求值留待后续。Debug Console 的变量路径及 Clang 标量算式能力不代表
完整 Scoop 语义；以反引号开头可执行 LLDB 命令。
