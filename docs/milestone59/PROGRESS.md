# M59 实施记录

状态：实施中。依据：[设计](DESIGN.md)、[终端库规范](../specs/SCOOP-TERMINAL-SPEC.md)。按功能完成本机验证后提交，最后进行 Linux 三目标范围内的完整验收；Windows 不在本期实现范围。

## 1. 功能进度

| 功能 | 状态 | 验证 |
| --- | --- | --- |
| 公共合同与普通 Cone | 已建立；后续功能逐项补规范 | 文档检查，开发 sysroot 已发布新 Cone |
| 命令／光标／屏幕 | 本机完成，PTY／Linux 随后验收 | 14 项单测 debug 与 release＋moving GC 均通过；3 项消费者 fixture、6 个变体通过 |
| 样式与颜色策略 | 本机完成，PTY／Linux 随后验收 | 新增 11 项单测；全库 25 项 debug 与 release＋moving GC 均通过；2 项消费者 fixture、4 个变体通过 |
| 事件数据与增量 parser | 本机完成，PTY／Linux 随后验收 | 新增 29 项单测；全库 54 项 debug 与 release＋moving GC 均通过；事件消费者 fixture 两个变体通过 |
| Unix 会话、raw、resize、read／poll、查询 | 本机完成；Linux 随后验收 | 会话、输入、查询各自的 debug／release PTY fixture 通过 |
| PTY host 与交互 fixture | 输出、输入、查询、raw、信号恢复与 asyncio TTY 本机完成 | host 5 项测试、runner 51 项规则测试通过；各 PTY fixture 的 debug／release 变体通过 |
| Kitty／OSC 52 与组合覆盖 | 本机完成；Linux 随后验收 | Kitty 协议／探测单测和 PTY 场景通过；OSC 52 的 3 项单测及两个 PTY 消费者变体通过 |
| asyncio TTY 与 terminal.asyncio | 本机完成；Linux 随后验收 | TTY、异步输入／查询的 debug／release PTY fixture、输出单测及相关 TCP／UDP 回归通过 |
| Linux GNU／musl 验收与清理 | 待实施 | — |

## 2. 工作约定与环境

- 工作分支 `codex/m59`，起点 `2f7411e5`；已有 M47／M57 文档修改保持独立。
- 本机 LLVM 22.1.8；Linux 可通过 `ssh nuc12` 的 `~/repos/scoop` 使用，开始时 checkout 为旧的 `e548b9ebe` 且干净，功能本机通过后再同步。
- 临时源码、构建缓存与工具 scratch 使用仓库忽略的 `tmp/m59`；原始日志位于仓库外，不写入绝对路径 `/tmp`。
- 开始时用 Cargo 清理旧 dev 产物（21.7 GiB），构建配套命令时关闭 Rust incremental，后续按功能检查与清理不用的 target。
- 本期优先执行新库和相关 fixture；涉及底层变更时再增加对应回归，不逐项重复整个编译器全量测试。

## 3. 首批输出功能

实现普通 Command、TextWriter／ByteSink 的 queue、ByteSink.execute、Print，以及光标移动／保存／形状和屏幕清除／滚动／模式／标题命令。文件按职责拆分，生产文件最大 52 行；未引入 compiler 或 runtime 特判。

本机验证包括纯编码、零基／一基及最大坐标转换、短写、无进展、错误传播、flush 次数、真实 BufferedSink、Unicode 和非法参数。三个独立 CLI fixture 消费预编译 `.slib`，分别覆盖光标、屏幕及自定义 Command／缓冲输出／GC 的组合；均在 debug、release 下启用 moving GC，完整原始字节匹配。Rust fmt 检查及 workspace Clippy 通过。

首轮测试中的混合值类型数组改用规范要求的显式接口装箱，并去掉普通数组字面量不接受的尾逗号；未改动语言语义。保存／恢复光标核对上游后采用 `ESC 7/8`。这批结果只证明输出编码与流组合，实际屏幕效果仍由后续 PTY 用例覆盖。

