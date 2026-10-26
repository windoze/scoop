# Scoop 日志库设计草案

状态：设计稿，尚未实施。

日志库分为 `scoop.log` facade 与 `scoop.log.console` provider 两个普通 Cone。库代码通过 facade 记录日志，应用选择 provider；首版提供同步 stdout／stderr 文本输出。所有能力使用现有 Scoop 接口、默认参数、原子引用、平台流和锁实现。

## 1. 库边界

| Cone | 公开内容 | 生产依赖 |
| --- | --- | --- |
| `scoop:scoop.log:0.1.0` | `LogLevel`、`LogRecord`、`LogProvider`、`NoopLogProvider`、`Logger`、`Log` | core、platform |
| `scoop:scoop.log.console:0.1.0` | `ConsoleLogProvider`、`ConsoleStream`、默认格式函数 | core、std、platform、log |

包名分别为 `scoop.log` 和 `scoop.log.console`，源码分别放在 `sysroot/src/scoop.log/` 与 `sysroot/src/scoop.log.console/`。使用日志的普通库只依赖 facade；可执行应用按需要依赖 console provider，并在入口配置。

正常日志的取时、输出流和文本布局由 provider 决定。facade 统一处理消息求值失败和 provider 失败，后者使用固定格式直接写 stderr，见第 5 节。console provider 借用 `standardOutput`／`standardError`，使用 `SystemClock` 和 `Mutex`。

facade 的 platform 依赖仅用于独立的 stderr fallback，公开协议不包含平台类型。现有 core `writeError` 不报告 I/O 错误，平台 ByteSink 则已有写入错误与断管处理，可直接复用；不为 fallback 新增 native source、编译器特殊处理或 runtime ABI。

现有能力依据：

- [语言规范 8.5、11.12、11.14](../specs/SCOOP-SPEC.md)：默认参数、调用处位置和原子引用。
- [平台库规范 5、8、10](../specs/SCOOP-PLATFORM-SPEC.md)：UTF-8 输出、标准流、时钟和线程同步。
- [实现规范 2.12](../specs/SCOOP-IMPL-SPEC.md)：普通库的依赖与产物消费。
- [运行时规范 3.5、6](../specs/SCOOP-RUNTIME-SPEC.md)：阻塞 native 调用及 core 输出后备。
- [库 API 风格](../specs/SCOOP-API-STYLE.md)：命名、失败、资源与回调约定。

## 2. facade 协议

级别为 `Trace`、`Debug`、`Info`、`Warn`、`Error`，严重程度依次增加。`LogLevel` 实现 `ToString`，固定返回 `TRACE`、`DEBUG`、`INFO`、`WARN`、`ERROR`，供文本格式使用。关闭日志通过 `NoopLogProvider` 表达，不把 `Off` 当成可发送的事件级别。

事件和 provider 的接口如下：

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

`target` 是调用方明确给出的分类名，例如 `app.server` 或 `scoop.http.client`。它是普通字符串，不自动推导 package，也不在 facade 中建立名称树或 logger 注册表。

`LogRecord` 保存已求值的 String。`cause` 保留调用方原本附带的 Throwable；`messageFailure` 保存消息闭包抛出的 Throwable，正常记录为 None。两者可以同时存在，闭包失败不覆盖业务异常。消息求值失败时，message 保存含异常摘要的替代文本，具体规则见第 5 节。

异常字段保留原对象引用；记录的值字段不可变，不意味着异常对象的可变字段获得深快照。provider 可以保留记录；需要延迟读取异常的实现必须明确自己的快照与同步行为。

provider 方法均为普通同步调用，允许被多个线程同时调用。实现负责自己的共享状态；`isEnabled` 应是廉价查询，不输出日志、不调用消息构造器。facade 调用 `emit` 前检查一次，首版 console 的过滤配置不可变，不在 `emit` 内重复同一检查。

LogProvider 是底层实现协议，方法可以抛出 Throwable 向 facade 报告失败。应用通过 Logger 和 Log 调用；facade 捕获这些异常并执行第 5 节的恢复行为。直接调用底层 provider 不经过这一边界。

`flush` 提交该 provider 在调用前已经完成接收的事件；它不表示关闭或停止其他线程。未来有缓冲的实现应在返回前完成这部分提交，失败则抛给 facade，由 Log.flush 执行 stderr fallback。无缓冲实现可以没有额外工作。

## 3. Logger 和默认 provider

`Logger(target: String)` 创建一个只保存分类名的轻量对象。它提供以下入口：

