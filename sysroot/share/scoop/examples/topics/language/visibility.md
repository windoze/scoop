## 运行

保存完整的 library/ 与 app/ 两个同级目录，然后执行：

```sh
cd app
scoop run --release .
```

## 预期结果

程序 stdout：

```text
Hello, Ada!
```

## 关键说明

app 的依赖以精确坐标和相对 path 引用 library。path 相对 app/Cone.toml 所在目录；import 引入公开类型，不能替代依赖。

声明默认 internal，边界是 Cone。Greeter 的类型、构造器和 message 方法分别显式 public；公开接口中的方法也显式 public，实现写 public override。私有 name 仍只供 Greeter 使用。

package 决定名字组织，不授予另一个 Cone 对 internal 成员的访问权。可以扩展公开 API，但不能因两边 package 相同就省略 public。

## 常见误用

只写 `public class Greeter(...)` 不会自动公开构造器。保存文件时保持 ../library 相对关系；只复制消费者源码无法复现依赖。
