# Scoop 终端库规范

版本：M59，分项实施中。范围与实施顺序见 [M59 设计](../milestone59/DESIGN.md)，实际完成项见[实施记录](../milestone59/PROGRESS.md)。本文随对应功能先于实现补齐公共合同；未完成项的范围仍以 M59 设计为准。

## 1. 库与平台边界

`scoop:scoop.terminal:0.1.0` 是普通 library Cone，显式依赖 core、std、platform，测试依赖 scoop.test，不默认导入。Scoop 实现命令、数据模型和协议，C11 shim 仅承担实际 OS／ABI 操作。生产代码不依赖 Rust crossterm，不新增编译器角色或 runtime ABI。

本期支持 `aarch64-apple-darwin`、`x86_64-unknown-linux-gnu`、`x86_64-unknown-linux-musl`。公共 API 不暴露 fd、termios 或 OS bitmask；平台实现按普通源码选择分开。Windows 留待工具链与 platform 实际支持后加入，本期没有 Windows 占位分支。

接口遵循[库 API 风格](SCOOP-API-STYLE.md)。兼容基线为 [crossterm 0.29.0](https://docs.rs/crossterm/0.29.0/crossterm/)，采用其终端序列和事件概念，按 Scoop 类型与资源规则表达。上游许可证随库保留。

## 2. 命令与输出

根 package `scoop.terminal`：

```scoop
public interface Command {
    public fun writeAnsi(writer: TextWriter)
}
public fun Command.toAnsiString(): String
public fun TextWriter.queue(vararg commands: Command)
public fun ByteSink.queue(vararg commands: Command)
public fun ByteSink.execute(vararg commands: Command)
public struct Print<T : ToString>(val content: T) : Command
```

`writeAnsi` 只编码，不隐式刷新或打开终端；自定义命令可实现同一接口。`toAnsiString` 返回独立字符串。`Print` 在实际编码时调用 content 的 `toString`，原样输出文本，不隐式换行、不解释或转义控制字符。

`queue` 按实参顺序编码，ByteSink 重载使用 UTF-8 适配，不调用 flush；底层可以立即写出或因缓冲区已满而写出。`execute` 队列写入成功后调用 sink.flush 恰好一次，包括空命令列表；编码／写入失败立即传播，后续命令与 flush 不执行。flush 失败照常传播。输出不是事务，失败前已写出的字节不回滚。一个批次的多线程互斥由调用者承担。

writer／sink 均为借用，本库不关闭调用者传入的端点。短写、无进展与错误处理复用[平台流合同](SCOOP-PLATFORM-SPEC.md)第 5 节。输出成功不证明终端接受了协议命令。

## 3. 光标命令

package `scoop.terminal.cursor`。命令均实现 Command。

| 类型 | 参数／值与 ANSI 序列 |
| --- | --- |
| `MoveTo(column: Long, row: Long)` | 零基坐标，`CSI row+1;column+1 H` |
| `MoveToColumn(column: Long)`、`MoveToRow(row: Long)` | 零基坐标，分别 `CSI column+1 G`、`CSI row+1 d` |
| `MoveUp(count: Long)`、`MoveDown(count: Long)` | `CSI count A/B` |
| `MoveRight(count: Long)`、`MoveLeft(count: Long)` | `CSI count C/D` |
| `MoveToNextLine(count: Long)`、`MoveToPreviousLine(count: Long)` | `CSI count E/F`，同时回到第一列 |
| `SavePosition`、`RestorePosition` | singleton 命令，`ESC 7/8`（DEC 保存／恢复） |
| `Hide`、`Show` | singleton 命令，`CSI ?25 l/h` |
| `EnableBlinking`、`DisableBlinking` | singleton 命令，`CSI ?12 h/l` |
| `SetCursorStyle` | enum 命令：DefaultUserShape、BlinkingBlock、SteadyBlock、BlinkingUnderScore、SteadyUnderScore、BlinkingBar、SteadyBar，对应 `CSI 0..6 SP q` |

坐标和计数接受 `0..65535` 的 Long；编码前检查，非法值抛 IllegalArgumentException。零基坐标用 Long 加一，最大坐标编码为 65536，不发生 u16 溢出。相对移动计数 0 按上游编码为 0，终端通常按 1 处理；不是库定义的空操作。参数检查在编码时执行且先于该命令任何输出，构造值本身不进行 I/O。

## 4. 屏幕命令

package `scoop.terminal.screen`。命令均实现 Command。

| 类型 | 参数／值与 ANSI 序列 |
| --- | --- |
| `Clear(type: ClearType)` | All → `CSI 2J`，Purge → `CSI 3J`，FromCursorDown → `CSI J`，FromCursorUp → `CSI 1J`，CurrentLine → `CSI 2K`，UntilNewLine → `CSI K` |
| `ScrollUp(count: Long)`、`ScrollDown(count: Long)` | `0..65535`；0 不输出，其余分别 `CSI count S/T` |
| `SetSize(columns: Long, rows: Long)` | `1..65535`，`CSI 8;rows;columns t`；仅发送调整窗口请求 |
| `SetTitle(title: String)` | `OSC 0;title BEL`；拒绝 C0、DEL 和 C1 控制字符，避免结束／破坏该协议字段 |
| `EnableLineWrap`、`DisableLineWrap` | singleton 命令，`CSI ?7 h/l` |
| `EnterAlternateScreen`、`LeaveAlternateScreen` | singleton 命令，`CSI ?1049 h/l` |
| `BeginSynchronizedUpdate`、`EndSynchronizedUpdate` | singleton 命令，`CSI ?2026 h/l` |

参数检查在编码时完成，失败在该命令输出前抛 IllegalArgumentException。没有通用终端能力探测保证这些命令生效，调用者按实际终端选择使用。

## 5. 颜色与样式

package `scoop.terminal.style`：

- `Color` 是值 enum：Reset、Black、DarkGrey、Red、DarkRed、Green、DarkGreen、Yellow、DarkYellow、Blue、DarkBlue、Magenta、DarkMagenta、Cyan、DarkCyan、White、Grey、`AnsiValue(value: Int)`、`Rgb(red: Int, green: Int, blue: Int)`。通道／索引范围为 `0..255`，编码前检查，非法值抛 IllegalArgumentException。
- `Color.parseOrNone(text)`／`parse(text)` 接受上述基础色的 ASCII 大小写无关名字，复合名字使用下划线；失败分别为 None／IllegalArgumentException，不回退成 White。`parseAnsiOrNone` 接受严格的 `5;n` 或 `2;r;g;b`，仅十进制数字；0～15 索引归一成对应基础色，其余为 AnsiValue。
- `Colored` 的 ForegroundColor／BackgroundColor／UnderlineColor 包含 Color。`toAnsiString` 返回 SGR 参数而非完整 CSI；`parseAnsiOrNone` 接受 `38/48/58;5;n`、`38/48/58;2;r;g;b` 和 `39/49/59`。该参数转换不受颜色输出策略影响。
- `SetForegroundColor`／`SetBackgroundColor`／`SetUnderlineColor`、`SetColors(Colors(foreground: Color? = None, background: Color? = None))` 均为 Command。基础色沿上游编码为 256 色槽，RGB 使用 24 位序列；SetColors 在同一 SGR 中按前景、背景顺序写出。None 表示不改变，Reset 表示恢复默认值。没有颜色时不输出。
- `ResetColor` 是 singleton Command，始终发送 `CSI 0m`，同时复位颜色和属性。

`forceColorOutput(enabled)` 用进程级原子覆盖控制颜色，`resetColorOutput()` 恢复环境策略，`isColorOutputEnabled()` 查询当前决定。无显式覆盖时，非空 NO_COLOR 禁用颜色；空值或未定义不禁用。环境策略在查询时读取，不使用隐含的一次初始化缓存。`availableColorCount()` 按 COLORTERM、再 TERM 推断：含 truecolor／24bit 返回 65535，含 256 返回 256，其余返回 8；只报告环境提示。

禁色时颜色命令仍验证参数，但完全不输出，不发送空 SGR 导致意外复位。显式属性命令与 ResetColor 不被 NO_COLOR 禁用。这里有意修正上游禁色时部分命令仍生成空 SGR 的行为。

`Attribute` 包含上游 0.29 的 28 项 SGR 属性：Reset、Bold、Dim、Italic、Underlined、DoubleUnderlined、Undercurled、Underdotted、Underdashed、SlowBlink、RapidBlink、Reverse、Hidden、CrossedOut、Fraktur、NoBold、NormalIntensity、NoItalic、NoUnderline、NoBlink、NoReverse、NoHidden、NotCrossedOut、Framed、Encircled、OverLined、NotFramedOrEncircled、NotOverLined。扩展下划线使用 `4:2..5`，不能与 Bold 等普通 SGR 混淆。

`Attributes(bits: UInt = 0u)` 是普通透明 struct，bits 为本库逻辑位集，按上面 28 项的声明顺序占据 bit 0～27，不是 OS bitmask。`with(attribute)`、`without(attribute)`、`toggled(attribute)` 返回新值，`has(attribute)` 和 `isEmpty` 查询；复制后修改派生值不影响原值。`SetAttribute(attribute)` 与 `SetAttributes(attributes)` 为 Command，集合输出按声明顺序，重复属性只输出一次；编码时遇到 bit 28～31 非零，在输出前抛 IllegalArgumentException。

`ContentStyle(foregroundColor: Color? = None, backgroundColor: Color? = None, underlineColor: Color? = None, attributes: Attributes = Attributes())` 是不可变值。withForegroundColor／withBackgroundColor／withUnderlineColor／withAttribute／withoutAttribute 返回派生值；`apply<T : ToString>(content)` 返回 `StyledContent<T>`。`T.styled(style = ContentStyle())` 提供同样的普通扩展。

`SetStyle(style)` 依次写出背景、前景、下划线颜色和属性；缺省字段不复位其他状态。StyledContent 保存 style 与 content，实现 Command 和 ToString；先取得文本，再写入样式、文本和结束复位。没有属性时分别复位所设置的背景／前景／下划线颜色（49／39／59）；有属性时使用 SGR 0 整体复位。禁色时不生成只为颜色配对的结束复位。复位到默认值，不承诺恢复此前任意终端样式；写入失败遵守普通部分输出规则。下划线颜色使用 59 复位，是对上游只复位前景色问题的有意修正。

## 6. 后续功能的既定合同

样式、事件、Unix 会话、PTY 交互、OSC 52 与异步适配均属于 M59 的交付范围，不因命令部分先完成而移出里程碑。后续功能的具体公共声明在各自实现前增加到本文；以下约束已经确定：

- 事件 parser 增量处理 UTF-8、CSI／SS3、鼠标、focus、paste、Kitty 与设备回复；分片本身不决定事件边界。
- Escape 判定默认等待 50 ms，可配置非负 Duration。零等待表示下一次无更多可用输入的推进即可判定 Escape。`poll` 使用独立的总截止时间，不能为了完成 Escape 判定越过调用时限。
- `read` 返回完整 Event；正常关闭前读到 EOF／断开抛 `TerminalDisconnectedException`，不返回伪造事件。`poll` 只为完整用户事件返回 true，终端断开沿同一异常合同处理。
- 查询默认等待 1 秒，超时抛 `TerminalQueryTimeoutException`；能力探测的正常“不支持”单独表达。查询回复与普通输入由同一读取者分流，不能吞掉用户事件。
- 显式终端会话实现 Closeable；raw mode 保存并恢复实际快照，同一会话上的嵌套作用域只撤销自身改变。正常返回、异常与取消使用现有 use／useAsync 清理合同。
- `SIGWINCH` 通知只在 native 层执行 async-signal-safe 操作，安装期间保留已有 handler 的行为并在关闭时恢复。FD、信号与关闭竞态属于平台实现和 PTY 验收。
- 同步和异步不能同时读取同一会话。异步通过单独 `scoop.terminal.asyncio` 交付，先补齐 asyncio 的实际 TTY 操作、取消与关闭，不使用阻塞 suspend 包装。
- PTY host 采用 portable-pty＋vt100，独立生成输入／查询回复，同时观察精确 cell 和 raw bytes；模式恢复检查实际 termios，结束时排空输出并检查 reader 结果。

终端恢复不依赖 GC release hook 或进程退出钩子。core.exit、SIGKILL 等无法展开作用域的退出不保证终端模式恢复。