| 入口 | 合同 |
| --- | --- |
| `isEnabled(level)` | 查询当前 provider；查询失败时 stderr fallback 并返回 false；后续 log 调用仍自行检查 |
| `log(level, message: String, cause: Throwable? = None, location: SourceLocation = currentSourceLocation())` | 普通急切字符串入口 |
| `log(level, cause: Throwable? = None, location: SourceLocation = currentSourceLocation(), message: () -> String)` | 通过过滤后才执行消息闭包 |
| `trace`／`debug`／`info`／`warn`／`error` | 与 log 对应的两种重载，固定各自的级别，其余参数合同相同 |

每个公开记录入口都在默认参数中取得 `SourceLocation`，转发时显式传递该值，保证记录业务调用点。用户自己包装日志函数时也使用同一方式转发位置。

一次惰性记录按以下顺序执行：

1. 读取并保留当前 provider 的强引用。
2. 调用该 provider 的 `isEnabled`；false 直接返回，查询抛异常则走 stderr fallback 并返回，不执行消息闭包。
3. 在调用线程执行消息闭包一次；成功取得消息，抛异常则构造保留该异常的失败记录。
4. 调用同一 provider 的 `emit` 一次；抛异常则走 stderr fallback，不再调用该 provider。

过滤正常返回 false 时，不执行消息闭包、不读取时钟、不格式化、不写输出。闭包对象及其捕获仍遵守普通语言求值与分配规则，不承诺禁用日志绝对零分配。字符串重载的实参会在调用前求值；其求值异常发生在进入日志函数之前，不能由日志函数捕获。需要日志库处理消息构造异常时，应使用闭包重载。

`Log` 是 facade 的普通 object：

| 入口 | 合同 |
| --- | --- |
| `setProvider(provider: LogProvider): LogProvider` | 原子替换默认 provider，返回旧 provider |
| `flush()` | 读取当前 provider 一次，调用其 flush；失败则 stderr fallback，随后正常返回 |

默认 provider 是公开的 `NoopLogProvider`：`isEnabled` 恒为 false，emit 丢弃记录，flush 无额外工作。未配置时保持静默，不自动发现实现，也不打印启动警告。普通库不调用 setProvider，由应用决定全局输出策略。

默认引用用 core 的 `AtomicRef<LogProvider>` 保存，以现有 SeqCst 操作完成发布和读取。提前创建的 Logger 不绑定旧 provider，所以顶层 logger 可在 main 安装 provider 后正常使用。单次记录始终使用步骤 1 取得的对象，不能在过滤和提交之间切换 provider。

setProvider 不取得资源关闭责任，也不自动 flush／close 旧对象。返回旧对象不表示其他线程已经停止使用它；关闭带资源的 provider 前，应用需要先停止并等待使用者。资源型 provider 可另外实现现有 `Closeable`，无需将关闭加入所有 provider 的接口。

首版不提供任务局部 provider 覆盖。现有 Context 可以继续供应用传递业务数据，但日志调用不要求安装 Context，也不增加可选 Context lookup 或传播规则。

## 4. console provider

公开构造参数：

| 参数 | 类型 | 默认值与含义 |
| --- | --- | --- |
| `stream` | `ConsoleStream` | `Stderr`；另有 `Stdout`，单个实例写入一个选定流 |
| `minLevel` | `LogLevel` | `Info`；接收严重程度不低于该级别的事件 |
| `clock` | `scoop.time.Clock` | `SystemClock` |
| `format` | `(LogRecord, Instant) -> String` | `::defaultConsoleLogFormat` |

配置在构造后保持不变。clock 和 format 可能由多个调用线程并发执行，自定义实现负责其共享可变状态。

默认文本形式：

```text
2026-10-26T10:20:30Z [INFO] app.server: listening on 8080
```

时间按现有 `Instant.toString()` 输出 UTC；小数位沿用该函数的规则。时间是在 provider 接收事件时取得的 wall-clock 时间，位于消息构造之后，不是日志函数刚进入的时间；系统时钟回拨以及线程调度都可能使时间戳与输出顺序不同。facade 不负责事件取时，后续异步 provider 可在同步 emit 接收时取时，再将结果放入自己的队列。

格式配置直接使用普通函数和 f-string，不增加 `{time}` 一类模板解析语法，也不另建 formatter 接口。format 返回正文，provider 统一追加一个 LF；正文中的换行和 U+0000 保留，不做 trim 或转义。因此正文自己以 LF 结束时会多出一个空行，格式函数应按需控制正文。

默认格式在有 cause 时追加 ` | cause: ...`，异常摘要规则见第 5 节。消息闭包异常的摘要已经包含在替代 message 中，不重复追加；messageFailure 仍保留原异常供 provider 或自定义格式访问。当前 Throwable 没有公共堆栈读取接口，首版不承诺动态异常类型名、堆栈或 cause 链，也不为日志扩展异常 ABI。

### 4.1 并发与输出

每次 emit 先在锁外读取时钟、执行格式函数并构造包含末尾 LF 的完整文本，再取得对应标准流的锁并完成整个写入。

