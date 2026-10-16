# M45 实施记录

M38 已完全验收后才开始本里程碑。本机批次逐项完成、提交并复用缓存；Linux GNU／musl 验收安排在所有本机修改稳定后。原始日志与机器报告保留在仓库外，本文只记录结果概要。

## M45-1：泛型扩展回调推断

2026-10-16 完成本机实现与回归。已知 receiver／普通实参先确定 lambda 参数及可选 receiver，再由正文确定返回类型；外层结果与其他延期实参仍可提供缺少的约束。失败探测保留候选隔离，最终 HIR 不包含部分函数类型，不引入库名特判或新的 IR／产物格式。

| 验证 | Darwin/AArch64 结果 |
| --- | --- |
| Rust 格式化与 workspace all-targets clippy | 通过 |
| scoop-hir-lower 全 crate | 1470 通过，含 5 项新增推断测试 |
| 相关正式 CLI 回归 | 148 通过；177 variants、497 processes、1 项既有结构 golden |
| 新增 M45 CLI | 8 通过；普通与挂起回调、构造／变量 receiver、隐式 it、嵌套调用、跨 Cone `.slib`、真实挂起恢复、debug/release 与 moving GC |
| 必须拒绝的调用 | 重载歧义、错误参数数目、缺少约束、普通 lambda 调用挂起函数、普通上下文调用挂起重载；精确诊断位置与原因通过 |

CLI 回归还覆盖既有 contextual inference、泛型约束、receiver function type、声明结果推导与重载专测。没有重复全 workspace／全 fixture 验收；Linux 未在本批执行，纳入最终组合验收。

## M45-2：platform Cone、系统字符串、路径与错误

2026-10-16 完成本机实现。普通 Cone 依赖 core／std，测试目标额外依赖 scoop.test；OsString／Path 的复制、UTF-8、字节比较、词法路径操作与错误对象均由 Scoop 实现。一个 C11 shim 负责依据本机 errno／EAI 常量分类，不读取延后的 errno，不携带 runtime 私有实现。

- `scoop test sysroot/lib/scoop.platform`：12 项通过，覆盖无损非 UTF-8 数据、严格／显式有损转换、NUL、路径组件／父路径／归一化、原始错误快照。
- native 错误码专测通过：实际 C 常量分类、EAI_SYSTEM 的 errno 快照与不改写线程 errno；作为普通 Scoop 测试无法直接读取 C macro 的窄 ABI 专测保留。
- 下游 `.slib` 消费 fixture 通过普通运行与 moving GC，源文件和工具始终存在；production build 使用普通 native requirements。
- C 格式化、`-Wall -Wextra -Werror`、Rust 格式化和 workspace all-targets clippy 通过；本批没有编译器／runtime 改动，未重跑 Rust 测试或全 fixture。debug/release 组合与 Linux 留到最终验收。

## M45-3 前置修复：异常对象身份

资源双失败测试暴露 throw/catch 复制 Throwable 的旧合同。已先修订语言、runtime 与 std 规范：throw、catch、重抛和跨挂起保存都保留原对象及字段别名。native record 通过现有 GcHandle 保活；删除两次对象复制、异常物化入口、无生产使用者的堆外对象 root 注册与扫描分支。runtime contract 升至 13，metadata 布局不变；旧产物按既有兼容性规则重建。

本机格式化、workspace all-targets clippy 与 C 严格告警检查通过；受影响的 MIR／LIR、lowering、codegen/runtime 与 slib 共 1732 项 Rust 测试通过。独立源码回归覆盖字段别名、catch 返回、重抛、实际挂起恢复、debug/release 和 moving GC。旧 ABI 固定向量及必要 LIR 期望已核对更新；11 份 MIR 代表快照只保存各自被测函数，从 5775 行缩至 605 行，异常与协程的专门 typed 断言仍保留。组合 CLI 与资源库结果在下一节记录，Linux 留到 M45-10。

