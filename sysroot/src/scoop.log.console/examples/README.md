# 日志使用示例

先完成[运行准备](../../../README.md#运行准备)，以下命令在本目录执行。每个文件都是独立程序，头部 `//! dep:` 声明 facade 和 console 依赖；单文件入口默认提供 core 和 std。

| 文件 | 用法 | 日志输出 |
| --- | --- | --- |
| [hello.scoop](hello.scoop) | 应用安装 provider、创建 Logger、字符串与闭包入口、flush | stderr，默认格式 |
| [levels.scoop](levels.scoop) | 五个级别、最低级别、惰性过滤与实参求值 | stdout，简短格式 |
| [formatting.scoop](formatting.scoop) | 自定义格式、固定 Clock、包装函数转发 SourceLocation | stdout，带调用位置 |
| [exceptions.scoop](exceptions.scoop) | 业务 cause、消息闭包异常、急切实参异常边界 | stderr，默认格式 |
| [configuration.scoop](configuration.scoop) | 更换和恢复全局 provider、已有 Logger 生效、关闭日志 | stdout，标识当前 provider |

```sh
scoop run --release hello.scoop
scoop run --release levels.scoop
scoop run --release formatting.scoop
scoop run --release exceptions.scoop
scoop run --release configuration.scoop
```

## 从默认配置开始

`hello.scoop` 在应用入口调用 `Log.setProvider(ConsoleLogProvider())`。普通库只需依赖 `scoop.log`，并通过 `Logger("分类名")` 记录事件。Logger 可以先于 provider 创建；未配置时默认静默。

示例输出两条日志，时间为实际 UTC 时间：

```text
<UTC时间> [INFO] example: ready
<UTC时间> [INFO] example: listening on 8080
```

默认流是 stderr，最低级别是 Info。可分别重定向业务输出与日志：

```sh
scoop run --release hello.scoop > output.txt 2> log.txt
```

`scoop run` 自身的构建诊断也写 stderr；只想观察程序日志时，先执行 `scoop build --release hello.scoop -o target/hello`，再运行 `./target/hello > output.txt 2> log.txt`。

## 级别与惰性消息

`levels.scoop` 的输出为：

```text
INFO levels: message evaluations=1
WARN levels: retrying request
ERROR levels: request failed
INFO levels: explicit level
```

Trace 和 Debug 被 Info 阈值过滤。`log.debug { details(calls) }` 不执行闭包；`log.debug(details(calls))` 的实参在进入 Logger 前已求值，所以计数仍增加一次。希望禁用日志时跳过消息构造，或把构造异常交给日志库处理时，使用闭包入口。`log.log(level, message)` 适用于调用处动态选择级别。

## 自定义格式和调用位置

`formatting.scoop` 传入固定 Clock，让两条记录都显示 `1970-01-01T00:00:00Z`。生产使用时可省略 clock，使用默认 SystemClock。format 是普通函数，接收已求值的 LogRecord 和时间，返回正文；provider 再追加一个换行。

两条记录分别包含 `direct call` 和 `request=42`，`function=main`，以及各自在 main 中的调用位置。`logRequest` 用 `location: SourceLocation = currentSourceLocation()` 取得调用方位置，再显式传给 `log.info`；这样记录定位到业务调用处。单文件入口的逻辑文件标识为 `scoop:single-file:0.0.0/main.scoop`，行号仍对应示例原文。

Clock 和 format 可能被并发调用，示例使用无共享可变状态的实现。完整写入由 console 的共享流锁串行化。

## 业务异常与消息构造

`exceptions.scoop` 输出以下正文，前缀仍是默认时间、级别和分类：

```text
request failed | cause: connection refused
[log message failed] cannot build details | cause: connection refused
caught eager argument failure in application code
continued after logging failures
```

业务异常通过 `cause = Some(failure)` 附带。消息闭包再失败时，原 cause 保留，闭包异常存入 `record.messageFailure`，默认格式从替代 message 输出其摘要。急切实参的异常发生在进入 Logger 前，仍由应用自己的 catch 处理。

provider 写出失败与 stderr best-effort 行为见 facade 的 [fallback.scoop](../../scoop.log/examples/README.md#写出失败与-best-effort)。

## 配置替换与禁用

`configuration.scoop` 输出：

```text
primary: ready
temporary: using another provider
primary: original provider restored
logging disabled
```

`Log.setProvider` 返回旧 provider，可以在 finally 中恢复；`NoopLogProvider` 关闭日志并跳过消息闭包。这里的替换影响整个进程，不是线程局部设置。替换不自动 flush 或关闭旧 provider，示例在切换前显式 flush；资源型实现的关闭还需要调用方先等待使用者结束。

实现自定义 provider 可继续阅读 [facade 示例](../../scoop.log/examples/README.md)。公开合同见[日志库规范](../../../../docs/specs/SCOOP-LOG-SPEC.md)。
