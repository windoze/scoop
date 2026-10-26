## 运行

在保存文件的目录执行：

```sh
scoop run --release main.scoop
```

## 预期结果

程序 stdout：

```text
6
0
7
calls=1
```

## 关键说明

`(Int) -> Int` 声明函数类型。transform 的最后一个参数可用尾随 lambda 提供，number 从期望类型取得 Int。factor 是外层 val，闭包创建时按值捕获。

共享可变状态显式放在 Counter 对象里：calls 的 binding 是 val，不能重新绑定，但其引用对象的 value 可以改变。负输入使用 `return@adjust 0` 只返回当前 lambda；正常路径增加计数并返回末尾表达式。

挂起 lambda 必须显式写 `suspend { ... }`；普通 lambda 不会因期望类型自动变成挂起函数，详见 `async/tasks`。

## 常见误用

不要让 lambda 直接捕获外层局部 var，读取也不允许；需要快照先绑定 val，需要共享状态用显式对象。lambda 的裸 return 非法，标签返回不能跨过另一个 callable。
