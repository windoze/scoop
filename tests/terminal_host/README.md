# 终端 PTY 测试工具

参考 atto-ui-test-host 的 portable-pty＋vt100 结构，在本仓库独立构建；
不依赖兄弟工作区，也不进入 Scoop 库的运行时依赖。

```sh
cargo build --manifest-path tests/terminal_host/Cargo.toml --release --target-dir target
cargo test --manifest-path tests/terminal_host/Cargo.toml --target-dir target
python3 tests/run_fixtures.py --filter m59-terminal-pty-output \
  --terminal-host "$PWD/target/release/terminal-host"
```

fixture runner 只执行已经构建好的工具，不自动构建 host。host 按运行平台构建，
Linux GNU host 可启动当前机器能运行的 GNU／musl Scoop 程序。

`terminal-host output PROGRAM` 以固定尺寸和 TERM／COLORTERM 启动真实程序，
清除 NO_COLOR。每一步等待本次输出标记，再检查原始 cell、颜色、光标、
宽字符、擦除、滚动和备用屏幕。完整尺寸及空格保留，不 trim 屏幕。
固定的 vt100 0.16.2 不模拟 DECAWM（自动换行开关）；该命令检查原始序列，
不根据模拟器的换行结果判断真实终端行为。

`session`／`session-raw` 场景检查 raw 嵌套、异常恢复、重复关闭、独立 fd 和重定向。
在启动程序前保存 PTY 属性，按 flag、控制字符、速率及 Linux line discipline
比较设置。排除动态 PENDIN 位：[Darwin 内核](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/tty.c)
恢复 ICANON 时会设置它，读入字符后再清除；测试不改变被测终端来强求位级相等。

`input` 场景发送分片 UTF-8、Escape、鼠标、focus、paste、Kitty 与非法字节，
验证 poll 保留事件、总截止时间、resize 和规范模式 EOF。逐次安装普通、
SA_SIGINFO、SA_RESETHAND、SIG_IGN、SIG_DFL disposition，检查原 handler、
mask 和一次性行为；恢复规则依据实际 OS 状态。Darwin 被测库使用 kqueue，
保留原 disposition；Linux 使用 self-pipe。输出标记返回实际匹配结束位置，
同一读取块中的后续标记不会被跳过。

`query` 场景禁用自动回复，按步骤发送独立的 CPR、主／次设备属性、Kitty
回复及交错用户输入，验证 stdin／stdout 重定向、超时、非法回复、旧回复、
零增强标志与不支持的区别。所有原始查询必须出现在当前 PTY。

读取线程只传递字节，主线程按顺序维护虚拟屏幕并发送输入和查询回复。
查询 callback 在查询发生的位置读取光标，支持分片；Kitty 不回复与返回 0 分开。
正常结束等待进程退出及 PTY EOF，检查读取结果并 join；失败／超时终止独立
PTY 进程组，回收进程并停止 reader。SIGINT／SIGTERM 触发相同清理流程。
每次运行最长 30 秒；失败打印 raw bytes、逐 cell 内容、光标和退出状态。
