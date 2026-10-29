# Scoop 配套工具

[scoop-example](scoop-example/README.md) 是独立的 `scoop example` 阅读器，按主题提供完整程序与说明，使用自己的 Cargo 项目和测试。[scoop-programming](scoop-programming/SKILL.md) 是面向 AI 的 Scoop 编程 skill，组合示例、库文档及 CLI 完成程序开发与验证。

Windows 主驱动使用 `sco.exe`，下文的 `scoop ...` 在 Windows 上对应 `sco ...`。例如 `sco example` 仍运行 `scoop-example.exe`；编辑器继续使用 `scoop-lsp.exe`。macOS／Linux 的命令名保持不变。

## Scoop 编程 skill

`scoop-programming/` 包含 `SKILL.md`、`agents/openai.yaml` 和按需阅读的 `references/`，用于编写、修改、调试和测试 Scoop 程序，不用于同名 Windows 包管理器。内容覆盖示例查询与文件还原、语言差异、库 API 查找、Cone 依赖、格式化、检查、构建、测试及运行；完整示例仍由配套 `scoop example` 提供，不在 skill 中另存副本。

`scripts/package_toolchain.py` 将整个目录复制到 dist 工具包内的 `tooling/scoop-programming/`，组件清单以 `programming_skill` 记录相对路径。构建及安装包不自动安装或注册 skill，不修改 AI 工具的配置；使用者可向 AI 提供 `SKILL.md`，或按所用工具的方式自行添加整个目录，保留相对引用。

维护 skill 时同步核对当前 CLI 帮助、示例和库声明；在临时工作目录用配套工具实际编译、运行涉及的写法。归档布局与参考文件完整性由 `python3 -m unittest discover -s scripts/tests -p test_package_toolchain.py` 覆盖；修改 Python 后先按 [scripts 说明](../scripts/README.md) 执行格式化与 lint。

## 编辑器工具

本目录提供独立的 Tree-sitter 语法包和 VS Code 扩展。语法以
[语言规范](../docs/specs/SCOOP-SPEC.md)为准，并使用编译器已有源码和专门的编辑器
fixture 检查两者的一致性。
- `tree-sitter-scoop/`：grammar、生成的 C parser，以及 `highlights.scm`、
  `folds.scm`、`indents.scm`。可以供其他 Tree-sitter 编辑器独立使用。
- `vscode-scoop/`：使用 `web-tree-sitter` 和打包的 WASM，在本地扩展宿主中解析
  `.scoop` 文档，提供高亮、折叠和缩进；通过 `vscode-languageclient` 启动
  `scoop-lsp --stdio`，接入编译器语言服务。

## 功能约定

编辑器 parser 产生可恢复、可增量更新的语法树。类型检查、名称解析和正式编译诊断
由编译器负责；这份 grammar 不作为编译 pipeline 的输入。未完成的源码仍可使用
已识别部分的高亮、折叠和缩进。LSP 提供诊断、补全、签名、导航、符号、引用、
重命名、选择范围和快速修复，不自动构建项目或准备 sysroot。

高亮查询覆盖声明、类型、参数、成员访问、关键字、运算符、字面量、注解和注释。
普通字符串中的 `$` 保持文本；只有 f-string 中的 `${...}` 解析成表达式。
VS Code 将这些语法分类映射到标准 semantic token 类型及主题 scope，使用者可以
沿用现有配色主题。

折叠来自实际的块、列表、跨行字符串和块注释；相邻 import 与行注释可以折叠。
缩进查询标出增加缩进的语法结构、闭合分隔符和需要保持原样的字符串／注释文本。
VS Code 支持输入换行及闭合分隔符时调整缩进，也支持对文档或选区重新缩进。
重新缩进只改行首空白，保留表达式排版及字符串、注释内部的内容。

VS Code 的 offset 和 Tree-sitter Web binding 都使用 UTF-16 code unit。文档更新先
对旧树应用编辑，再增量解析新文本；文档关闭及扩展停用时释放 tree、parser 和 query。

## 开发

依赖 Node.js 22.18 或更新版本和 C 编译器。Tree-sitter 0.27 的 `build --wasm`
使用 WASI SDK 和 Binaryen，首次构建会自动下载配套版本；也可以用
`TREE_SITTER_WASI_SDK_PATH`、`TREE_SITTER_BINARYEN_PATH` 选择本机安装的工具链。
Tree-sitter CLI、Web runtime 和其他 npm 工具由 lockfile 固定。
生成的 C parser、节点描述、WASM 和 VSIX 均为构建产物，不纳入版本控制。

```sh
cd tooling
npm ci
npm run format
npm run lint
npm run build
npm test
npm run test:vscode
npm run package
```

最终 VSIX 位于 `vscode-scoop/`。扩展安装与调试说明见
[扩展 README](vscode-scoop/README.md)。安装 VSIX 后使用扩展不需要 Node.js、
WASI SDK 或外部 Tree-sitter 服务。语义功能需要 `scoop-lsp` 和已准备的 sysroot；
默认发现 `~/.scoop/bin/scoop-lsp`，配置和测试准备见扩展 README。

完整工具链发行由仓库根的 `python3 scripts/package_toolchain.py` 统一构建；该入口
按同一 lockfile 安装 npm 依赖并生成 VSIX，放入安装包的 `tooling/`。无需预先手动
构建扩展，安装包的使用者可直接安装 `~/.scoop/tooling/` 中的 VSIX。
