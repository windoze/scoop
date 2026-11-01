# 下载与后台校验

这是组合 `scoop.ui.component`、`scoop.window.asyncio`、HTTP client 和真实后台线程的可执行示例。先按仓库流程准备 aarch64 开发 sysroot，再运行：

```sh
scoop run sysroot/src/scoop.ui.component/examples/download --release --target aarch64-apple-darwin --sysroot /path/to/sysroot -- https://example.com/file.zip /tmp/file.zip
```

点击 Start / Restart 开始下载。进度有两个明确阶段：Downloading 表示异步传输，Verifying 表示后台线程正在对已经提交的文件计算 SHA-256；摘要成功返回后才显示 Completed。HTTP 的未知长度保持未知，不把字节百分比当作完成结果。选择较大的文件可以直接观察校验阶段；小文件可能很快完成，不人为添加延迟。

下载和校验期间都可以操作点击计数、取消或重新开始。点击计数是组件自己的 State，下载模型是应用持有的普通 class；两种方式同时存在。取消校验保留已下载文件，重新开始会在成功下载后替换目的文件。旧任务的迟到进度和结果不会覆盖新的任务。

`HttpClient` 绑定 UI owner 的 EventLoop，网络及异步文件等待挂起协程，进度回调在 owner 更新 State。计算密集的摘要不放进这些回调：`Thread.start` 在另一条原生线程上创建、使用和关闭 File、NativeByteBuffer 及 Digest。它只发送只读字节数／总量快照，经 ApplicationProxy 回到 owner 后才更新 State；工作线程不读写组件、State 或 Canvas。

后台的 `ChecksumMailbox` 只保留最新进度，至多排一个待执行通知；UI 展示另按真实回调节流。二者都没有周期任务或自己的时钟。进度条的真实数值过渡复用 Surface Animatable，终态立即收尾；静止时没有动画或持续帧需求。

组件的资源作用域关闭 DownloadController 时撤销发布、取消 HTTP Job 和后台计算，不等待后台线程。应用拥有 ChecksumWorkers：正常完成时回收线程句柄并移除任务，事件循环退出后再取消、join 尚未回收的线程，覆盖关闭时 proxy 拒绝或丢弃通知的情况。应用的最外层 use 管理宿主、共享字体与后台任务集合，普通组件不需要逐个 use。

校验线程在每个文件块之间检查取消。UI 与异步 I/O 共用既有宿主，所有绘制和 State 更新都在窗口 owner 执行。

本 Cone 的 `scoop test --release` 使用真实 loopback HTTP 服务、临时文件、原生 Thread 和两个 Metal 窗口，不访问外部服务。普通测试覆盖摘要结果、块尾、取消及进度回调异常；窗口组合覆盖真实按钮输入、共享状态与局部状态、未知长度、取消／替换、隐藏时完成、单根原生关闭及应用异常退出。安静区间同时检查逻辑／绘制计数、底层帧需求、display link 暂停及实际回调／提交计数，截图不代替这些断言。