console 库内部为 stdout 和 stderr 各保存一个共享 Mutex，同一流的不同 ConsoleLogProvider 实例也使用同一把锁。锁覆盖 `ByteSink.writeText` 的完整短写循环，而非仅覆盖一次系统调用。这样，通过 console 正常输出到同一标准流的成功记录不会彼此穿插；不同线程的先后顺序由实际取得锁的顺序决定。所有退出路径都释放已取得的锁，写入失败交回 facade 后再执行 fallback。

输出复用平台库的 `standardOutput`／`standardError` 与完整 UTF-8 写入。provider 不关闭标准流、不增加缓冲；flush 在同一流锁下调用 ByteSink.flush，当前标准流无用户态缓冲，因此没有额外刷新工作。库内部的共享锁随正常进程内使用存续，不对外提供并发销毁入口。

该串行化范围不包括第 5 节的独立 fallback、其他进程、core 的 println／eprintln 或直接 fd 写入，也不建立 stdout 与 stderr 之间的全序。与 core 的 libc stdio 输出混用且需要顺序时，调用方按已有合同显式 flushOutput；不能逐条日志隐式替所有输出刷新。

首版是同步 provider，慢管道会阻塞当前调用线程。沿用平台 NativeSafe I/O，阻塞期间其他线程仍可推进 GC。format 在持有输出锁之前完成；格式函数不得向同一日志链再次记录自身，否则普通递归会持续发生。

## 5. 日志调用的失败处理

Logger 的记录入口、isEnabled 和 Log.flush 捕获执行过程中可捕获的 Throwable，失败留在日志处理流程内，不向业务调用方传播。消息求值失败与 provider 失败使用独立的 catch 边界，不能把 provider 的错误误记成消息闭包错误。构造配置或调用前实参求值仍遵守普通语言规则；进程终止、runtime fatal 等不是可捕获的 Throwable。

异常摘要只读取既有 Exception.message：有消息时使用该消息，无消息时使用 `Exception (no message)`；其他 Throwable 使用 `Throwable`。摘要过程不调用用户定义的 toString、属性 getter、格式函数或日志入口。

### 5.1 消息闭包抛异常

消息闭包最多执行一次。抛异常后，facade 保留原来的 level、target、location 和 cause，构造以下替代记录：

- message 为 `[log message failed] ` 加该异常的摘要。
- messageFailure 为 Some，保存闭包抛出的原 Throwable 对象。
- cause 仍保存调用方原本附带的异常，两者不互相覆盖。

这条记录沿原级别提交给本次已选中的 provider，不再次过滤、不调用 Logger.error，也不重新执行消息闭包。它已通过本次 isEnabled 检查；保持原级别可保留调用意图与过滤结果。即使自定义文本格式只读取 message，也能取得失败标记和异常摘要。

失败记录提交成功后正常返回。如果 provider 处理这条记录时也抛异常，则进入 5.2：fallback 同时保留消息求值失败、原 cause 和 provider 失败的信息。失败记录构造本身抛出可捕获异常时，也进入保护后的 fallback，不让恢复路径的异常逸出。

### 5.2 provider 抛异常

写入异常、时钟或用户格式函数抛出的异常都从 emit 返回给 facade，统一执行 stderr fallback。fallback 直接使用平台 standardError 的完整 UTF-8 写入，采用库内固定文本，包含失败阶段、provider 异常摘要，以及已经取得的原记录级别、target、位置、message、cause 和 messageFailure 信息。

其他入口按已有数据处理：

| 失败阶段 | fallback 内容与后续行为 |
| --- | --- |
| 记录入口的 isEnabled | 输出查询异常与请求的 level、target、location；闭包消息标记为未求值，不执行闭包，随后返回 |
| 公开 Logger.isEnabled | 输出查询异常、level 和 target，返回 false |
| emit，包括取时、格式与实际写入 | 输出上述原记录与提交异常，随后返回 |
| Log.flush | 输出刷新异常；不重放历史记录，随后返回 |

fallback 不重新查询 provider、不读取时钟、不调用用户格式函数，也不通过任何日志入口输出。即使失败的 provider 原本就在写 stderr，仍按固定格式独立尝试一次。

I/O 失败可能已经输出部分或全部内容，无法回滚；emit 抛异常不等于原记录完全没有写出。facade 不重试原 provider，在 stderr 用明确的 `[log fallback]` 标记输出原记录及失败诊断，因此可能与原输出的部分内容重复。

### 5.3 stderr fallback 是 best effort

每次失败最多发起一次 fallback 写入请求。平台流内部的正常短写推进属于这次完整写入；一旦报告错误，立即放弃剩余内容，不重新提交整条 fallback。

