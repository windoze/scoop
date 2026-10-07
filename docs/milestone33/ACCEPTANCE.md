# M33 实施与验收记录

开始日期：2026-10-08。基线：`670a45477`（M32 完成），分支：`codex/m33`。

本文件只记录实际完成的实现和验证；设计方案与验收要求见 [DESIGN.md](DESIGN.md)，未完成项目不计为通过。

| 批次 | 能力 | 状态 |
| --- | --- | --- |
| M33-1 | NativeSafe/collector、GCLeaf、DirectC | NativeSafe、GCLeaf 完成并通过三平台验收；DirectC 待实现 |
| M33-2 | 作用域数据借用、计数 pin | 待实现 |
| M33-3 | 严格／可空／lossy UTF-8、C 字符串 | 待实现 |
| M33-4 | main、argv、退出码、输出与 ABI 11/7 | 待实现 |
| M33-5 | errno 捕获 | 待实现 |
| M33-6 | native C/C++、系统库与源码选择 | 待实现 |
| M33-7 | sysroot 默认定位、Equality | 待实现 |
| M33-8 | 原子类型、内存序与 GC | 待实现 |
| M33-9 | 线程退出规则与组合验收 | 待实现 |

开发验证先格式化、lint，再执行受影响测试。运行真实 CLI fixture，覆盖源码、产物消费、链接与运行；新增行为保留独立／组合／negative／golden。全量测试集中在必要的回归节点，已有通过结果在输入不变时复用。

Linux 使用 `nuc12`。其 `~/repos/scoop` 有既存未提交变更，M33 测试使用其 `target/m33-linux/` 下的独立源码副本和构建目录，不覆盖既有工作。所有本机临时文件放在仓库 `tmp/m33/`，工具通过 `TMPDIR=<repo>/tmp` 使用仓库临时目录；Linux 副本采用同一约定。阶段结束后清理过期构建产物。

## M33-1a：NativeSafe 与 collector

- `MANAGED_PENDING` 并入原子 mode，移除独立 `managed_segment`；scanner 只在停稳后读取 anchor、根链和 parked 来源。公开 debug mode 保持既有编号。
- NativeSafe 发布与 RETURNING/phase 检查使用 seq_cst，与 collector 的 STOPPING/mode 检查配对。常见进出不取 world lock、不广播；阻塞返回保持冻结根并在唤醒后重新握手。NativeBorrowed 的非停止路径也不取 world lock。
- collector 条件等待采用 50 微秒的超时复查，等待期间释放 world lock，以 C11 `timespec_get` 取得截止时间，兼容严格 C11 的 musl 构建。保留 GC 开始、park、结束及初始化通知。world lock 下的 phase 转换已经保证单 collector，因此删除会与 world lock 反向取得的冗余 collector mutex。
- release runtime 使用 `-O2 -DNDEBUG`，完整 transition 链查重仅保留在 debug；必要边界与 LIFO 检查继续执行。编译命令和 runtime cache key 复用同一组优化参数。没有改变公共 runtime ABI；11/7 升级在 M33-4 实施。
- 新增三个受控交错：扫描中出现 RETURNING、返回线程先读到 RUNNING 后发起 GC、NativeSafe 发布不通知 collector。每个配置运行 20 轮，覆盖 O0/O2 及 minor/full relocation；检查真实对象和冻结栈根的回写。

已完成的验证：

- `cargo fmt --all`、工作区 `cargo clippy --workspace --all-targets`，后续仅改动 scoop runtime 构建参数时补做该 crate 的 clippy；C 文件经格式化和 `-Wall -Wextra -Werror` 检查。
- Darwin：18 项 `scoop-codegen` runtime collector 测试通过；删除冗余锁后同组复验通过。3 项 `scoop` runtime 构建／缓存测试通过。
- Linux GNU / clang 22.1 TSan：新增交错的 full/minor 两个配置，以及 moving-thread、nursery-thread、gateway、initialization-GC、nursery-mutators，共 7 个进程通过；`halt_on_error=1`，无 TSan suppression。
- Darwin 正式 CLI 回归 31 项全部通过：53 个变体、170 个进程、117 份 golden，覆盖 M13/M15/M27/M31 与新增 transition fixture。最终可移植时钟改动后另复验四个受控交错配置通过。
- Linux GNU / musl 分别定向回归 6 项：各 12 个变体、52 个进程、31 份 golden 全部通过，包含 callback、moving roots、context lifetime、nursery GC 以及新增 transition fixture。三平台新增 fixture 均覆盖 debug/release × normal/moving/minor，HIR/MIR 内容一致，LIR 按 target/profile 保存并在非更新模式下校验。
- musl 另有 PIE + artifact-only 重链接 fixture，debug/release 两个变体、16 个进程通过；原构建和独立链接的 link-plan fingerprint 一致，重新链接的程序通过 moving/minor GC 运行。最初 fixture 漏写工具的正常 stdout 预期，补齐后仅重跑此项。
- fixture runner 的 44 项公共规则测试通过。没有运行与本批 runtime 变化无关的全量语言 fixture。

