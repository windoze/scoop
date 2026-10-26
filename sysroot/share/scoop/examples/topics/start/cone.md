## 运行

按显示路径保存三个文件，在 Cone.toml 所在目录执行：

```sh
scoop run --release .
```

从空目录创建另一份基础项目时，可使用现有命令（目标目录需尚不存在）：

```sh
scoop new --group examples.demo my-app --no-tests
```

## 预期结果

程序 stdout：

```text
Hello, Ada!
Hello, Lin!
```

## 关键说明

`kind = "executable"` 声明可执行 Cone；默认发现 `src/` 中的源码。两个文件都属于 `examples.greeting` package，它与 manifest group 对齐。普通顶层函数默认 internal，所以同一 Cone 的 main 能调用 greeting，不必为跨文件调用加 public。

manifest 模式显式声明依赖。本例的 `listOf` 来自 std，因此除 core 外还声明 `scoop.std`。import 仍负责名称导入。可以增加另一个 src 文件或替换名字列表，不要把程序源码移到默认发现范围外。

## 常见误用

不要用 `scoop run src/main.scoop` 期待它加载整个项目：那会选择单文件入口。应传项目目录或 Cone.toml。跨 Cone 的 public 规则见 `language/visibility`。
