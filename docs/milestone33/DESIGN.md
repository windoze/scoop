# M33 设计：平台库的编译器、driver 与 runtime 前置能力

状态：实施中。以现有设计与三份规范修订为起点，按第 12 节逐项实现、验证并提交；实际进展与测试结果见[验收记录](ACCEPTANCE.md)。数据竞争、NativeSafe/collector、GCLeaf/DirectC、errno、main/root gateway、C++ 与源码选择的既有结论保持；D3～D7 已确定，包含严格、可空和 lossy UTF-8 三种转换，并同步到相关规范。D8 的 M32 依赖已满足。第 10 节记录已定结论、技术验证状态和后续实施任务，不再保留待选择的 D 项。

Equality 按 9.2 的统一接口方案实施并已同步到规范：`==` 使用 Equality 契约，值类型派生相等时同时产生接口实现，泛型通过普通 interface bound 消费。

日期：2026-10-08。

基线：M32 完成后的仓库；依赖 M12/M13 的 FFI 与 foreign-thread callback、M15 的精确根与 moving GC、M23 的多 Cone、`.slib` 与 artifact-only program-link、M24 的 release hook、M28 的三个 target、M31 的 nursery 与卡表，以及 M32 的源码 Any/Nothing 和底类型闭环。M32 已于 2026-10-07 完成，见[路线图](../ROADMAP.md)和[验收记录](../milestone32/ACCEPTANCE.md)；现有 [roots.scoop](../../sysroot/lib/scoop.core/src/roots.scoop) 已声明两个根类型。2.2 的 `exit` 直接使用实际 Nothing 声明，M33 不再等待 M32 合并。

数据竞争规则已写入语言规范 1.2、运行时规范 3.5 和实现规范 2.19；NativeSafe、collector 及初始化等待者的握手已写入运行时规范 3.5、4.5～4.6；`@GCLeaf` 与标量直接 C ABI 调用已写入语言规范 13.2、13.4.1、13.8、运行时规范 3.2、3.5、4.5 和实现规范 2.2、2.4～2.6；errno 按次捕获与 tuple 返回已写入语言规范 9.1.6、13.4、13.4.2、13.8，运行时规范 3.8、4.5，以及实现规范 2.2～2.6、2.8。main、argv、成功／失败退出码与 root gateway 已写入语言规范 11.7、12.4.4，运行时规范 2.8、第 7 章和实现规范 2.2～2.4、2.6～2.8、2.14。native C++ 开关、工具链与链接要求、musl 限制已写入语言规范 12.2.1、实现规范第 1 章及 2.7～2.8，并在运行时规范 5.5、第 7 章明确 native C++ 边界。源码选择已写入语言规范 12.2.2，同步修订了 11.12、12.1、12.2、12.4.4、第 15 章及实现规范 2.1、2.7。其他能力的路线图条目、spec 修订和交叉引用由后续完成；第 11 节列出每项涉及的 spec 章节。

## 0. 目标与边界

下一阶段计划提供一个普通 library Cone 形式的平台库，覆盖文件、文件系统、进程和线程。调研结论是：平台库本身可以完全用普通 Scoop 加 Cone 自带的 C 源码写成，但它依赖一批目前不存在、只能由编译器、driver、runtime 或 GC 提供的能力。M33 只交付这些能力，不交付平台库本身。

M33 交付的能力分为五组：

1. **程序入口与进程生命周期**：`main` 接收参数、返回退出码，提供 `exit`；未捕获异常的退出行为确定，runtime 自己的输出不再卡住 GC。
2. **并发与 FFI 基础**：语言内存模型、原子操作 intrinsic、程序退出时线程与 callback token 的处理规则、NativeSafe 进出的无锁快路径、通过 `@GCLeaf` 跳过状态切换的小型 C FFI，以及消除标量 FFI 桥接调用与临时存储的直接 C ABI 路径。
3. **native 数据交换**：有作用域的数组与 String 数据借用（GC 侧采用计数的线程局部 pin 帧）、严格／可空／lossy UTF-8 解码、String 与 C 字符串互转，以及 C 调用后的 errno 捕获。
4. **构建与链接**：Cone 内的 C/C++ 源码、显式 C++ 编译与链接模式、manifest 声明的 native 库、按平台选择源码，以及依赖 Cone 的默认定位。C++ 首版支持 Darwin 与 Linux GNU，暂不支持 musl。
5. **core 契约**：泛型相等能力，使普通库能写 `HashMap<K, V>`。

| 决策 | M33 范围 |
| --- | --- |
| 平台库形态 | 普通 library Cone，不进入 core；M33 只提供前置能力 |
| main 与退出码 | 无参数／完整 argv × Unit／Int 四种形态；argv[0] 为程序自身启动路径；Unit 正常退出为 0，Int 保留任意返回值，语言级 panic／未捕获异常退出为 1 |
| 泛型相等 | `Equality<T>` 提供唯一的 equals operator 契约；结构相等同时派生接口实现，Hash 独立；Float/Double 支持 Equality 并保留 IEEE 语义 |
| 平台差异 | 主要放在 Cone 自带 C/C++ 源码的 `#ifdef` 中；必要时按文件选择 Scoop/C/C++ 源码，不做 Scoop 表达式或声明级条件编译 |
| C++ 模式 | `[native] cxx = true` 显式启用；使用配套 C++ 编译器与最终链接配置，需求沿 `.slib` 依赖闭包传递；C++ + musl 暂不支持 |
| thread spawn | 不新增 runtime spawn 入口；库用 `pthread_create` 加 OneShot `foreignCallback` 实现 |
| 线程同步原语 | 库用 pthread 的 C ABI 实现（阻塞期间 NativeSafe）；编译器只提供原子操作 |
| 原子类型 | `AtomicInt`/`AtomicLong`/`AtomicBoolean`/`AtomicRef<T : ref>` 为 intrinsic final class，直接公开；`Atomic<T : value>` 用原子 flag 自旋锁加普通字段组合 |
| 原子内存序 | 首版开放 Relaxed/Acquire/Release/AcqRel/SeqCst，默认 SeqCst，合法组合遵循 C11；native `Ptr<T>` 原子操作留待实际需求 |
| 正常退出的资源约束 | main 返回前完成 join 与 token 释放；有遗留 attachment、活动 callback 或 token ownership 时诊断、刷新并以 1 退出，不提供 daemon |
| UTF-8 转换 | 严格抛 CharacterCodingException、严格返回 Option、lossy 三种入口；lossy 按 maximal subpart 将非法序列替换为 U+FFFD |
| 数据竞争与撕裂 | 未同步的数据竞争一律为未定义行为，不保证内存安全或不撕裂；并发安全依赖显式使用 Atomic 或同步原语 |
| errno | 由 extern 声明选择性捕获，Scoop 返回 `(R, Int)`；bridge 调用前清零、返回后立即保存 errno，结果按值交回，不设置线程级 last-error |
| 数据指针 | 有作用域的借用，不暴露 `pin` 加硬编码偏移 |
| NativeSafe 切换与 collector | 原子状态与 RETURNING 握手，常见进出无全局锁、无广播；collector 使用释放 world lock 的条件变量超时等待并复查停稳条件 |
| 小型 C FFI | 显式 `@GCLeaf` 保持原线程状态；完整调用链无 safepoint、无 Scoop 回调、无阻塞等待，普通 C FFI 仍默认 NativeSafe |
| C ABI 直接调用 | 当前 target 上，固定参数的标量 C 签名且未选择 errno 捕获时直接调用 native symbol；与 NativeSafe/GcLeaf 协议独立，按值 C-layout struct 与 errno 捕获继续使用 bridge |
| Scoop 源码清单 | 无顶层 `sources` 时递归扫描 `src/`；出现后使用完整显式清单，公共目录也须列出，不追加默认扫描；当前 target 的选中根与文件不得重叠或重复 |
| 条件编译谓词 | 封闭集合 `os` / `arch` / `env`，用于 manifest 的源码和 native 库选择 |
| 系统 native 库 | 显式 library roots 无候选时，从所选 target 的工具链/sysroot/SDK 默认目录解析，不回退到交叉编译宿主库 |
| sysroot 中的 Cone | 仅为已声明的 `scoop` group 依赖补充默认源码查找；显式 locator 和 artifact 候选优先 |
| 完成条件 | 共同能力在三个 target 上完成源码编译、产物消费、链接与运行；C++ 在 Darwin/GNU 完成闭环，musl 验证明确拒绝；含一个组合各项共同能力的示例平台 Cone fixture |

M33 不做以下内容（另见第 14 节）：平台库的公开 API（File、Path、Thread、Mutex、Process、HashMap、Duration 等）；fork 安全（库用 `posix_spawn`）；off-heap ByteBuffer；arena 扩容；把 NativeSafe 状态切换内联进生成代码；新增 target。

## 1. 当前基线

以下以 M32 完成后的实现为基线；已在规范中确定的 M33 规则仍需由本里程碑实现。M32 的 Any/Nothing 源码、底类型控制流及产物消费已交付，其 runtime ABI contract 为 10、metadata ABI 为 6；M33 第 2 节在此基础上升级到 11/7。

| 现状 | 对平台库的影响 | 主要落点 |
| --- | --- | --- |
| executable 的 `main` 必须无参数且返回 `Unit`；linker 生成 `int main(void)` | 拿不到 argv，没有退出码 | 语言规范 12.4.4；[startup.rs](../../compiler/linker/src/startup.rs) |
| 正常结束固定返回 0；启动失败和未捕获异常一律 `abort()`（退出码 134） | 不能报告失败退出码；`abort` 不刷新 stdio | [startup.c](../../runtime/src/startup.c)、[failure.c](../../runtime/src/startup/failure.c) |
| core 的 `write` 是 Scoop ABI extern，实现为 `fwrite(stdout)` | Scoop ABI 调用期间线程处于 NativeBorrowed，GC 必须等它返回；stdout 管道写满时整个进程的 GC 停住。另外与 fd 直接写入的输出顺序不一致 | [io.scoop](../../sysroot/lib/scoop.core/src/io.scoop)、[rt.c](../../runtime/src/rt.c) |
| 已明确数据竞争为未定义行为；尚无源码原子操作和配套内存序 API | 平台库无法用 Scoop 原子操作实现无锁状态、引用计数、once 或线程间发布 | 语言规范 1.2；[M13 设计](../milestone13/DESIGN.md) |
| `main` 返回时，只要还有已 attach 的非主线程，或有未释放的 callback token，进程就打印诊断并终止 | 没有 join 的线程和 daemon 线程都会让进程异常退出 | 运行时规范第 7 章；[thread.c](../../runtime/src/thread.c)、[callback.c](../../runtime/src/callback.c) |
| 普通 managed C ABI 调用的进入和返回各获取一次全局 `scoop_thread_world_lock`，并各广播一次；`registry_lock` 使用的就是这把锁 | 高频短调用在两个状态切换点争用全局锁；实际 C 函数执行期间已释放锁，可以并行 | [transitions.c](../../runtime/src/thread/transitions.c) 的 `enter_native` / `leave_native`；[thread.c](../../runtime/src/thread.c) |
| 普通 managed caller 尚无显式的无切换 C FFI 模式；release hook 的 raw leaf 是独立入口 | 小型 C helper 也要建立 transition 和 caller roots | [call/protocol.rs](../../compiler/lir-lower/src/function/call/protocol.rs) |
| C extern 一律经过独立编译的 storage bridge；标量实参与返回值也通过地址传递 | 即使省去 GC 协议，仍有桥接调用和临时内存读写；现有 O2 不能跨对象消除该接口 | [c_bridge.rs](../../compiler/codegen/src/c_bridge.rs)、[call/emission.rs](../../compiler/lir-lower/src/function/call/emission.rs)、[c_bridge_invocation.rs](../../compiler/lir/src/c_bridge_invocation.rs) |
| collector 读取 MANAGED 线程的非原子字段 `managed_segment` 来判断 PENDING | 状态切换去锁后会形成数据竞争 | [collection.c](../../runtime/src/thread/collection.c)、[collector_roots.c](../../runtime/src/gc/collector_roots.c) |
| collector 的停稳等待使用无超时的 condvar；等待操作会释放 world lock，返回前重新取锁 | NativeSafe 去掉广播后须增加超时复查；替换等待时必须保持目标线程能取得 world lock 来 park | [collection.c](../../runtime/src/thread/collection.c)、[thread.c](../../runtime/src/thread.c) |
| `pin` 返回对象头地址，元素地址只能按 runtime 布局手算（UByte 数组为 +24） | 库代码依赖未承诺的布局 | 语言规范 14.1；[scoop_rt.h](../../runtime/include/scoop_rt.h) |
| `pin` 不计数，一次 `unpin` 就解除；`unpin` 线性扫描 pinned 列表；pin/unpin 都要拿 heap 锁和 roots 锁 | 两个线程同时借用同一个 buffer 会出错；每次 IO 借用都是全局加锁 | [handles.c](../../runtime/src/gc/handles.c) |
| String 不以 NUL 结尾；只有 `fromUtf8Unchecked`，没有受检解码 | 路径、argv、目录项、文件内容都无法安全转成 String | [strings.scoop](../../sysroot/lib/scoop.core/src/strings.scoop)；[TODO](../TODO.md) 基础库接口 |
| 源码没有 errno 机制；runtime 的状态切换会调用 pthread 函数，没有保存 errno 的契约 | 在第二次 extern 调用里读 errno 不可靠，每个系统调用都要写 C shim | [transitions.c](../../runtime/src/thread/transitions.c) |
| 不支持 C 变参；生成的 bridge 一律输出非变参原型 | Darwin arm64 上 `open(..., mode)`、`fcntl(fd, cmd, arg)` 会传错参数 | 语言规范 13.4；[c_bridge.rs](../../compiler/codegen/src/c_bridge.rs) |
| manifest 没有 platform 条件，也没有 native link option；program-link 不编译用户 C 代码 | 库不能自带 C shim，也无法声明系统库 | 语言规范 12.2；实现规范 2.8 |
| 实现仍默认递归扫描 `src/`，尚无按 target 选择 Scoop 源码的入口 | 平台文件放在 `src/` 子目录仍会一起编译，需要让显式清单替代默认扫描 | 语言规范 12.2.2、第 15 章；实现规范 2.7 |
| 只有 core 能从 sysroot 自动定位；scoop.json 需显式传 `--cone-path` | sysroot 中的其他库对用户不可用，除非手动指定 | [trusted_core.rs](../../compiler/toolchain/src/trusted_core.rs)；语言规范 12.2、12.6 |
| `Hash` 只有 `hash()`；`==` 只从 lhs 静态类型的成员 `operator fun equals` 中决议 | 泛型 `K` 只有 `Hash` bound 时不能写 `k1 == k2`；String 与整数声明在 core，外部库无法让它们事后实现新接口 | [types.scoop](../../sysroot/lib/scoop.core/src/types.scoop)；语言规范 11.11 |

