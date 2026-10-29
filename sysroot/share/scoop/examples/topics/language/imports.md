## 运行

按显示路径保存全部文件，在 Cone.toml 所在目录执行：

```sh
scoop run --release .
```

## 预期结果

程序 stdout：

```text
Hello, Ada!
Good day, Ada.
Hello, Lin!
Good day, Lin.
Hello, Lin!
Good day, Grace.
```

## 关键说明

两个 greeting 属于同一 Cone 的不同 package。文件级 import 导入 friendly.greeting；main 中的 alias 导入使 formalGreeting 可在后续语句使用。

块级 import 从声明之后生效，嵌套块继承，离开块后恢复外层绑定。因此 if 块中第一次调用 greeting 使用 friendly，导入之后使用 formal，离开 if 后又使用 friendly。lambda 中的 star import 只影响该 lambda。三个形式分别是 `import a.b.f`、`import a.b.f as alias` 和 `import a.b.*`。

块级导入可用于函数、lambda、局部函数、初始化、accessor 和控制流的语句块；局部类型的正文按定义位置继承有效的导入作用域。同一块先考虑 exact／alias，再考虑 star；嵌套块的导入先于外层导入和文件级导入。局部变量、局部函数和隐含 this 的真实成员仍按原有优先级查找；同层不同声明发生歧义时使用明确的 exact import 或 alias。

import 在编译期解析名称，没有运行期装载动作。它可以指向当前 Cone 中可访问的声明或直接依赖提供的公开名称，但不会自动增加依赖。这里两个 helper 的默认 internal 可见性允许整个 Cone 使用；跨 Cone 需按 `language/visibility` 声明依赖和 public API。

普通函数导入后可使用短名引用 `::greeting`，alias 也可以作为引用名。compiler intrinsic 是否可引用由各自语义决定；`scoop.compiler` 中的编译期输入函数不可引用，见 `language/compiler-inputs`。

## 常见误用

- 文件级 import 写在 package 后、顶层声明前；块级 import 写在使用它的语句之前，不能让前面的语句提前看到它。
- 块内不允许 public import；re-export 是文件级声明。star import 不能加 alias。
- exact import 的终点必须是可导入的声明；导入一组名称用 `.*`，不写 `import examples.imports.formal`。
- 包限定顶层函数调用 `examples.imports.formal.greeting(...)` 和引用 `::examples.imports.formal.greeting` 不受支持。先导入，再使用短名或 alias。
- 运行整个示例传 Cone 目录；`scoop run src/main.scoop` 是单文件模式，不会加载另外两个源文件。