## 4. 颜色与样式

完成基础色、索引色、RGB、前景／背景／下划线颜色、28 项属性和不可变 ContentStyle／StyledContent，以及 NO_COLOR、显式覆盖和环境色数提示。基础色保持上游色槽；禁色时不生成空 SGR，下划线颜色以 59 复位。非法颜色和属性位集在输出前检查。生产文件最大 69 行。

新增 11 项单测覆盖全部属性编码、颜色解析、组合与复位、禁色、无效参数、内容转换异常和 Unicode。全库 25 项测试在 debug、release＋moving GC 下均通过。两个独立消费者 fixture 在两种构建模式下共执行 8 次程序，检查真实缓冲输出、GC、NO_COLOR 与 COLORTERM 的独立进程环境；全部通过。开发 sysroot 已重新发布，文档与源码空白检查通过。

## 5. 移植暴露的 Mach-O 查找表问题

键盘 parser 的 `letterKey` 返回带载荷 enum，LLVM 在 release 下为同一函数生成两张数值查找表。原 Mach-O literal pool 只接受段首表，第二个局部符号被拒绝，导致正常库不能生成 `.slib`。实现规范先明确多表边界，再按代码实际引用的局部符号起点划分表，保留后续已有 atom 和固定宽度 literal pool 的原有规则。没有关闭优化、跳过引用处理或新增 ABI。

新增独立 `m59-terminal-key-switch` fixture，旧工具在 release 下复现相同错误；修复后 debug／release＋moving GC 均通过。4 项 literal pool 测试（含新增多表专测）和 6 项 callable registration／fingerprint 回归通过，workspace fmt／Clippy 通过。该修复作为独立功能提交。

## 6. 输入事件、模式与增量解析

完成普通值类型 Key／Mouse／Event、类型化修饰键及键盘增强集合，鼠标／focus／paste／Kitty push-pop 命令，以及纯 Scoop 增量 parser。字节、raw 状态和单调时间显式输入，用户事件、设备回复和非法输入分流。覆盖 UTF-8、C0、Alt、CSI／SS3／Linux 功能键、传统／rxvt／SGR 鼠标、focus、paste、Kitty 和查询回复。Unicode 大写字符沿 Uppercase 属性处理，固定 Unicode 17.0 数据并保留许可证及生成脚本。

新增 29 项单测，以固定向量覆盖每个两段切分位置及逐字节输入；另测 Escape 截止时间、零等待、数字溢出、Unicode 标量、坐标下溢、非法输入重同步、粘贴部分结束标记和 EOF。旧式 F15～F20 对照 xterm 修正上游偏差；Kitty 标志解析完整十进制数字，256 修饰字段保留全部位，释放事件不伪造修饰状态。

全库 54 项 debug、release＋moving GC 测试通过；后者完整运行一次，后续按实际改动选测，避免反复执行全部分片组合。独立消费者验证公开 enum／struct 的跨 Cone 值传递、GC 和全部输入模式命令，两种构建模式共 4 个进程通过。预编译 sysroot 已重新发布。Scoop 生产文件最大 78 行，生成的 Unicode 范围表 117 行；生成脚本 Ruff 通过。Rust dev 产物再清理 6.0 GiB，target 剩余约 654 MiB；release 工具保留。

## 7. PTY host 与真实输出

独立 Rust host 固定 portable-pty 0.9.0／vt100 0.16.2，以普通工具参数接入 fixture runner；不新增 fixture DSL，不自动构建测试工具。读取线程只传字节，主线程维护精确 cell、raw output 和查询 callback。查询在实际解析位置取光标；正常退出同时等待子进程及 reader EOF，错误／中断路径显式终止 PTY 进程组并回收 reader。

host 的 4 项测试覆盖全部查询切分位置、Kitty 零标志与不回复的区别、真实 PTY 输出排空、空格／空行、交互、resize 和失败清理。runner 51 项规则测试通过。首个 Scoop PTY fixture 在 debug 与 release＋moving GC 下通过，共 4 个进程；覆盖 RGB／索引色／样式复位、光标保存、Unicode 宽字符覆盖、擦除、滚动、备用屏幕、标题和尺寸变化，比较实际 termios 未被纯输出改变。

