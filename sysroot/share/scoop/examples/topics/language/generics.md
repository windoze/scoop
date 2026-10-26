## 运行

在保存文件的目录执行：

```sh
scoop run --release main.scoop
```

## 预期结果

程序 stdout：

```text
42
Scoop
Hello, Ada!
Hello, Scoop!
```

## 关键说明

Box<T> 保存一个 T。`Box<Int>(42)` 明确给类型，`Box("Scoop")` 从构造实参推断为 Box<String>；需要在类型位置写 Box 时必须提供完整参数。

`fun <T : Named>` 约束实际类型具有 Named 能力，所以函数体可调用 displayName。User 与 Project 的调用分别使用实际类型单态化。名义泛型不变：即使 User 可以转换为 Named，也不能把 Box<User> 当成 Box<Named>；需要时先转换元素再构造目标 Box。

## 常见误用

不要写 Kotlin 的 in/out 投影、星号投影，或省略类型位置的泛型参数。普通泛型函数和接口成员职责不同；不要给 interface 方法本身声明泛型参数。
