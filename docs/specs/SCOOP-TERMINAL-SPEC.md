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

内部 parser 只接收字节、单调时间读数和当前 raw 状态，不读取 fd 或墙上时钟。一次 feed 可产生多项结果，按输入顺序排队；结果区分用户 Event、设备回复与非法输入。普通 UTF-8 严格验证，粘贴载荷才使用有损解码。普通大写字符按 Unicode 17.0 的 Uppercase 属性附加 Shift，与上游 Unicode 判断方式一致；不把该局部判断扩展成新语言能力。

- 完整的 UTF-8 标量立即产生一个字符事件；非法首字节消费该字节，非法续字节先结束坏前缀，再从该续字节重新解析，后续合法字符不丢失。完整但非法的标量消费其编码。未完成的普通 UTF-8 等待后续字节或 EOF。
- CR 为 Enter；LF 在非 raw 时为 Enter，在 raw 时为 Ctrl+J；Tab、DEL 分别为 Tab、Backspace；NUL 为 Ctrl+空格。其余 C0 按上游 Ctrl 字符映射，Escape 单独处理。
- Escape 判定默认 50 ms，配置必须非负；非零等待从前缀的首个 Escape 起算，时限到达先处理过期前缀，再接受新字节。等待为零时，同一次或连续有数据的 feed 仍可组成序列，显式无输入推进才结束歧义。时间不得倒退。
- 单独 Escape 到期产生 Esc；仅 `ESC [`／`ESC O` 到期分别为 Alt+[／Alt+O；更长的不完整转义序列到期产生一次非法输入。两个连续 Escape 保留为两个输入，不丢掉第二个。Alt+UTF-8 使用同一 Escape 截止时间；bracketed paste 开始后不再使用该超时。
- CSI 按终结字节分帧，支持 SS3 和 Linux `ESC [[A..E` 功能键。完整未知或非法序列整体消费，产生一次非法输入。CSI、SS3 和 Linux 功能键前缀内的非法控制／非 ASCII 字节结束已有坏前缀，再从该字节重同步；新的 Escape 开始新的前缀。
- 旧式 `25/26/28/29/31/32/33/34 ~` 分别为 F13～F20，依据 [xterm 功能键表](https://invisible-island.net/xterm/xterm-function-keys.html)，修正上游 0.29 对 F15～F20 的编号偏差。
- 传统鼠标固定收取三个原始字节；rxvt 与 SGR 按数字字段解析，允许上游接受的末尾可选分号。协议坐标必须为正数，转换为零基 Long；不允许下溢。普通十进制字段先受检累积，再按具体协议范围检查，不接受符号、空的必填数字、额外字段或溢出。
- `CSI 200~` 开始粘贴，至第一个完整 `CSI 201~` 结束。期间包括 Escape、换行和近似结束标记在内的载荷均按原样收集；结束标记不进入 Paste，后续字节继续解析。
- EOF 保留已完成结果，单独 Escape 转为 Esc，其余未完成前缀／粘贴产生一次非法输入，然后结束。非法输入由会话转为 TerminalInputException，消费后可继续读取；EOF／断开的异常合同见下一节。

### 6.2 键盘增强与设备回复

CSI-u 支持 Unicode 标量、修饰键、Press／Repeat／Release、锁定状态、F13～F35、keypad、媒体和左右修饰键。修饰字段允许 1～256，以支持所有八个位；省略／空的可选字段使用协议默认值。显式非法事件类型或 Unicode 标量属于非法输入。带 Shift 的 alternate shifted codepoint 替换字符并移除 Shift，沿上游事件表示；base-layout 字段验证后忽略。本期不请求 associated-text；收到第三字段时验证其标量再忽略，与上游未暴露该字段的边界一致。修饰键释放保持报文中的实际修饰位，不强行补上被释放的键。

光标位置、主／次设备属性、设备状态和 Kitty 标志回复只产生内部回复，不成为用户 Event。CPR 的行列允许 1～65536。`CSI row;column R` 与旧式带修饰的 F3 有固有歧义，两个普通数字字段优先解释为 CPR；F3 使用 SS3、`13~` 或无参数 `CSI R`。Kitty 标志按完整十进制数字解析，设备属性保留实际参数。回复与用户事件的队列顺序不会因为 feed 分片变化。

## 7. 后续功能的既定合同

Unix 会话、PTY 交互、OSC 52 与异步适配均属于 M59 的交付范围，不因前面的功能先完成而移出里程碑。后续功能的具体公共声明在各自实现前增加到本文；以下约束已经确定：

- 事件 parser 增量处理 UTF-8、CSI／SS3、鼠标、focus、paste、Kitty 与设备回复；分片本身不决定事件边界。
- Escape 判定默认等待 50 ms，可配置非负 Duration。零等待表示下一次无更多可用输入的推进即可判定 Escape。`poll` 使用独立的总截止时间，不能为了完成 Escape 判定越过调用时限。
- `read` 返回完整 Event；正常关闭前读到 EOF／断开抛 `TerminalDisconnectedException`，不返回伪造事件。`poll` 只为完整用户事件返回 true，终端断开沿同一异常合同处理。
- 查询默认等待 1 秒，超时抛 `TerminalQueryTimeoutException`；能力探测的正常“不支持”单独表达。查询回复与普通输入由同一读取者分流，不能吞掉用户事件。
- 显式终端会话实现 Closeable；raw mode 保存并恢复实际快照，同一会话上的嵌套作用域只撤销自身改变。正常返回、异常与取消使用现有 use／useAsync 清理合同。
- `SIGWINCH` 通知只在 native 层执行 async-signal-safe 操作，安装期间保留已有 handler 的行为并在关闭时恢复。FD、信号与关闭竞态属于平台实现和 PTY 验收。
- 同步和异步不能同时读取同一会话。异步通过单独 `scoop.terminal.asyncio` 交付，先补齐 asyncio 的实际 TTY 操作、取消与关闭，不使用阻塞 suspend 包装。
- PTY host 采用 portable-pty＋vt100，独立生成输入／查询回复，同时观察精确 cell 和 raw bytes；模式恢复检查实际 termios，结束时排空输出并检查 reader 结果。

终端恢复不依赖 GC release hook 或进程退出钩子。core.exit、SIGKILL 等无法展开作用域的退出不保证终端模式恢复。
