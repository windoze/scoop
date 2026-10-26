# Scoop 日志库规范

日志库由 `scoop.log` facade 和 `scoop.log.console` 同步 provider 组成。设计依据和实施范围见[日志库设计](../log/DESIGN.md)。语言、异常、源码位置和并发规则沿用[语言规范](SCOOP-SPEC.md)；输出、锁和时钟沿用[平台库规范](SCOOP-PLATFORM-SPEC.md)。

## 1. 库与依赖

`scoop:scoop.log:0.1.0` 是普通 library Cone，生产依赖 core、platform。公开类型位于 scoop.log；platform 用于独立的 stderr fallback，公开协议不包含平台类型。`scoop:scoop.log.console:0.1.0` 依赖 core、std、platform、log，公开类型位于 scoop.log.console。普通库只依赖 facade，应用选择 provider。两库使用现有原子引用、普通接口和平台 I/O，不增加 native source、语言特例或 runtime ABI。

## 2. 事件和 provider

LogLevel 的五个 variant 为 Trace、Debug、Info、Warn、Error，严重程度依次递增。它实现 ToString，分别返回 TRACE、DEBUG、INFO、WARN、ERROR。关闭日志使用 NoopLogProvider，不存在可发送的 Off 事件。

```scoop
public struct LogRecord(
    val level: LogLevel,
    val target: String,
    val message: String,
    val location: SourceLocation,
    val cause: Throwable? = None,
    val messageFailure: Throwable? = None
)

public interface LogProvider {
    public fun isEnabled(level: LogLevel, target: String): Boolean
    public fun emit(record: LogRecord)
    public fun flush()
}
```

target 是调用者提供的普通分类字符串，不自动推导包名或建立名称树。message 是已求值的 String。cause 保存调用者附带的业务异常，messageFailure 保存消息闭包抛出的异常，两者均保留原对象身份，不复制异常对象或覆盖彼此。正常消息的 messageFailure 为 None。

provider 方法是可能被多线程并发调用的普通同步方法。实现负责共享状态；isEnabled 是廉价过滤查询，不输出、不求值消息。facade 检查成功后对同一 provider 调用 emit，不重复相同过滤。直接调用底层 provider 方法不经过 facade 的恢复边界。

provider 可抛 Throwable 向 facade 报告失败。flush 提交调用前已完成接收的记录，不表示关闭或停止并发调用；有缓冲实现返回前完成这部分提交，失败交给 facade。NoopLogProvider 的 isEnabled 恒为 false，emit 丢弃事件，flush 无额外工作。

## 3. Logger 和全局配置

Logger(target: String) 保存只读 target，提供以下方法：

| 方法 | 行为 |
| --- | --- |
| isEnabled(level: LogLevel): Boolean | 查询当前 provider；失败执行 fallback 并返回 false |
| log(level, message: String, cause: Throwable? = None, location: SourceLocation = currentSourceLocation()) | 接收已求值的字符串 |
| log(level, cause: Throwable? = None, location: SourceLocation = currentSourceLocation(), message: () -> String) | 过滤成功后同步执行闭包一次 |
| trace／debug／info／warn／error | 与 log 对应的两种重载，省去 level 参数并固定为对应级别 |

每个记录入口都通过默认参数取得调用处 SourceLocation，内部转发显式保留该值。位置遵守语言规范 11.12，不依赖调试信息或内联决策。

Log 是普通 object：setProvider(provider: LogProvider): LogProvider 原子替换默认 provider 并返回旧对象；flush(): Unit 读取当前 provider 一次并刷新。初始为 NoopLogProvider；未配置时静默，不发现插件或打印警告。应用负责配置，普通库仅记录日志。

默认引用用 AtomicRef<LogProvider> 的 SeqCst 操作发布和读取。Logger 不在构造时绑定 provider：安装前创建的 Logger 后续使用当前配置。一次记录只取得一个 provider；过滤、消息求值和提交期间即使发生配置替换，该次调用仍使用原对象。

setProvider 不自动刷新或关闭旧对象，返回旧对象不表示旧调用已经结束。应用关闭资源型 provider 前需要停止并等待使用者；资源型 provider 可以另外实现 Closeable。首版不增加任务局部覆盖或 provider 关闭协议。

过滤正常返回 false 时不执行消息闭包、取时、格式化或 I/O。闭包构造及捕获遵守普通求值规则，不承诺零分配。字符串重载的实参在调用前求值，日志函数无法捕获尚未进入函数时发生的异常；需要保护消息构造时使用闭包入口。

## 4. 失败处理

Logger 记录入口、isEnabled 和 Log.flush 捕获其消息求值与 provider 调用中的 Throwable，不向业务传播。消息与 provider 使用独立 catch 边界。构造配置、调用前实参求值以及 runtime fatal／进程退出不属于该恢复边界。

异常摘要只读取 Exception.message：Some 使用消息，None 使用 `Exception (no message)`；其他 Throwable 使用 `Throwable`。不调用用户 ToString、虚拟属性或格式函数。

### 4.1 消息闭包失败