fallback 的固定文本构造和 stderr 写入都在独立的保护边界内。再次抛出可捕获异常时直接丢弃，不向调用方传播、不再次 fallback、不切换到其他输出流。stderr 已关闭或管道断开时，沿用平台库的错误与 SIGPIPE 处理，日志调用仍可正常返回。

fallback 不使用 provider 的输出锁或共享缓冲，避免恢复路径重新进入失败的实现。并发 fallback 与正常 stderr 日志可能穿插；这里采用尽力输出且不递归重试的行为，不承诺完整交付或跨记录顺序。

## 6. 使用示例

以下为拟定 API 的使用示例，需在实现后纳入实际编译验证。

```scoop
import scoop.log.*
import scoop.log.console.*

private val log = Logger("app.server")

fun main() {
    Log.setProvider(ConsoleLogProvider(
        stream = ConsoleStream.Stderr,
        minLevel = LogLevel.Info
    ))

    val port = 8080
    log.info { f"listening on {port}" }
    log.debug { f"debug port: {port}" }

    try {
        throw Exception(Some("example failure"))
    } catch (error: Exception) {
        log.error(cause = Some(error)) { "request failed" }
    }

    Log.flush()
}
```

自定义格式也只在应用入口配置：

```scoop
Log.setProvider(ConsoleLogProvider(
    stream = ConsoleStream.Stdout,
    minLevel = LogLevel.Debug,
    format = { record, timestamp ->
        f"{timestamp} {record.level} {record.target} " +
        f"{record.location.file}:{record.location.line} {record.message}"
    }
))
```

## 7. 后续 provider 边界

文件、网络、JSON 文本或异步输出都通过同一 LogProvider 接收事件。其缓冲、重试、文件轮转、资源关闭和输出格式归具体实现；facade 承担调用入口、事件交付和同步调用的失败处理。扩展接口通过首版真实使用的三个方法提供，不增加插件发现、provider 工厂注册、动态装载或生命周期状态机。

同步调用期间的异常交由 facade 恢复。异步 provider 在 emit 返回后发生的后台写入失败，由该实现按相同的 stderr best-effort 原则处理，不能向无关业务调用异步抛出；这部分随具体 provider 一起实施。

首版还不增加结构化业务字段、MDC、span、分布式 tracing、按 target 配置树、配置文件或环境变量解析。后续如果需要结构化业务字段，应单独设计 LogRecord 的数据合同；现有接口不意味着这些未定义语义已经完成。

## 8. 实施与验证

实施前新增 `docs/specs/SCOOP-LOG-SPEC.md`，将本稿确定的公开 API、求值、并发、失败和资源责任写入规范，并从语言规范的普通库说明增加引用。现有语言语义、编译 pipeline 与 runtime ABI 均无需变化。

随后按以下顺序落地：

1. 实现 facade、NoopLogProvider、普通记录用测试 provider，以及 Logger 的字符串／惰性入口、消息失败记录和受保护的 stderr fallback。
2. 实现 console provider，复用现有时钟、日期输出、ByteSink 和 Mutex，补齐文档与可编译示例。
3. 接入现有 sysroot 准备、预编译库交付和 API 文档流程，按现有 Darwin／GNU／musl 目标验证。

两库普通行为测试放在各自 Cone 的 tests 中，使用 scoop.test。覆盖五种级别、禁用时不执行闭包、启用时只执行一次、急切参数求值、默认 provider、配置后已有 Logger 生效、过滤和 emit 使用同一 provider，以及包装函数显式转发源码位置。更换全局 provider 的测试在 finally 恢复原值，不并行运行这些全局配置测试。

消息失败测试验证失败标记、异常摘要、原 level／target／location、messageFailure 对象身份，以及原 cause 同时保留；不重新执行闭包或过滤，调用后仍继续执行。单独覆盖普通 Throwable、无消息的 Exception、禁用的会抛异常的闭包，以及在业务 catch／finally 内记录失败后原控制流保持。

console 测试覆盖级别阈值、默认与自定义格式、固定 Clock、异常消息、Unicode、空正文、嵌入 NUL 和换行。时钟和格式异常交由 facade fallback，不产生正常格式的输出。源码位置跨 Cone 的普通 API 验证使用测试 Cone；不因它消费预编译库而另建 fixture。

只为需要外部进程驱动的 stdout／stderr 分流、fallback 输出和多线程整条记录验收保留少量 fixture。失败组合覆盖 isEnabled／emit／flush、消息闭包与 provider 连续失败、formatter 抛异常、原输出部分成功后失败，以及 stderr 关闭／断管后日志调用仍然返回；验证 fallback 使用固定格式且最多一次，不重入 provider、消息闭包或自定义格式函数。并发正常输出用例包含多个 provider 实例写同一流。

测试复用预编译系统库和共享缓存，普通模式使用 release，不重复平台库底层短写、信号和 GC 合同的完整验证。
