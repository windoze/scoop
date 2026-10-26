# 日志 facade 与 provider 示例

先完成[运行准备](../../../README.md#运行准备)，以下命令在本目录执行。日常使用先看 [console 示例](../../scoop.log.console/examples/README.md)；这里的两个程序只依赖 facade，演示 provider 协议和异常恢复。

| 文件 | 用法 |
| --- | --- |
| [memory.scoop](memory.scoop) | 实现过滤、接收 LogRecord、原子保存输出与 flush |
| [fallback.scoop](fallback.scoop) | provider 写出／刷新失败、闭包与 provider 连续失败、stderr 不可用 |

```sh
scoop run --release memory.scoop
scoop run --release fallback.scoop
```

## 实现 provider

`memory.scoop` 输出：

```text
INFO memory: ready
ERROR memory: [log message failed] failed to construct message
```

实现 `LogProvider` 的三个方法即可供应用安装：

- `isEnabled` 廉价检查 level 和 target，不输出、不求值消息。示例过滤 Trace。
- `emit` 接收已通过过滤、消息已求值的 LogRecord，不需要再次过滤或调用消息闭包。这里把文本写入 AtomicRef；其更新回调可能重试，因此不在回调内做 I/O 或其他副作用。
- `flush` 提交先前接收的记录。此例的记录在 emit 返回前已经写入内存，无待提交工作。

provider 可能被多线程调用，实现负责共享状态。这个示例只保留内存文本，用于展示接口；业务应用可按目标存储选择自己的实现。LogRecord 还包含 location、cause 和 messageFailure，可在 emit 中读取；异常字段保留原对象。

## 写出失败与 best effort

`fallback.scoop` 故意让 provider 的 emit 和 flush 抛异常。运行后 stdout 输出 `continued normally`，stderr 出现三条以 `[log fallback]` 开头的诊断：

1. 原消息 `original message` 与 provider 写出错误；
2. 消息构造错误、业务 cause 与 provider 写出错误；
3. provider 的 flush 错误。

这些诊断由 facade 使用固定格式写出，不重新调用故障 provider。查看程序自身的两个输出流：

```sh
scoop build --release fallback.scoop -o target/fallback
./target/fallback > output.txt 2> log.txt
```

在 Unix shell 中关闭示例进程的 stderr，可以观察最后一次 fallback 也失败时的行为：

```sh
./target/fallback 2>&-
```

它仍输出 `continued normally` 并正常退出。stderr fallback 是 best effort，构造或写出再次失败时直接丢弃，不重试、不递归、不改写到 stdout，也不把异常抛回业务。

消息构造异常通过同一个 provider 记录，写出异常才进入 stderr fallback；业务 cause 与消息闭包异常的用法见 [exceptions.scoop](../../scoop.log.console/examples/exceptions.scoop)。完整合同见[日志库规范](../../../../docs/specs/SCOOP-LOG-SPEC.md)。
