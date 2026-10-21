# Scoop Language Support

为 `.scoop` 文件提供基于 Tree-sitter 的语法高亮、代码折叠和缩进。
- 高亮声明、类型、参数、成员、关键字、注解、文档注释与字面量，支持 Unicode
  标识符、反引号名称，以及普通／raw f-string 中的嵌套插值。
- 按实际语法结构折叠代码块、跨行参数和实参列表、模式、字符串和注释；支持连续
  import 和行注释折叠。
- 输入换行或 `}`、`]`、`)` 时修正当前行缩进；“格式化文档”和“格式化选区”
  执行重新缩进，遵守当前编辑器的空格／tab 设置。重新缩进只调整行首空白，保留
  raw 字符串及块注释内部文本。

高亮通过 VS Code 的 semantic token API 接入已有主题，分类依据为语法树。
扩展默认启用 Scoop 文件的 semantic highlighting 和输入时格式化；如在用户设置中
显式关闭了 `editor.semanticHighlighting.enabled` 或 `editor.formatOnType`，需要重新
启用相应设置才能使用高亮或输入时缩进。文档／选区重新缩进可以独立执行。

扩展内含 parser 和 runtime WASM，支持本地及远程工作区中的桌面 VS Code 1.100
及更新版本，无须安装 Scoop 编译器。当前版本不包含 LSP、补全、跳转、重命名或
类型诊断。

## 构建和安装

从仓库根目录执行：

```sh
cd tooling
npm ci
npm run build
npm test
npm run package
code --install-extension vscode-scoop/scoop-language-support-0.1.0.vsix
```

也可以在 VS Code 的扩展菜单中选择“从 VSIX 安装”。本地扩展标识为
`scoop.scoop-language-support`；打包命令只生成 VSIX，不发布到 Marketplace。

## 调试和测试

构建后用 VS Code 打开 `tooling/vscode-scoop/`，按 F5 启动扩展开发宿主。
语法源及三个查询位于同级的 `tree-sitter-scoop/`。

```sh
cd tooling
npm run format
npm run lint
npm test
npm run test:vscode
```

`test:vscode` 使用独立的测试配置和扩展目录，通过真实扩展宿主验证 `.scoop`
识别、激活、高亮、折叠、文档／选区缩进和编辑后的刷新。默认下载稳定版 VS Code；
可设置 `VSCODE_EXECUTABLE_PATH` 使用已有 VS Code 可执行文件。

Tree-sitter 捕获和语法行为的独立／组合 fixture 由普通 Node 测试及 Tree-sitter
corpus 覆盖，包含编辑中不完整源码和 UTF-16 增量编辑。