## M45-3：core Disposable 与普通／挂起 use

2026-10-16 完成本机实现。三个普通 core 声明提供资源协议、两个 use 重载与保存原始双失败的 ResourceUseException；成功结果先保存，正文失败只清理一次，清理失败不被同一 catch 再次捕获后重试。挂起调用在最后恢复完成或失败时清理，嵌套资源从内向外退出；不新增语法、intrinsic 或专用 compiler role。

- `scoop test sysroot/lib/scoop.platform`：26 项全部通过，其中 14 项资源用例覆盖精确接收者／结果推断、含引用结果跨清理 GC、四种退出组合、异常身份、嵌套与获取失败、真实挂起及异常恢复。
- 正式 CLI 组合回归共 121 个用例闭合，覆盖 M45、现有异常／finally／callback／产物消费与全部 M38 用例。新增 core-only 资源及异常身份用例均通过 debug/release、普通与 moving GC。
- 批次初次为 120 通过、1 项旧 LIR 期望失败；核对删除复制调用及其临时变量、root/site 编号的变化后，仅复跑该 SSA roots 用例，debug／release／minor 三个变体通过，包含链接后重运行。没有重复整批或 Linux 验收。
- 格式化、workspace all-targets clippy、C 严格告警及前置修复的 1732 项 Rust 回归已通过。原始输出保留在仓库外，文档 validation 目录无原始日志。

## M45-4：NativeByteBuffer 与 GC pressure

2026-10-16 完成本机实现。普通 final class 独占 native 字节区，提供受检游标、显式增长、compact、数组复制、未对齐整数／浮点字节序操作及同步 unsafe 借用。成功写入才提交游标；借用期间拒绝重入修改，finally 保活并恢复状态。显式释放先清空 owner，GC hook 只读取字段并兜底释放。C shim 仅承担 realloc/free/memmove，失败 errno 在 NativeSafe 返回握手前保存。

core 增加一个直接 C leaf pressure 提示，适配层接入既有 GC 操作边界；生产实现有意为无状态 no-op。容量成功变化后报告增减，不主动收集或失败重试；没有新增 intrinsic、ABI 格式、生产计数器或资源预算。

- Rust 格式化、workspace all-targets clippy、C 格式化与严格告警通过；受影响的 toolchain 32 项测试通过。
- 新增 13 项 scoop.test 行为用例，debug、release 与 debug moving GC 均全部通过；覆盖初始化边界、大小溢出、独立数组、两种字节序／NaN／负零、关闭、重入冲突、异常与无效返回不提交游标。
- M45 正式 CLI 共 14 个用例闭合。新增 GC 专测通过 debug/release、普通和 moving：实际分配／扩容失败保留状态和地址、捕获 ENOMEM、容量通知配平、临时 owner 跨借用收集保活、显式关闭后不重复 free，以及遗弃 owner 的 hook 释放。仅 fixture 替换 GC 操作并包装实际 realloc/free 以观察通知与注入一次失败；原源码和工具始终保留。
- 独立 no-op 专测只链接真实 pressure 操作与 facade，不初始化 runtime，从主线程及四个未 attach pthread 反复传入零和 Long 极值；通过。下游 `.slib` 消费新增 buffer/use/借用组合，普通与 moving 通过。
- 首轮新代码有 lambda Unsafe 上下文、局部返回及可变捕获用法错误，已按既有语言规则修正；只复验受影响的 13 项库测试和两个 fixture，未重跑已通过的整批，也未进行 Linux 验收。

## M45-5：同步 IO、文件／目录、缓冲与文本

2026-10-16 完成本机实现。普通 ByteSource／ByteSink 与扩展提供部分／完整传输、数组、复制、UTF-8 文本及 TextWriter 组合；缓冲包装默认借用，显式工厂接管关闭责任，flush 失败保留未写后缀。File、DirectoryEntries 与标准流遵守各自所有权，关闭先终结 owner、再执行一次 raw close；活动 File 使用原子计数保活并允许 OS 支持的并发 IO。C 仅处理 native ABI、真实系统操作和信号边界，没有编译器／runtime 改动。