可以直接复用、不需要改动的现有能力：

- **默认 C ABI 调用的阻塞不影响 GC**：线程处于 NativeSafe，collector 不等它。
- **foreign thread 回调**：`foreignCallback` 会自动 attach，闭包中可以分配内存、触发 GC。OneShot 的 invocation 结束后自动 detach；异常保存在 token 中，由 observer 重新抛出。
- **初始化线程安全**：object/companion 的 exactly-once 初始化跨线程安全，等待方进入 PARKED，可以参与 GC。
- **release hook 兜底释放**：release hook 中可以直接调用 C ABI（例如 `close`、`free`、`pthread_mutex_destroy`）。
- **线程局部存储**：`@ThreadLocal` 可以保存 GC-free 值，包括 `GcHandle` 的 raw 值。
- **源码底类型**：M32 已完成实际 Nothing 声明、无正常返回求值和跨产物传播；`exit` 不需要另设伪 Unit 返回或缺失声明兜底。

## 2. 程序入口与进程生命周期

### 2.1 `main` 的形态与 root gateway ABI

executable root 的 `main` 从一种扩展为下面四种组合，仍然只允许具有普通 Scoop 函数体的 top-level、ordinary、non-generic、non-suspend 入口：

| 源码签名 | 正常结束时的退出码 |
| --- | --- |
| `fun main(): Unit` | `0` |
| `fun main(args: Array<String>): Unit` | `0` |
| `fun main(): Int` | 实际返回的 `Int` 值 |
| `fun main(args: Array<String>): Int` | 实际返回的 `Int` 值 |

参数名不必是 `args`；有参数时恰有一个普通参数，类型为实际 core 的 `Array<String>`，透明 alias 按普通规则展开。HIR 只从 root Cone 发现唯一入口，其他签名、没有入口或入口不唯一时报错，诊断指出实际签名和允许的形态。library 与依赖 Cone 中的同名函数不参与入口选择。

`args[0]` 是可执行程序自身的启动路径，用户参数从 `args[1]` 开始；没有用户参数时数组仍有第 0 项。**这与 Java/Kotlin 不一致：Java/Kotlin 的 `main(args)` 不包含程序路径。** Scoop 保留原生 `argv[0]` 的字符串写法，不保证绝对路径或规范化真实路径；`scoop run` 使用稳定输出路径。其余参数保留顺序及空字符串，不重新拆分。

Int main 可返回任意有符号 32-bit Int，包括负数与边界值，Scoop 不限制到 `0..255` 或主动截断。当前 POSIX target 的父进程只能按系统规则观察正常退出状态的低 8 位；这与 Scoop 传递完整 Int 是不同层面的规则。Unit main 正常结束的退出码为 0，四种 main 因语言级 panic／未捕获异常结束时均为 1；Int main 正常返回 1 仍属于成功调用。正常返回沿用现有 shutdown 检查，线程／token 的后续选择仍见 4.3。

typed root entry 在 HIR/MIR/LIR 与 program metadata 中完整保存四种形态、实际 main identity 和源码签名，生成的 gateway 决定是否构造 argv 及如何取得退出码。四种形态共用下列 C ABI，避免把用户退出码与异常状态混在一个返回值中：

```c
typedef uint32_t (*ScoopRootEntryGatewayFnV1)(
    int32_t argc,
    const char *const *argv,
    int32_t *out_exit_code);

int scoop_rt_run_program(
    const ScoopImageDescriptorV1 *const *images,
    uint64_t image_count,
    const ScoopRootEntryDescriptorV1 *root_entry,
    int32_t argc,
    const char *const *argv);

int32_t scoop_rt_program_argc(void);
const char *scoop_rt_program_argv(int32_t index);
```

- **成功**：gateway 向 runtime 提供的独立、4-byte 对齐的 Int32 输出槽写入 Unit 对应的 0 或 main 的完整 Int 返回值，随后返回 status 0。
- **失败**：gateway 捕获 argv 构造或 main 抛出的 Throwable，物化后写入 root-entry failure root，完成 EndCatch，再返回 status 1。runtime 忽略退出码槽，按 2.2 的失败路径输出诊断并以 1 终止。其他 status 非法；语言级 panic 可直接进入失败终止路径，不伪造异常或空 failure root。
- eager startup gateway 保持独立的 `uint32_t(void)` ABI。runtime 使用不同的精确函数指针原型发起调用，不能通过强制转换复用旧 root 调用路径。

启动顺序与 GC 责任为：

1. linker 生成 `int main(int argc, char **argv)`，将完整原生 argc/argv 交给 `scoop_rt_run_program`，原样返回其 C int 结果。runtime 先保存原始 pointer/count；调用方提供有效、包含 argv[0]、末尾为 null 的 argv，并保证这些字节直到进程结束均有效且不被修改。raw argv 访问器是只读 NoGC 操作，越界 index 返回 null，eager 初始化即可使用。
2. runtime 完成全部 image、根、类型及 callable/safepoint 登记，建立 GC、主线程及 root TaskContext，依次运行 eager gateway。root TaskContext 在首个 gateway 的入口 poll 后创建；失败不执行 main。
3. 全部 eager 成功后，runtime 为 root gateway 建立 EntryPending boundary，提供初始化为 0 的退出码槽。gateway 先执行入口 poll，再进入 ActiveManagedSegment，不能在此之前构造 managed 参数。
4. 只有带参数的形态才构造 `Array<String>`：包括 argv[0] 在内的各项严格按 UTF-8 解码并复制；非法 UTF-8 由构造 helper 先经 NativeSafe stderr 报告参数下标，再抛出异常交给 gateway，退出为 1，main 不执行。无参数形态不构造或解码参数，需原始字节的程序使用 raw argv 访问器。部分数组、当前 String 及其他跨 safepoint 的引用遵守普通 roots、relocation 与写屏障规则；C coordinator 不保存未登记 managed 引用，原始 argv 不需要 pin。
5. gateway 按实际 Scoop ABI 调用 main，处理正常／异常出口后返回 C，runtime 按 LIFO 离开 boundary。成功时完成正常 shutdown，再把完整退出码返回 C main；失败按 2.2 终止。

runtime ABI contract 从 10 升为 11，metadata ABI 从 6 升为 7；同步更新公共头文件、各层 typed entry、metadata／指纹与缓存、gateway 发射、runtime 调用和 linker 启动代码。RootEntry 字段顺序和 192-byte 大小保持，source signature fingerprint 覆盖实际 main 签名，gateway definition fingerprint 覆盖对应正文；runtime 无需额外形态 tag。旧版产物／runtime 不能混用，不能以仍为 192 bytes 为理由按旧 `uint32_t(void)` 原型调用。

环境变量不需要 runtime 参与：库在自己的 C 源码中读取 `environ` 或调用 `getenv`。

### 2.2 `exit` 与异常退出

- 新增 runtime 入口 `scoop_rt_exit(code)`，由 core 暴露为普通函数 `exit(code: Int): Nothing`；返回类型使用 M32 交付的源码底类型 `Nothing`，调用之后的代码按不可达处理。语义如下：
  - 先刷新 runtime 自己持有的输出缓冲（stdout、stderr），刷新操作结束后调用 `_exit(code)` 终止进程。
  - 刷新可能等待 stdio 锁、管道或文件 I/O，这是该退出接口允许的正常行为，不承诺退出耗时上限；不要求超时后丢弃输出或绕过刷新。刷新沿用 2.3 的 NativeSafe 输出路径，等待输出时不阻止其他线程推进 GC。
  - 不执行 `finally`，不执行 release hook，也不做 shutdown 时的线程与 token 检查；不等待其他 Scoop 线程 join，其存在本身不阻止请求进程退出。
  - 不调用 C 的 `exit()`，避免 atexit、C++ 静态析构等与仍在运行的 managed 线程交错。
- 未捕获异常与语言级 panic：先把诊断写到 stderr，再刷新 stdout/stderr，然后调用 `_exit(1)`，固定以退出码 **1** 结束。适用于四种 main，不使用尚未产生的 Int 返回值；异常诊断不调用用户 `toString`。runtime 内部 ABI 错误仍然 `abort()`，以便保留 core dump。
- 启动失败（eager 初始化或 argv 构造失败）和未捕获异常使用同一条退出路径，不执行 main、不等待其他 Scoop 线程 join，也不销毁仍可能被它们访问的 GC 状态。刷新等待仍遵守 2.3 的 NativeSafe 协议，允许等待输出完成。

需要跳过输出刷新与清理、直接终止进程的接口，后续由平台库封装目标平台的 `_exit`／`_Exit` 等能力提供，具体名称随平台库设计确定；M33 不为此新增退出模式或 runtime 入口。

### 2.3 runtime 输出不再卡住 GC

`write` 改为在 NativeSafe 状态下完成实际的写入：

1. core 中的 `write(message: String)` 改为普通 Scoop 函数：先对 String 做第 6 节的数据借用，得到 `(指针, 字节长度)`，然后调用 C ABI 的 runtime 入口 `scoop_rt_stdout_write(ptr, len)`。
2. `scoop_rt_stdout_write` 仍然使用 stdio 缓冲，所以和现在一样，`print`/`println` 不会每次都触发一次系统调用；此时线程处于 NativeSafe，管道阻塞不影响 GC。
3. 同时新增 `scoop_rt_stderr_write` 和 `scoop_rt_flush_stdout`，core 提供 `writeError`、`eprint`、`eprintln` 和 `flushOutput`。平台库需要与 fd 1 的直接写入交错时，先调用 `flushOutput`。
4. stdio 本身是线程安全的；不承诺多线程同时 `println` 时的行级原子性。

## 3. 内存模型与原子操作

### 3.1 内存模型

并发安全与数据竞争的基本规则见语言规范 1.2。Scoop 不提供类似 Swift / Rust 的并发安全保障，不通过类型系统排除数据竞争，不引入 Sendable 一类的编译期线程安全检查，也不保证诊断无管控的并发访问。

- **happens-before**：由同一线程内的程序顺序、同步原语建立的跨线程同步关系及其传递闭包组成。对同一原子对象的 release 写，与读到该值的 acquire 读建立同步关系。所有 SeqCst 操作另有一个全序，但该全序本身不额外建立不同线程之间的 happens-before。
- **C ABI 同步操作**：`pthread_create`/`pthread_join`、mutex lock/unlock、condvar 等由 C 实现的同步操作，按各自契约在 Scoop 侧建立 happens-before。平台库据此实现 Thread、Mutex 等同步原语。
- **对象发布**：分配并初始化对象后，经原子 release 写与匹配的 acquire 读，或 C ABI 同步操作发布给另一线程，接收方能看到完整初始化后的字段和 TypeDescriptor。普通共享字段中的引用读写不提供此保证；`AtomicRef` 的原子访问也不自动保护所指对象的后续可变字段。
- **数据竞争**：不同线程访问同一或重叠的存储位置，至少一个访问是写、至少一个访问是非原子访问，且这些冲突访问之间没有 happens-before 顺序。此规则覆盖 class 字段、`MutableArray` 元素、普通全局存储、`@Global` 和 native 内存等，不按其存放的是引用还是值类型区分。局部变量或参数持有的引用也可能指向共享对象。
- **竞争一律为未定义行为**：引用、标量、struct、tuple 和 enum 都不例外；不保证内存安全、读到的值有效或任何特定结果。值类型本身不可变，不代表存放该值的共享位置可以无同步地重新赋值和读取。
- **撕裂同样为未定义行为**：竞争读写导致的单字撕裂、不同字段来自不同次写入，以及 enum 的 tag/payload 不一致，都不提供结果保证。不能依赖自然对齐、单条机器指令、GC 或当前优化方式获得竞争安全。
- **显式并发控制**：需要并发安全时，必须使用 3.2 的原子类型、3.4 的 `Atomic<T>` 或以后通过库提供的 mutex / lock 等同步原语。锁保护的数据，其冲突访问必须遵守同一同步协议；原子量应使用满足所需发布与可见性关系的内存序，`Relaxed` 只保证原子量自身的原子访问，不为其他位置补充同步。
- **GC 与借用的边界**：GC 保活、pin、safepoint、native 状态切换和 GC 写屏障都不能替代用户数据的同步；不承诺在有数据竞争时仍能保持内存安全或 collector 不变量。正确同步程序仍须满足现有布局、根登记、写屏障和移动 GC 契约。

### 3.2 原子类型

原子量以 core 中的 intrinsic final class 提供，直接作为公开 API，不再包一层：

```scoop
public final class AtomicInt(initial: Int)
public final class AtomicLong(initial: Long)
public final class AtomicBoolean(initial: Boolean)
public final class AtomicRef<T : ref>(initial: T)
```