闭包最多执行一次。失败后创建替代 LogRecord，保留原 level、target、location、cause，message 为 `[log message failed] ` 加异常摘要，messageFailure 保存 Some(原异常)。按已通过的过滤结果交给本次 provider，不重新过滤或调用另一个日志入口。成功提交后正常返回。

替代记录提交失败时，fallback 同时包含闭包异常、原 cause 和 provider 异常。替代记录构造失败也进入受保护的 fallback，不让恢复路径的 Throwable 逸出。

### 4.2 provider 失败

isEnabled 失败时输出查询错误与已有的级别、target、位置，不执行消息闭包；公开 isEnabled 返回 false。emit 失败，包括时钟、format、锁或写入失败，fallback 输出原事件及提交异常。Log.flush 失败只输出刷新错误，不重放历史记录。

fallback 直接使用 standardError.writeText 和库内固定格式，以 `[log fallback]` 开头，包含阶段、错误摘要与已有事件信息。没有事件的位置不伪造消息。它不调用 provider、用户 format、消息闭包或系统时钟。原 provider 已经写 stderr 时，仍独立尝试 fallback。

emit 抛异常可能发生在部分或全部字节写出后。原输出不回滚、不重试；fallback 可能重复原输出的一部分，固定前缀标识这是失败诊断。

### 4.3 fallback 的 best effort 边界

每次失败至多发起一次完整 fallback 写入。平台流的正常短写推进属于本次写入；报告错误后立即放弃剩余内容，不重新提交。

fallback 的文本构造和写入都捕获 Throwable；再次失败直接丢弃，正常返回，不再 fallback、不换其他流、不抛给业务。使用平台已有的错误与 SIGPIPE 处理，因此 stderr 关闭或断管可作为普通写入失败结束。

fallback 不使用 provider 的锁或缓冲。它与并发 fallback 或其他 stderr 输出可能穿插，不承诺完整交付、跨记录顺序或非阻塞。

## 5. console provider

ConsoleStream 为 Stdout、Stderr。ConsoleLogProvider 的构造参数如下：

| 参数 | 类型 | 默认值 |
| --- | --- | --- |
| stream | ConsoleStream | ConsoleStream.Stderr |
| minLevel | LogLevel | LogLevel.Info |
| clock | scoop.time.Clock | SystemClock |
| format | (LogRecord, Instant) -> String | ::defaultConsoleLogFormat |

构造后配置不可变。isEnabled 接收严重程度不低于 minLevel 的事件，不按 target 过滤。直接 emit 消费已经通过过滤的事件。clock 和 format 可能被并发调用，自定义实现负责自己的共享可变状态。

emit 在调用线程先读取 clock.now，再执行 format，最后追加一个 LF 并完整写入选定标准流。取时位于消息求值之后，表示 provider 接收时间；wall-clock 回拨及线程调度可使时间与输出次序不同。正文中的换行、Unicode 和 NUL 保留，末尾已有 LF 时仍追加一个 LF。

默认函数 defaultConsoleLogFormat(record: LogRecord, timestamp: Instant): String 返回 `时间 [级别] target: message`；时间沿用 Instant.toString 的 UTC 表示。若 cause 存在，追加 ` | cause: ` 和第 4 节的异常摘要。messageFailure 的摘要已经在替代 message 中，不重复追加。自定义 format 是普通函数，不引入模板语言。当前 Throwable 没有公共堆栈读取接口，不生成虚构的类型名、堆栈或 cause 链。

stdout 和 stderr 各有一个 console 库共享 Mutex，同一流的不同 provider 实例共用对应锁。锁覆盖包含换行的完整写入，包括平台内部短写循环；所有退出路径释放已取得的锁。取时和用户 format 在锁外执行，失败交回 facade 后才 fallback。format 不应向同一日志链递归记录自身。

成功的 console 正常记录不互相穿插。该保证不覆盖独立 fallback、其他进程、core stdio 或直接 fd 写入，不建立 stdout 与 stderr 全序。与 core stdio 混用且需要顺序时由应用显式 flushOutput。

provider 借用 standardOutput／standardError，不关闭标准描述符、不增加缓冲。flush 在对应锁下调用 ByteSink.flush；现有标准流无用户态缓冲，因此没有额外刷新工作。同步 I/O 可以阻塞，沿用平台 NativeSafe，不阻止其他线程推进 GC。

## 6. 扩展与验收

其他 provider 实现同一协议，其输出格式、缓冲和资源责任随实现定义。异步 provider 在 emit 返回后发生的后台失败由该实现按相同的 stderr best-effort 原则处理，不向无关业务调用异步抛出。首版交付 facade 和 console，不增加文件轮转、网络、结构化业务字段、MDC、tracing 或插件注册。

普通行为和组合测试使用各库的 scoop.test；只有必须外部控制标准流、断管和进程输出的测试使用 fixture。普通模式为 release，复用预编译系统库。测试须覆盖消息与 provider 连续失败、原异常身份和位置、只执行一次、配置替换、不同实例的并发输出，以及 stderr fallback 失败后业务继续执行。
