# M59 设计：crossterm 的 Scoop 移植与 PTY 交互验收

状态：实施中。日期：2026-10-20。本文记录移植范围、库与平台边界、实施顺序及测试方案；公共合同见[终端库规范](../specs/SCOOP-TERMINAL-SPEC.md)，实际实现与验证见[实施记录](PROGRESS.md)，参考项目的测试结果不算作 Scoop 的完成证据。

依据：[语言规范](../specs/SCOOP-SPEC.md)第 11～14、16 章、[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)、[实现规范](../specs/SCOOP-IMPL-SPEC.md)、[库 API 风格](../specs/SCOOP-API-STYLE.md)、[平台库规范](../specs/SCOOP-PLATFORM-SPEC.md)、[异步 I/O 规范](../specs/SCOOP-ASYNCIO-SPEC.md)及 [M52 工具链与 sysroot 分发](../milestone52/DESIGN.md)。

## 1. 目标与范围

将 [crossterm 0.29.0](https://docs.rs/crate/crossterm/0.29.0) 的终端操作与输入事件能力移植为普通 Scoop 库。生产逻辑使用 Scoop，少量 C11 shim 处理 OS 接口与 native 存储；应用不链接 Rust crossterm。首版支持现有三个目标：

- `aarch64-apple-darwin`；
- `x86_64-unknown-linux-gnu`；
- `x86_64-unknown-linux-musl`。

本期不实现 Windows，但公共类型、输入事件与内部平台调用边界必须允许后续增加 Windows 实现。其余 Unix 目标不因使用 POSIX 接口而自动成为受支持目标。

移植保持可辨认的 crossterm 功能与行为，API 遵守 Scoop 的命名、值类型、异常和资源规则，不追求 Rust 源码兼容。`execute!`／`queue!` 用普通方法、扩展函数与必要的 `vararg` 表达，不为移植增加语言宏、Rust trait 模拟层或编译器特殊入口。

| 范围 | 本期安排 |
| --- | --- |
| 命令、样式、光标、屏幕操作 | 基础交付，包含 ANSI 编码、排队／刷新和内存 writer 使用 |
| Unix 终端控制 | 基础交付，包含 TTY、raw mode、尺寸、resize 通知及确定性恢复 |
| 同步事件 | 基础交付，包含 `read`／`poll`、键盘、鼠标、焦点、粘贴及查询回复分流 |
| Kitty 键盘增强 | 纳入输入协议与能力探测，保留事件种类、修饰键和增强标志 |
| OSC 52 复制 | 单独功能项，显式调用才发送；复用 std 的 Base64，不扩展为桌面剪贴板服务 |
| 异步事件 | 单独适配 Cone 与实施阶段；复用同一解析器，补齐 asyncio 的真实 TTY 支持后交付 |
| Windows | 只落实边界，本期无 Windows 代码、空实现或验收承诺 |

终端库不承担 TUI 控件、布局、diff renderer 或生产用 virtual screen buffer。虚拟屏幕是本期测试 host 的观察工具。

## 2. 调研基线与现有能力

上游基线固定为 crossterm 0.29.0，包括公开 API、Unix 输入解析、TTY 操作及测试向量。实现时逐项记录兼容行为与有意差异；直接移植的代码和测试保留上游 MIT 许可证与署名。Rust feature 不机械映射成 Scoop 的编译开关，按实际依赖拆分普通 Cone。

| 已有能力 | 本期用法与缺口 |
| --- | --- |
| core 的 `TextWriter` | 仅提供 `write(String)`，适合命令编码；没有 `flush`，不能单独承担 `execute` 的刷新合同 |
| platform 的 `ByteSource`／`ByteSink`、`Utf8Writer`、`BufferedSink` | 复用字节传输、UTF-8 输出和刷新；任意 writer 不等于可查询的终端 |
| `Closeable`／`use`、`ResourceUseException` | 管理会话、raw mode 恢复和正文／清理双重失败 |
| `NativeByteBuffer`、普通 C FFI、`captureErrno` | 承担 native buffer 与同步调用，不新增 runtime ABI |
| 单调时钟、Mutex、线程 | 支持同步等待与必要状态保护；不自行建立事件运行时 |
| Cone 平台源码选择及 native sources | 选择 Darwin／Linux 实现与 C shim；当前谓词不接受 `windows` |
| M49 asyncio、libuv 1.53.0 | 已有取消、借用与关闭合同；公开 API 尚不支持 TTY、signal 或任意 fd 接管 |
| `scoop.test` 与 fixture schema 1 | 纯协议测试、真实应用编译和外部 PTY host 的普通进程步骤 |

具体复用点见 [流接口](../../sysroot/src/scoop.platform/src/io/streams.scoop)、[文本适配](../../sysroot/src/scoop.platform/src/io/text.scoop)、[资源作用域](../../sysroot/src/scoop.core/src/resources.scoop)与 [EventLoop](../../sysroot/src/scoop.asyncio/src/event_loop.scoop)。不得读取 platform 对象的私有 fd 或 asyncio 的私有 loop 指针。

有一项需要单独闭合的兼容差异：上游 Unix parser 会用 Unicode `is_uppercase` 推导字符键的 Shift，而 Scoop 当前大小写转换只覆盖 ASCII。若保持该行为，应提供所需的 Unicode 大写属性数据与版本说明，并以非 ASCII 字符验证；不能用 ASCII 判断冒充完全兼容，也不因此把本期扩展成完整 Unicode 文本库。

## 3. 普通库分层

| 组成 | 职责 | 依赖 |
| --- | --- | --- |
| `scoop:scoop.terminal:0.1.0` | 命令、样式、事件值、增量 parser、同步终端会话与 Unix shim | core、std、platform；测试依赖 scoop.test |
| `scoop:scoop.terminal.asyncio:0.1.0` | 异步读取、等待、取消与关闭，使用 terminal 的协议逻辑 | terminal、asyncio 及直接使用的基础库 |
| Rust PTY test host | 启动 Scoop 可执行文件、模拟终端、发送输入、断言屏幕和协议 | 仅测试使用；portable-pty、vt100 等 |

建议源码布局如下；目录与类型名在公共规范落地时统一，不代表这些文件已经存在：

```text
sysroot/src/scoop.terminal/
  Cone.toml
  src/command/
  src/style/
  src/cursor/
  src/event/
  src/terminal/
  src/internal/ansi/
  src/internal/input/
  src/internal/platform/unix/
  native/
  tests/
sysroot/src/scoop.terminal.asyncio/
tests/terminal_host/
tests/fixtures/m59-terminal-*/
```

ANSI 编码与输入解析只处理数据；终端会话负责实际 I/O、等待、请求关联和恢复；C shim 只承担 `termios`、`ioctl`、fd、poll 与 signal 的 ABI 操作。不要把输入 parser、事件对象或 ANSI 命令格式移入 C。

平台差异通过普通私有函数及 Cone 选择的源码实现表达。仅为目前的 Unix 实现和明确的 Windows 入口保留边界，不建立 backend 注册表、插件工厂或运行时平台选择框架。

## 4. 输出与公共数据合同

### 4.1 命令和 writer

保留 `Command`、颜色、属性、`ContentStyle`／`StyledContent` 等可组合概念；命令值表达移动、清屏、着色等操作，ANSI encoder 负责协议字节。命令编码可以写入内存 `TextWriter`，实际输出适配持有能够刷新的 `ByteSink`。

- `queue` 按顺序把命令写入目标 writer，不主动调用 `flush`。这不保证没有系统调用：底层 writer 可以直接写出或因缓冲区已满而写出。
- `execute` 写入本次命令后刷新目标 sink。一次调用不自动成为多线程原子输出；共享输出端由调用者按批次同步。
- 部分写入继续完成剩余字节；无进展与真实 I/O 失败沿用平台流合同。失败前已经输出的字节不可回滚。
- 写入／flush 成功只表示本地输出成功，不表示终端支持或接受命令。终端设置尺寸、同步更新、光标形状等请求尤其不能据此宣称生效。
- 调用者提供的 writer／sink 为借用，适配器不擅自关闭它；会话自行打开的终端句柄由会话负责关闭。

上述排队语义以 [crossterm Command API](https://docs.rs/crossterm/0.29.0/crossterm/trait.Command.html)为兼容基线，刷新边界按现有 Scoop IO 接口落实。

### 4.2 功能与表示

| 类别 | 需要移植的功能 |
| --- | --- |
| 颜色与样式 | 基础色、256 色、RGB；前景／背景／下划线颜色；属性设置与清除、reset、样式组合 |
| 光标 | 绝对／相对移动、行列移动、保存／恢复、显示／隐藏、闪烁与形状、位置查询 |
| 屏幕 | 清除各区域、滚动、换行模式、备用屏幕、标题、尺寸查询／设置请求、同步更新 |
| 输入模式命令 | 鼠标报告、焦点报告、bracketed paste、键盘增强 push／pop |
| 颜色策略 | 明确 `NO_COLOR` 与显式覆盖的优先级；环境推断的色数只是提示，不是设备探测结果 |
| 剪贴板 | OSC 52 复制的 selection 与 Base64 payload；由终端及中间复用器决定是否执行 |

公开坐标使用零基的列／行，转换到 CSI 参数时处理一基差异。尺寸、坐标及计数遵守现有库的 `Long` 约定，在操作边界检查非负、零尺寸规则和实际协议／OS 范围；不把 Unix `winsize` 或 Rust `u16` 布局作为公共 ABI。

`KeyEvent`、`MouseEvent`、`Event` 等用普通 struct／enum 表达。修饰键与属性集合使用类型化值和必要的集合操作，不公开 OS bitmask；键盘事件保留 Press／Repeat／Release、状态及增强标志。只有实际收到协议支持时才能提供释放、重复、锁定键状态等信息，普通终端输入不能可靠推导所有物理按键动作。

## 5. 输入解析、等待与查询

### 5.1 增量 parser

Parser 保存未完成的协议前缀，以显式输入字节和时间推进状态。输入覆盖普通 UTF-8、控制字符、Alt 前缀、CSI／SS3、功能键与修饰键、X10／传统／rxvt／SGR 鼠标、焦点、bracketed paste、Kitty CSI-u 与设备回复。

一次 feed 可以产出零个或多个事件。UTF-8、CSI、粘贴终止标记可以跨任意字节边界；既不能把一次 `read` 当作一个事件，也不能把暂时读不到下一段当成完整序列。粘贴载荷按上游约定有损解码 UTF-8，普通键输入的非法编码／非法序列处理则需单独规定，不能全局套用有损解码。

对未知、非法和未完成序列分别规定消费范围、错误与恢复位置；任何错误路径都必须前进或等待新的输入／超时，不能原地重试。协议数字的溢出、鼠标坐标转换和 Unicode 标量合法性在解析边界检查。

单独 Escape、Alt 组合和转义序列前缀存在固有歧义。本期采用明确、可配置的 Escape 判定等待时间，公共规范在实现 parser 前固定默认值和到期规则；parser 测试使用显式时钟推进，不依赖机器速度。该时间影响不完整 Escape 前缀的解释，不取代 `poll` 调用自己的截止时间。原始输入的分块方式不应改变尚未到期时的解析结果。

上游解析对照见 [Unix parse.rs](https://docs.rs/crate/crossterm/0.29.0/source/src/event/sys/unix/parse.rs)；键盘增强以 [Kitty keyboard protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/)为依据。需要为协议差异保留明确测试，不能为了通过测试直接调用被测 parser 生成期望事件。

### 5.2 会话和单一读取者

交互输入使用显式拥有资源的终端会话，提供 `read`、`poll(Duration)`、尺寸、raw mode 以及需要回复的查询。首版每个进程只允许一个活动的交互输入会话；输出到独立 writer 不受此限制。同一会话的读取和查询由一个消费者串行驱动，不允许同步／异步读取混用或多个线程抢读。

`poll` 的含义是“已经有完整的用户事件可读”，不是“fd 上存在字节”。返回 true 后，在没有其他消费者取走事件的前提下，下一次 `read` 不阻塞。查询回复不能让用户事件 `poll` 错误返回 true；未完成的 UTF-8／CSI 同样不能。

等待用单调时钟计算截止时间。零 timeout 不等待；EINTR、内部通知与部分输入不重置总时限。若调用截止时间先于 Escape 判定时间，保留前缀并返回 false。EOF／终端断开明确结束读取或报对应错误，不能忙循环，也不能把无事件的结束伪装成成功的 poll。

具体异常和 EOF 返回形状在公共规范中统一；参数／状态错误沿用 core 异常，OS 失败保留 `PlatformException` 和实际 native error，不用字符串消息承担错误分类。[上游 read／poll 使用限制](https://docs.rs/crossterm/0.29.0/crossterm/event/index.html)作为迁移说明的一部分。

### 5.3 查询回复与普通事件共用通道

光标位置、设备属性和键盘增强探测经同一个输入 reader／parser 收取。内部解析结果区分用户 `Event` 与查询回复；查询只能消费自己等待的回复，交错到达的键盘、鼠标和粘贴保留原顺序，随后仍由 `read` 交付。

查询串行发起并设置有界等待。成功、超时、不支持、非法回复、终端关闭分别有明确出口；超时后到达的协议回复仍作为内部回复处理，不当作用户输入，也不能无条件用于完成另一项查询。没有请求 ID 的终端协议不能完全区分旧回复与同种新查询，规范须说明这个限制，不能宣称实现了无法由协议保证的关联。

查询输出必须到达对应输入会话的实际终端。普通 ANSI 输出允许重定向到文件；交互输入可按上游方式优先使用 TTY stdin，否则打开 `/dev/tty`。不能在 stdout 已重定向时，把设备查询写到 stdout 后等待另一个终端回答。无可用终端时，纯编码／输出仍可用，交互入口报告实际失败。

查询若需要临时关闭规范输入或回显，复用第 6.3 节的模式作用域；回复成功、超时和解析失败均恢复进入前状态，不能让一次位置查询永久改变终端模式。

## 6. Unix native 层与资源生命周期

### 6.1 最小 OS 桥接

C shim 持有 opaque native 状态，不把 `termios`、fd 数字或平台常量暴露给公共 API。所需操作包括：

- 确认／打开终端，读取输入与写出会话查询；
- `tcgetattr`／`tcsetattr` 与 raw mode 转换；
- `ioctl(TIOCGWINSZ)` 等尺寸操作；
- 等待终端输入及 resize 唤醒；
- 关闭句柄与释放 shim 自有存储。

终端句柄的取得方式需要保持所有权清楚。`dup` 不隔离 file status flags，不能为了异步或非阻塞读取随意改变调用者 stdin 的 `O_NONBLOCK`；需要设置这些标志时使用合适的自有句柄，并验证原端点状态保持。不能从 platform 的 `File` 私有布局取 fd。

阻塞 read、poll 和可能等待的终端调用使用默认 NativeSafe。只有满足短小、无阻塞等完整合同的函数才能标 `@GCLeaf`。错误在 native 返回边界保存；C 不持有未登记的 managed 引用，signal handler 不进入 Scoop 或 GC。

### 6.2 Resize 与 signal

同步 Unix 路径使用 `SIGWINCH` 唤醒等待，并重新查询实际尺寸；handler 只执行 async-signal-safe 的通知操作，例如向非阻塞 self-pipe 写入。pipe 已满时可合并通知，因为后续读取最终尺寸，而不是要求每个信号对应一项历史尺寸事件。

signal 安装与输入会话共享同一生命周期，保存并正确恢复原 handler，处理安装／关闭竞态以及自有通知 fd 的关闭顺序。对已有 handler 的共存策略必须在实现前写入 terminal 规范并测试，不能静默覆盖后不恢复。这里仅实现终端 resize 所需的协调，不顺带发布通用 signal 框架。

Resize 事件承诺可观察到尺寸变化后的实际状态；快速连续变化可以合并，不保证每个中间尺寸都交付。通知处理、窗口尺寸查询和普通输入仍由同一读取者协调。

### 6.3 Raw mode 与关闭

首次启用 raw mode 保存实际 `termios` 快照，关闭／禁用时恢复该快照，不能重建一个猜测的 cooked mode。会话上重复启用为幂等操作；作用域 helper 记住本次是否改变状态，同一会话上的嵌套作用域只撤销各自引入的改变。进入前已经是 raw 的终端也应恢复到其原状态。

会话及作用域资源遵循 `Closeable`／`use`：

1. 部分初始化失败时，按已成功取得的资源和改变的状态回退。
2. 正常返回与异常展开都进行确定性清理；清理某一步失败，仍尝试其余必要恢复与关闭。
3. 正文和清理同时失败按现有 `ResourceUseException` 合并；重复关闭不重复消费 fd。
4. 备用屏幕、鼠标、焦点、粘贴等模式由显式 scope helper 管理时，只撤销该作用域启用的部分。无法查询的 ANSI 前态不能承诺精确还原；低层直接发送的命令仍由调用者配对。
5. GC release hook 只执行允许的 native 资源释放，不承担刷屏、等待、termios 恢复或 managed 回调。终端恢复依赖显式 close／use。

core 的 `exit` 走 `_exit`，不运行 finally、release 或 atexit；SIGKILL 等也不能被恢复逻辑接管。异常展开恢复与强制退出是不同合同，不为终端库增加 runtime 退出钩子。

## 7. Windows 扩展边界

预留的是现有功能的实现位置和数据边界：

| 边界 | Unix 首版 | 后续 Windows 接入 |
| --- | --- | --- |
| 终端 native 状态 | fd、termios、自有通知资源 | console handle 与 console mode，保持 opaque |
| 输出 | 共享 ANSI encoder 与可刷新的 sink | 优先启用 VT processing 后复用 encoder |
| 输入 | 字节输入加共享 parser | VT 输入或原生 console records 转成同一 `Event` |
| 尺寸与变化 | ioctl、SIGWINCH | console 查询及窗口变化事件 |
| 资源与错误 | 原状态恢复、PosixErrno | console mode 恢复及对应 native error domain |
| 构建选择 | Darwin／Linux 源码与 C shim | 工具链支持 Windows 后再加入对应源码选择 |

不强制所有平台都把原生输入事件编码回 Unix 字节再解析。共享的是语义事件、命令与生命周期；平台内部可以采用不同输入机制。输出设计以现代 [Windows VT 支持](https://learn.microsoft.com/en-us/windows/console/console-virtual-terminal-sequences)为后续方向，旧式 WinAPI 绘制兼容是否需要另行决定。

当前语言规范 12.2.2 仅允许 `os = darwin/linux`，manifest 会检查非法谓词。因此本期不添加 `when = { os = "windows" }`、空 Windows 文件或抛“尚未实现”的分支。未来增加 Windows 首先需要工具链、runtime、platform 与错误域的实际目标支持；本设计不把这些前置工作算作已经完成。

## 8. 异步事件适配

crossterm 的 EventStream 是可选能力。Scoop 同步终端库保持独立，异步使用单独的 `scoop.terminal.asyncio`；两者复用输入 parser、事件值与查询状态，不能为 EventStream 复制一套协议实现。

现有 asyncio 的公开范围明确排除 TTY。实现异步事件前，需要先在 asyncio 规范中增加实际需要的 TTY 操作与 native 生命周期，使用现有 libuv 的 [TTY](https://docs.libuv.org/en/v1.x/tty.html)／[signal](https://docs.libuv.org/en/v1.x/signal.html)能力。只有有具体需要的操作才成为公开能力，不为 terminal 开放整个私有 loop 或泛化任意 fd 接管。

异步合同至少包括：唯一读取者、取消与完成竞争的一次结果选择、真实 native 停止后解除 buffer 借用、读取取消不丢失已排队事件、关闭排空 native callback、恢复终端状态，以及 loop 退出后的资源失效。同步 signal 机制与 libuv signal 不能同时争用同一会话。

不能把阻塞 `read` 包在 `suspend` 函数中作为异步交付，也不直接照搬 Rust 的线程＋futures 实现。若这一阶段尚未完成，进度应明确标为“同步终端已交付，异步适配待完成”；不能把 async 占位入口算作支持。

## 9. PTY host 与 virtual screen buffer 测试

### 9.1 参考 atto-ui 的现有实现

用户提供的兄弟仓库 `../atto-ui` 已有完整的交互测试方式。本次查看的提交为 `613b29ff3c8513cd6e5b693c306cb39e276be4d8`，以下是源码依据：

| 参考位置 | 已有机制 | 本期采用方式 |
| --- | --- | --- |
| [atto-ui-test-host](https://github.com/windoze/atto-ui/blob/613b29ff3c8513cd6e5b693c306cb39e276be4d8/crates/atto-ui-test-host/src/lib.rs) | portable-pty 启动任意程序；后台读取 raw output，持续送入 vt100 parser；提供发送输入、resize、cell 与等待接口 | Rust host 直接启动 Scoop build 产物，无需把测试程序改成 Rust |
| [host 依赖](https://github.com/windoze/atto-ui/blob/613b29ff3c8513cd6e5b693c306cb39e276be4d8/crates/atto-ui-test-host/Cargo.toml) | portable-pty 0.9.0、vt100 0.16.2；crossterm 0.29.0 用于输入辅助类型 | 沿用成熟 PTY／终端解析库，固定依赖与 lockfile；不依赖 atto-ui 控件库 |
| [pty_test_host_api.rs](https://github.com/windoze/atto-ui/blob/613b29ff3c8513cd6e5b693c306cb39e276be4d8/tests/pty_test_host_api.rs) | 固定屏幕大小，发送按键／鼠标，等待目标状态，resize 后检查结果 | 作为 host 最小自测和 Scoop 交互 fixture 的结构参考 |
| [pty_wide_overlap.rs](https://github.com/windoze/atto-ui/blob/613b29ff3c8513cd6e5b693c306cb39e276be4d8/tests/pty_wide_overlap.rs) | 宽字符覆盖后的逐 cell 检查 | 检查真实擦除、列位置和字符残留，不只搜索文本 |
| [pty_clipboard.rs](https://github.com/windoze/atto-ui/blob/613b29ff3c8513cd6e5b693c306cb39e276be4d8/tests/pty_clipboard.rs) | 同时使用交互输入和 raw OSC 输出断言 | 用于 OSC 52 等不可见协议行为 |
| [terminal/dsr.rs](https://github.com/windoze/atto-ui/blob/613b29ff3c8513cd6e5b693c306cb39e276be4d8/crates/atto-ui-terminal/src/terminal/dsr.rs)、[callbacks.rs](https://github.com/windoze/atto-ui/blob/613b29ff3c8513cd6e5b693c306cb39e276be4d8/crates/atto-ui-terminal/src/terminal/callbacks.rs) | 查询回包、标题／OSC 52／光标形状观察 | 参考所需的协议支持；这些能力目前在 terminal 组件里，并非 test host 自动具备 |

测试支持代码放在 Scoop 仓库内，按实际用例提取／实现小型 host，不让常规验收依赖兄弟仓库的位置或工作区状态。引入依赖时复核维护状态并保留适用的许可证和归属。

### 9.2 执行模型

```text
fixture runner
  -> scoop build -> Scoop test executable
  -> Rust PTY host
       -> PTY slave: Scoop test executable
       <- PTY master: raw bytes -> vt100 screen + protocol observations
       -> PTY master: keys / mouse / paste / terminal replies
       -> PTY resize: dimensions + OS resize notification
       -> assertions -> compact result / failure diagnostics
```

Host 固定初始行列、`TERM` 和影响被测行为的环境；显式设置或清除 `COLORTERM`、`NO_COLOR` 等，避免继承开发者配置改变结果。测试程序进入预期状态并 flush 后，host 再发送对应输入；ready／步骤完成使用独特的可观察状态，不用固定 sleep 猜测。

所有等待均有截止时间，采用 `waitForText`、`waitForScreen`、`waitForOutput` 等谓词。底层短间隔轮询可以使用，但测试逻辑依赖状态，不依赖轮询次数或运行速度。重复协议输出使用已消费 offset 或新的步骤标记，不能总在整个历史输出中匹配同一个旧结果。

### 9.3 两类输出观察

| 观察通道 | 适合的断言 |
| --- | --- |
| Virtual screen buffer | 固定 cell 的字符、前景／背景、属性、光标位置、擦除与滚动效果、备用屏幕切换后的可见状态 |
| Raw output／协议观察 | 准确的 escape sequence、标题、OSC 52、同步更新和 parser 不保存的模式 |

保留两种通道。相同画面可能由错误命令偶然产生，而正确字节也未必产生预期屏幕效果；根据被测合同选择精确断言，不强制每个用例同时完整快照所有状态。

`queue`／`execute` 是否调用 flush，用记录调用次序的测试 sink 精确验证，并以真实缓冲 writer 覆盖输出组合；不能通过“短时间内 PTY 没收到字节”推断 queue 的正确性。

atto-ui 的 `region_snapshot` 会对每行执行 `trim()` 并删除末尾空行，适合宽松文本检查，不能承担终端精确验收。Scoop host 增加保留空格、空行和完整尺寸的读取；位置、清屏、宽字符覆盖用逐 cell 断言。统一 helper 的列／行参数约定，显式转换 vt100 返回的 row／col，避免坐标对调。

vt100 未保存或未完整模拟的属性由已有 callback／raw protocol 断言补充。宽字符用例验证命令交互，不把该模拟器的 Unicode 宽度或 emoji 行为当成所有真实终端的保证；不为本期另写完整终端模拟器。

### 9.4 查询回包

PTY 只提供终端设备通道，不会自动扮演终端模拟器回复查询。Host 必须额外处理本期用到的 CPR／DSR、DA 和 Kitty flags 请求，将回复写回 PTY master；不能仅维护屏幕后等待测试程序自行结束。

回复在解析到查询的位置取得当时状态。例如同一批输出包含“移动光标、查询、再次移动”，CPR 应使用查询时的光标位置，不能等整批数据处理完再读取最终位置。查询识别支持跨 read 分片，且不能误把 OSC payload 等其他控制字符串中的字节当作独立 CSI 查询。

Host 支持按用例设置正常回复、不回复、延迟回复及特定回复内容，用来覆盖查询超时、不支持、非法回复和与用户输入交错。Kitty 的“支持但当前 flags 为 0”与“不回复 Kitty 查询”分别测试，不用 `CSI ? 0 u` 冒充不支持。

这里只增加被测命令需要的应答行为。协议输入由 host 的独立编码或固定向量提供；不得调用 Scoop 的待测 encoder／parser 生成期望结果。

### 9.5 OS 状态、退出和错误

Virtual screen buffer 看不到 `termios`。raw mode、回显、规范输入与恢复必须读取实际 PTY 的属性或通过独立 native probe 验证。比较相关 flag、控制字符等语义字段，不对带 padding 的 C struct 做字节快照；保存进入前的状态，覆盖正常退出、抛异常、嵌套作用域及部分初始化失败。

Resize 测试同时改变 PTY 尺寸和虚拟屏幕大小，再等待 Scoop 收到事件并重绘。host 对输出处理与屏幕 resize 的次序进行同步，避免用错误尺寸消费 resize 后的画面。

程序退出不代表读取线程已经取得全部输出。正常结束要等待子进程、排空输出至平台认可的 EOF／PTY 关闭状态，再 join reader 并检查错误；退出码、读取失败和断言失败分别保留。不能只依赖 `Drop` 中忽略结果的 kill／wait／join。

超时与失败路径显式终止并回收 host 启动的子进程、解除阻塞读取并关闭 PTY。PTY 子进程可能有独立 session，不能假定外层 fixture runner 杀掉自身进程组就完成清理。失败诊断保留步骤、转义后的 raw bytes、最后屏幕、光标、退出状态和 reader 错误；原始日志放仓库外，不提交逐次运行归档。

## 10. 测试分层与验收矩阵

| 层次 | 主要内容 | 观察依据 |
| --- | --- | --- |
| Scoop 单元测试 | 命令编码、样式组合、事件值、非法参数；增量 parser 与错误恢复 | 小而精确的字节／类型化结果 |
| Parser 分片组合 | 每个切分点、多个事件合并、UTF-8／CSI／粘贴边界、Escape 到期、非法与溢出输入 | 显式 feed 与模拟时间；不靠 PTY 写入分次保证 read 分片 |
| 同步输入／查询 | poll 零等待、有限等待、完整事件保证、查询回复分流、EOF、resize、输入顺序 | PTY host＋真实 Scoop 程序 |
| 输出交互 | 颜色与属性、光标、clear、scroll、wrap、备用屏幕、宽字符覆盖 | cell／cursor／screen 状态及必要 raw 输出 |
| 模式与恢复 | raw mode、嵌套作用域、异常、重复关闭、初始化失败、stdin／stdout 重定向 | PTY 属性、独立 probe 与可见行为 |
| 扩展协议 | focus、鼠标按下／释放／拖动／滚轮、paste、Kitty、OSC 52、同步更新 | host 注入事件、查询应答及协议观察 |
| 异步阶段 | 取消与到达竞争、连续事件、查询交错、close、loop 收尾、移动 GC | 真实 EventLoop＋PTY；先完成 asyncio TTY 合同 |
| 产物与组合 | `.slib` 消费、native 链接、debug／release、分配与 moving GC 下的交互 | 普通应用构建与运行，保留源码、工具和产物 |

每个功能至少有独立用例及必要的组合用例。代表性组合包括“raw＋备用屏幕＋样式＋按键退出”、“鼠标＋resize 重绘”、“查询期间收到用户输入”、“粘贴包含转义字符”和“异常展开恢复 raw mode”。负向测试聚焦实际参数、资源状态和协议错误；若实施发现语言／FFI 缺口，再在对应层增加必要的编译诊断 fixture。

PTY 集成沿 [fixture schema 1](../../tests/fixture_runner/README.md)执行：先普通 `scoop build`，再以普通 argv 步骤运行 host 并传入实际产物路径。Host 输出小型确定性结果，必要时使用现有 `json = "stdout"`，失败以非零退出和诊断报告；不为 PTY 新增 fixture DSL、远程控制服务或第二套测试 runner。

Host 的 Rust 构建属于测试工具准备，按本机宿主编译；Linux GNU host 可以启动本机可执行的 GNU／musl Scoop 产物，不要求 host 自身也按目标 libc 重建。Scoop 库更新后按现有[准备脚本](../../scripts/README.md)显式准备开发 sysroot，普通 fixture 不自动打包发行物。

三目标均运行适用的纯协议、真实 PTY 与产物消费用例；GC 组合聚焦持久会话状态、buffer、异常与异步在途操作。测试 host 自身用少量固定协议用例验证坐标、查询应答、resize 和退出排空，避免模拟器错误掩盖生产问题。

最后保留少量真实终端 smoke test，覆盖 macOS／Linux 上实际使用的终端及 tmux 场景，记录具体终端版本与支持项。真实终端用于检查模拟器不能证明的互操作性；OSC 52 自动测试只捕获协议，不操作宿主用户的实际剪贴板。

## 11. 实施顺序与规范变更

| 阶段 | 工作与出口 |
| --- | --- |
| M59-A：公共合同与协议基础 | 新增 terminal 规范；确定 API、Escape 默认等待、EOF／错误、signal 共存和资源范围；建立普通 Cone、命令编码、事件类型与 parser 单元测试 |
| M59-B：Unix 同步闭环 | C shim、TTY 所有权、raw、尺寸／resize、read／poll、查询分流与恢复；完成最小交互应用和独立 PTY host |
| M59-C：功能覆盖 | 样式／屏幕／光标、鼠标／focus／paste、Kitty、能力探测、OSC 52；补齐 host 查询回复、精确屏幕和故障诊断 |
| M59-D：同步交付验收 | 三目标 debug／release、移动 GC、实际 `.slib` 消费及必要真实终端 smoke；记录与 crossterm 的差异和具体支持范围 |
| M59-E：异步适配 | 先扩展 asyncio TTY 规范与实现，再交付独立 terminal.asyncio、取消／关闭／查询组合和三目标验收 |

同步基础在 A～D 后可以独立交付，E 的状态单独记录；整个移植的进度不得把未完成的可选模块隐去。Windows 不列为本期完成条件。

实施遵守 spec 先行：

- 新增 `docs/specs/SCOOP-TERMINAL-SPEC.md`，承载公共 API、协议、错误、所有权、兼容差异与上述尚需确定的具体参数；本设计中的拟议名称同步收敛。
- 只有确实需要公共 native 支持入口时才修改 platform 规范，既有私有句柄继续私有。非 ASCII Shift 推导若需要共享 Unicode 属性，先明确 std 的最小合同与数据版本。
- 异步阶段先修订 asyncio 的 TTY 范围、buffer 借用、取消和关闭合同；若新增普通终端异步规范，同时更新双方引用。
- 没有实际语言、runtime ABI 或 pipeline 变化时，不修改三份核心规范来加入终端专属职责。真实底层缺陷在所属层按规范修复，终端库不维护绕过实现。
- 用现有 native sources、`.slib` 和 sysroot 准备／分发规则集成新库，不改变普通 build 的职责，也不引入 Windows manifest 占位规则。

本期验收记录只保留验证范围、结果概要、差异与剩余限制。实现及各目标验证状态持续记录在[实施记录](PROGRESS.md)，尚未验证的目标和功能不得由参考实现的结果推定完成。
