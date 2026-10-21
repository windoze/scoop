# Scoop 编辑器工具

本目录提供独立的 Tree-sitter 语法包和 VS Code 扩展。语法以
[语言规范](../docs/specs/SCOOP-SPEC.md)为准，并使用编译器已有源码和专门的编辑器
fixture 检查两者的一致性。
- `tree-sitter-scoop/`：grammar、生成的 C parser，以及 `highlights.scm`、
  `folds.scm`、`indents.scm`。可以供其他 Tree-sitter 编辑器独立使用。
- `vscode-scoop/`：使用 `web-tree-sitter` 和打包的 WASM，在本地扩展宿主中解析
  `.scoop` 文档，提供高亮、折叠和缩进。

## 功能约定

编辑器 parser 产生可恢复、可增量更新的语法树。类型检查、名称解析和正式编译诊断
由编译器负责；这份 grammar 不作为编译 pipeline 的输入。未完成的源码仍可使用
已识别部分的高亮、折叠和缩进。此次交付不包含 LSP。

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
WASI SDK、Scoop 编译器或外部 Tree-sitter 服务。
