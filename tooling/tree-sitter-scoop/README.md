# Tree-sitter Scoop

依据 [Scoop 语言规范](../../docs/specs/SCOOP-SPEC.md)实现的独立 Tree-sitter grammar，
包含 `queries/highlights.scm`、`queries/folds.scm` 和 `queries/indents.scm`。
编辑器语法树支持错误恢复和增量更新；正式诊断及类型检查由 Scoop 编译器负责。

从仓库根目录构建：

```sh
cd tooling
npm ci
npm run build
npm test
```

`npm run generate` 生成 `src/parser.c`、节点描述和 C 头文件，`npm run build` 另外
生成 `tree-sitter-scoop.wasm` 并构建 VS Code 扩展。这些生成产物不纳入版本控制。
源 grammar 按声明、类型、表达式、模式和字面量分成小模块；`src/scanner.c` 只负责
换行分界和字符串文本的词法处理。表达式等语法类别声明为 Tree-sitter supertype，
查询可以直接使用这些类别。

仅验证原生 parser 时：

```sh
cd tooling
npm run generate
cd tree-sitter-scoop
../node_modules/.bin/tree-sitter test
../node_modules/.bin/tree-sitter parse test/fixtures/control.scoop
```

corpus 固定少量代表语法树；Node 测试覆盖每项语法的独立／组合 fixture、歧义分界、
查询编译和 `sysroot/src` 下的实际 Scoop 源码。新增语言语法时，应同步更新 grammar
及相关查询，并增加 fixture。

WASM 构建依赖 WASI SDK 和 Binaryen；完整工具要求与版本锁定方式见
[tooling README](../README.md)。VS Code 对查询的使用方式见
[扩展 README](../vscode-scoop/README.md)。
