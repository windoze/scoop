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

## 6. 输入事件与增量解析

package `scoop.terminal.event` 使用普通值类型：

```scoop
public struct KeyEvent(val code: KeyCode, val modifiers: KeyModifiers = KeyModifiers(),
    val kind: KeyEventKind = KeyEventKind.Press, val state: KeyEventState = KeyEventState())
public enum KeyEventKind { Press, Repeat, Release }
public struct MouseEvent(val kind: MouseEventKind, val column: Long, val row: Long,
    val modifiers: KeyModifiers = KeyModifiers())
public enum Event {
    Key(val event: KeyEvent), Mouse(val event: MouseEvent),
    FocusGained, FocusLost, Paste(val text: String), Resize(val columns: Long, val rows: Long)
}
```

`KeyCode` 包含 Backspace、Enter、Left／Right／Up／Down、Home／End、PageUp／PageDown、Tab／BackTab、Delete／Insert、`F(number: Int)`、`Char(character: Char)`、Null、Esc、CapsLock／ScrollLock／NumLock、PrintScreen、Pause、Menu、KeypadBegin、`Media(code: MediaKeyCode)`、`Modifier(code: ModifierKeyCode)`。MediaKeyCode 与 ModifierKeyCode 的具名值覆盖 crossterm 0.29 对应类型，保留左右修饰键区别。

`KeyModifier` 为 Shift、Alt、Control、Super、Hyper、Meta；`KeyState` 为 Keypad、CapsLock、NumLock。对应 `KeyModifiers(bits: UInt = 0u)`／`KeyEventState(bits: UInt = 0u)` 为透明逻辑位集，按各自声明顺序占位，提供 has／with／without／isEmpty。`MouseButton` 为 Left、Right、Middle；`MouseEventKind` 为 Down／Up／Drag（携带按钮）、Moved、ScrollUp／ScrollDown／ScrollLeft／ScrollRight。传统鼠标协议无法指出释放按钮时，沿上游约定产生 Up(Left)；SGR 小写 m 保留确切按钮。

`KeyboardEnhancement` 为 DisambiguateEscapeCodes、ReportEventTypes、ReportAlternateKeys、ReportAllKeysAsEscapeCodes；`KeyboardEnhancementFlags(bits: UInt = 0u)` 同样提供值集合操作，占 bit 0～3。查询回复保留原始未知位，但 push 命令只接受本期四个已定义位。查询返回 0 表示支持协议且当前没有开启增强，不能当作不支持。

输入模式命令均实现 Command：Enable／DisableMouseCapture 按上游顺序开启／逆序关闭 1000、1002、1003、1015、1006；Enable／DisableFocusChange 使用 1004；Enable／DisableBracketedPaste 使用 2004。PushKeyboardEnhancementFlags(flags) 使用 `CSI >bits u`，PopKeyboardEnhancementFlags 使用 `CSI <1u`。无效 push 位集在输出前抛 IllegalArgumentException。

### 6.1 字节、时间与恢复

Parser 只接收字节、单调时间读数和当前 raw 状态，不读取 fd 或墙上时钟。一次 feed 可产生多项结果，按输入顺序排队；结果区分用户 Event、设备回复与非法输入。普通 UTF-8 严格验证，粘贴载荷才使用有损解码。普通大写字符按 Unicode 17.0 的 Uppercase 属性附加 Shift，与上游 Unicode 判断方式一致；不把该局部判断扩展成新语言能力。

