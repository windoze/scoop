## 运行

在保存文件的目录执行：

```sh
scoop run --release tasks.scoop
```

## 预期结果

程序 stdout：

```text
42
Cancelled task cleaned up
```

## 关键说明

普通 main 显式创建 EventLoop，并通过 loop.run 进入挂起上下文。scope.async 创建带结果的任务，两个 await 取得结果之后才打印 42；睡眠长短不决定输出顺序。

scope.launch 用于不返回业务结果的任务。例子让出循环，使 pending 开始等待，再 cancel 并 join；finally 在取消退出时打印收尾文字。真正需要“任务已启动”的业务协议应显式建立同步条件，不把延时当作通用同步手段。

suspend 必须写在 lambda 上。创建任务、遇到挂起点和创建线程是不同事情；此代码不承诺后台线程或 CPU 并行执行。

## 常见误用

不要在普通 main 中直接调用挂起 API，也不要把 cancel 当成清理已经完成。需要等待退出时仍使用 join/await，并让资源清理处在实际任务的作用域里。
