## 运行

在保存文件的目录执行：

```sh
scoop run --release main.scoop
```

## 预期结果

程序 stdout：

```text
stop writing
closed=true
saved before failure
temporary directory removed
```

## 关键说明

File.open 返回拥有句柄的 File。writer.use 在正常或异常退出时调用 dispose，Closeable 的 dispose 转发到 close。例子故意在写入后抛出异常，并在捕获后观察 isClosed；读取文件的正常路径同样放在 use 中。最外层 finally 删除临时目录，输出不会泄漏随机路径。

use 负责同步清理，也有允许挂起 body 的重载；useAsync 用于异步清理对象。资源是拥有还是借用，由相应 API 合同决定，不能把借来的资源一律关闭。正文与 dispose 同时失败时，ResourceUseException 保存 primaryFailure 与 cleanupFailure。

关闭、缓冲 flush、格式 finish、协议结束与持久化有各自职责。这里直接写 File，没有额外缓冲器或压缩格式；不要把 close 当成所有协议步骤的替代品。

## 常见误用

不要依赖 GC 的时间来关闭文件。需要重复使用句柄时，安排好拥有者的作用域，而不是在 use 退出后继续读写已关闭的 File。
