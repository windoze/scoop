# scoop.terminal.asyncio 示例

先完成[运行准备](../../../README.md#运行准备)，以下命令在本目录的真实交互终端执行。会话需要 TTY，不能通过普通 pipe 提供输入。

| 文件 | 内容 |
| --- | --- |
| [keys.scoop](keys.scoop) | 异步读取按键/尺寸变化、raw mode 作用域与退出恢复 |
| [countdown.scoop](countdown.scoop) | 查询终端尺寸、异步命令输出与定时器 |

```sh
scoop run --release keys.scoop
scoop run --release countdown.scoop
```

按键示例打印收到的字符，按 Escape 退出；raw mode 由 `withRawMode` 恢复。倒计时示例显示终端列数/行数，约 0.9 秒后显示 `Countdown: done` 并自动退出。关闭会话使用 `useAsync`，输出命令的 `execute` 会等待 flush。