验证发现 vt100 0.16.2 不实现 DECAWM；关闭自动换行改用精确原始序列断言，不扩大 host 为新终端模拟器。Rust fmt／Clippy 与 Python Ruff 均通过，host 最长文件约 200 行。

## 8. Unix 会话与 raw 生命周期

完成 TerminalSession、标准端点 TTY 检测、原生尺寸、自有 TTY sink、raw 幂等开关和 withRawMode。Unix Scoop 适配与 C shim 通过显式平台源码选择接入，公共 API 不暴露 fd／termios。原生句柄独立打开，不改变 stdin 的 O_NONBLOCK；重复会话以 EBUSY 失败。关闭先尝试恢复，再消费句柄，恢复失败仍继续关闭；无 GC／atexit 终端恢复钩子。

真实 PTY fixture 覆盖普通初态及已 raw 初态、嵌套作用域、异常恢复、重复启停／关闭、独占会话、关闭后操作、stdin／stdout 重定向后的控制终端使用、独立文件状态和无 TTY 的连续失败。debug／release 两个变体的 8 个进程均在 moving GC 下通过；之前的 PTY 输出 fixture 回归通过。host 新增退出早于预期输出的回收回归，5 项测试通过。

恢复检查按 termios 语义字段比较，排除 Darwin 内核在恢复 ICANON 时自动设置的 PENDIN 动态位；依据见 [host 说明](../../tests/terminal_host/README.md)。输入端只比较本功能承诺不改变的 O_NONBLOCK，避免把既有平台输出流设置的防 SIGPIPE 状态误判为会话修改。C 严格告警检查、Rust fmt／Clippy 通过；新生产文件均不超过 84 行。

## 9. OSC 52 复制

完成有序 ClipboardSelection、ClipboardType、CopyToClipboard 和 UTF-8 文本便捷入口，复用 std 的标准带填充 Base64。选择器在输出前拒绝控制字符、非 ASCII 和分号；保留默认目标、顺序及重复项。复制不受 NO_COLOR 影响，不引入桌面剪贴板或读取协议。

新增 3 项单测在 debug 和 release＋moving GC 下通过；按 Clipboard 筛选运行，没有重复全部 parser 切分测试。真实 PTY 消费者两个变体、4 个进程通过，核对 UTF-8、二进制与空内容的完整 OSC 52／ST，并确认屏幕和光标不变。host 只捕获数据，未修改宿主剪贴板。开发 sysroot 已发布，Rust fmt／Clippy 通过；生产文件为 21／47 行。

## 10. 同步输入、截止时间与 resize

完成 read／poll、可配置 Escape 等待、完整事件保留、非法输入恢复、EOF 与断开分类。Scoop 管理单调总截止时间和 parser，native 只做非阻塞读取与等待；EINTR／EAGAIN 不重新开始计时。读取使用显式 NativeByteBorrow，不捕获外层可变局部。

Linux 使用非阻塞 self-pipe，保留原 handler、mask 及一次性语义，拆除等待在途 handler 后释放通知 fd。Darwin 实测及 XNU 源码确认 sigaction 查询遗漏 SA_RESETHAND，故改用 kqueue EVFILT_SIGNAL 观察；不替换原 disposition，避免把无法恢复的一次性行为当作已支持。设计与规范已同步。

真实 PTY 输入覆盖 UTF-8 分片、连续队列、零／有限 poll、Escape、鼠标／focus／paste／Kitty、非法字节后继续读取、输入与 resize 交错、五种原 signal disposition 及规范模式 EOF。输入与已有会话 fixture 的 debug／release 共 4 个变体、12 个进程在 moving GC 下通过；host 5 项测试、Rust fmt／Clippy、Darwin C 严格告警检查通过。Linux self-pipe 的运行验证留待最后的 Linux 验收。生产文件最长 114 行，target 约 772 MiB。

