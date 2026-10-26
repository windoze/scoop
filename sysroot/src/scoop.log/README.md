# scoop.log

日志 facade。普通库依赖它并创建分类 Logger，由应用通过 Log.setProvider 选择实现。默认 provider 静默丢弃日志；官方同步输出实现见 [scoop.log.console](../scoop.log.console/README.md)。

```toml
[dependencies]
"scoop:scoop.core" = "0.1.0"
"scoop:scoop.log" = "0.1.0"
```

```scoop
import scoop.log.*

private val log = Logger("app.worker")

fun process(id: Long) {
    log.debug { f"processing ${id}" }
    log.info("ready")
}
```

级别为 Trace、Debug、Info、Warn、Error。闭包只在过滤成功后执行一次；字符串实参仍在调用前求值。每个入口自动保存业务调用处的 SourceLocation；自行包装日志函数时，用默认参数取得位置并显式向下转发。

消息闭包抛异常时，记录保留原级别、分类、位置和业务 cause，messageFailure 保存原异常，message 包含失败摘要。provider 抛异常时直接向 stderr 尝试固定格式的 fallback；fallback 再失败就放弃，不抛回业务、不重试或递归记录。需要保护消息构造异常时使用闭包入口。

自定义实现只需实现 LogProvider.isEnabled、emit 和 flush。方法可被并发调用，实现负责自身共享状态。emit 接收已通过过滤、已求值的 LogRecord。资源型实现可另外实现 Closeable；setProvider 返回旧对象但不自动刷新、关闭或等待其在途调用结束。

facade 为 fallback 依赖平台库，公开协议不包含平台类型。完整合同见[日志库规范](../../../docs/specs/SCOOP-LOG-SPEC.md)。

开发时先准备 sysroot，再运行 `scoop test sysroot/src/scoop.log --release`。普通行为使用 scoop.test，标准流与失败输出由少量外部进程 fixture 验证。