- 完整的 UTF-8 标量立即产生一个字符事件；非法首字节消费该字节，非法续字节先结束坏前缀，再从该续字节重新解析，后续合法字符不丢失。完整但非法的标量消费其编码。未完成的普通 UTF-8 等待后续字节或 EOF。
- CR 为 Enter；LF 在非 raw 时为 Enter，在 raw 时为 Ctrl+J；Tab、DEL 分别为 Tab、Backspace；NUL 为 Ctrl+空格。其余 C0 按上游 Ctrl 字符映射，Escape 单独处理。
- Escape 判定默认 50 ms，配置必须非负；非零等待从前缀的首个 Escape 起算，时限到达先处理过期前缀，再接受新字节。等待为零时，同一次或连续有数据的 feed 仍可组成序列，显式无输入推进才结束歧义。时间不得倒退。
- 单独 Escape 到期产生 Esc；仅 `ESC [`／`ESC O` 到期分别为 Alt+[／Alt+O；更长的不完整转义序列到期产生一次非法输入。两个连续 Escape 保留为两个输入，不丢掉第二个。Alt+UTF-8 使用同一 Escape 截止时间；bracketed paste 开始后不再使用该超时。
- CSI 按终结字节分帧，支持 SS3 和 Linux `ESC [[A..E` 功能键。完整未知或非法序列整体消费，产生一次非法输入。CSI、SS3 和 Linux 功能键前缀内的非法控制／非 ASCII 字节结束已有坏前缀，再从该字节重同步；新的 Escape 开始新的前缀。
- 旧式 `25/26/28/29/31/32/33/34 ~` 分别为 F13～F20，依据 [xterm 功能键表](https://invisible-island.net/xterm/xterm-function-keys.html)，修正上游 0.29 对 F15～F20 的编号偏差。
- 传统鼠标固定收取三个原始字节；rxvt 与 SGR 按数字字段解析，允许上游接受的末尾可选分号。协议坐标必须为正数，转换为零基 Long；不允许下溢。普通十进制字段先受检累积，再按具体协议范围检查，不接受符号、空的必填数字、额外字段或溢出。
- `CSI 200~` 开始粘贴，至第一个完整 `CSI 201~` 结束。期间包括 Escape、换行和近似结束标记在内的载荷均按原样收集；结束标记不进入 Paste，后续字节继续解析。
- EOF 保留已完成结果，单独 Escape 转为 Esc，其余未完成前缀／粘贴产生一次非法输入，然后结束。非法输入由会话转为 TerminalInputException，消费后可继续读取；EOF／断开的异常合同见 7.1。

### 6.2 键盘增强与设备回复

CSI-u 支持 Unicode 标量、修饰键、Press／Repeat／Release、锁定状态、F13～F35、keypad、媒体和左右修饰键。修饰字段允许 1～256，以支持所有八个位；省略／空的可选字段使用协议默认值。显式非法事件类型或 Unicode 标量属于非法输入。带 Shift 的 alternate shifted codepoint 替换字符并移除 Shift，沿上游事件表示；base-layout 字段验证后忽略。本期不请求 associated-text；收到第三字段时验证其标量再忽略，与上游未暴露该字段的边界一致。修饰键释放保持报文中的实际修饰位，不强行补上被释放的键。

光标位置、主／次设备属性、设备状态和 Kitty 标志回复只产生内部回复，不成为用户 Event。CPR 的行列允许 1～65536。`CSI row;column R` 与旧式带修饰的 F3 有固有歧义，两个普通数字字段优先解释为 CPR；F3 使用 SS3、`13~` 或无参数 `CSI R`。Kitty 标志按完整十进制数字解析，设备属性保留实际参数。回复与用户事件的队列顺序不会因为 feed 分片变化。

### 6.3 同步与异步共用的协议接口

package `scoop.terminal.protocol` 提供无 I/O 的公开协议接口，供独立的
terminal.asyncio Cone 复用，不复制 parser 或通过可见性例外访问内部声明：

```scoop
public enum InputItem {
    User(val event: Event), CursorPosition(val column: Long, val row: Long),
    KeyboardFlags(val flags: KeyboardEnhancementFlags),
    DeviceAttributes(val secondary: Boolean, val parameters: Array<Long>),
    DeviceStatus(val status: Long), Invalid(val detail: String)
}
public class InputParser(escapeDelay: Duration = Duration.milliseconds(50L)) {
    public val hasPending: Boolean
    public val escapeRemaining: Duration?
    public fun feed(bytes: List<Byte>, now: Duration, rawMode: Boolean = true)
    public fun advance(now: Duration)
    public fun feedResize(columns: Long, rows: Long)
    public fun next(): InputItem?
    public fun finish()
}
public enum QueryKind { Cursor, Primary, Secondary, Keyboard }
public class QueryReply(kind: QueryKind) {
    public val command: String
    public fun accept(item: InputItem): InputItem?
}
```

InputParser 由单一读取者串行使用。next 消费一项已完成结果，None 不表示 EOF；
hasPending 只报告已完成队列。feedResize 接受非负行列，将 Resize 追加到同一
队列，不结束字节前缀。finish 幂等并保留队列，之后 feed／advance／feedResize
报状态错误。escapeRemaining 相对于最后一次 feed／advance 的显式时间计算。

QueryReply 为一次查询保存必要状态；command 是第 9 节规定的请求序列，
accept 忽略不匹配项，匹配时返回结果；Kitty flags 必须等主设备属性结束回复。
调用方负责用户事件排队、非法项、截止时间和 I/O，不把 QueryReply 当作通用事件
消费器。完成后不复用同一个 QueryReply 发起新查询。

## 7. 终端会话、尺寸与 raw mode

package `scoop.terminal`：

```scoop
public enum StandardStream { Input, Output, Error }
public fun isTerminal(stream: StandardStream): Boolean
public struct TerminalSize(val columns: Long, val rows: Long,
    val widthPixels: Long, val heightPixels: Long)
public class TerminalSession : ByteSink, Closeable {
    public companion object {
        public fun open(escapeDelay: Duration = Duration.milliseconds(50L)): TerminalSession
    }
    public val isClosed: Boolean
    public val isRawMode: Boolean
    public fun size(): TerminalSize
    public fun enableRawMode()
    public fun disableRawMode()
    public fun read(): Event
    public fun poll(timeout: Duration): Boolean
}
public fun <R> TerminalSession.withRawMode(block: () -> R): R
```

`isTerminal` 只检测选定的标准端点，返回 Boolean，不打开会话。会话先使用 stdin
指向的 TTY；stdin 不是 TTY 时打开当前进程的控制终端。Unix 通过终端路径重新
打开自有读写句柄，不用 dup 后改变共享 file status flags。打开成功不会修改原
stdin／stdout 的 O_NONBLOCK；stdout 重定向时会话仍写入自己的 TTY。没有可用
TTY 或原生调用失败抛带 PosixErrno 的 PlatformException，不回退为普通管道输入。

进程内同时至多存在一个打开的输入会话，重复打开以原生 EBUSY 报错。会话操作
由调用方串行使用；与正在执行的操作并发或重入会立即抛 IllegalStateException，
不会关闭正在使用的句柄。公共 API 不暴露 fd、termios 或平台标志。

`size` 返回 OS 当前报告的列、行和像素尺寸；0 表示 OS 未提供该字段，不能猜成
80×24。输出仍使用普通 ByteSink：writeFrom 遵守 buffer 借用、短写与读位置推进
规则，直接写入 TTY；flush 检查会话打开，因为此层没有用户态缓冲，不等待终端
显示完毕。调用者可继续组合 BufferedSink／execute。

首次 enableRawMode 保存调用时实际 termios，使用平台 raw 转换后设置；重复启用
不覆盖快照。disableRawMode 恢复保存的快照，重复禁用为空操作。isRawMode 表示
本会话是否拥有一次尚未恢复的 raw 变更，不声称能够观察外部程序修改。
进入前已经 raw 的终端也保存并恢复原状态。

withRawMode 使用现有 use 清理规则，只有本次启用才在离开时禁用；同一会话嵌套
调用不会提前恢复外层状态。正文与恢复同时失败合并为 ResourceUseException。
调用方不应在该作用域内手工改变 raw 所有权；提前关闭整个会话则由 close 恢复，
作用域退出不重复操作已关闭句柄。

close 在恢复失败后仍尝试关闭其他自有资源；消费句柄后会话保持关闭，即使关闭
报告错误。重复 close 幂等，其他已关闭会话操作抛 IllegalStateException。
显式 close／use 是终端恢复和资源释放的必要入口；本会话不注册 GC／atexit
恢复钩子，不让 GC 线程执行 tcsetattr 或阻塞等待。

### 7.1 同步输入与 resize

TerminalSession 的 `read(): Event` 等待下一项完整用户事件；
`poll(timeout: Duration): Boolean` 使用非负总等待时间，0 不阻塞。
poll 为 true 时保留该事件，随后同一串行读取者的 read 直接消费；重复 poll 不
移除事件。非法输入抛 TerminalInputException 并消费该错误项，后续事件仍可读取。
内部设备回复不使 poll 返回 true，也不作为用户输入交付。

read／poll 不隐式启用 raw；立即逐键输入需要调用方使用 raw 作用域。传给 parser
的 raw 标志跟随会话的 raw 所有权。所有输入、解析状态及用户事件归同一会话，
并发 read／poll／close 触发第 7 节的操作冲突错误。

等待使用单调时钟的总截止时间，并取剩余总时间与 Escape 剩余时间的较小值。
非阻塞读取和 signal 中断不会重新开始计时。零等待可以处理当前可读数据，但
不承诺在一次调用中排空任意长的持续输入；分片未完成时返回 false 并保留前缀。
Escape 延迟为 0 时，只有确认暂时无更多可读数据才结束歧义。
OS poll 的毫秒精度与线程调度影响实际返回时刻，不代表允许重新延长等待窗口。

EOF／断开在排空已经完成的用户事件后抛 TerminalDisconnectedException。
EOF 时未完成前缀沿 6.1 先产生一项解析错误；随后读取报告断开，不忙循环。
调用方仍须 close／use；若断开的设备无法恢复 termios，关闭继续释放资源并报告
实际原生错误。

Linux 同步会话安装 SIGWINCH 的非阻塞 self-pipe 通知，close 恢复原 disposition。
原有普通 handler／SA_SIGINFO handler 继续调用，原 signal mask、SA_ONSTACK、
SA_NODEFER、SA_RESTART 行为保留；SA_RESETHAND 的原 handler 只调用一次，之后
恢复为默认 disposition。SIG_DFL／SIG_IGN 不调用函数指针。signal handler 保存
errno，不分配、不调用 Scoop，pipe 满时合并通知。

安装与拆除期间使用 native 原子引用和在途 handler 计数保护自有 fd／存储。
Linux 同一进程的应用不得在会话存续期间并发重新注册 SIGWINCH；如果它已经被其他
代码替换，close 不覆盖新的 disposition。

Darwin 使用 kqueue 的 EVFILT_SIGNAL 观察进程 SIGWINCH，不替换 disposition，
因此原 handler 的一次性、mask 与恢复行为由内核保留。Darwin sigaction 查询不
返回 SA_RESETHAND，不能通过重新安装查询所得的 action 声称完整恢复；依据为
[XNU sigaction 实现](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_sig.c)
及 [kqueue 合同](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/man/man2/kqueue.2)。

标准信号及切换边界允许合并，不承诺
每一次 resize 都交付。会话保存最近观察到的行列，通知后重查；只有行列改变
才排入 Resize。尺寸为最后一次实际观察值，输入字节与 signal 没有全局先后保证。

## 8. OSC 52 复制

package `scoop.terminal.clipboard`：

```scoop
public enum ClipboardType { Clipboard, Primary, Other(val selector: Char) }
public struct ClipboardSelection(val destinations: Array<ClipboardType>) : ToString
public struct CopyToClipboard(val content: Array<Byte>,
    val destination: ClipboardSelection = ClipboardSelection.clipboard) : Command
```

ClipboardSelection 的 clipboard／primary 为常用值；parse／parseOrNone 从字符串
构造有序目标序列，toString 按原顺序转换。c／p 对应具名枚举，其他字符使用 Other。
空序列表示终端默认目标，重复目标及顺序保留；终端可能只采用首个目标。

目标字符接受 ASCII 可打印字符但排除分号；拒绝控制字符、DEL、非 ASCII 和字段
分隔符，在编码前抛 IllegalArgumentException。直接构造 Other 仍在 toString／
命令编码时检查。这是对上游任意字符选择器的有意收窄，保证完整的 OSC 字段。
parseOrNone 对这些非法字符返回 None。

CopyToClipboard.fromText(text, destination = ClipboardSelection.clipboard) 使用
UTF-8 内容；直接构造接受任意不可变字节数组。编码复用 std Base64.Default，
输出 `ESC ] 52 ; targets ; padded-base64 ESC \`。目标与 Base64 均在写出前计算；
空内容有效，不变成读取剪贴板的 `?` 请求。NO_COLOR 不影响复制命令。

构造命令不进行 I/O，只有显式编码／queue／execute 才写出。发送成功不保证终端
或 multiplexer 执行；库不调用桌面剪贴板服务，不增加 OSC 52 读取能力。
自动测试仅观察 PTY 协议。

## 9. 会话查询

package `scoop.terminal`：

```scoop
public struct CursorPosition(val column: Long, val row: Long)
public enum DeviceAttributeKind { Primary, Secondary }
public fun TerminalSession.cursorPosition(
    timeout: Duration = Duration.seconds(1L)): CursorPosition
public fun TerminalSession.deviceAttributes(kind: DeviceAttributeKind = DeviceAttributeKind.Primary,
    timeout: Duration = Duration.seconds(1L)): Array<Long>
public fun TerminalSession.keyboardEnhancementFlags(
    timeout: Duration = Duration.seconds(1L)): KeyboardEnhancementFlags?
public fun TerminalSession.supportsKeyboardEnhancement(
    timeout: Duration = Duration.seconds(1L)): Boolean
public class TerminalQueryTimeoutException : Exception
```

CPR 返回零基列／行；设备属性返回收到的有序数字，不猜测终端品牌。
负 timeout 非法；默认 1 秒，零只检查当时已有的完整回复。等待使用一个单调
总截止时间，收到用户事件、无关回复、EINTR 或分片均不重新开始计时。
该时限约束回复等待；查询字节的写出及 termios 恢复仍服从同步 OS 操作合同。

每次查询持有同一会话的操作权，经该会话实际终端写出并从同一 parser 收取；
stdout 重定向不改变查询端点。需要时临时启用 raw，成功、超时或异常均恢复
进入前模式；原已启用 raw 的会话保持 raw。正文与恢复同时失败沿 ResourceUseException。

发送前清理已完成的内部回复，并非阻塞读取一次当前可读数据做同样分流；
用户事件保留原顺序，部分前缀保留。等待期间用户事件进入会话队列，随后
read／poll 仍可交付，不能被查询吞掉。解析错误消费后抛 TerminalInputException；
EOF 沿 7.1 报告断开。无关／迟到回复在无匹配查询时丢弃，不成为用户事件。
终端协议没有请求 ID；旧回复在新请求发出后才到达，或仍在尚未读到的 OS
缓冲区时，可能与同种新回复无法区分。本库不保证协议无法提供的关联。

键盘增强探测按 [Kitty 探测协议](https://sw.kovidgoyal.net/kitty/keyboard-protocol/#detection-of-support-for-this-protocol)
依次发送 `CSI ? u` 和主设备属性 `CSI c`，以主设备
属性回复结束这次探测。此前收到的 Kitty flags 返回 Some（包括 0 和未知位）；
只收到主设备属性返回 None，表示不支持。两种回复都缺失或缺少结束回复
则超时，不能把超时当作不支持。supportsKeyboardEnhancement 仅检查是否 Some。

## 10. 异步终端适配

普通 Cone `scoop:scoop.terminal.asyncio:0.1.0` 显式依赖 core、std、platform、
terminal、asyncio，package `scoop.terminal.asyncio`：

```scoop
public class AsyncTerminalSession : AsyncByteSink, AsyncCloseable {
    public companion object {
        public suspend fun open(loop: EventLoop,
            escapeDelay: Duration = Duration.milliseconds(50L)): AsyncTerminalSession
    }
    public val isClosed: Boolean
    public suspend fun size(): TerminalSize
    public suspend fun isRawMode(): Boolean
    public suspend fun enableRawMode()
    public suspend fun disableRawMode()
    public suspend fun read(): Event
    public suspend fun poll(timeout: Duration): Boolean
}
public suspend fun <R> AsyncTerminalSession.withRawMode(block: suspend () -> R): R
public suspend fun AsyncTerminalSession.cursorPosition(
    timeout: Duration = Duration.seconds(1L)): CursorPosition
public suspend fun AsyncTerminalSession.deviceAttributes(kind: DeviceAttributeKind = DeviceAttributeKind.Primary,
    timeout: Duration = Duration.seconds(1L)): Array<Long>
public suspend fun AsyncTerminalSession.keyboardEnhancementFlags(
    timeout: Duration = Duration.seconds(1L)): KeyboardEnhancementFlags?
public suspend fun AsyncTerminalSession.supportsKeyboardEnhancement(
    timeout: Duration = Duration.seconds(1L)): Boolean
public suspend fun AsyncByteSink.queue(vararg commands: Command)
public suspend fun AsyncByteSink.execute(vararg commands: Command)
```

会话拥有 EventLoop.openTty 的资源，沿 asyncio 第 9 节的 loop／Job、取消、
借用与关闭合同；同步与异步共用进程独占，不接管外部 fd。所有 suspend 操作
要求所属 loop 和活动 Job；disableRawMode、isRawMode、close 允许取消后的清理。
完全关闭后重复 close 为空操作。

同一会话可各有一个输入和输出操作在途；read／poll 共用输入方向，查询及
尺寸／模式控制同时占用两个方向，冲突报 IllegalStateException。普通 read／poll
不隐式启用 raw。完成事件、Escape 延迟、超时、非法输入和 EOF 沿第 7.1 节；
协议解析和查询匹配复用第 6.3 节。已排队事件可直接返回，但仍检查当前 Job。

取消选定后，已经读入的字节仍按实际进度进入 parser；已完成的用户事件及部分
前缀保留，下一次健康读取继续消费。取消不是关闭。close 先拒绝新操作、关闭
TTY 并唤醒在途等待，再等待这些会话操作释放自身状态；返回后没有 native 借用
或仍占用输入／输出的操作。loop 收尾由 AsyncTty 恢复与关闭底层终端。

查询语义沿第 9 节，实际写入和等待均 suspend；回复总 timeout 从请求写完开始，
不约束写出及模式恢复。关闭／取消也恢复临时 raw，正文与清理双重失败沿
ResourceUseException。withRawMode 只恢复本次引入的改变，支持嵌套；正文期间
不允许手工改变 raw 所有权。正在关闭的会话由 close 承担恢复。

异步 queue 逐条先编码到 UTF-8 字节，再用 AsyncByteSink.writeFully 完成写出；
execute 成功后恰好 flush 一次。编码失败不写出当前命令，之前命令已输出的
字节不回滚；取消与部分写入沿 asyncio 合同。适配器不关闭调用者的 sink。

终端恢复不依赖 GC release hook 或进程退出钩子。core.exit、SIGKILL 等无法展开作用域的退出不保证终端模式恢复。