性能使用 `runtime/tests/native_transition_benchmark.c` 和独立编译的 `native_transition_leaf.c`，同一 native 操作比较直接 C、NativeSafe、NativeBorrowed。GC 测量通过只在测试构建中存在的同步点区分 STOPPING→停稳与 STOPPING→RUNNING；production 无测试回调。完整条件、原始样本、吞吐与等待分布见 [PERFORMANCE.md](PERFORMANCE.md)：常见调用的全局争用显著减少，但超时复查使部分 GC 等待尾部增加，未把吞吐提升报告为所有 GC 延迟改善。

## Nursery 分配竞争修复

GCLeaf 并发 fixture 在 release + minor 压力模式下暴露了既有分配重试问题：其他 mutator 可以在收集结束后抢先占用单块 nursery，旧逻辑以尝试一次 minor、一次 full 的次数上限误报 arena OOM。nursery 容量竞争现在继续收集和重试，只有实际完成 full 后仍无法取得物理块才报告耗尽。内部 full 请求区分自己完成收集与加入另一轮收集，避免把加入 minor 当成已完成 full；完整收集后的尝试也不再受软 threshold 阻挡。公共 runtime ABI 不变。

验证覆盖真实双线程分配和 GC：测试在收集后连续三次安排另一线程抢先占用 nursery，再确认等待方成功分配；旧实现的临时副本在 O0/O2 均稳定复现原 OOM，修复后 O0/O2 均通过。其余 18 项 runtime collector 测试通过。Linux GNU / clang 22.1 TSan 的原 7 个进程与新增竞争测试均通过，未使用 suppression。C 格式化、严格编译检查及 `scoop-codegen` clippy 通过。GCLeaf 的真实 CLI 并发 fixture 在 debug/release × normal/moving/minor 下均通过。

## M33-1b：GCLeaf 调用模式

- `@GCLeaf` 只允许 C ABI extern，保留已有签名、unsafe、top-level、non-generic 和非 suspend 规则；C extern 显式 `@NoGC` 现在报错。旧 fixture 中的冗余注解已删除，普通 C extern 仍使用 NativeSafe。
- `CAbiCallMode` 从源码声明、可调用接口和泛型模板经 MIR 保留至 LIR。模式独立于 native symbol 的物理合同，同 symbol 的普通与 GCLeaf 声明可并存，链接合并不传播调用模式；跨产物 release 的 C 目标同样保留声明模式。
- LIR 的 `NativeGcLeaf` 是独立的完整调用分支，不带 safepoint/root plan。LLVM 调用保持 `gc-leaf-function` 和 `nounwind`，不发射 native 进出、statepoint、根发布或 relocation reload，也不添加纯函数／无内存副作用属性。实参求值中的分配、异常和正常 GC 继续执行。物理调用本批仍采用 StorageBridge，DirectC 单独实现。

验证记录：

- workspace fmt/clippy 通过；codegen、HIR、HIR-lower、MIR-lower、LIR-lower 的受影响测试组已执行，wire 固定向量随显式模式更新后定向复验通过。新增模式编码测试覆盖两种模式、缺失字段和未知 tag；新增 LLVM 测试在 statepoint rewrite 后检查 native 副作用仍在且没有 GC 边界操作。
- 三平台各 10 项正式 CLI fixture、13 个变体、51 个进程、24 份 golden 均在非更新模式下通过。覆盖标量／aggregate、同 symbol 双模式、显式／缺省实参的 GC、实参抛异常、NoGC helper、泛型、真实 foreign-thread 并发，以及移除 provider/consumer 源码后的 `.slib` 消费和独立链接。debug/release 均执行 normal/moving/minor；8 份公共 HIR/MIR 在三个目标间一致。
- 7 类 negative fixture 断言完整诊断与源码位置：非法声明目标、语法位置、generic/suspend、managed 签名、注解参数／重复、C extern 上的 NoGC，以及 unsafe 调用。nursery 修复后另有 20 次 release minor 并发运行全部通过。
- 旧 fixture 只回归受影响的 imported value、Char、浮点、优化 profile 与 release 调用路径。AST/HIR golden 的变更对应删除注解、源码位置变化和 C 声明显式记录 NativeSafe 模式；对既有默认模式的调试拼写做机械更新，MIR/LIR 指令未改变。未重复执行无关的全量语言验收。
