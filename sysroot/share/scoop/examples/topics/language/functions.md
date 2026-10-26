## 运行

在保存文件的目录执行：

```sh
scoop run --release main.scoop
```

## 预期结果

程序 stdout：

```text
Hello, Scoop!
Hi, Ada.
Hello, Lin!
Hello, Lin!
```

## 关键说明

`greeting` 使用表达式体，`repeatGreeting` 使用块体和显式 return。参数的 binding 不可重新赋值。调用者可以按位置提供 name，也可用名字提供参数并改变书写顺序。

省略参数时才求值其默认表达式。公开 API 的参数名会影响命名调用，重命名需考虑调用者。例子只调用正数 count；如果业务允许零或负数，应在函数里明确处理，不要用循环范围猜测校验语义。vararg 是另一种具名函数调用约定，不属于普通函数值的参数类型。

## 常见误用

表达式体直接给出结果，不在其中再写 return。把函数保存为函数值后，调用按函数类型的位置参数进行，不能继续使用原声明的参数名或默认参数。
