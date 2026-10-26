## 运行

在保存文件的目录执行：

```sh
scoop run --release main.scoop
```

## 预期结果

程序 stdout：

```text
original=(3, 4)
moved=(8, 4)
true
```

## 关键说明

主构造器的 `val` 字段属于值。`original.{ x: 8 }` 产生一个只更新 x 的副本；original 和副本的 y 保持原值。`val (x, y) = moved` 按字段声明顺序解构，不依赖用户定义 componentN。

值复制不等于深复制：如果字段保存一个引用对象，两个副本仍可引用同一个对象。本例的 Int 字段没有共享可变状态。这里的结构 `==` 可直接比较 Point，但不会自动使 Point 满足 `Equals<Point>`、`Hash` 或 `ToString` 接口。

## 常见误用

不要写 Kotlin 的 `copy(x = ...)`，也不要给 struct 加可变存储字段。需要作为字典 key 或直接做字符串转换时，应按实际接口要求显式提供能力；本例直接打印各字段。
