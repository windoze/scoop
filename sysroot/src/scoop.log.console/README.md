# scoop.log.console

同步 stdout／stderr 日志 provider，默认输出到 stderr，最低级别 Info。应用显式配置，普通依赖库只使用 [scoop.log](../scoop.log/README.md)。

```toml
[dependencies]
"scoop:scoop.core" = "0.1.0"
"scoop:scoop.log" = "0.1.0"
"scoop:scoop.log.console" = "0.1.0"
```

```scoop
import scoop.log.*
import scoop.log.console.*

fun main() {
    Log.setProvider(ConsoleLogProvider())
    val log = Logger("app")
    log.info { "ready" }
    Log.flush()
}
```

可通过 stream、minLevel、clock 和 format 配置实例。format 接收 LogRecord 和 Instant，返回正文；provider 统一追加一个 LF，保留正文中的换行、Unicode 和 NUL。默认格式为 `UTC时间 [级别] target: message`，有业务异常时追加 cause 摘要。自定义格式使用普通函数与 f-string：

```scoop
ConsoleLogProvider(
    stream = ConsoleStream.Stdout,
    minLevel = LogLevel.Debug,
    format = { record, timestamp ->
        f"${timestamp} ${record.level} ${record.location.file}:${record.location.line} ${record.message}"
    }
)
```

取时和 format 在输出锁之外，可被多线程同时调用；自定义实现负责共享可变状态。多个 provider 写同一标准流时共用锁，覆盖完整记录及短写循环。独立 fallback、core stdio 和其他直接写入不在此同步范围内。

输出借用标准流，不关闭描述符、不增加缓冲。慢输出会阻塞调用线程。通过 facade 调用时，取时、格式或写入异常转为 stderr best-effort fallback；直接调用底层 provider 方法则由调用方处理异常。

完整合同见[日志库规范](../../../docs/specs/SCOOP-LOG-SPEC.md)。可运行示例见 [examples](examples/README.md)。开发验证使用 `scoop test sysroot/src/scoop.log.console --release`。