- **为什么是 class 而不是 struct**：原子操作必须作用在同一块存储上，而 Scoop 的值类型方法拿到的是独立的 `this` 副本（语言规范 3.3、13.7），Scoop 也没有对存储位置的引用。用 struct 承载就要新增“不可复制的值类型”和“方法作用于存储位置”两条语言规则。用 class 时对象引用本身就提供了稳定的存储位置，复制、传参、存进字段复制的都只是引用。代价是每个原子量多一次分配和一次间接访问。
- **intrinsic 方法**：`load`、`store`、`exchange`、`compareAndSet`、`compareAndExchange`；`AtomicInt`/`AtomicLong` 另有 `fetchAdd`、`fetchSub`、`fetchAnd`、`fetchOr`、`fetchXor`；`AtomicBoolean` 另有 `fetchAnd`、`fetchOr`、`fetchXor`。全部是 safe 调用，直接降低为 LLVM 原子指令。
- **普通方法**：`getAndUpdate`、`updateAndGet` 等 CAS 循环形式的便利方法，在同一个类中写成普通 Scoop 方法体，与 `Ptr<T>.equals` 的做法相同，不需要 intrinsic。
- **内存序（D4 已定）**：首版全部开放 `Relaxed`、`Acquire`、`Release`、`AcqRel`、`SeqCst`，以编译期常量参数 `order: MemoryOrder = MemoryOrder.SeqCst` 表达。CAS 使用 `successOrder` 和 `failureOrder`，两者缺省均为 SeqCst；不会根据成功序暗中改写失败序。合法组合遵循 C11，完整方法签名及合法失败序表见语言规范 11.14；非法组合和非编译期常量配 negative fixture。
- **返回与比较**：exchange/fetch 返回旧值，compareAndSet 返回是否成功，compareAndExchange 返回观察到的旧值。CAS 为 strong，AtomicRef 按对象身份比较 expected，不调用用户 equals；整数 fetchAdd/fetchSub 按位宽 wrapping。
- **约束**：这四个类型不能被继承，不能在源码中访问其值字段，值字段只能经上述方法读写，以保证同一位置不会同时有原子与非原子访问。它们不实现 `ToString`、`Hash`，也没有 `equals`；需要比较时比较 `load()` 的结果。
- **无符号与其他宽度**：`UInt`、`ULong` 等由库在 `AtomicInt`/`AtomicLong` 上做按位转换包装；8/16 位原子量等出现实际需求再加。
- **`Ptr<T>` 上的原子操作**：M33 不提供；用于与 C 共享 native 存储的接口，等平台库出现具体需求后再增加，不影响本节四种原子对象的实现。

### 3.3 IR 与代码生成

- HIR/MIR 为原子操作使用独立的 typed intrinsic，携带操作种类、值类型和内存序，不复用普通字段读写节点，也不以 `Option` 字段区分原子与非原子访问。
- LIR 使用专门的指令：`AtomicLoad`、`AtomicStore`、`AtomicRmw`、`AtomicCmpXchg`，携带对象 base、值字段的字节偏移、值类型和内存序。
- codegen 降低为 LLVM 的 `load atomic`、`store atomic`、`atomicrmw`、`cmpxchg`，对象地址按现有 HeapLoad/HeapStore 的方式计算。Boolean 按 i8 原子操作处理。
- 普通字段、数组元素、全局存储和聚合值复制仍按普通非原子访问生成；不为竞争场景改用 `unordered` 原子 load/store，也不增加单字或逐字段不撕裂的实现要求。runtime/GC 自身的同步操作仍按各自协议实现。
- `AtomicRef` 的特殊处理：
  - **技术验证**：M33-8a 的最小 IR 已验证 `addrspace(1)` 指针上的 `load atomic`/`store atomic`/`xchg`/`cmpxchg`，覆盖全部合法序、RewriteStatepointsForGC、GC verifier 与三个目标的 codegen，实际记录见 [ACCEPTANCE.md](ACCEPTANCE.md)。语言 API、写屏障与真实 Scoop moving GC fixture 仍须单独完成，不能用这项编译验证代替运行验收。
  - `store`、`exchange` 和成功的 `compareAndSet`/`compareAndExchange` 写入引用后，按普通引用存储生成写屏障（标记卡表）；多 mutator 下卡表标记已是原子操作。CAS 失败时没有写入，不需要屏障。
  - 从计算对象地址到原子指令完成，以及引用写入到写屏障完成之间不能插入 safepoint；base、expected/new 引用和读取结果跨后续 GC 时须正常保活并重定位。这些要求纳入 M33 的 GC 验证。
- M31 的 release 优化：确认首批 IR pass 不会合并或删除原子操作，也不会把非原子访问移过 Acquire/Release 边界（LLVM 本身保证，用 golden 和运行测试锁定）。

### 3.4 `Atomic<T : value>`

任意值类型无法直接对应到 LLVM 原子指令，由普通 Scoop 代码组合实现，放在 core 或平台库都可以，不需要编译器支持：

```scoop
public class Atomic<T : value>(initial: T) {
    private val lock = AtomicBoolean(false)
    private var value: T = initial
    // Copy the stored value only while holding the lock.
}
```

- `lock` 用 `compareAndSet(false, true, successOrder = MemoryOrder.Acquire, failureOrder = MemoryOrder.Relaxed)` 加锁、`store(false, MemoryOrder.Release)` 解锁，所有对 `value` 的冲突访问都在同一锁内完成，通过 happens-before 避免 3.1 定义的数据竞争。只传 Acquire 而省略失败序会得到非法的 Acquire/SeqCst 组合。
- 该方案适用于任意值类型，包括普通 struct、tuple 和 tagged enum：读写都在锁内整体完成，读取方不会观察到撕裂或不同次写入的混合值。
- 临界区只做值复制，复制是 memcpy，中间没有 safepoint；持锁线程不会因 GC 停在临界区内。
- 自旋循环写在 Scoop 代码中，循环回边自带 safepoint poll，自旋的线程不会阻塞 GC。自旋若干次后调用 `sched_yield`，避免持锁线程被 OS 调度走时空转。
- 读也必须加锁，不能用 seqlock 优化：seqlock 的读者需要无锁地先读一遍数据，这正是 3.1 规定为未定义行为的竞争读。
- 小而简单的值类型（例如 8 字节以内且不含引用）理论上可以直接用整数原子操作，但需要先有按位转换（transmute），M30 将其留到了后续，M33 不做。

## 4. 线程生命周期与 NativeSafe 切换

### 4.1 thread spawn 由库实现

M13 的设计已经覆盖了需要的机制，runtime 不新增 spawn 入口：

- 库的 C 源码调用 `pthread_create`，线程入口调用 OneShot `foreignCallback` 的 trampoline；invocation 结束即 detach。
- join 侧先 retain observer ownership，`pthread_join` 返回后用 `foreignCallbackFailure` 取出异常并重新抛出，在 `finally` 中 release。
- 子线程使用注册时的 Context 快照，这一点写入平台库文档即可。
- 库不得提供 `pthread_exit`。在 managed frame 中途退出线程会让 registry 残留该线程，之后的 GC 会永远等待。

M33 需要补的只有运行时规范中的两条说明：每个 OS 线程只需 attach 一次；以及“线程在 callback 中途退出属于 ABI 错误”。

### 4.2 C FFI 调用模式与 collector 停顿

保留现有单 collector、合作式 safepoint 与 STW 移动 GC，按已注册的 OS 线程协调停顿。未标注 `@GCLeaf` 的普通 managed C ABI 调用仍使用 NativeSafe，常见进入和返回路径取消全局 world/registry 锁及 condvar 广播；线程通过 TLS 取得已有 thread state，FFI 往返不重新 attach 或增删 registry。显式 `@GCLeaf` 使用 4.2.1 的无状态切换模式。

| C FFI 声明 | caller 行为 | 对 GC 的影响 |
| --- | --- | --- |
| 普通 `@Extern(abi = "c")` | 发布冻结根，进入 NativeSafe，返回时握手 | collector 可在 C 调用期间收集；适用于可能阻塞或回调的调用 |
| 同时标注 `@GCLeaf` | 保持原线程状态，不增加本次调用的 transition 或 roots | 从活动 MANAGED 调用时，collector 等该线程之后到达正常 safepoint；适用于短小、无阻塞等待、无 Scoop 回调的调用 |