- 58 项 scoop.test 在 debug、release、debug moving GC 全部通过，其中本批新增 19 项，覆盖短传输、EOF／失败进度、缓冲重试／双失败／重入、文件偏移与复制／排他移动、目录提前退出、symlink 增长读取、临时路径与 UTF-8。
- 文件／目录 GC 专测通过 debug/release 与普通／moving：从实际 read/write/readdir 回调触发收集，临时 owner 保活；两个 native 线程重叠读取同一 File；fd 已消费但 close 返回 EINTR 后不重复关闭。标准输入／输出／错误和独立副本实际运行通过。
- native 专测通过真实短 IO、EINTR 重试、close-on-exec 副本／临时文件，以及 BrokenPipe 不调用或替换用户 SIGPIPE handler、保留原 mask/pending。Darwin fd 使用 F_SETNOSIGPIPE；Linux 线程局部屏蔽／消费的实现留待最终目标运行。
- 下游 `.slib` 消费新增文件、缓冲、泛型接管与文本组合，普通和 moving 均通过。Rust 格式化、workspace all-targets clippy、C 格式化与严格告警通过；没有重跑无影响的 Rust 测试或全 fixture。
- 首轮库测试 55/56：本机文件系统拒绝非法 UTF-8 名称，已把原假定改为 C 与 Scoop 同路径实际结果对照；保留原始 errno/context。后续新增的重入／并发测试一并通过。fixture 中 Option<Throwable> 相等及泛型 lambda 末尾无结果的错误按现有语言规则修正后，仅复验受影响用例。

## M45-6 前置修复：Unit lambda 末尾推断

平台时间测量测试暴露一个合法调用被拒绝的问题：期望返回 Unit 的 lambda 会把 Unit 错当成末尾泛型调用的结果约束。语言规范已明确，末尾值正常求值后丢弃，内部泛型推断不使用该 Unit；末尾控制语句遵守丢弃结果的规则，显式局部 return 仍检查 Unit 类型。实现只调整 HIR 的 lambda 正文入口和块处理，没有新增 IR／runtime 合同。

- 格式化、workspace all-targets clippy 与 scoop-hir-lower 1472 项测试通过，含新增泛型／控制末尾和显式 return 拒绝回归；已有算术尾值与嵌套 callable 作用域测试保持通过。
- 新 CLI 用例在 debug/release、普通／moving GC 通过；core 资源用例也通过。其余 67 个函数适配、返回标签、尾随参数、M45 推断、系统边界及产物消费用例统一通过（76 variants、231 processes，无新结构 golden），没有重跑全仓库。
- 首轮回归捕获过度使用普通语句检查而拒绝任意末尾值的问题，已保留其求值后丢弃路径并复验；新增 Boolean when 测试补齐既有穷尽规则。平台库直接使用原本合法的泛型测量调用，没有补显式类型或库侧绕行。

## M45-6：环境、系统信息、时钟与系统熵

2026-10-16 完成本机实现。原始 argv 复用 runtime 既有查询；环境值、只读快照和目录查询保留字节。系统信息读取实际 OS 值，时钟组合 std Clock／TimeSource／Instant／Duration；sleep 使用 monotonic 截止时间与 NativeSafe 等待。SystemRandom 实现 std Random，批量填充数组和 NativeByteBuffer，逐次提交已初始化尾部，可直接传给 Uuid.random。

