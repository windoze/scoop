# scoop.platform 示例

先完成[运行准备](../../../README.md#运行准备)，以下命令在本目录执行。

| 文件 | 内容 |
| --- | --- |
| [files.scoop](files.scoop) | 创建临时目录、写入/追加文本、读取 metadata 和遍历目录 |
| [threads.scoop](threads.scoop) | 启动线程、原子共享计数、join 与资源释放 |

```sh
scoop run --release files.scoop
scoop run --release threads.scoop
```

文件示例的完整说明见 `scoop example io/files`（[正文](../../../share/scoop/examples/topics/io/files.md)），文件句柄的正常和异常清理见 `scoop example io/resources`。

线程示例中主线程与工作线程各增加计数 1000 次，等待工作完成后输出 `2000`。