## 11. 查询、回复分流与 Kitty 探测

完成光标位置、主／次设备属性、键盘增强 flags 和支持检测。查询复用输入 reader、parser 与总截止时间，保留交错用户事件和 poll 已保留的事件；发送前丢弃已完成的旧内部回复。无请求 ID 的协议关联限制已写入规范，不把同类迟到回复宣称为可完全区分。Kitty 以主设备属性结束探测，区分 Some(0)、None 和超时。

新增 3 项 Query 单测在 debug 与 release＋moving GC 下通过；同步输入 fixture 回归通过。查询 PTY fixture 的两个变体、4 个进程通过，覆盖 stdout／stdin 重定向、分片回复、用户事件保序、无关回复、正／零超时、缺少 Kitty 结束回复、解析错误、已排队旧回复和现有 raw 作用域。最终实际 termios 恢复；Rust fmt／Clippy 与 C 严格告警检查通过。Scoop 新文件最长 75 行，开发 sysroot 已发布。

## 12. asyncio 的真实 TTY

完成 EventLoop.openTty 和 AsyncTty，提供字节／resize／超时／EOF 接收、非阻塞输出、尺寸及 raw 控制。同步与异步的 native TTY owner 移至 platform，由两个实际使用者共享进程独占和恢复；libuv poll 观察自有 TTY 与原有通知 fd，不重新安装 signal handler。现有 TCP／UDP 的私有资源生命周期扩为三个端点共用，没有开放任意 fd 接管或私有 loop。

真实 PTY 验证同步／异步相互独占、参数与借用检查、定时器在等待输入时继续运行、取消后继续读取、输出流控暂停下的取消及剩余字节、resize、规范模式 EOF、并发关闭、取消 Job 中恢复 raw，以及 loop 收尾关闭逸出资源。两个变体、4 个进程在 moving GC 下通过；连同同步会话／输入／查询回归，共 8 个变体、20 个进程通过。

TCP 取消／关闭 3 项和 UDP 8 项回归在 release＋moving GC 下通过。C 严格告警检查、Rust fmt／Clippy、空白检查通过；新增生产文件最长 77 行，共用 owner 121 行。开发 sysroot 已发布，target 约 771 MiB；Linux 与 terminal.asyncio 验收仍待后续完成。

## 13. 独立终端异步适配

完成普通 scoop.terminal.asyncio Cone：AsyncTerminalSession、异步 read／poll、查询、raw 作用域和 AsyncByteSink 命令输出。同步与异步通过公开的纯协议 InputParser／InputItem／QueryReply 复用同一实现，不使用跨 Cone 私有访问。输入／输出各一个在途操作，查询及控制占两方向；关闭唤醒等待并等会话状态释放。取消后的实际读取进度仍进入 parser，保留已完成事件和部分前缀。

真实 PTY 的异步输入／查询在 debug、release＋moving GC 下通过，覆盖前缀跨取消、排队事件的取消检查、读写双向进展、Escape、鼠标／focus／paste／Kitty、错误恢复、resize、EOF、嵌套及异常／取消 raw 恢复、并发关闭、loop 收尾及错误 context。查询沿用同步版固定应答，另测取消后用户事件保留与真实 termios；同步输入／查询回归通过。

新增 3 项异步输出单测和 2 项协议接口单测在 release＋moving GC 下通过，覆盖短写、UTF-8、编码失败、flush、无进展、resize 与字节前缀共存、结束状态及共享请求编码。Rust fmt／Clippy 与空白检查通过；异步生产文件最长 78 行。开发 sysroot 已包含 14 个 Cone。再次使用 Cargo 清理 dev 产物 25.4 GiB，target 约 722 MiB，release 工具保留。

本机阶段汇总运行全部 15 项 M59 文件 fixture：30 个 debug／release 变体、68 个进程全部通过。Linux GNU／musl 的同范围验收继续单独记录。