- scoop.test 共 69 项在 debug、release、debug moving GC 全部通过，本批新增 11 项，覆盖环境快照／参数错误、实际 cwd、系统信息、时间源／测量／sleep，以及随机位范围、数组边界、buffer 进度和 UUID 格式；不以随机字节差异或统计阈值判定成功。
- native 系统 fixture 在 debug/release、普通／moving GC 通过：execve 注入空参数、空格、原始非 UTF-8 参数／环境及 HOME；passwd 查询模拟扩容、缺失与系统错误；getentropy 注入 EINTR、成功一块后失败，检查 buffer 游标和数组未写尾部，并从实际 native 调用回调 GC 检查借用／pin。
- sleep 专测用 condition variable 协调真实 pthread：worker 在实际 nanosleep 边界等待，另一个 mutator 完成 GC 后才放行并注入 EINTR，验证收集与等待同时推进。另覆盖时钟读取失败；无 sleep 猜测线程进度。
- 下游 `.slib` 组合增加原始环境、系统信息、时钟和随机调用，普通／moving 通过。格式化、workspace all-targets clippy 和 C 严格告警通过。库侧首轮的导入、整数宽度、数组初始化及 argv 可空指针绑定已修正；argv 与既有 native contract 对齐，runtime ABI 不变。

下一批为 M45-7；Linux GNU／musl 仍集中安排在全部本机功能稳定后。Linux getrandom 的实际短读取与其目标 ABI 随最终验收运行。所有原始记录在仓库外，docs/validation 未纳入原始输出。

## M45-7：线程与同步

2026-10-16 完成本机实现。Thread 复用 OneShot callback 与 TaskContext，普通 completion 保存原始 Throwable；join 回收整个 pthread，hook 仅 detach 并释放原生观察权。Mutex 使用非递归 ERRORCHECK 锁；ConditionVariable 绑定并保活 Mutex，超时使用 monotonic／相对等待。活动调用保活 owner，忙碌同步对象不会被 hook 强行销毁。

- scoop.test 共 77 项在 debug、release、debug moving GC 全部通过，本批新增 8 项。覆盖共享写入、锁内 GC、原始异常与 use 双失败、重复 join/dispose、self-join、独立 TaskContext、谓词循环、等待中保活、零／负超时、错误持锁状态和关闭后操作。
- 新增 native 生命周期 fixture 在 debug/release、普通／moving GC 均通过。实际创建入口注入 pthread／mutex／condition 初始化失败，检查回滚；真实 worker 阻塞时丢弃 Thread，确认 detach 不提前释放 worker 记录，完成后恰好释放；临时 withLock owner 跨 GC 保活，忙碌锁的 hook 不销毁存储，探针随后显式清理。
- 下游 `.slib` 组合新增线程、锁及条件变量，普通／moving 通过。格式化、workspace all-targets clippy、C 严格告警通过。首轮源码的初始化接收者捕获、泛型回调结果及 fixture 直接依赖已修正并闭合，无编译器或 runtime 协议变更。

下一批为 M45-8。Linux GNU／musl 继续留到所有本机功能完成后的统一验收；只记录结果概要，不提交原始日志或机器报告。

## M45-8 前置：安全调用语句

实现进程端点清理时，`endpoint?.dispose()` 暴露语句检查缺口：安全调用已经展开到条件分支，表达式只剩 Option 临时变量，旧检查错误地拒绝丢弃结果。按语言规范 7.3 修复源调用形式的识别，保留 receiver／实参／缺省值的分支求值与异常传播；没有放开单纯安全字段读取。

2026-10-16 本机格式化、workspace all-targets clippy、HIR 全 crate 1474 项通过。新增正／负 Rust 回归；正式 CLI 5 项、8 variants、19 processes 通过，包含新的方法／扩展／callable property／泛型清理调用及既有安全调用、挂起组合和 Unit lambda 回归。新例 debug/release、普通／moving GC 全部通过。Linux 随 M45-10 验收；产物结构及 runtime ABI 无变更。

## M45-8：进程、匿名管道与同步通信