collector 的等待采用带超时的条件变量及停稳谓词复查，参考 [Go 的 STW 等待](https://github.com/golang/go/blob/go1.26.1/src/runtime/proc.go#L1628) 中释放调度器锁后等待通知、超时重试的做法。Scoop 的超时参数按自身测量选择；Scoop 的 minor/full GC 均可移动对象，因此仍须完整保留根冻结、GC 回写和返回后重读引用的契约。

**内部状态与停稳判定**

新增 `NATIVE_SAFE_RETURNING`，表示 NativeSafe 返回握手尚未完成。将非原子的 `managed_segment` 并入 mode，新增 `MANAGED_PENDING` 表示 gateway 的新 managed 段尚未激活；普通 `MANAGED` 表示活动 managed 段。park 的来源状态保留 pending 区别，供停稳后的 scanner 使用。

collector 对其他线程的停稳判定只读取原子 mode 和 epoch，不读取尚可能变化的 anchor、根链、transition 或其他普通字段：

| mode | 本轮是否停稳 |
| --- | --- |
| `NATIVE_SAFE` | 是，已发布的栈段与根被冻结 |
| `MANAGED_PENDING` | 是，新 managed 段为空，已有根被冻结 |
| `PARKED` | 仅当 `observed_gc_epoch` 等于本轮 epoch 时是 |
| `MANAGED` | 否，等待有效 safepoint |
| `NATIVE_BORROWED` | 否，仍可能使用 direct ref，必须 park 并确认本轮 epoch |
| `NATIVE_SAFE_RETURNING` | 否，返回检查可能已通过，继续等待 |

`MANAGED_PENDING → MANAGED` 的激活仍在 world lock 内进行，并要求 phase 为 `RUNNING`。`PARKED` 的 `parked_from` 在发布停稳状态前写好，scanner 由它区分活动 managed、pending 与 NativeBorrowed 的根形态。新增状态只属于 runtime 内部；已有公开状态码通过明确映射保持不变。

**进入 native（MANAGED → NATIVE_SAFE）**

1. 完整写入 transition record、caller roots 及冻结栈段的 PC/SP/FP 和边界。
2. 以 seq_cst 将 mode 写为 `NATIVE_SAFE`，发布可扫描状态。
3. 直接执行 C 调用；进入侧不为发送通知而读取 phase、取得 world lock 或广播。collector 会从状态检查、其他必要通知或超时复查中观察到本次转换。

发布后直到通过返回握手或有锁 managed 入口的 `RUNNING` 检查前，线程不得修改或撤销冻结的 transition、栈段、根链及分配状态，也不得提前读取 collector 可能正在回写的引用槽。native 对已固定存储的合法访问仍遵守既有 FFI/pin 契约。

**从 native 返回（NATIVE_SAFE → MANAGED）**

1. 以 seq_cst 将 mode 写为 `NATIVE_SAFE_RETURNING`。
2. 以 seq_cst 读取 world phase。
3. 若是 `RUNNING`，恢复 transition 中保存的 managed 状态并更新本线程的 epoch 记录，以 release 发布 `MANAGED`，按既有 LIFO 顺序完成 transition 清理；调用者按根协议重读引用后再撤销相应 root frame。此路径不取 world lock，也不广播。
4. 若不是 `RUNNING`，以 seq_cst 将 mode 写回 `NATIVE_SAFE`，保持根冻结；取得 world lock，在 condvar 循环中等待 `RUNNING`。释放锁后重新执行第 1 步，不能凭一次唤醒直接恢复 managed。

**collector 发起、等待与结束**

1. 在 world lock 内检查 phase。仅在 `RUNNING` 时取得 collector 独占权；已有 collection 时沿现有 park 路径协调。递增 GC epoch，以 seq_cst 发布 `STOPPING`，发布自身的 collector 状态与根，并广播 GC 开始通知。
2. 在 world lock 内检查其余注册线程的原子 mode/epoch。尚未全部停稳时，调用带有限超时的 condvar wait：等待操作原子地释放 world lock，返回前重新获取该锁。通知、超时及虚假唤醒后都重新检查停稳条件。collector 的 mode 读取使用 seq_cst，与 NativeSafe 返回握手配对。
3. 等待期间保留 collector 独占权，但不持有 heap lock、roots lock；world lock 必须释放，使目标线程能够取得它并 park。超时使未发送通知的 NativeSafe 转换最终被复查发现；不再要求纯自旋、yield、sleep 的等待循环。
4. 全部目标停稳后，在 world lock 内发布 `COLLECTING`，释放 world lock，再取得既有 heap/roots 锁，执行精确根扫描、对象移动与根回写。GC 开始使用其他线程的普通字段前，必须已完成本轮停稳判定。
5. 完成全部引用、根及分配状态更新，释放 heap/roots 锁后，在 world lock 内恢复 collector 自身状态，再以 release 发布 `RUNNING` 并广播，最后释放 world lock。collector 独占由 world lock 保护的 phase 转换保证；删除原先多余的 collector mutex，避免与 world lock 形成反向取锁关系。

attach、detach、shutdown 和 registry 成员变更继续在 world lock 内通过 `RUNNING` 检查。从发布 `STOPPING` 到恢复 `RUNNING`，注册表成员、链结构和已注册 thread state 的生命周期保持稳定，可复用既有 registry，不新增快照或引用计数。线程一旦满足本轮停稳条件，其可扫描数据持续冻结到恢复 `RUNNING`。

**必须保留的通知与 epoch 确认**

- 保留 GC 开始、park 确认、GC 结束，以及初始化完成/失败的必要通知；只取消 NativeSafe 常见进入/返回路径不再需要的广播。
- 初始化等待者可能在 world 为 `RUNNING` 时已处于 `PARKED`。新 GC 的开始广播必须唤醒它，使其确认新 epoch；否则 collector 可能一直等待旧 epoch 的线程，而负责初始化的线程又已被 GC 停住。
- 上一轮 GC 的 parker 尚未恢复、下一轮 GC 已开始时，等待循环必须确认新 epoch。初始化等待者只有在初始化已有结果且 world 为 `RUNNING` 时才可恢复。
- `parked_from`、anchor 和根链在一次 park 区间开始时发布并保持冻结。虚假唤醒或确认下一轮 epoch 只更新必要的原子确认状态，不重写 collector 正在使用的普通字段。

**无锁握手的正确性**

参与双方相互检查的四个操作必须明确为 seq_cst：返回线程的“写 RETURNING → 读 phase”，collector 的“写 STOPPING → 读 mode”。不能仅在返回侧增加屏障、仍沿用 collector 原来的 release/acquire 组合。

- 若 collector 先观察到 `NATIVE_SAFE`，线程随后进入 `RETURNING`，则在本轮恢复 `RUNNING` 前，其 phase 检查不能通过，根仍被冻结。全部目标停稳后的 `scan_thread` 可将此时的 `RETURNING` 按 NativeSafe 扫描；这不改变停稳判定中 `RETURNING` 为“未停稳”的规则。
- 若返回线程先通过 `RUNNING` 检查，collector 就不能使用该次返回之前的旧 `NATIVE_SAFE` 状态来开始扫描，必须等待线程随后达到可扫描状态。
- 发布 `NATIVE_SAFE` 的 release 语义与 collector mode 读取的 acquire 语义发布冻结根；结束时写 `RUNNING` 的 release 与返回线程 phase 读取的 acquire 则发布 `current_task_context`、caller roots 等 GC 回写。seq_cst 的相互排除与这些数据发布关系都属于 runtime 内部同步，不为用户数据竞争补充保证。

**其他入口与校验**

- **NativeBorrowed**：Scoop ABI 调用的进出在 `RUNNING` 时也采用快路径；非 `RUNNING` 时复用既有 park、根登记与重读协议。不能将 NativeBorrowed 直接按 NativeSafe 计为停稳。
- **低频入口**：gateway、callback、attach/detach、shutdown 保留锁。gateway/callback 在 world lock 内完成 `RUNNING` 检查和 managed 状态转换即可，不再额外要求 RETURNING；共同要求是进入 managed 前通过当前 GC 的协调。
- **统计与 shutdown 计数**：诊断/统计中的 `RETURNING` 归入 NativeSafe；detach 仍须 attachment owner 已退出全部相关 frames，不能因该分类放宽条件。
- **校验**：进入时遍历 transition 链查重等完整性检查移到 debug runtime；release runtime 保留 O(1) 的必要边界和 LIFO 检查。停稳判定不重复遍历根或重放完整校验。

**验收与测量**

除多线程压测和 TSan 外，使用测试专用同步点控制 NativeSafe 发布、RETURNING 发布、phase 读取及扫描前后的交错；覆盖初始化等待者、连续两轮 GC、pending 激活、NativeBorrowed park、callback 和 attach/detach。结合强制 relocation 的 minor/full GC，验证恢复后使用更新的引用，且无漏唤醒或停顿死锁。

改动前后记录单线程连续空 C 调用的单次耗时、1/2/4/8 线程吞吐，以及 GC 压力下 `STOPPING → 全部线程停稳` 的等待耗时和 `STOPPING → RUNNING` 的完整停顿耗时。现有 `collector.c` 的 pause 计时从 `begin_collection()` 成功返回后才开始，不能代替后两项测量。性能记录给出线程数、GC 压力、等待超时参数以及耗时分布和最大值；超时参数保持为 runtime 实现细节，不照搬 Go 的 100 微秒。

如果单线程的函数调用本身就是主要开销，把“状态切换内联进生成代码”作为后续独立项，不在 M33 内做。

#### 4.2.1 `@GCLeaf`：小型 C FFI 的无切换调用

```scoop
@GCLeaf
@Extern(abi = "c", name = "native_max")
fun nativeMax(a: Int, b: Int): Int
```

GC-leaf 表示整个同步调用链不进入 Scoop GC 或 safepoint。`@GCLeaf` 再把短时、无阻塞等待的要求作为选择这种 FFI 模式的契约。普通 C ABI callee 本身不直接接收 managed ref 或使用 Scoop GC，但完整调用链仍可能阻塞或经独立 trampoline 回调，不自动满足无切换调用的要求。

- **适用范围**：只允许 `@Extern(abi = "c")` 函数。普通函数、Scoop ABI extern、变量和其他声明上使用均报编译错误；C ABI 原有的类型与 unsafe 调用规则不变，也不因此允许再叠加 `@NoGC`。
- **完整调用链**：不得触发 Scoop GC、执行 safepoint、park、转换 Scoop 线程状态、进入需要这些动作的 runtime API，或回调 Scoop，包括静态 FunPtr/NoGC 回调及注册 trampoline。允许调用同样满足契约的其他 C helper。
- **执行时间与等待**：不得主动执行可能阻塞的 I/O、mutex/condvar 等等待、join、sleep、无界忙等或等待其他 managed 线程推进。用于预期短时完成的调用；不设固定纳秒或指令数阈值，不插入运行时计时检查。调用越慢，GC 停稳等待可能越长。
- **调用序列**：只做必要的 C ABI 参数/返回适配和实际调用，保持原线程状态；不发射 NativeSafe/RETURNING 进出、statepoint、专用于该调用的 caller-root push/pop 或 relocation reload。从 MANAGED 调用期间 collector 仍须等待，不能把该线程当作 NativeSafe。
- **求值与数据**：显式/缺省实参和调用后的表达式照常保留 GC、异常和 safepoint 行为。注解不代表纯函数，不解除 C-FFI-safe、native unwind、pin/借用、指针有效期或用户数据同步要求；跨后续真实 safepoint 的引用仍按正常规则保活。
- **外部实现责任**：FFI 作者保证 native 实现满足调用契约。违反无回调、无运行时重入或无阻塞等待等要求属于 unsafe FFI 契约违例，不保证安全或必有诊断；不要求编译器证明任意 C/C++ 正文的行为。

编译器在 C extern 声明上保存完整的 NativeSafe/GcLeaf 模式，默认填入 NativeSafe，跨 HIR/MIR/LIR、导出接口和泛型实例化保留。LIR 以 NativeGcLeaf 表示独立 C 调用分支，保留完整 C ABI 调用计划；不把它伪装成普通 Scoop NoGc ABI，也不把它变成新的 runtime 线程状态。物理调用按 4.2.2 选择 DirectC 或 StorageBridge，不因注解增加 readnone/readonly/nosync 等属性。

此模式是当前 Scoop 声明的 caller 协议，进入相关声明、调用 metadata 及构建缓存，不改变 native symbol 的物理 ABI。同一 C symbol 可以由不同 Scoop 声明分别按普通或 GCLeaf 模式调用；既有签名冲突检查仍有效，链接去重不得把一种模式传播到另一种声明。

`@GCLeaf` 可与第 5 节的 `captureErrno = true` 组合；M33 对该组合保留立即捕获 errno 的 StorageBridge，因此仍有桥接与捕获成本，但没有 GC 边界操作。libc errno 的清零、快照及 `(R, Int)` 结果传递只使用本次调用的 native 局部值与 caller-owned storage，不访问 Scoop TLS/thread runtime，不引入分配、park 或状态切换。release block 中的直接 C 调用也可捕获，仍按第 5 节及语言规范 9.1.6 满足既有 ReleaseValue 与 raw leaf 限制。

验收比较同一 native callee 的普通 NativeSafe、GCLeaf 和直接 C caller 基准，分别记录单线程延迟及 1/2/4/8 线程吞吐；GC 压力下同时测停稳等待。结构验收检查 GCLeaf 没有额外 transition/root/safepoint，并按 4.2.2 检查 DirectC 没有桥接调用或桥接缓冲区；行为验收覆盖实际副作用、跨 Cone 与泛型调用、GC 前后存活引用以及不被注解抹去的参数求值。

#### 4.2.2 标量 C ABI 直接调用

性能目标：`@GCLeaf` 不引入 GC 边界开销；对本节支持的 DirectC 签名，不引入额外桥接调用或由桥接协议导致的参数/结果内存往返。ABI 必需的参数搬运、栈传参、正常寄存器 spill 和动态链接跳板仍按普通 C 外部调用处理；显式 errno 捕获及 pin/unpin 等操作保留自身成本。

M33 在当前三个 target 上，为固定参数的 cdecl C extern 提供 DirectC。选择只依赖 canonical C signature、目标 ABI 和声明的附加调用行为，不增加源码注解或开关。它与 NativeSafe/GcLeaf 调用协议独立：

| C extern 条件 | 物理路径 | GC 协议 |
| --- | --- | --- |
| 所有参数和非 void 结果均具有标量 C 表示，且 `captureErrno = false` | DirectC，直接调用 native symbol | 普通声明仍使用 NativeSafe；`@GCLeaf` 保持原线程状态 |
| 含按值 C-layout struct 参数或结果，且 `captureErrno = false` | StorageBridge，由目标 C compiler 分类并适配 | 同上；GCLeaf 仍可省去 GC 边界操作 |
| `captureErrno = true` | 带立即捕获的 StorageBridge | 同上；捕获操作还须满足 GCLeaf 契约 |

这里的标量 C 表示包括定宽整数、Boolean、Float/Double、data pointer 和 code pointer，Unit 只允许作为 void 结果。判断采用已有 C ABI projection：Char 的 UInt32 表示、PinnedPtr/GcHandle 的 UInt64 表示及合法 nullable pointer 都纳入；保留原 exact type 和必要的表示转换，不改变这些类型的 Scoop ABI。指向 C-layout struct 的指针仍是标量；普通按值 C-layout struct 即使只有一个字段也继续走 bridge。参数/结果中的函数指针可以直接传递，本节不扩展 FunPtr 调用、callback 或 native global/TLS 的入口协议。

实现要求：

1. **确定真实 C ABI**：LIR lowering 依据 canonical C signature 和目标 profile 构造完整的 DirectC/StorageBridge 计划。DirectC 包含物理参数/结果类型、calling convention、小整数符号/零扩展及 Boolean 表示转换；codegen 发射准确的 LLVM 类型和 ABI 属性，由目标后端分配寄存器与栈位置。不能套用 Scoop typed ABI。
2. **直接传值并取得返回值**：DirectC 不在 LIR 中创建 bridge 专用参数局部变量、返回缓冲区或 memcpy 往返，codegen 直接调用 native symbol。保留显式/缺省实参的求值顺序、副作用、异常和真实 safepoint；不能为省开销删除正常的源码值转换。
3. **按实际用途生成对象**：DirectC 的调用 relocation 直接引用 native symbol，继续保留相同的 native contract 与 library requirement。只为实际 StorageBridge 调用生成相应 outbound bridge recipe、定义与 member；callback 等独立桥接用途照常保留。调用计划通过已有 LIR/metadata、指纹和缓存传递，跨 Cone、泛型与 artifact-only 链接均不得重新插入 bridge。
4. **限定 M33 范围**：按值 C-layout struct 继续复用现有 bridge，包括混合整数/浮点、packed、同质浮点聚合体及大结构体传参/返回。完整 aggregate 直接 ABI lowering 留待后续，不能仅设置 LLVM C calling convention 就跳过分类。标量路径不依赖 bridge 的 `inline`、跨对象 LTO 或导入 Clang bitcode；当前独立编译 bridge 的流程继续用于保留路径。

验收在三个 target 的 debug/release 下验证 ABI 与行为，覆盖有符号/无符号窄整数的参数和返回、Boolean、Float/Double、整数/浮点混合及超出寄存器参数数量的签名、合法 null 指针、Char 和已有透明 C 表示，并回归 aggregate bridge。优化后的机器码须确认 DirectC 没有 bridge 调用及桥接专用存储，GCLeaf 还须没有 GC 边界操作；同时验证普通 NativeSafe 使用 DirectC 时保留协调与根发布。

基准使用同一 native callee，并保持 target、优化级别、链接方式及被测操作一致；禁止仅在 C 基准中内联 callee、将其替换成 builtin 或消除调用。未捕获 errno 的 DirectC、aggregate bridge 和带 errno 的 bridge 分别报告；不把可选行为的成本归为 GCLeaf transition 成本，也不承诺不同 caller 的整段机器码或测量耗时逐项相同。

### 4.3 程序退出时的线程与 token

D3 已确定保持现行的资源约束，改进诊断和退出方式；M33 不提供 daemon 线程。

- main 正常返回后，按既有 shutdown 协议拒绝新的 attach 和 callback registration，并检查已 attach 的非主线程、活动 callback 和未释放的 token ownership。全部清空后才销毁 GC 状态并使用 main 返回码。
- 有遗留项时，在既有同步协议下取得剩余非主线程数、活动 callback 数以及有未释放 ownership 的 token 数，释放相关锁后报告诊断、刷新 stdout/stderr，再 `_exit(1)`。这是 shutdown 失败，覆盖 Unit/Int main 的原返回码，不使用 `abort()`。
- 失败路径不等待 join、不销毁其他线程仍可能访问的 GC 状态；刷新使用 NativeSafe，允许等待输出完成。它不保证等待遗留线程自行结束后转为成功。
- 后续平台库用结构化作用域／join 和 token 释放完成正常生命周期；公开 API 随平台库设计确定。需要带着后台线程结束进程时，显式调用 2.2 的 `exit(code)`。

## 5. errno 捕获

errno 是本次调用返回值的一部分：生成的 bridge 在原调用返回后、状态切换之前取得快照，再与 native 结果一起按值交给 Scoop。接口参考 [Go cgo 的双结果调用](https://pkg.go.dev/cmd/cgo)；[Java FFM 的 captureCallState](https://docs.oracle.com/en/java/javase/25/docs/api/java.base/java/lang/foreign/Linker.Option.html)同样及时取快照，但使用调用者提供的 MemorySegment。Scoop 采用显式声明 tuple 结果的方式，不提供线程级 `lastErrno()` 或 Scoop errno 槽位。

### 5.1 源码接口与真实 C 签名

`@Extern` 新增编译期常量参数 `captureErrno: Boolean = false`。仅 C ABI extern 函数可启用；Scoop ABI extern 或 extern 变量设置为 `true` 是编译错误。

```scoop
@Extern(name = "close", captureErrno = true)
fun closeWithErrno(fd: Int): (Int, Int)

@Extern(name = "close")
fun closeRaw(fd: Int): Int

@Unsafe
fun closeErrorCode(fd: Int): Int {
    val (result, error) = closeWithErrno(fd)
    if (result == -1) return error
    return 0
}
```

- 启用捕获时，源码返回类型必须在透明 alias 展开后为二元 tuple `(R, Int)`；`R` 是实际 C 返回类型在 Scoop 中的表示，第二项为 `Int32` errno。`R` 遵守既有 C-FFI-safe 返回规则，包括 scalar、pointer 和合法 C-layout struct。`R = Unit` 对应 C `void`，Scoop 返回 `((), capturedErrno)`。
- 目标 C 函数的参数不变，返回值只取 `R`；外层 tuple 由 Scoop 调用适配器产生，不作为 C struct 返回，也不使一般 tuple 成为 C-FFI-safe。上例两个声明都对应 `int close(int)`。不开启捕获时继续直接声明、返回 `R`。
- 调用表达式与泛型消费均保留声明的固定 Scoop 类型；不按接收变量数量或 expected type 选择模式。函数引用遵守既有 unsafe 规则：unsafe extern 不能直接存入 managed function type，安全包装函数的引用保留完整 tuple 返回类型。解构时丢弃 errno 或忽略整个结果，不关闭该声明要求的捕获。
- 错误元数、第二项不是 `Int`、`R` 不满足 C 返回约束、含 managed reference 或违反既有 extern 声明规则时，在前端报告错误。普通 C extern 返回 tuple 不会隐式启用捕获。
- errno 只提供原始错误值，不自动抛异常或创建 error 对象。先按该 C API 的返回值规则判断成功；成功时 errno 仍可能非零，失败时也不自动补造错误码。C 函数内部的清理或回调若覆盖 errno，由该 C 实现维护其返回契约，FFI 只捕获函数最终返回时的状态。

### 5.2 桥接与捕获顺序

M33 的捕获调用保留 StorageBridge，不使用 4.2.2 的 DirectC。沿用原有 native 结果缓冲区写回非 void 的 `R`，bridge 自身改为以 C `int32_t` 返回捕获的 errno；Scoop 侧按 LIR 已确定的布局重建 `(R, Int)`。这只是 bridge 私有 ABI，真实 C 函数仍保持原签名。`R = Unit` 时省略原结果缓冲区，只返回 errno 并构造零 payload 的 Unit 元素。

以下是 `closeWithErrno` 的简化 generated-C bridge；实际参数存储、符号与声明由既有 C ABI 计划生成：

```c
#include <errno.h>
#include <stdint.h>
#include <unistd.h>

int32_t scoop_bridge_close_capture(void *result, const void *arg0) {
    int32_t fd;
    __builtin_memcpy(&fd, arg0, sizeof(fd));
    errno = 0;
    int32_t native_result = close(fd);
    int captured_errno = errno;
    __builtin_memcpy(result, &native_result, sizeof(native_result));
    return (int32_t)captured_errno;
}
```

- 完成 Scoop 实参求值、适用的 NativeSafe 进入，以及 bridge 参数解包后，紧接实际 C 调用之前清零 errno，避免把调用前的旧值作为本次错误返回；未开启捕获时不增加清零或捕获步骤。当前三个 target 的 C `int` 为 32 位，可精确映射为 Scoop `Int`。
- 原调用返回后，以独立语句先把 errno 读到 native 局部整数，再复制结果、执行 helper 或返回握手。不能先调用另一个函数取得错误槽位，也不能在返回 managed 后重新读取 libc errno。
- 每次调用使用独立、在 native 期间地址稳定的 GC-free 临时存储；嵌套、递归及 callback 内的新调用不能覆盖尚在使用的外层结果。捕获的 errno 作为普通整数返回，不创建堆对象、共享缓冲区或 Scoop TLS 状态。
- codegen 使用目标工具链的 `<errno.h>` 及 native 结果布局，保留因此产生的 libc 引用与链接需求；不硬编码宿主 errno accessor，不添加公开 runtime setter/getter。

### 5.3 GC、release 与产物

- 普通 NativeSafe 的捕获发生在返回握手之前，结果在握手后按正常规则使用。`@GCLeaf` 保持原线程状态，libc errno 的清零、读取及结果传递不增加 Scoop allocation、safepoint、root、park 或线程切换；桥接和捕获仍有自身成本。
- release block 可在显式 unsafe 上下文中直接调用捕获 extern，并使用或丢弃返回的 tuple；全部参数及完整结果仍须满足 `ReleaseValue`，native 实现仍须遵守 raw leaf 约束。bridge 不访问 Scoop TLS/thread runtime，collector 无需保存/恢复其他调用的错误值。不放宽一般 TLS 访问、Scoop helper 推导、回调或 managed 操作限制。
- `(R, Int)` 是普通值。已取得的结果不会被后续分配、GC、release 或另一捕获调用自动改写；跨协程挂起时由普通 frame 保存，恢复到其他 OS 线程也不改变其中的 errno。libc errno 本身之后仍可变化，返回值中 raw pointer 等成分的生命周期继续按原 FFI 契约处理。
- HIR 保存完整 Scoop 签名、native 返回投影与捕获选项，MIR 传递固定结果类型，LIR 保存 native `R` storage、bridge 的整数返回及 tuple 重建。相关字段进入 `.slib`、语义/Code 指纹与缓存，跨 Cone、泛型及 artifact-only 消费保留。
- `captureErrno` 属于 Scoop 声明的调用选项，不进入 SourceExtern 的 native ABI 合并键；program-link 比较投影后的真实 native 签名。普通返回 `R` 与捕获返回 `(R, Int)` 可引用同一 C symbol，真正的 library、kind、calling convention、参数或 native 结果冲突仍报错。OutboundFunction bridge recipe 的身份和复用包含结果适配，不能把同一 C symbol 的普通 bridge 与捕获 bridge 合并，也不能把捕获选项传播到另一声明。

本节已同步到语言规范 9.1.6、13.4、13.4.2、13.8，运行时规范 3.8、4.5，以及实现规范 2.2～2.6、2.8。

## 6. native 数据借用、UTF-8 与 C 字符串

### 6.1 有作用域的数据借用

core 提供以下借用 API，由编译器 intrinsic 实现，元素区偏移由编译器按实际 layout 计算，不在库中硬编码：

```scoop
@Unsafe public fun <T : value, R> MutableArray<T>.withDataPointer(block: (Ptr<T>, Long) -> R): R
@Unsafe public fun <T : value, R> Array<T>.withDataPointer(block: (Ptr<T>, Long) -> R): R
@Unsafe public fun <R> String.withUtf8Bytes(block: (Ptr<UInt8>, Long) -> R): R
```

- `T` 必须是 GC-free 的值类型，与 `Ptr<T>` 的要求一致。
- 指针只在 `block` 内有效；把它保存到 block 之外属于 unsafe 契约违规，不做额外的运行期检查。
- `Array<T>`（不可变数组）和 String 的借用指针只能读；违反同样属于 unsafe 契约。
- 借用与 pin 只保证作用域内的对象保活和地址稳定，不提供独占访问或数据同步。对同一数据的并发读写遵守 3.1，调用方必须显式同步。
- 零长度数组也会给出一个非零、可比较但不可解引用的指针，满足 C 函数“非空指针”的要求。

### 6.2 GC 侧：线程局部的 pin 帧

借用不走现有的全局 pin 列表，而是在线程状态中压入一个 pin 帧：

- pin 帧和 caller root 帧一样，按栈严格嵌套。压入和弹出只修改本线程状态，不加锁。
- collector 在 STW 期间遍历所有线程的 pin 帧：帧中的对象作为根，且不移动。nursery 中被 pin 住的对象沿用现有的 `PINNED_PARTIAL` block 处理。
- 同一个对象可以出现在多个线程、多个帧中，天然就是计数的；所有帧都弹出后，对象重新可以移动。
- block 内抛出异常时，借用经过普通的 typed cleanup 弹出帧。
- block 内未标注 `@GCLeaf` 的 C ABI 调用进入 NativeSafe，GC 可以并行进行，但被借用的对象不会移动；GCLeaf 调用按 4.2.1 保持原线程状态，借用域的 pin/保活规则仍然适用。

现有的 `pin`/`unpin`（用于跨越调用、无法按栈嵌套的场景）同时修正：

- 改为计数：同一对象 pin 两次需要 unpin 两次，计数归零才解除。
- `unpin` 改为 O(1)：pinned 表保存对象和显式计数，对象头的 GC 私有状态字保存登记索引；删除时交换末项并更新索引。登记与扫描共用已有 heap lock，不再为 pinned 表重复取得 roots lock。对象头大小与 `PinnedPtr.raw` 的含义不变。

### 6.3 严格与 lossy UTF-8 解码

D5 已确定提供三个转换入口；原有 unsafe unchecked 转换保持其调用者保证合法输入的契约：

```scoop
public fun String.Companion.fromUtf8(bytes: Array<UInt8>): String
public fun String.Companion.fromUtf8OrNone(bytes: Array<UInt8>): String?
public fun String.Companion.fromUtf8Lossy(bytes: Array<UInt8>): String
@Unsafe public fun String.Companion.fromUtf8(pointer: Ptr<UInt8>, length: Long): String
```

- **严格抛异常**：fromUtf8 对非法起始字节、孤立 continuation、截断、过长编码、代理项码点及超出 U+10FFFF 的码点抛出 CharacterCodingException；公开 `byteOffset: Long` 为首个非法子序列起始字节的零基偏移。
- **严格返回 Option**：fromUtf8OrNone 成功返回 Some(String)，编码错误返回 None；不吞掉其他异常或分配失败。
- **lossy**：fromUtf8Lossy 将所有非法 UTF-8 子序列替换为 U+FFFD。替换粒度采用语言规范 11.4 的 Unicode maximal subpart：截断的合法前缀整体替换，独立非法字节分别替换，后续合法字符继续处理。例如 `E1 80 41` 得到 `"�A"`，`F0 90 80` 得到一个 U+FFFD，`80 80`、`C0 AF` 各得到两个，`ED A0 80` 得到三个。不丢弃非法字节或将任意连续错误合成一个替换字符。
- **共同规则**：合法 UTF-8 原样保留，包括 U+0000 与原有 U+FFFD；空输入得到空串，不做 normalization。结果拥有独立存储，实际输出长度按替换后的 UTF-8 字节计算；lossy 不改变既有溢出、分配失败和数据竞争规则。argv 仍使用严格解码，不能因提供 lossy API 而静默替换启动参数。
- **pointer 重载**：fromUtf8(pointer, length) 使用相同严格规则。负长度抛 IllegalArgumentException；非空区间在整个调用期间须可读、稳定且有效，managed 区间按借用/pin 规则固定。零长度不读内存，Ptr 自身仍非零。内容受检不免除 unsafe 地址和生命周期契约。
- **后备实现**：扩充 runtime 现有 [utf8.c](../../runtime/src/utf8.c)，共用严格/lossy 的解码逻辑和错误分段；按输入和结果长度线性处理，不引入外部库。严格结果经普通 Scoop ABI 明确返回成功 String 或失败 offset，由 core 转成异常/None；复用已验证且不变的内容，不在 core 和 runtime 重复校验。分配、输入保活和结果发布见运行时规范 2.4、第 6 章。

### 6.4 String 与 C 字符串

- **String → C 字符串**：`String.withCString(block: (Ptr<Int8>) -> R): R`（unsafe），在 core 中用普通代码实现：
  - 先检查字符串内不含 U+0000，含有时抛出 `IllegalArgumentException`，避免路径被截断；
  - 把字节复制到长度加 1 的 `MutableArray<UInt8>` 并补 `\0`，再按 6.1 借用指针。
  - 短字符串是否改用栈上缓冲，等有测量数据再决定。
- **C 字符串 → String**：`String.fromCString(pointer: Ptr<Int8>): String`（unsafe），先 `strlen`，再走 6.3 的受检解码。

## 7. Cone 内的 C/C++ 源码与 native 库

### 7.1 manifest

```toml
[native]
cxx = false
include = ["native/include"]
c_flags = []
cxx_flags = []

[[native.sources]]
path = "native/fs.c"

[[native.sources]]
path = "native/process.c"

[[native.libraries]]
name = "dl"
when = { os = "linux", env = "gnu" }

[[native.libraries]]
name = "CoreFoundation"
kind = "framework"
when = { os = "darwin" }
```

- `native.sources` 始终是带 `path` 和可选 `when` 的表数组，不再与字符串数组混用；`path` 和 `include` 都是 Cone 相对路径，不能越出 Cone 根目录。
- `cxx` 是默认 `false` 的 Boolean 开关，显式声明当前 Cone 需要 C++ 支持；仅出现 C++ 后缀或 `cxx_flags` 不会隐式开启。选中 C++ 源码而未启用时，在 native 编译前报错。C++ 工具链和支持矩阵见 7.4。
- `c_flags` 和 `cxx_flags` 是 argv 字符串数组，分别作用于本 Cone 的 C 和 C++ 源码，保持声明顺序，不做 shell 展开，不向依赖者传播。允许语言标准、宏定义和常规编译选项；不能覆盖 driver 管理的 target、sysroot、输入语言、编译阶段和输出位置，也不改变 generated-C bridge 或 runtime 的编译配置。
- `native.libraries` 按第 8 节的 `when` 筛选后进入 `.slib` 的逻辑 native library requirement，沿依赖闭包传递，与 `@Extern(lib = ...)` 共用合并和冲突规则。D6 已确定允许从所选 target 的系统目录/SDK 解析，细则见 7.3；C++ 开关产生的运行库需求也进入产物。
- single-file 模式不支持 `[native]`。

### 7.2 编译

- `.c` 源码交给当前 target 的 C 编译器；启用 `cxx` 后，`.cc`、`.cpp`、`.cxx` 和 `.C` 源码交给同一工具链的配套 C++ 编译器。混合 Cone 按各文件的语言编译，不能把全部 `.c` 改按 C++ 解释。generated-C bridge、runtime 和启动 C 代码继续使用各自的 C 编译配置。
- profile 提供 debug O0、release O2 的 native 编译默认选项，并附加相应的 `c_flags`／`cxx_flags`；driver 保持 `-fPIC` 等 target 必需配置。C++ 使用独立的语言配置，不能把 `-std=c11` 等 C 专属默认参数传给 C++ driver。
- 注入 target 宏：编译器自带的宏（`__linux__`、`__APPLE__`、`__aarch64__`、`__x86_64__`）加上 Scoop 提供的 `SCOOP_TARGET_OS_*`、`SCOOP_TARGET_ARCH_*`、`SCOOP_TARGET_ENV_GNU`/`SCOOP_TARGET_ENV_MUSL`，使 glibc 与 musl 可以区分。
- native 代码可以 include 公开 FFI 头 [scoop_rt.h](../../runtime/include/scoop_rt.h)，driver 自动加上它的 include 路径；公开头必须能被 C/C++ 编译器消费，并为 C ABI 声明提供相应的 C linkage。不能 include runtime 的内部头文件。
- 每个源码文件产出一个 object，作为普通 native `LinkObject` 成员写入 `.slib`，object format 按 target 选择。C++ 编译模式、所需标准库／ABI 运行库与最终链接要求同时写入产物，不通过 object 后缀或 C++ 符号拼写反推。
- 缓存 key 包含：实际选中的 C/C++ 源码、include 到的头文件（含公开的 `scoop_rt.h`）、`cxx`、实际生效的编译参数与顺序、target、profile、C/C++ toolchain／SDK／标准库 ABI 配置及宏。头文件依赖必须在外层 Cone 缓存命中前发现或校验，不能仅在子编译器执行后才写 depfile；系统头由相应工具链／SDK 内容配置覆盖。
- **输入一致性**：按原 Cone 的相对 include 布局完成预处理和 depfile 发现，把完整 `.i`／`.ii` 字节、源码和实际头文件摘要及生效配置纳入既有构建输入快照。缓存查询和子编译器使用同一预处理内容；子编译器不再读取工作区头文件。准备期间发现输入变化时丢弃受影响结果并重新准备，不以旧 key 发布新内容的产物。行标记与内建文件名保留稳定逻辑路径和源行号，系统 SDK/toolchain 头文件的实际内容进入同一输入。已确认的不变快照与依赖结果直接复用。状态与验收任务见 10.2。

### 7.3 链接与符号

- program-link 把这些 object 当作普通的 LinkObject 消费。`@Extern(lib = "")` 的默认 namespace 已经包含“已引入的 native 对象”，所以 Cone 自己的 C 函数不需要额外的库名。
- **D6 的解析顺序**：manifest native 库与非空 Extern lib 使用同一流程，先检查全部显式 library roots；没有候选时，再由已选 target 的平台 provider 解析默认系统库。显式候选损坏、不兼容或歧义直接报错，不回退掩盖错误。Linux 使用所选 native toolchain/sysroot 的系统库目录，Darwin 使用所选 SDK 的库、framework 与 `.tbd` providers；交叉编译不加入宿主 `/usr/lib` 或其他宿主目录。
- 默认系统解析沿用 target/link mode 与库 kind 的规则，静态模式不以动态库满足需求；只满足已声明的逻辑库，不扫描任意相邻库或扩大空 lib namespace。实际 provider 内容、加载合同、系统目录配置进入现有 link plan 与缓存，`.slib` 仍只保存逻辑需求，不固化构建机路径。
- 最终程序的完整 `.slib` 闭包中，只要任一 Cone 要求 C++，就改用目标配套 C++ driver 的链接配置：GNU 使用 `g++` 与 `libstdc++`／C++ ABI 运行库，Darwin 使用 `clang++` 与 `libc++`／C++ ABI 运行库。root 没有 C++ 源码或 `cxx = false` 也不能取消该需求。生成的启动 C 代码先由 C 编译器生成 object，再交给最终链接。
- C++ 链接配置负责标准库、ABI 运行库、系统依赖及 unwind provider 的一致选择；不能只把命令名改成 `g++` 后照搬抑制默认运行库的 C 链接参数。GNU C++ 使用配套 `libgcc_s` 为 Scoop Level I 和 C++ 共用的异常展开提供实现，不额外再链接 LLVM unwind；Darwin 使用所选系统 unwind provider。C++ driver 仍调用目标平台的 linker，现有 GC、EH、对象格式和 image 要求继续适用。
- C++ 一侧通过 `extern "C"` 包装函数暴露 Scoop 的 C ABI 入口，异常在 native 一侧处理后再返回。标准 native C++ 初始化／析构由 CRT／C++ 运行库负责，runtime 建立前或 shutdown 后不得进入 Scoop；不注册为 Scoop 初始化或析构。
- C 符号是全局的。不同 Cone 定义了同名的非 static 符号时，program-link 报告冲突并指出两个来源。推荐的命名约定是以 Cone 名作为前缀，但不强制。
- 每个 Cone `LinkObject` 按现有 canonical Cone/member 顺序消费一次；普通 native archive 仍按实际 undefined references 选择成员，不新增 whole-archive 之类的选项。

### 7.4 C++ 开关与暂不支持 musl 的说明

需要 C++ 的 Cone 显式配置，例如：

```toml
[native]
cxx = true
cxx_flags = ["-std=c++20"]

[[native.sources]]
path = "native/bridge.cpp"
```

| target | C++ 编译与链接 driver | C++ 支持 |
| --- | --- | --- |
| Darwin aarch64 | 所选 Xcode 的 `clang++` | 支持，使用配套 libc++ 与 C++ ABI 运行库 |
| Linux x86_64 GNU | 所选 GNU 工具链的配套 `g++` | 支持，使用配套 libstdc++ 与 C++ ABI 运行库 |
| Linux x86_64 musl | — | 暂不支持，静态和动态模式都拒绝 |

**暂不支持 C++ + musl**：当前 Scoop musl 工具链只配套了 libc 等既有 C/runtime 输入，尚未配套面向该 target 的 C++ 标准库与 C++ ABI 运行库；musl 本身只提供 libc，不包含 C++ 库。这是当前工具链集成范围的限制，并非 musl 原理上不能支持 C++。

- 当前 Cone 在 musl 下启用 `cxx` 时，在 native 编译前直接报告不支持；不能等到最后才报一串 C++ undefined symbols。
- artifact-only program-link 同样检查依赖闭包中的 C++ requirement，在调用最终 linker 前诊断，并指出要求 C++ 的 Cone。只有预编译产物也不能绕过限制。
- 不回退到宿主 `g++`、glibc 的 libstdc++ 或其他不匹配的 C++ 运行库；即使额外提供库路径，M33 也不开放该组合。纯 Scoop／C 的 musl 构建继续使用既有配置。

## 8. 按平台选择源码

没有顶层 `sources` 字段时，保留递归扫描 `src/` 的默认行为；出现该字段后，它就是完整的 Scoop 源码清单，不再追加默认扫描。公共目录也必须显式列出，例如：

```toml
[[sources]]
path = "src/common"

[[sources]]
path = "src/os/linux"
when = { os = "linux" }

[[sources]]
path = "src/os/darwin"
when = { os = "darwin" }

[[native.sources]]
path = "native/linux_epoll.c"
when = { os = "linux" }
```

- **默认与显式模式互斥**：`[[sources]]` 每项具有必需的 `path` 和可选的 `when`，省略 `when` 表示无条件。只配置 `native.sources` 不改变 Scoop 的默认扫描；显式 `sources = []`、所有条件都不匹配或选中目录没有源码时，不回退到 `src/`，最终源码集合为空就是构建错误。
- **目录与文件**：`path` 是 Cone 相对目录或单个 `.scoop` regular file，不是 glob，也不限于 `src/`。目录递归收集扩展名精确为 `.scoop` 的 regular files。只能选择整个目录或整个文件，不支持文件内的 `#if`、声明级的 `@Target` 标注，或 `if (Target.os == ...)` 这类编译期分支。
- **谓词是封闭集合**：`os ∈ {darwin, linux}`、`arch ∈ {aarch64, x86_64}`、`env ∈ {gnu, musl, none}`，值的列表从现有 target 定义中取得，按构建 target 求值。一个 `when` 中的多个键是“与”，同一个键的值可以是字符串数组，表示“或”。未知字段、键、值或错误的数据类型是 manifest 错误；新增 target 时扩展这个列表。该谓词也用于 `native.sources` 和 `native.libraries`，不扩展到 Cone dependency。
- **筛选先于文件检查**：先检查全部条目的字段、路径形式及 `when`，再按 target 筛选；仅对选中路径检查文件系统存在性、枚举源码并进行解析和语义检查。未被任何匹配条目选中的平台文件不参与编译，不会被默认扫描补入。选中路径不存在、类型不符、非 UTF-8、symlink 逃出 Cone 根或循环等错误在 parse 前报告。
- **不允许重叠或重复**：同一 target 下选中的目录不能相同或互相包含，单文件不能同时被选中目录覆盖；规范化路径重复、或 symlink 解析到同一源文件的重复选择，均报错并指出相关条目，不静默去重，不定义先后覆盖。互斥条件在不同 target 选中同一路径是合法的；无条件选择整个 `src/` 再选择其平台子目录则是重叠错误。
- **native 选择**：`[[native.sources]]` 选择 C/C++ 源码，`[[native.libraries]]` 选择 native 库；先按 target 筛选，再检查选中 native 源码的 C++ 开关。`cxx = true` 对整个当前 Cone 声明 C++ 需求，不能因该 target 未选中 `.cpp` 就撤销 musl 限制。
- **identity 与缓存**：最终源码按完整的 normalized Cone-relative `/` path 的 UTF-8 byte order 排序。source identity 仍是 `(ConeIdentity, Cone 相对路径)`；file-private 实体与 SourceLocation 保留从 Cone 根开始的完整路径，不截去源码根前缀，不因默认/显式模式、清单顺序或根分组改变同一文件的身份。缓存命中判断前完成选择与枚举；manifest 语义投影、target、选中文件的规范化路径及内容进入既有 compile key，未选中文件的内容不作为源码输入。
- **编译入口一致**：`scoop` 与直接调用 `scoopc` 使用相同规则，parser 接收当前 target 的完整选中集合，后续阶段不再补扫目录。single-file 模式继续只编译指定文件，不读取相邻清单。
- **诊断**：同一 target 下被选中的两个文件中出现重名声明，按普通重名规则报错。不同 target 之间的公开 API 是否一致，不做额外检查。

本节已同步到语言规范 12.2.2 和实现规范 2.1、2.7；语言规范第 15 章对 `expect`/`actual` 的排除理由已改为“平台差异由 native 层和按 target 的源码选择承担”。

## 9. 依赖 Cone 的定位与 core 契约

### 9.1 sysroot 中的库

目前只有 core 能从 sysroot 自动定位。M33 让 sysroot 中的其他库（scoop.json，以及以后的平台库）也能在不传 `--cone-path` 的情况下被找到：

- **D7 已定，只适用于 `scoop` group**：除 core 沿用语言规范 12.6 的默认定位规则外，其他已声明的 `scoop` 依赖既没有显式 locator，也没有在任何显式 artifact search root 中找到候选时，最后检查 `<sysroot>/lib/<name>/Cone.toml`；其 coordinate 必须与依赖的 key 和 version 完全相等，且为 library。
- 找到之后，按普通的 path 依赖构建，进入普通缓存，不享有任何特殊身份；显式 locator 解析失败时仍按普通依赖报错，不回退到 sysroot。
- 其他 group 不作默认 sysroot 查找；显式候选损坏、歧义或不兼容不触发回退。默认定位不注入依赖、不扩大可见性；artifact-only 链接仍只消费已有 `.slib`，不通过 sysroot 源码补建依赖。

### 9.2 `Equality<T>` 与泛型相等能力

core 用普通 `Equality<T>` 接口统一表达 `==` / `!=` 所需的值相等能力：

```scoop
public interface Equality<T> {
    public operator fun equals(other: T): Boolean
}

public fun <T : Equality<T>> same(left: T, right: T): Boolean = left == right

struct Point(val x: Int, val y: Int)

fun pointExample(): Boolean = same(Point(1, 2), Point(1, 2))
```

`T` 表示可比较的右操作数类型，不是隐式 Self。相同类型比较写 `T : Equality<T>`；这一形式使用既有的泛型上界规则，现有 `m23-generic-equality` fixture 已包含 `T : EqualTo<T>`。不需要新增相等性专用 bound 或运行期 dictionary。平台库的 `HashMap<K : Hash, V>` 另写 `where K : Equality<K>`，同时要求两项独立能力。

`lhs == rhs` 只从 lhs 静态类型实现／继承的实际 `Equality<R>` 或其 bounds 中选取 equals slot，沿普通参数适配与成员重载规则选择唯一目标；`!=` 对同一调用的结果取反。两侧各求值一次，不交换顺序。普通同名函数、extension 和用户自己声明的同名接口不构成该契约；用户接口可以显式继承 core `Equality<T>`。`===` / `!==` 继续表示引用 identity。

接口实现分为以下几种：

| 类型／来源 | Equality 实现方式 |
| --- | --- |
| String、Boolean、Char、八种定宽整数、Float/Double、`Ptr<T>` | core 显式实现各自同类型的 Equality，现有 equals 改为合法 override；alias 保持同一 conformance |
| 普通 struct、enum、tuple | 全部字段／payload 可比较时，同时派生 `Equality<完整宿主类型>` 与结构比较正文；如上例 Point 可直接满足 bound |
| Unit | 无条件实现 `Equality<Unit>`，结果恒为 true |
| 手写相等方法的类型 | 显式实现所需 `Equality<R>` 并提供 `public override operator fun equals(other: R): Boolean`；该方法优先于同签名派生体 |
| class/object | 通过普通声明显式实现或继承 Equality，不自动派生结构相等 |

派生接口和派生方法必须同时存在或不存在；不能只生成一个供 `==` 查找的隐藏成员。struct/tuple 逐字段短路，enum 先比较 tag 再比较 active payload。字段可比较沿其自身的 Equality 契约决议，不必恰好是 `Equality<字段类型>`。泛型条件只来自实际字段／payload：`Box<T>(val value: T)` 不因新增 Equality 而限制 Box 本身的类型实参，只在相应字段比较条件成立时获得派生接口；未存储的 phantom 参数不产生条件。显式写出 Equality 却依赖派生正文的声明必须用足够的 bounds 保证字段条件，不能把显式 implements 变成条件关系。手写同签名成员遵守普通 override 和冲突规则，不生成第二个同签名方法。

Equality 表示“提供比较操作”，不保证任意实现都满足自反性、对称性或传递性。Float/Double 因此实现 Equality，继续保持 `NaN != NaN` 和正负零相等；含 NaN 的结构比较也保留该行为，不能做 identity／memcmp 或 `x == x` 恒真优化。Float/Double 仍不实现 Hash；本接口不为它们补 hash，不改变 Hash 的独立 opt-in 规则。

Equality 保持 invariant。继承 `Equality<Base>` 不自动成为 `Equality<Derived>`；两个 `Equality<Point>` 接口值的 equals 参数仍是 Point，不能因二者类型相同就彼此比较。`Any` 没有 Equality 契约，装箱不提供 Any 级相等 fallback。值类型已有的派生接口则可正常用于 bound、装箱、itable、`is` / `as`，这些位置使用同一个 conformance。

实现需要保持以下两处衔接：

1. HIR 在普通类型／继承信息齐备后、完成相关 interface obligation 和 bound 检查前，建立派生 Equality 签名、字段条件与真实 conformance；派生声明必须先于接口实现检查可用。普通接口、MIR/LIR、boxing／itable、`.slib` 和跨 Cone 泛型消费都须保存同一 slot 到实现的关系。派生方法保持 Equality 的 safe slot 合同，InteriorMutable 的 unsafe 限制在实际值使用处执行。
2. 保留整数、Char、浮点 equals 的 NoGc intrinsic 实现。value method 允许以 NoGc 实现普通 Managed interface 方法；静态具体类型直接调用仍为 NoGc，接口／bound 调用遵循接口合同，实际 itable 使用匹配 Managed ABI 的普通 value/interface adapter。不能为了实现 Equality 就把普通数值比较改为必须装箱、间接调用或进入 GC；也不能把允许任意用户实现的 Equality 接口整体标为 NoGc。

这是源码规则的统一：既有手写 operator equals 的类型需要补齐 Equality application 与 override；已有用户相等接口需要继承 Equality。普通结构相等的使用方式保持，接口实现由编译器同步派生。不保留“未实现 Equality 但 operator equals 仍可参与 `==`”的兼容分支；M33 同批更新 core、相关 fixtures 和受影响的产物兼容指纹。

## 10. 决策记录与实施跟踪

### 10.1 已确定的结论

D1～D7 均已有结论；D8 的前置依赖已满足。保留编号用于对应 review，以下不表示实现或验收已完成。

| 编号 | 已确定的规则 | 落点 |
| --- | --- | --- |
| D1 | argv 严格 UTF-8；非法参数报告下标并以 1 退出，无参数 main 可读取 raw argv | 2.1 |
| D2 | 语言级 panic、启动未捕获异常与异常逃离 main 统一退出码 1 | 2.2 |
| D3 | 保留正常退出的线程/token 约束；遗留项诊断、刷新并以 1 退出，无 daemon，显式 exit 可结束整个进程 | 4.3；M33-9 |
| D4 | 五种 C11 内存序全部开放，默认 SeqCst；native Ptr 原子操作等实际需求再加 | 3.2；M33-8 |
| D5 | 严格抛异常、严格返回 Option、lossy 三种转换；异常含零基 byteOffset，lossy 按 maximal subpart 替换为 U+FFFD | 6.3；M33-3 |
| D6 | 显式 library roots 无候选时使用所选 target 的系统目录/SDK；不回退到交叉编译宿主库 | 7.3；M33-6 |
| D7 | sysroot 默认源码查找只服务已声明的 scoop group 依赖；显式来源优先，core 保留自身定位规则 | 9.1；M33-7 |
| D8 | M32 已完成，基线改为 M32 的实际源码 Any/Nothing 与现有产物版本；不再等待合并 | 第 1 节；M33-4 |

### 10.2 review 遗留项的状态与后续任务

| 项目 | 当前状态 | 后续任务与完成条件 |
| --- | --- | --- |
| native 头文件缓存与输入一致性 | 7.2 和实现规范 2.7 已补齐契约；实现尚未开始 | M33-6 在外层缓存命中前发现/复核 include 依赖，把 C/C++ 源码、非系统头、公开 runtime 头和配置纳入现有输入快照；key 与子编译器读取相同内容。验收仅头文件修改、依赖集合变化、快照完成后工作区变化、配置/SDK 变化和不变输入复用；不再仅依赖 child 编译后写 depfile。 |
| AtomicRef 与 LLVM moving GC | M33-8a 已完成全部合法序的 AS1 最小 IR、statepoint rewrite/verifier 与三目标 codegen 验证 | 继续实现语言 API，用真实 Scoop fixture 覆盖对象和所指对象移动、返回引用保活、成功/失败 CAS、old→young 写屏障及 debug/release；编译级验证不代替实际 GC 验收。 |
| M32 基线与实施依赖 | 已纠正文档，并核对 roots.scoop 的 Any/Nothing 声明及 M32 验收记录；依赖已满足 | M33-4 直接消费实际 Nothing 实现 exit，验证无正常返回控制流、源码与 artifact-only 消费；以当前 runtime ABI 10 / metadata ABI 6 为旧版基线，验收 M33 的 11/7 升级与不兼容产物拒绝。其他批次复用 M32 已交付能力，按实际编码增量更新受影响版本。 |

## 11. 需要修订的规范章节

数据竞争的基本规则已同步到语言规范 1.2、运行时规范 3.5 和实现规范 2.19；NativeSafe 快路径、collector 停顿及初始化等待者的握手已同步到运行时规范 3.5、4.5～4.6；`@GCLeaf` 与标量直接 C ABI 调用已同步到语言规范 13.2、13.4.1、13.8、运行时规范 3.2、3.5、4.5 和实现规范 2.2、2.4～2.6；errno 按次捕获与 tuple 返回已同步到语言规范 9.1.6、13.4、13.4.2、13.8，运行时规范 3.8、4.5，以及实现规范 2.2～2.6、2.8。main、argv、退出码与完整 root gateway ABI 已同步到语言规范 11.7、12.4.4，运行时规范 2.8、第 7 章，以及实现规范 2.2～2.4、2.6～2.8、2.14。显式 C++ 开关、工具链与链接需求、musl 限制及 native C++ 边界已同步到语言规范 12.2.1、实现规范第 1 章及 2.7～2.8、运行时规范 5.5 和第 7 章。源码选择已同步到语言规范 12.2.2 及相关路径、编译单元章节，和实现规范 2.1、2.7。

D3 的 shutdown 失败与显式 exit 已同步到语言规范 12.4.4、运行时规范第 7 章；D4 的原子 API、内存序和 GC 写屏障已同步到语言规范 1.1～1.2、11.14、运行时规范 3.6、实现规范 2.10；D5 的严格/可空/lossy 解码及异常已同步到语言规范 11.4、11.7、运行时规范 2.4、第 6 章、实现规范 2.15；D6/D7 已同步到语言规范 12.2、12.2.3、实现规范 2.7～2.8，native 输入一致性已补到实现规范 2.7。其余数据借用、输出等入口的具体实现仍按 spec 先行，在对应批次补齐所需签名和交叉引用。

Equality 统一入口、条件派生接口和 NoGc 实现的接口适配已同步到语言规范 3.1～3.2、4.1～4.4、9.3、11.2、11.4、11.11、13.1～13.2、13.10，运行时规范 2.3、第 6 章，以及实现规范 2.2～2.3、2.6、2.9。

| 内容 | 语言规范 | 运行时规范 | 实现规范 |
| --- | --- | --- | --- |
| `main` 形态、argv、退出码与 root gateway ABI | 12.4.4 | 2.8、第 7 章 | 2.2～2.4、2.6～2.8、2.14 |
| `exit` | 12.4.4 | 第 7 章 | — |
| 未捕获异常的退出 | 11.7 | 第 5 章、第 7 章 | — |
| `write`/stderr/flush | 11（io 部分）、14.4 | 第 4 章 | — |
| 内存模型、原子类型 | 1.1、1.2、11.14 | 3.5、3.6 | 2.3、2.4、2.10、2.19 |
| 线程 attach 与退出规则 | 12.4.4、14.3 | 3.5、4.3、4.5、第 7 章 | — |
| NativeSafe 快路径与 collector 停顿 | — | 3.5、4.5、4.6 | — |
| `@GCLeaf` 小型 C FFI | 13.2、13.4.1、13.8 | 3.2、3.5、4.5 | 2.2、2.4、2.5、2.6 |
| 标量 C ABI 直接调用 | 13.4.1、13.8 | 4.5 | 2.4、2.5、2.6 |
| errno 捕获与 tuple 返回 | 9.1.6、13.4、13.4.2、13.8 | 3.8、4.5 | 2.2、2.3、2.4、2.5、2.6、2.8 |
| 数据借用、pin 计数 | 14.1、14.3 | 3.4、4.2 | 2.10 |
| 严格/可空/lossy UTF-8、C 字符串 | 11.4、11.7 | 2.4、第 6 章 | 2.15 |
| Cone native C/C++ 源码、输入快照、编译配置与系统库 | 12.2、12.2.1、12.2.3、13.4 | 5.5、第 7 章 | 第 1 章、2.6、2.7、2.8 |
| 按平台选择源码 | 11.12、12.1、12.2、12.2.2、12.4.4、第 15 章 | — | 2.1、2.6、2.7 |
| sysroot 默认定位 | 12.2、12.6 | — | 2.7 |
| Equality 与条件派生、接口 bound、NoGc 实现适配 | 3.2、4.1～4.4、9.3、11.11、13.2 | 2.3、第 6 章 | 2.2、2.3、2.6、2.9 |

## 12. 实施批次

批次之间的依赖关系已在说明中标出；没有依赖的批次可以并行。

1. **M33-1 C FFI 调用模式与 collector 停顿（4.2）**：先记录 C 调用及停稳/完整停顿耗时基线，完成 runtime 的 `managed_segment` 合并、seq_cst 返回握手、collector 条件变量超时等待与必要通知；再完成 `@GCLeaf` 的前端检查、HIR/MIR/LIR 与 metadata 传递、无切换发射，以及 4.2.2 的标量 DirectC ABI、按实际引用生成 bridge 和 native 链接需求。验收受控交错、TSan、多线程、moving GC、标量 ABI 交互、最终机器码和三种调用基准。不依赖其他批次；`captureErrno` 与 GCLeaf 的桥接组合在 M33-5 补验。
2. **M33-2 数据借用与 pin（6.1、6.2）**：编译器 intrinsic、线程 pin 帧、pin 计数、O(1) unpin。
3. **M33-3 UTF-8 与 C 字符串（6.3、6.4）**：实现严格抛异常、严格返回 Option、lossy 三种转换，公开 byteOffset，复用 maximal subpart 解码规则；覆盖 pointer 严格重载、输出长度和 GC 安全的 String 构造。依赖 M33-2。
4. **M33-4 程序入口与输出（第 2 节）**：四种 main 的 typed entry、完整 argc/argv 与 GC 安全的参数构造、root gateway 的 status／退出码双通道、runtime ABI 11／metadata ABI 7 与旧产物拒绝、linker 启动代码、`exit`、固定为 1 的失败退出、`write` 改走 NativeSafe、stderr 与 flush。依赖 M33-2（`write` 需要借用）和 M33-3（`args` 需要受检解码）。
5. **M33-5 errno 捕获（第 5 节）**：extern 注解与 `(R, Int)` 结果检查、native 签名投影、bridge 清零/快照与整数返回、Scoop tuple 重建、release 组合、产物字段与 bridge recipe 区分、同 symbol 捕获/不捕获的链接兼容。
6. **M33-6 Cone native C/C++ 源码、系统库与按平台选源码（第 7、8 节）**：manifest 的完整 Scoop 源码清单与默认扫描互斥、target 选择与路径冲突检查；显式 `cxx` 开关、C/C++ driver 及编译参数；完成 10.2 的头文件发现与输入快照任务、缓存失效；`.slib` 的 C++ 需求传递、C++ 最终链接与运行库配置、musl 组合诊断；目标平台系统库解析、链接与符号冲突。
7. **M33-7 sysroot 定位与泛型相等（第 9 节）**：9.1 限 scoop group、显式来源优先的默认查找；9.2 的 core Equality 与 intrinsic 声明／shape 验证、唯一 operator 契约、派生接口与正文的衔接、generic 条件和 bound、NoGc 直接实现与 Managed 接口适配、boxing／itable 与跨 Cone metadata。同步迁移既有手写 equals 声明并验证 Hash 独立、浮点语义及无新增数值比较开销。
8. **M33-8 内存模型与原子操作（第 3 节）**：首先完成 10.2 的 AtomicRef/LLVM 技术验证，再实现五种内存序、四个 intrinsic 原子类型及 HIR/MIR/LIR/codegen、写屏障和真实 GC fixture，最后是 core 或平台库中的 `Atomic<T : value>`。普通非原子访问保持数据竞争为未定义行为的语义；不加入 native Ptr 原子接口。
9. **M33-9 线程退出规则与总验收（4.1、4.3、第 13 节）**：实现遗留 attachment/callback/token 的计数诊断与刷新后退出码 1；验证正常 join/释放、Int 返回码覆盖、显式 exit 带后台线程退出，完成其余组合验收。

## 13. 验收

每项能力都要有独立 fixture、与其他能力的组合 fixture、对应编译错误规则的 negative fixture，以及受影响 stage 的 HIR/MIR/LIR golden；共同能力在三个 target（Darwin aarch64、Linux gnu、Linux musl 静态/动态）均需通过。C++ 构建、产物消费与运行只在 Darwin/GNU 验收；musl 的静态／动态模式均验证明确拒绝 C++。

并发验收只对无数据竞争的执行断言结果；不为故意竞争的程序规定输出、是否崩溃或必有诊断，也不建立普通读写在竞争时不撕裂的验收要求。

重点组合：

- **示例平台 Cone**：在 `tests/fixtures` 中建一个最小的平台 Cone，带 C 源码、按平台选择的源码、系统库依赖和 `captureErrno` 的 extern，实现 open/read/write/close、stat、readdir，以及基于 pthread 加 OneShot callback 的线程 spawn/join，由 executable 经产物依赖消费并运行。它只用于验收，不作为公开平台库发布。
- **阻塞与 GC**：一个线程阻塞在 `read(pipe)` 或写满的 stdout 上，另一个线程反复 `gcCollect()` 并分配，验证 GC 不被阻塞；同时打开 moving GC 的强制 relocation 模式。
- **借用与并发**：两个线程同时借用同一个已通过同步发布、借用期间不再修改的数组，调用 C `write` 读取其内容，同时触发 minor/full GC，验证对象不移动且借用结束后可以正常移动。
- **原子操作与同步**：多线程 `fetchAdd` 计数、`AtomicBoolean` 自旋锁保护普通字段及数组元素、`AtomicRef` 的 CAS 栈（含 moving GC 与 nursery 晋升，验证写屏障与 CAS 失败路径）、Acquire/Release 发布对象、`Atomic<T>` 在多线程读写 struct、tuple 和 tagged enum 时始终读到完整的值（含引用字段和 moving GC）；非法内存序组合、继承原子类型、访问其值字段的 negative fixture；debug 与 release 两种 profile 都要通过。
- **原子内存序与 GC lowering**：覆盖全部五种 order、CAS 成功/失败序矩阵、默认 SeqCst、非编译期常量的拒绝、strong CAS 与 fetch 旧值返回；AtomicRef 的对象与所指对象分别移动、跨 GC 的读取结果和 expected/new 引用保活、old→young 引用与成功/失败 CAS。10.2 的 LLVM IR 验证与实际 Scoop/三 target 结果分开记录，不把前者替代后者。
- **UTF-8 三种转换**：合法多字节、空输入、U+0000、已有 U+FFFD、孤立 continuation、截断、过长编码、代理项和超范围码点；严格异常检查准确的 byteOffset，OrNone 在相同错误输入上返回 None，lossy 精确断言 6.3 的替换数量及合法前后缀。检查结果 byteLength、length、输入/结果存储独立、pointer 严格重载和分配期间 moving GC；argv 仍拒绝非法 UTF-8。
- **Equality 与 bound**：String、Boolean、Char、八种整数、Float/Double、Ptr 的显式接口与普通自定义 class/struct 的实现均可经 `T : Equality<T>` 比较；Hash 与 Equality 的双 bound 用于实际 key 比较。只有 Hash、缺少 Equality 的手写 operator、缺少 override、未继承 core Equality 的同形接口、Any 比较、重载歧义与缺失 generic bound 均有 negative fixture；验证左右求值一次及 `!=` 只取反。
- **Equality 派生与产物**：struct、enum、tuple、Unit、嵌套与 generic application 的派生同时满足直接比较、bound、装箱接口调用和 `is` / `as`；字段不可比较的 application 仍可构造，但请求相等能力时报错，phantom 参数不产生条件。手写实现优先、继承的 Equality application 保持 invariant；跨 Cone、泛型模板及 artifact-only 链接消费相同 conformance，HIR/MIR/LIR golden 与 ODR 检查覆盖真实 slot／派生 body／adapter。
- **Equality 的浮点与 NoGc 组合**：NaN、正负零及含浮点字段的结构在 debug/release 和 bound 调用中保持 IEEE 语义，Float/Double 不满足 Hash。直接整数／Char／浮点比较继续可用于 NoGC 代码，接口调用使用自身 Managed 合同；验证装箱与 moving GC、NoGc 实现对应的 Managed adapter，以及优化后具体标量比较没有新增装箱、间接调用或 safepoint。
- **errno 返回与语义**：覆盖 scalar、pointer、C-layout struct 与 void 的 native 返回，验证 Scoop 得到 `(R, Int)` 或 `(Unit, Int)`；覆盖调用前清零、成功但 errno 非零、失败时的原始错误值，以及忽略第二项时仍采用捕获模式。错误 tuple 元数、第二项非 `Int`、非法 native `R`、含 ref、Scoop ABI/extern 变量启用捕获、未启用捕获却返回 tuple，均有 negative fixture；HIR/MIR/LIR golden 区分 Scoop tuple、native `R` 与 bridge 的整数返回。
- **errno 生命周期与重入**：保存一次结果后进行其他捕获调用、分配、GC 与协程挂起/恢复，再读取原值；普通 NativeSafe 回调中嵌套捕获，验证每次结果独立。GC 在当前线程执行带捕获的 release hook 时，mutator 先前的错误值不变；hook 的非法 `ReleaseValue` 与 raw leaf 操作继续拒绝。清零与读取仅触及 libc errno，不产生 Scoop TLS/thread runtime 或 collector 保存/恢复调用。
- **errno 产物与链接**：同 Cone、不同 Cone、泛型消费与 artifact-only 链接中，同一 C symbol 的返回 `R` 与捕获返回 `(R, Int)` 均可共存；真实 native 签名冲突仍报错。捕获/不捕获的 bridge recipe、私有签名与缓存不得混用。普通 NativeSafe、GCLeaf 与 release 分别验证捕获发生在结果复制/helper/返回握手之前；三个 target 及 debug/release 覆盖。
- **入口与退出码**：四种 main 在 manifest、single-file 与 artifact-only 链接中均可运行；非法签名、没有入口及多个入口有 negative fixture，依赖 main 不参与选择。Unit 正常为 0；Int 覆盖负数、0、1、255、256 与最小／最大值，gateway 测试检查完整值，进程测试按 POSIX 的可见状态检查，不能把正常返回 1 当作 gateway 失败。四种入口的未捕获异常及语言级 panic 退出均为 1。
- **argv 与启动失败**：args[0] 为稳定输出路径、无用户参数时仍有第 0 项、空参数／空格／Unicode 原样保留；非 UTF-8 参数（含 argv[0]）诊断下标并以 1 退出，main 不执行。无参数形态仍可从 raw argv 读取原始字节且不强制解码。eager 初始化可读 raw argv；初始化失败不执行 main。argv 构造期间强制 moving GC，验证数组与 String 的 roots、relocation 和异常 failure root。
- **gateway 与产物兼容**：HIR/MIR/LIR golden 保留入口形态、真实 main 签名、gateway C 原型与成功／失败控制流；root 与 eager gateway 分别按正确原型调用，参数、GC-free 退出码槽及 failure root 不混用。旧版 runtime／metadata／gateway 拒绝，不复用旧启动对象或链接缓存；三个 target 与 debug/release 覆盖。
- **输出与退出**：`exit` 时仍有线程运行、异常退出前 stdout 缓冲有内容，验证刷新后终止及输出保留；输出端最终继续消费时允许刷新完成，等待期间其他线程仍可推进 GC，不把刷新可能等待视为失败或要求立即终止。
- **正常 shutdown 约束**：main 返回前完成 join 和 token 释放时保留 Unit 的 0 或实际 Int 返回码；分别遗留非主 attachment、活动 callback、observer token 时报告相应计数并以 1 退出，覆盖 Int main 原本返回其他值的情况。显式 exit 不作这些检查；不将正常失败诊断验收为 abort 或等待线程自行结束。
- **构建**：Cone 内 C 源码或其包含的头文件修改后重建，无关修改可以复用缓存；未知的 `when` 键或值、越出 Cone 根目录的路径、两个 Cone 中的同名 C 符号，均有 negative fixture。
- **native 输入一致性**：只修改传递 include 的头文件或公开 scoop_rt.h 时，外层 Cone cache 失效；include 依赖集合/宏/配置变化重新准备。快照完成后修改工作区头文件，本次子编译器仍使用快照内容，下一次构建才按新内容生成 key；不得把新 object 写入旧内容的缓存。相同输入复用已确认快照，系统 SDK/toolchain 变化正确失效。
- **系统库与默认 Cone 定位**：manifest native 库及同名 Extern requirement 共用目标系统 provider；覆盖 Darwin SDK 和 Linux GNU/musl 的实际系统库、显式 roots 优先、显式候选错误不回退、跨 target 不取宿主库及静态模式拒绝动态需求。scoop group 的已声明依赖无需 cone-path 即从 sysroot 源码构建；其他 group、错误 coordinate/version/kind、显式 locator 失败和 artifact-only 缺失产物不被默认查找掩盖。
- **源码选择与兼容**：无顶层 `sources` 的既有 manifest 继续递归扫描 `src/`；只配置 `native.sources` 不抑制该扫描。公共目录加 Linux/Darwin 目录的显式清单只编译当前 target 的文件，未选中平台文件中的语法或类型错误不影响本次构建。覆盖单文件条目、`src/` 外的源码目录、`when` 的与/或规则和互斥条目；`scoop` 与直接调用 `scoopc` 得到相同集合，single-file 行为不变。
- **源码选择错误**：重叠目录、目录与单文件重叠、重复规范化路径、symlink 重复文件，以及选中路径不存在或类型不符、非 UTF-8、symlink 逃逸或循环、未知字段或非法 `when`，均有 parse 前失败的 negative fixture；显式空清单、条件全部不匹配、最终无 `.scoop` 文件时拒绝，不回退到默认目录。
- **源码身份与缓存**：同一文件在默认/显式模式以及不同清单顺序或分组下保持 source identity 与 file-private 身份，SourceLocation 保留完整 Cone 相对路径。新增、删除或修改选中源码及修改选择配置后正确重建；仅修改未选中平台文件的内容可以复用缓存。缓存命中前仍执行选择与冲突检查，不能掩盖后来出现的空集合或重复文件。
- **C++ 编译与链接**：带 `cxx = true` 的 library Cone 混合编译 `.c`／`.cpp`，通过 `extern "C"` 包装实际使用 C++ 标准库，并验证 native 异常处理与初始化／析构边界。不含 C++ 源码、未开启 `cxx` 的 root 经直接及传递 `.slib` 依赖消费它，验证最终自动采用 C++ driver 与运行库。覆盖 debug/release、C/C++ 编译参数及头文件变化后的缓存失效、缺少开关或配套工具链的诊断。
- **C++ + musl negative**：musl static/dynamic 下当前 Cone 开启 `cxx`，以及 artifact-only 闭包带 C++ requirement，均在相应 native 编译／链接前明确拒绝并指出来源；不开启 C++ 的既有 musl fixture 继续通过。
- **NativeSafe 与 collector 交错**：用测试专用同步点覆盖 collector 已看到 NativeSafe 后线程进入 RETURNING、线程刚读到 RUNNING 后 collector 发起 GC、扫描期间出现 RETURNING；初始化等待者已 PARKED、上一轮 parker 尚未恢复即开始下一轮 GC、pending gateway 激活、NativeBorrowed park、callback 进入和 attach/detach 均有组合覆盖。强制 minor/full relocation 后检查引用更新与正常完成。
- **等待与通知**：NativeSafe 转换不发广播且没有其他通知时，collector 经超时复查完成停稳；正常通知和虚假唤醒均重新检查谓词；初始化结果未就绪或 GC 尚未结束时不能错误恢复。验证必要的 GC 开始/结束、park 确认及初始化完成/失败通知。
- **GCLeaf 编译与调用**：标量及 C-layout aggregate 的参数/返回、native 副作用、普通与 GCLeaf 声明指向同一 C symbol、跨 Cone 与泛型消费均有正例；普通函数、Scoop ABI extern、变量、类型等非法注解位置及与 C ABI `@NoGC` 的非法组合有 negative fixture。HIR/MIR/LIR golden 和生成代码检查确认模式保留、C ABI 正确且没有额外 transition/root/safepoint。
- **DirectC ABI 与产物**：覆盖 4.2.2 的全部标量类别、小整数扩展、混合及栈传参、void 返回、nullable pointer 与透明 C 表示；C-layout struct 和 `captureErrno = true` 确认保留 bridge。普通 NativeSafe 与 GCLeaf 复用 DirectC，并分别验证自身 GC 协议。跨 Cone、泛型及 artifact-only 链接保留真实 native symbol、library requirement 和调用模式；直接调用不要求无用途的 bridge unit。debug/release 验证行为，优化后机器码确认无额外桥接调用或桥接缓冲区。
- **GCLeaf 与 GC/求值**：真实短 C 调用在循环和 GC 压力下运行，引用跨调用及后续 moving GC 保持有效；显式/缺省实参中的分配、异常和真实 safepoint 不能被注解移除。包含 `captureErrno` 的组合验证原 C 调用的捕获结果。外部函数违反无回调/无阻塞契约不作为必须自动检测的负例。
- **FFI 回归与性能**：TSan 构建的 runtime 跑多线程压测，M13、M15、M27、M31 的多线程与 moving GC fixture 全部回归。同一 native callee 比较普通 NativeSafe、GCLeaf 和直接 C caller 的单次耗时及 1/2/4/8 线程吞吐，保持 target、优化级别、链接方式和被测操作一致，避免只在 C 基准中内联、builtin 替换或消除调用。DirectC 与 aggregate/errno bridge 分别记录；GC 压力下另测停稳等待和完整停顿耗时，并注明条件变量超时参数，不能只使用现有不含停稳等待的 pause 指标判断改进。

## 14. 明确留待后续

- 平台库本身的公开 API 及其设计（File、Path、目录遍历、Process、管道、Thread、Mutex、Condvar、HashMap、Duration、时钟）。
- 平台库中跳过输出刷新与清理、直接终止进程的接口；core `exit` 保持 2.2 的先刷新后退出语义。
- fork 安全与 `pthread_atfork`；平台库只使用 `posix_spawn`。
- SIGPIPE、SIGCHLD 等信号策略，由平台库决定是否在初始化时设置。
- off-heap ByteBuffer 与外部内存压力反馈；arena 扩容。
- 把 NativeSafe 状态切换内联进生成代码，以 M33-1 的测量数据作为依据；显式 `@GCLeaf` 已纳入本里程碑。
- 完整按值 C aggregate 的直接 ABI lowering：包括各 target 的寄存器分类、packed、同质浮点聚合体及大结构体参数/返回；M33 以现有 storage bridge 保持其功能。后续扩展直接调用不改变 `@GCLeaf` 源码契约。
- 声明级或表达式级的条件编译，以及跨 target 的公开 API 一致性检查。
- daemon 线程及其配套 shutdown 语义。
- `Ptr<T>` 所指 native 内存上的原子操作，等平台库出现实际需求后增加。
- C++ + musl：先提供匹配的 C++ 编译器、标准库与 C++ ABI 运行库，再单独开放并验收；M33 暂不支持。
- Windows、Linux arm64 等新 target。