2026-10-16 完成本机实现。Command 保存参数／环境／cwd 快照，posix_spawn 配置标准流与 child 信号状态，PATH 搜索使用命令环境。ChildProcess 明确拥有观察／reap 与父端 pipes，借用视图不提供关闭，take 转移端点；退出与信号分开表示，wait 缓存结果，ECHILD 终结观察权。communicate 用 poll 与 nonblocking pipes 同时推进三个方向，错误时关闭、kill 并 reap，不建立后台工作线程。

- scoop.test 共 89 项在 debug、release、debug moving GC 全部通过，本批新增 12 项。覆盖超过管道容量的三向传输、原始参数、环境快照、child cwd/PATH、文件重定向／stderr 合并、EOF／BrokenPipe、显式信号、重复 wait/dispose、take 与线程组合、通信失败回收。
- native fixture 在 debug/release、普通／moving GC 全部通过：真实入口注入 argv 分配、file action、spawn 失败并确认 fd 回滚；close 已消费后返回 EINTR 仍继续关闭和 reap；wait/poll EINTR、读写 EAGAIN 正确恢复；raw argv／环境／cwd、CLOEXEC、child 信号默认值与父信号状态均由辅助子进程检查。
- 同一 native 专测从 spawn、wait、signal、poll、read/write 边界回调 GC，覆盖临时 owner；已退出 child 的 hook reap，仍停止运行的 child hook 不等待或终止，探针随后清理。外部 reap 造成 ECHILD 后不再 wait 或 signal 旧 pid。最后补强活动 wait/signal 的 owner 保活后，仅重跑受影响 process 10 项的 debug/release/moving 和本专测，全部通过。
- 下游 `.slib` 增加子进程与管道组合，普通／moving 通过；该用例最后的数组参数修正只重跑自身。格式化、workspace all-targets clippy、C 严格告警与 diff 检查通过。库源码的数组构造、元组、闭包局部变量与安全调用前置修复均已闭合；runtime ABI 不变。

下一批为 M45-9。Linux GNU／musl 留至所有本机功能完成后的 M45-10，当前结果不代表其 libc 差异已通过。原始输出保留仓库外，清理结束的 fixture 工作目录并保留复用缓存。


## M45-9：阻塞 DNS、TCP 与 UDP

2026-10-16 完成本机实现。地址保留 IPv4／IPv6 位、scope 和端口；getaddrinfo 复制结果后释放，EAI_SYSTEM 保存同次 errno。socket 复用 File owner、NativeByteBuffer 与 NativeSafe，创建与配置失败回滚，活动调用保活。TCP 支持部分传输与半关闭；UDP 保留空包、实际复制长度及截断标志。Linux 使用 MSG_NOSIGNAL，Darwin 使用 SO_NOSIGPIPE，不改全局信号状态；没有新增编译器或 runtime 协议。

- 本库新增 10 项行为测试，当前 99 项在 debug、release、debug moving GC 全部闭合。覆盖两种地址族、解析／DNS、socket 选项、缓冲／线程组合、半关闭、UDP 空包／截断／大包失败及游标。debug 首轮 98/99，修正半关闭测试的对端生存期后，仅补跑受影响的 3 项 TCP 测试；release 与 moving 均一次 99/99。
- native 专测在 debug/release、普通／moving 全部通过。覆盖创建／配置／bind／connect 失败回滚、已消费 close 的 EINTR、真实短发送与 EINTR、DNS 错误域／释放、CLOEXEC、SIGPIPE，以及实际 accept/read/send/recv/DNS 边界触发 GC 的 owner 保活。
- 本机未监听端口的连接受网络策略影响会等待超时，因此连接拒绝错误通过实际 connect 边界注入，避免依赖主机防火墙行为；成功传输仍使用真实 loopback 和动态端口。fixture 的 readBytes 参数遗漏及失败诊断已修正，只复跑该专测。
- 下游 `.slib` UDP 组合在普通／moving 通过；Rust 格式化、workspace all-targets clippy、C 格式化与严格告警通过。原始记录保留在仓库外；Linux 留到 M45-10 集中运行。
