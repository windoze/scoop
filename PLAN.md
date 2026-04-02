# Scoop 0.1 编译器与运行时实现计划（LLVM/inkwell + 长期 C runtime/GC（Immix 路线））

> 目标：把 `SCOOP_FULL_SPEC.md` 落地为可用的 `scoopc` 编译器与最小运行时（含 GC、effect runtime、sysroot），并建立一套“可持续扩展”的 fixture/测试体系，保证规范与实现长期一致。

---

## 0. 总体原则（强约束）

1. **永远可回归**：每个阶段都要产出可执行的最小子集（能编译/能跑），并有 fixtures 覆盖新增语义。
2. **规范驱动**：以 `SCOOP_FULL_SPEC.md` 为唯一语言规范来源；代码块示例要能自动变成 fixtures（类似 doctest）。
3. **LLVM 为后端**：所有代码生成走 LLVM IR（Rust `inkwell`），最终产物为 `.o` + 链接运行时。
4. **长期 C runtime（平台依赖隔离 + ABI 最小化）**：
   - GC/effect runtime/平台能力长期由 C 实现，编译依赖 `clang`（可通过 Rust `cc` crate 或显式调用 clang）。
   - 平台相关代码统一收敛到 `runtime/c/` 的 platform/backends 层（按 OS/ABI 拆分）；不得把平台差异与 OS 细节泄漏到 Scoop 侧（sysroot/stdlib）或 codegen 逻辑中。
   - 对 Scoop 暴露的 runtime ABI 必须尽可能小、稳定、可审计（避免把 libc/OS API 逐个“直通”暴露给 Scoop）。
5. **多线程友好**：effect dispatch/unwinding 的运行时状态必须是 TLS；`Continuation` 允许跨线程 `resume`（语义为恢复其捕获的 handler stack）。

---

## 0.1 维护备注（TODO 顺序）

- 2026-04-03：T1405（GC backend 抽象）范围过大且与后续 Immix/adapter 等实现点耦合较多，为保持“首个 `[TODO]` 可直接实现”的粒度，已拆分为 T1405a/T1405b：先落地 backend 选择机制 + baseline/minimal 双后端回归（T1405a），再补齐 capability 矩阵与检查点（T1405b）。
- 2026-04-03：完成 T1405a：引入 `SCOOP_GC_BACKEND` 编译期选择与 `gc-baseline/gc-minimal` Cargo features；把 `scoop_gc_self_check` 与 `scoop_gc_type_descriptor_trace` 提取为 shared 编译单元；新增 minimal backend（单线程/无 STW，检测到多线程时 collect 退化为 no-op）。
- 2026-04-03：完成 T1405b：补齐 GC backend capability matrix（STW/多线程 roots 枚举/moving/精确 roots 更新/shadow stack roots），并把这些能力以编译期常量接入测试 gating：`gc_stop_the_world` 在 `gc-minimal` 下显式 `ignored`；新增 `gc_capabilities` 集成测试固化矩阵值。
- 2026-04-03：T1406（Immix v0：单线程、非移动）范围较大。为保持 TODO 顺序“首个 `[TODO]` 可直接实现”，已拆分为 T1406a～T1406d：先接入 `gc-immix` feature + build 选择 + capability matrix（T1406a），再逐步实现 allocator（T1406b）、mark-region（T1406c）与 microbench/fixtures（T1406d）。
- 2026-04-03：完成 T1406a：新增 `gc-immix` feature 与 `SCOOP_GC_BACKEND_IMMIX`（C/Rust 两侧 capability matrix 同步），并新增 Immix backend 的 v0 scaffold 编译单元；更新 `scoop_runtime` 测试 gating（`gc_stop_the_world` 在 `gc-immix` 下 ignore）。
- 2026-04-03：完成 T1406c：Immix backend 落地 mark-region（line mark bitmap）+ region sweep（holes 复用，优先 partial blocks），并新增 `scoop_runtime` 集成测试回归“回收→再分配”循环与 live 对象不被覆盖。
- 2026-04-03：完成 T1406d：新增 GC microbench（吞吐/碎片化）与一键对比脚本；引入 `scoop_gc_debug_heap_bytes_reserved` 观测 reserved bytes（baseline≈live；Immix=blocks+large），并新增集成测试固化语义。
- 2026-04-03：完成 T1407：Immix backend 支持 moving/compaction：选择性 block evacuation + forwarding pointer + shadow stack roots 更新；新增 `gc_immix_compaction` 集成测试回归，并同步 capability matrix（Immix：moving/precise_roots_update）。
- 2026-04-02：完成 T1403c：收敛 once/guard 相关 OS 调用到 platform backend：新增 dynlib symbol lookup（RTLD_DEFAULT 类语义）platform API，并在 POSIX backend 通过 `dlsym/dlerror` 实现（Windows 占位）；`runtime/c/scoop_once.c` 改为仅依赖 platform thread self/yield + dynlib lookup，不再直接包含/调用 pthread/sched/dlfcn。
- 2026-04-02：完成 T1404：sysroot/std 平台 API surface 审计与固化清单：新增 `PLATFORM_API_SURFACE_AUDIT.md`（覆盖 env/time/fs/path/io/process/thread/sync/channels/net 的 API surface ↔ runtime 符号 ↔ backend 支持现状），并新增 typecheck fixture `tests/fixtures/typecheck/std_platform_api_no_os_types_is_error.scoop` 回归“平台 API 不泄漏 FILE/HANDLE/pthread_t 等 OS 概念类型”。
- 2026-04-02：完成 T1326b：delegated properties 的 `observable/vetoable` 并发可见性与回调规则：HIR lowering 为 observable/vetoable 注入并初始化 per-property mutex 字段（`<name>$delegate_mutex`），getter/setter 读写 backing field 时通过 mutex 加锁保证可见性；`observable` 在写入后且锁外触发回调、`vetoable` 在写入前且锁外触发回调（允许 reentrancy，并避免 `Raise`/异常路径导致锁泄漏）；新增 run-pass fixtures 覆盖并发回调次数/顺序、veto 失败分支可见性与 Raise 交互；更新 sysroot 与 spec §10.4 并发说明。
- 2026-04-02：完成 T1326c：delegated properties 跨平台 policy（capability matrix + 编译期 gating）：引入 `TargetPlatform`（platform id + threads/mutex capabilities），typecheck 为标准 delegates（`lazy/observable/vetoable`）在无 mutex 平台给出稳定错误码与迁移提示；fixtures 支持 `// ARGS: --target-platform <id>` 覆盖目标平台并新增 typecheck 用例回归；更新 `STDLIB_DESIGN.md` 与 `sysroot/delegates.scoop` 文档说明。
- 2026-04-02：完成 T0602b：typecheck 支持“泛型 effect op call”（`Async.await<T>`）与 handler arm head 的实例化；新增 typecheck fixtures 回归 `Async.await(task)` 推断、显式 type args 与稳定错误码（推断冲突 / not inferred）；为 `generic_type_arg_not_inferred` 增加“显式类型实参”提示；后续清理 stdlib 的 `__TaskAwaitInt` 适配层已由 T1320e 覆盖。
- 2026-04-02：完成 T1320e：stdlib `Task` async adapters 清理：移除 `__TaskAwaitInt` workaround 并改用 `Async.await(task)`；typecheck 支持在 handle arm head 通过 binder 类型注解反推并实例化 op 自身 type params（解锁 stdlib 在 arm body 内调用 `Task<Int>.onComplete/tryStart`）；新增 typecheck_cone fixtures 回归 `std_task_async_await_impl_ok`。
- 2026-04-02：完成 T1327a：类初始化（单继承链）顺序与字段 layout 前缀：LLVM codegen 在 class ctor call 中按 base → derived 逐层执行 init steps，并保证子类 layout 以前缀形式包含基类字段；当前阶段 super ctor call 仅支持 0-arg（`: Base()`）；新增 run-pass fixture `class_init_order_inheritance_chain_basic` 回归三层继承初始化顺序。
- 2026-04-02：完成 T1327b：类初始化期间 Raise/effect unwinding cleanup：LLVM codegen 为 class ctor call 的临时 GC frame 增加 unwind cleanup wrapper，在 `Raise.raise` / custom effect `perform` 发生时先 pop 临时 GC frame，再跳转外层 catch（或无 handler 时 return 默认值向外传播）；新增 run-pass fixtures `class_init_raise_cleanup_property_init_gc_basic` 与 `class_init_raise_cleanup_init_block_gc_basic` 回归 GC 后 heap object count delta 为 0。
- 2026-04-02：完成 T1327c：类初始化补齐 super ctor args 与 secondary ctor delegation（`this(...)`/`super(...)`）：parser/resolver/typecheck/lowering/codegen 全链路落地并固化 Kotlin-like 初始化顺序；新增 run-pass fixtures 覆盖 super args 求值顺序与 delegation 行为；同时修复 LLVM reachable top-level fun 收集覆盖 class init/ctor delegation 语境，避免链接阶段出现未定义符号。
- 2026-04-02：完成 T1328：`for` 迭代协议升级为 `iterator()/next(): Option<T>`（不再依赖 `hasNext()`）：typecheck 更新迭代协议门禁与 performed effects 记录；新增稳定错误码 `scoop::typecheck::for_next_not_option` 并更新/新增 typecheck fixtures 回归。
- 2026-04-02：完成 T1325c：vararg spread 的迭代器视图桥接：sysroot 增加 `scoop.collections.Iterable/Iterator`（对齐 spec §16.2）并补充 `IntIterable/IntIterator`（规避当前阶段“上转到带实参 interface”的门禁），声明 `IntIterable.toArray(): Array<Int>`；新增 typecheck fixture 回归 `*view.toArray()` 的调用点类型检查。当前 stdlib `toArray()` 为最小可编译 stub（返回空数组），真实“迭代消费并收集”语义待 member call + for lowering 打通后补齐。
- 2026-04-02：完成 T1402：引入 `runtime/c/platform` 平台层抽象 v0（POSIX backend）：新增 `platform.h` 并将 env/time/io（含 print/println）路径改为通过 platform API 调用 `getenv/gettimeofday/read/write`；Windows backend 先提供占位实现；所有 platform API 保持 `static` 以避免污染 runtime ABI allowlist（T1401）。
- 2026-04-02：拆分 T1403（platform backend 隔离：thread/sync/channels/task/net）为 T1403a/T1403b/T1403c：先做 sync primitives（Mutex/CondVar/Thread self/equal）抽象与现有模块接入，后续再补 thread primitives 与 once/guard OS 调用收敛，以保持每步“可单独实现 & 单独验证”。
- 2026-04-02：完成 T1403a：platform 补齐 sync primitives（mutex/condvar）与线程自识别（self/equal），并将 `runtime/c/scoop_sync.c`、`runtime/c/scoop_channels.c`、`runtime/c/scoop_task_executor.c` 内部锁从 `pthread_*` 直接调用收敛到 platform API；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。
- 2026-04-02：修复 LLVM 后端 `Unit -> Any` 的最小装箱：支持把 `Unit` coerces 到 `Any`（ref），避免 `block/when` 等在 `Any` 语境下触发 `Unit -> Ref` coercion 失败；解锁 run-pass fixture `effect_escape_continuation_resume_cross_thread` 在 `--features llvm` 下回归通过。
- 2026-04-02：修复 LLVM codegen 的 escape continuation 捕获/恢复：continuation state 允许捕获 `Bool/Int`（word-sized handle，如 `Task<T>`/`Executor`），避免 step trampoline 中引用外层变量时报 `unknown local value`；custom non-resuming `handle` 的 body 以 expected type context codegen，避免 `Any` 语境下触发 `Ref -> Int` coercion；同时 stdlib `Task.andThen` 以“两段 handle”规避 v0 后端“单个 perform 点”的限制。
- 2026-04-01：完成 T0632：类型系统为函数类型保留 effect row 的 `closed` 标记（区分 `/ Pure` 与 `/ Pure!`，并在类型显示中输出 `!`）；typecheck 在所有“写入/转换到 Any”的位置加入门禁：仅允许 `(...)->R / Pure!` 的函数值擦除到 `Any`（effects 不可运行时保真）；新增 typecheck fixtures 回归。
- 2026-04-01：完成 T0921：`k.resume(value)` 的 one-shot 违规语义从进程级 `exit(3)` 改为 `Raise.raise(RuntimeError.ContinuationAlreadyResumed)`；sysroot `RuntimeError` 增加对应 variant；typecheck 将 `k.resume` required effects 升级为 `E + Raise<RuntimeError>`；runtime 侧在重复 resume 时写入 Raise perform slot 并置位 flag；更新 run-pass/typecheck fixtures 回归。
- 2026-04-01：完成 T1319e：sysroot 新增 `scoop.task`（`Executor` + `Task<Int>` 最小适配接口），LLVM codegen 将其映射到 `runtime/c/scoop_task_executor.c`；新增 typecheck + run-pass fixtures 回归 task state 与完成回调行为。
- 2026-04-01：将 `TODO.md` 中 T1320（`std` v4：net/async adapters/testing/utilities）拆分为 T1320a～T1320d，以保持“可单独实现 & 单独验证”的粒度。
- 2026-04-01：完成 T1320a：sysroot/std 增加 `scoop.test` 最小断言工具（`assertTrue/assertFalse/assertEqInt/assertSomeInt/assertNoneInt`），并新增 typecheck + run-pass fixtures 回归。
- 2026-04-01：完成 T1320b：sysroot/std 增加 `Executor.spawn/await` 与 `Task<Int>.map/andThen`（v0：Int 专用），并新增 typecheck + run-pass fixtures 回归 async adapters 的基本行为（初版 stdlib 内部曾用 `__TaskAwaitInt` 作为 escape continuation 适配层；已在 T1320e 中清理）。
- 2026-04-01：完成 T1320c：sysroot `scoop.io` 新增 `stdoutWriteLine/stderrWriteLine`（写入并追加换行），runtime C 落地并接入 LLVM codegen 映射；新增 typecheck + run-pass fixtures 回归。
- 2026-04-01：完成 T1320d：sysroot 新增 `scoop.net` 最小声明表面（TCP handle + capability gating），并新增 typecheck fixture 回归。
- 2026-04-01：完成 T1322：typecheck 放开“命名参数后 trailing lambda”Kotlin-like 例外，并为形参映射补齐 trailing-lambda + 默认参数的 fallback；新增 infer fixtures 覆盖 disambiguate/ambiguous 两类场景回归。
- 2026-04-01：完成 T1323：默认参数在命名实参语境下支持“中间参数省略”（`f(a = 1, c = 3)`），并新增 typecheck + hir fixtures 回归。
- 2026-04-01：完成 T1324：parser 支持多 trailing lambda；typecheck 放开“命名参数后多个末尾 lambda”并扩展形参映射 fallback；新增 infer/typecheck fixtures 回归。
- 2026-04-01：将 `TODO.md` 中 T1325（varargs spread：集合/迭代器桥接与调用规则）拆分为 T1325a～T1325c，以保持“可单独实现 & 单独验证”的粒度；完成 T1325a：sysroot 增加 `MutableArray<Int>.toArray` 声明面，stdlib 落地复制实现，并新增 typecheck fixture 回归 `*xs.toArray()` spread。
- 2026-04-01：完成 T1325b：typecheck 在 vararg spread 的非 `Array/tuple` 诊断中，对常见集合追加迁移提示（`toArray/asSet/asMapView`），并新增 typecheck fixture 回归错误信息。
- 2026-04-01：由于依赖 T1328（迭代协议升级为 `Iterator.next(): Option<T>`），将 `TODO.md` 中 T1325c 移动到 T1328 之后以保持依赖顺序。
- 2026-04-01：完成 T1326a：delegated properties 的 `lazy` 线程安全模式（None/Publication/Synchronized）：HIR lowering 支持解析 `LazyThreadSafetyMode.*`，class init side table 注入并初始化 lazy mutex 字段，getter lowering 实现三种模式；新增 run-pass fixtures 覆盖 None/Synchronized/Publication 并发语义回归；并修复 object/class init lowering 产生的 ctor call-sites 未合并导致 ctor call 误判的问题。
- 2026-04-01：完成 T1018：基于 T1017 审计结论，`std v1` 与 Kotlin runtime gap 的“纯 Scoop 补齐”目标边界内无 `needs_new_intrinsic` blocker；T1018 作为 gate 任务以 no-op 形式完成（无新增 intrinsic/backends），保留未来出现真实 case 时再 reopen 的策略。
- 2026-04-01：完成 T1027：sysroot 引入 `scoop.unsafe.__AtomicInt`（内部原子整型）与 `__atomicIntLoad/__atomicIntStore/__atomicIntCompareExchange`，LLVM codegen 直接生成 atomic load/store/cmpxchg（SeqCst）；新增 run-pass fixture `unsafe_atomic_int_basic.*` 回归。
- 2026-04-01：完成 T1025：sysroot `scoop.unsafe` 补齐 raw pointer 规范 API（`addressOf/stackAlloc` + `Ptr<T>.cast/load/store/plus/minus`）并保留迁移期旧名；typecheck 支持 `var` 形参的 lvalue 门禁（用于 `addressOf(var: T)`）、成员调用显式类型实参，以及在泛型实例化/substitution 中保持 `Ptr<T>` 的 GC-free pointee 门禁；新增/更新 unsafe_nogc fixtures 回归。
- 2026-03-31：将 `TODO.md` 中 T1317f（`std`：`Hashable` + `List/Set/Map` 与迭代/算法）进一步拆分为 T1317f1～T1317f4，以保持“可单独实现 & 单独验证”的粒度（先落地 `Hashable` 约束/typecheck，再推进集合与算法 run-pass 回归）。
- 2026-03-31：完成 T1317f1：sysroot 增加 `Hashable` 接口并让 primitive types 声明实现；typecheck 在“builtin 标量 → interface”的可赋值判断中复用 sysroot FQN 的 supertypes，使 `where T: Hashable` 对 `Int/Bool/...` 生效；新增 typecheck fixtures 回归 pass/fail。
- 2026-03-31：完成 T1317f3：stdlib 为 `Array/MutableArray/List/MutableList`（优先 `Int` 版本）提供 `forEach/map/filter/fold`（effect-polymorphic）；并扩展 typecheck 的 lambda 期望类型下推到 2 参数以解锁 `fold(0) { acc, x -> ... }`；新增 run-pass fixture `stdlib_iter_algorithms_basic.*`，在 `cargo run -p scoop --features llvm -- test` 下可回归通过。
- 2026-03-31：完成 T1317f4：stdlib 基于 `MutableArray` 落地 `Set/MutableSet`（Int 专用）与 `MutableMap`（Int->Int 专用，线性查找 + copy 语义），并提供只读视图 `Set`/`MapView`；新增 run-pass fixture `stdlib_set_map_basic.*`，与 sysroot 的 `scoop.collections.Map`（delegated property 表面）保持兼容不冲突。
- 2026-03-31：拆分 T1318（`std` v2：io/fs/path/process/env/time）为 T1318a～T1318e，并完成 T1318a：sysroot 新增 `scoop.env.getOrNull` 与 `scoop.time.nowUnixMillis`；LLVM codegen 映射到 runtime C（`getenv/gettimeofday`）；新增 typecheck + run-pass fixtures `std_env_time_*` 回归。
- 2026-03-31：完成 T1318b：sysroot 新增 `scoop.fs.readAllText/writeAllText`（UTF-8）声明面；runtime C 落地为 `scoop_fs_read_all_text_utf8/scoop_fs_write_all_text_utf8`；LLVM codegen 映射到 runtime 符号；新增 run-pass fixture `std_fs_text_basic.*` 回归（并修正 run-pass fixtures 中误用关键字 `out` 的用例）。
- 2026-03-31：完成 T1318c：sysroot 新增 `scoop.process.args/exit`；LLVM 入口 `main(argc, argv)` 保存 argv 到 runtime；runtime C 提供 `scoop_process_args_array/scoop_process_exit`；driver `scoop run` 支持 argv 透传；新增 run-pass fixture `std_process_args_exit_basic.*` 回归。
- 2026-03-31：完成 T1318d：sysroot 新增 `scoop.path.normalize/join/basename/dirname`；runtime C 提供 `scoop_path_*`（最小归一化与切分）；LLVM codegen 映射到 runtime 符号；新增 typecheck + run-pass fixtures `std_path_*` 回归。
- 2026-03-31：完成 T1318e：sysroot 新增 `scoop.io.stdoutWriteString/stderrWriteString/stdinReadLine`；runtime C 落地 `scoop_io_*`（stdout/stderr 写入 + stdin readLine）；LLVM codegen 映射到 runtime 符号；fixtures runner 新增 `RUN-STDIN` 支持并新增 `std_io_*` fixtures 回归。
- 2026-03-31：拆分 T1319（`std` v3：sync/thread/channels/task support）为 T1319a～T1319e：先以 T1319a 固定 `scoop.sync` 的最小声明面并用 typecheck fixtures 回归，后续再逐步接入 runtime/LLVM/run-pass（避免一次性引入多线程与调度耦合）。
- 2026-03-31：完成 T1319a：sysroot 新增 `scoop.sync`（`Mutex/CondVar/Once`）最小声明面与操作函数签名；新增 typecheck fixture `std_sync_api_surface_ok.scoop` 回归；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。
- 2026-03-31：完成 T1319b：runtime/c 新增 `scoop_sync_*`（pthread backend）并接入 `crates/scoop_runtime` 构建；LLVM codegen 映射 `scoop.sync.*` 到 runtime 符号并处理 `destroy` overload；新增 run-pass fixture `std_sync_basic.*`；同时修复 run-pass fixtures 中的关键字 `out` 与 `std_path_basic.stdout` 末尾空行，保证 `cargo run -p scoop --features llvm -- test` 可全量回归通过。
- 2026-03-31：完成 T1319c：sysroot 新增 `scoop.thread`（`Thread` + `threadSpawn/join/sleepMillis/yield/currentId`）声明面；runtime/c 新增 `scoop_thread_*`（pthread backend）；LLVM codegen 映射到 runtime 符号并支持传递 `() -> Unit` closure；新增 typecheck + run-pass fixtures `std_thread_*` 回归。
- 2026-03-31：完成 T1319d：sysroot 新增 `scoop.channels`（`Channel<T>` + `channelCreate/send/recv/close`）声明面；runtime/c 新增 `scoop_channels_*`（pthread backend，unbounded 队列）；LLVM codegen 映射 `scoop.channels.*` 到 runtime 符号并让 `recv` 返回 `Option<T>`；为保证 run-pass 能通过 `threadSpawn` 传递 channel，LLVM codegen 补齐 closure capture v0（immutable captures，env 用 `malloc` 分配并经 `env_ptr` 传入）；新增 typecheck + run-pass fixtures `std_channels_*` 回归（`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test` 通过）。
- 2026-03-29：`TODO.md` 中 T1307（trailing lambda）原任务同时要求前端推断与 run-pass 回归；但当前 LLVM `main` codegen 尚未支持闭包/函数值调用，因此将其拆分为：
  - T1307a：前端（resolver/typecheck）补齐隐式 `it` + 期望类型下推推断；
  - T1307b：后端/run-pass 回归（lambda 被调用）。
  并将 T1307b 移动到 T1314 之后，避免阻塞“首个 `[TODO]` 可直接实现”的顺序。
- 2026-03-29：完成 T1307a：resolver 在 `{ body }` lambda 作用域内预注入隐式 `it` 绑定；typecheck 在期望函数类型为单参数时将其解释为隐式 `it: T`，并新增 infer fixture 回归 `takes { it + 1 }` 可推断通过。
- 2026-03-30：完成 T1307b：LLVM codegen 补齐最小“函数值/闭包”可执行链路（closure object + indirect call），并新增 run-pass fixture 回归 `takes { it + 1 }` 在运行期确实调用 lambda（stdout：`42`）。
- 2026-03-30：完成 T1023：为顶层 `@ThreadLocal/@Global var` 生成 TLS/进程全局静态存储，并新增 run-pass fixture 回归跨函数读写。
- 2026-03-29：完成 T1308：支持 `vararg` 参数与调用点 `*expr` spread（最小语义：最多一个 vararg 且必须为最后一个形参；spread operand 仅支持 `scoop.core.Array<T>` 或 tuple，并逐元素做可赋值检查），新增 typecheck fixtures 回归覆盖。
- 2026-03-29：完成 T1309：操作符重载补齐位运算/移位（`& | ^ << >>`）到 `and/or/xor/shl/shr` 的映射，并支持 unary `~` 绑定到 `inv()`；新增 typecheck fixtures 回归覆盖与稳定诊断。
- 2026-03-29：完成 T1310：import alias 补齐 shadowing 与显式 import 的优先级（同包/本地声明 > 显式 import（含 alias）> star import），并新增 resolve fixtures 回归覆盖（alias shadowing + alias vs star）。
- 2026-03-29：完成 T1311：object/companion object 语义补齐——支持单例值在表达式位置引用并触发 once 初始化；并把 companion 自身可见性作为 `TypeName.member` 的访问上界，新增 run-pass/typecheck_multi fixtures 回归覆盖。
- 2026-03-29：完成 T1312：类初始化顺序补齐——支持 class ctor call 与实例字段读取/写入；初始化按 Kotlin-like 顺序执行（property initializer → `init {}` blocks → secondary ctor body），并新增 run-pass fixture 回归覆盖（含 primary ctor `val` 参数属性在 init 阶段可通过 `this.x` 访问）。
- 2026-03-29：完成 T1313：标准 delegated properties（`lazy/observable/vetoable/map-backed`）最小可执行语义——通过 HIR lowering 特判 + class init side table 注入落地，避免依赖运行期 `PropertyMeta` 与闭包调用；新增 3 个 run-pass fixtures 覆盖 lazy 初始化一次、observable/vetoable 回调顺序与 map-backed 读取。
- 2026-03-29：完成 T1212：在 typecheck 中支持 `callee<T>()` 的显式类型实参调用（`Call(TypeApply(...))`），并用规范说明 + comptime/typecheck fixtures 固化“反射调用在运行期语境下遵循 const fun 规则回退”为普通调用。
- 2026-03-29：完成 T1213：sysroot 补齐 scope functions（`let/run/also/apply`）的 effect-polymorphic 声明，并打通 `x.run { ... }` 的前端链路（resolver 泛型 receiver 扩展候选、跨文件 `<eff E>` 签名 lowering、receiver lambda expected-context 推断、跨文件 effects lowering 防 panic）。
- 2026-03-29：完成 T1214：补齐反射 intrinsics（`alignOf/variantsOf/superTypesOf/paramsOf`）的 sysroot 声明与 const 解释器支持，并新增 comptime fixture + 单测回归覆盖。
- 2026-03-29：完成 T1215：补齐编译期元数据（`TypeKind/VariantMeta/ParamMeta` 等），升级 `variantsOf/paramsOf` 返回值并在 const 解释器侧产出可读字段（含 annotations/default args 的最小回归），更新 comptime fixtures + 单测回归覆盖。
- 2026-03-29：完成 T1218：编译期注解访问补齐复杂注解参数（常量表达式/数组/enum/class-literal）求值与读取，并新增 comptime fixture + 单测回归覆盖。
- 2026-03-29：完成 T1217：sysroot `scoop.delegates.lazy` 补齐 `lazy(mode, initializer)` 重载，并新增 fixtures 覆盖 `lazy(LazyThreadSafetyMode.None) { ... }` 与“缺失导入时报错”；同时修复 delegated property 的 delegate nominal type 推导，使其在 overload set 返回名义类型一致时仍可工作。
- 2026-03-29：规范更新：补齐 value-only enum、`@CLayout(aligned, packed)`、`@ThreadLocal/@Global`、`@Extern(lib, name)`、`@CallingConvention`、`@Safe`、`Platform/getPlatform()`、type descriptor release callback、`Ptr/FunPtr/stackAlloc/addressOf` 与 internal atomics、以及 `Cone.toml` 的平台选择器；并同步更新 `PLAN.md`/`TODO.md` 对应任务拆分。
- 2026-03-29：完成 T1020：扩展内建 `@Extern`（支持 `lib/name` 参数 + extern 顶层变量声明），并在 driver clang 链接阶段透传 `-l<lib>`；新增 typecheck fixtures 与 driver 单测回归。
- 2026-03-29：完成 T1022：对顶层 `var` 强制要求 `@ThreadLocal/@Global` 显式标注，并在 typecheck 阶段门禁其类型必须为 GC-free 值类型；新增 typecheck fixtures 回归。
- 2026-03-29：完成 T1303：确认并回填 TODO 状态：`object`/`companion object` 的语法与 name resolution 已实现，并由 parse/resolve fixtures 回归覆盖。
- 2026-03-29：完成 T1304：支持 `for (x in xs) { ... }` 语句解析，并在 typecheck 中按迭代协议做最小语义检查（v0：`iterator/hasNext/next`；规范已升级为 `Iterator.next(): Option<T>`，迁移见 TODO T1328）。
- 2026-03-29：完成 T1305：实现尾部默认参数的调用点补齐：HIR lowering 将“少传默认参数”的调用改写为 block（局部 `val` 绑定 + 完整调用），并在 typecheck 中纳入形参默认值表达式的类型检查；新增 run-pass fixture 回归 `f()` 输出 `3`。
- 2026-03-29：完成 T1306：实现命名参数语义：命名实参按形参名匹配并允许重排；命名参数之后禁止位置参数；重复/未知 name 在 name span 报稳定诊断；新增 typecheck fixtures 回归覆盖。
- 2026-03-29：完成 T1302：typealias 补齐泛型实例化与跨包循环检测，并把 typealias RHS 作为 `alias_of` 导出到 `.cone`（ScoopIR）；下游注入 RHS 后可在 typecheck lowering 阶段展开；新增 typecheck/typecheck_multi/typecheck_cone_archive fixtures 回归覆盖。
- 2026-03-28：完成 T1103：新增 `scoopc::cone::scoopir`（public API 的稳定 JSON schema + 导出器），并把 `tests/fixtures/scoopir/**` 接入 `scoop test` 作为新 phase（`.scoopir.json` golden 回归）。
- 2026-03-28：完成 T1106：为 `api.scoopir` 的 schema.version 增加版本协商（允许读取 <= 当前版本；更高版本给出稳定错误码 `scoop::cone::scoopir_schema_version_not_supported`），并新增单测覆盖。
- 2026-03-28：完成 T1107：`scoop build` 支持读取 consumer `Cone.toml` 的 `.cone` 依赖图（DAG；循环依赖报错），并在启用 `llvm` 时复用“同一编译单元” lowering 结果生成目标产物（避免后端二次 parse/resolve）；新增单测覆盖 build+deps 与（llvm 下）运行 stdout。
- 2026-03-28：完成 T0629b：Cone.toml 支持 `[entry-points].exports` 配置库导出入口，并在 typecheck 阶段按 entry point 规则强制其显式声明 `/ Pure!`；新增 typecheck_cone_archive fixtures 回归覆盖多入口与错误场景。
- 2026-03-28：完成 T0321b：`.cone` 增加 `SYMBOL_VISIBILITY.json` 并在消费侧注入 non-public 符号占位符，使下游引用依赖的 `internal/private` 得到稳定诊断 `scoop::resolve::not_visible`（新增 typecheck_cone_archive fixtures 回归）。
- 2026-03-28：完成 T0322：resolver 支持跨包 extension 导入与发现（显式 import / star import / 可见性过滤），并把可见 extension 候选写入调用点候选集；新增 resolve_cone fixtures `extension_imports` 回归。
- 2026-03-28：完成 T1109：pre-specialize 扩展到类型实例（`[pre-specialize].types`）；`PRE_SPECIALIZE.json` 新增 `types` 索引，并在 typecheck_cone_archive fixtures 中新增 hit/miss 回归用例。
- 2026-03-28：完成 T1201：HIR `FunDecl` 增加 `is_const` 标记并从 AST 传播；typecheck headers 为 `const fun` 增加最小门禁（禁止 non-Pure effect row 与 `eff` 参数）；新增 hir/typecheck fixtures 回归。
- 2026-03-28：完成 T1202c：const 解释器支持 `const fun` 调用（局部 `val`/`return`/block 末尾表达式返回 + 递归上限），并把 `tests/fixtures/comptime/**` 接入 `scoop test` 新 phase（`.comptime` golden 回归），新增 pass/fail fixtures 覆盖。
- 2026-03-28：完成 T1203：const 解释器支持执行 `comptime { ... }` 与 `comptime if`（含 else-if 链），并新增单测 + `tests/fixtures/comptime` 回归覆盖。
- 2026-03-28：完成 T1204：sysroot 补齐反射 intrinsics v0（`nameOf/sizeOf/fieldsOf`）声明；parser 支持 `callee<T>()` 显式类型实参调用；const 解释器内建实现 `nameOf/sizeOf/fieldsOf`（v0：struct 字段名列表 + 基础类型 size）；新增单测与 comptime fixture 回归覆盖。
- 2026-03-28：完成 T1205：splice operator `value.[field]` 最小实现：const eval 支持按字段名/`{name:String}` 读取 struct 常量字段；typecheck 支持字符串字面量字段 splice，并对非字面量 field 保守退化为 `Any`；新增单测与 fixtures 回归覆盖。
- 2026-03-28：完成 T1206：新增 RTTI v0（type id + size/align + struct 字段 offset）与 `scoop dump-rtti` 子命令（支持 `--type`）；新增单测覆盖。
- 2026-03-28：完成 T1207：const 解释器支持执行 `comptime for`（v0：整数范围 `a..b` + tuple/array 迭代；暂不支持 break/continue），并新增单测 + comptime fixture 回归覆盖。
- 2026-03-28：完成 T1208：sysroot 新增 `TypeMeta/FieldMeta/PropertyMeta` 并将 `fieldsOf<T>()` 升级为返回 `FieldMeta` 列表；const 解释器内建产出字段 `name/type/index`（`TypeMeta.name` 可读），并允许 target 为 `struct/class`；新增单测与 comptime fixture 回归覆盖。
- 2026-03-28：完成 T1209：sysroot 新增 `AnnotationMeta/AnnotationArgMeta` 与 `annotationsOf<T>()` 声明；const 解释器内建实现 type-level `annotationsOf<T>()`（读注解名与字面量/常量表达式参数），并新增单测与 comptime fixture `annotation_access_v0_basic` 回归覆盖。
- 2026-03-28：完成 T1210：HIR lowering 为 delegated property 生成合成 `PropertyMeta` 常量引用，并在 `getValue/setValue` 调用处传参；新增 HIR fixture `delegated_property_lowering` 回归覆盖。
- 2026-03-28：完成 T1211：typecheck 为 `const fun` 增加更完整静态门禁：仅允许调用 `const fun/@Intrinsic`，并禁止 lambda、class ctor 与 boxing（可能分配）；新增 typecheck fixtures 回归覆盖。
- 2026-03-27：完成 T0618：新增 `__scoop_thread_spawn_join_resume_u64`（sysroot + LLVM codegen 映射 + runtime pthread helper），并新增 run-pass fixture `effect_escape_continuation_resume_cross_thread` 回归跨线程 resume。
- 2026-03-27：完成 T0915b：复用 `effect_escape_continuation_resume_cross_thread` 用例，并回填 `TODO.md` 状态与验收命令。
- 2026-03-27：完成 T0621：新增 run-pass fixture `generator_yield_iter_int_basic`，用 effect + escape continuation（`, k ->`）构造最小 yield/迭代器 demo，并用 stdout golden 回归输出顺序。
- 2026-03-27：完成 T0916：新增 run-pass fixture `effect_handler_stack_nearest_three_levels_and_arm_outside_scope` 回归三层嵌套 handler 的最近匹配与 arm self-capture 避免。
- 2026-03-27：完成 T0625：LLVM codegen 支持最小自定义 non-resuming effect（slot 1-word payload）的 `perform/handle`，并新增 run-pass fixture `effect_custom_nonresuming_nested_nearest_and_arm_outside_scope` 回归嵌套 handler 的最近匹配与 arm re-perform 不自捕获。
- 2026-03-27：完成 T0917：runtime 增加最小 `Task<T>`/executor 原语（task 状态机 + continuation 入队/恢复 + completion 回调 + 显式 start），并新增 `scoop_runtime` 集成测试回归回调顺序与状态转换。
- 2026-03-27：完成 T0918：runtime 增加 once/guard 原语（`scoop_once_begin/scoop_once_end`），LLVM object/companion init 接入该原语，并新增跨线程访问的 run-pass fixture 与 `scoop_runtime` 多线程回归测试。
- 2026-03-27：完成 T0919：runtime 增加 `scoop_once_guard_canonicalize`（基于 `dlsym(RTLD_DEFAULT, ...)` 选取进程内 canonical guard），并新增 `scoop_runtime` 集成测试 `once_guard_cross_dylib` 覆盖“先访问后 dlopen”的动态链接场景（Linux 同步补齐 `-ldl`）。
- 2026-03-27：完成 T1001：parser 支持声明注解使用 `@Name(...)` 并写入 AST（最小支持无参/字面量参数），新增 parse fixture `annotation_use_fun_basic` 与更新相关 AST golden 回归。
- 2026-03-27：完成 T1002：typecheck 增加 `annotation class` 识别与 `@Name(...)` 引用校验（data-only 形态约束 + 非注解类用作注解报错），并新增 typecheck fixtures 覆盖。
- 2026-03-27：完成 T1003：typecheck 增加内建注解 `@Unsafe/@NoGC/@Extern/@Intrinsic` 的最小合法性检查；并对 `@Extern/@Unsafe` 调用点施加 unsafe context 门禁；新增 `tests/fixtures/unsafe_nogc/*` 回归。
- 2026-03-27：完成 T1004：parser/typecheck 支持 `@Unsafe { ... }` 块并在 typecheck 传播 unsafe context；新增 unsafe_nogc fixtures 覆盖“block 内允许调用 @Extern / block 外仍禁止”。
- 2026-03-27：完成 T1005：typecheck 增加 `@NoGC` 的最小静态门禁（禁止调用非 `@NoGC/@Extern`、禁止已知 boxing 分配点），并新增 unsafe_nogc fixtures 覆盖。
- 2026-03-27：完成 T1006：LLVM codegen 支持 `@Extern("symbol")` 的符号名映射与 C ABI 调用；HIR lowering 提取 extern side table；新增 run-pass fixture `extern_symbol_println_basic` 回归。
- 2026-03-27：完成 T1007：sysroot 新增 `@Intrinsic sizeOf` 的最小可调用声明（以 overload 形式暴露），LLVM codegen 将 `scoop.core.sizeOf` lowering 为编译期常量（按 LLVM TargetData 计算 store size），并新增 run-pass fixture `intrinsic_size_of_int_word` 回归。
- 2026-03-28：完成 T1013：sysroot 补齐 `@TailRec/@AllowIntrinsic/@Suppress/@CLayout/@Target/@Retention` 与 `AnnotationTarget` enum；parser 支持 `@Target(AnnotationTarget.X, ...)` 的最小注解参数解析；typecheck 对非法 target 名给出稳定错误码，并新增 parse/typecheck fixtures 覆盖。
- 2026-03-28：完成 T1016a：typecheck 读取注解类上的 `@Target/@Retention` 并在使用点强制执行；限制 `@Target/@Retention` 只能用于 `annotation class`；对 `@Retention("comptime"|"cone")` 做最小合法性检查；新增 typecheck fixtures 覆盖。
- 2026-03-28：完成 T1016b：`.cone` 新增 `ANNOTATION_CLASSES.json`（schema v0）导出 cone-preserved 注解类元信息，并在消费侧注入为 `annotation class`；同时从 `api.scoopir` 过滤 comptime-only 注解类；新增 typecheck_cone_archive fixtures 回归覆盖可见性边界。
- 2026-03-28：完成 T1019：注解参数解析支持常量表达式/数组字面量/enum 值/class literal；typecheck 在注解使用点执行“参数绑定 + 编译期常量判定 + 类型匹配”，并新增 parse/typecheck fixtures 覆盖（含非常量参数报错）。
- 2026-03-28：完成 T1102：实现 cone source package 加载（`src/**/*.scoop` + `src/main.scoop`），并让 `scoop build/run` 支持输入包目录；新增 `scoop` 单测覆盖。
- 2026-03-27：完成 T1008：sysroot 暴露 `GC.pin/GC.unpin` 并在 typecheck/codegen lowering 到 runtime `scoop_pin/scoop_unpin`；`Pinned` 为非泛型 handle（`value: Any`）；新增 run-pass fixture `gc_pin_unpin_basic` 与 compile-fail fixture `gc_pin_value_type_is_error` 回归。
- 2026-03-27：完成 T1009：typecheck 支持最小 unsafe 指针原语 `addrOf/load/store` 并强制 unsafe context 门禁；新增 unsafe_nogc fixtures 覆盖。
- 2026-03-27：完成 T1010：sysroot 新增 `scoop.unsafe` 模块声明（`Ptr<T>` + `ptrToUIntPtr/uintPtrToPtr`），并新增 resolve fixture 覆盖 `import scoop.unsafe.*` 与符号引用。
- 2026-03-27：完成 T1011：typecheck 为 `scoop.unsafe.Ptr<T>` 增加 pointee 必须为 GC-free 值类型的 well-formedness 校验（含新错误码），并新增 unsafe_nogc fixtures 覆盖 `Ptr<Int>`/`Ptr<String>`/`Ptr<Option<String>>`。
- 2026-03-27：完成 T1012：Index 记录函数 type params 与 builtin flags，使 sysroot 泛型 intrinsics 可在跨文件调用点被 typecheck；并确保 `ptrToUIntPtr` 只能在 unsafe context 调用，`as` cast 不作为指针转换；新增 unsafe_nogc fixtures 覆盖。
- 2026-03-25：完成 T0902：runtime `scoop_alloc` 改为基于 `malloc` 的最小可用实现，并新增 `scoop_runtime` 集成测试覆盖。
- 2026-03-25：完成 T0819：`scoop build` 支持 `--emit-llvm/--emit-obj/--emit-asm`，fixtures runner 新增 build phase 与 `emit_llvm_basic` 用例（产物写入 `target/fixtures`）。
- 2026-03-25：完成 T0821：runtime 最小字符串承载（`ScoopString`）与 `scoop_print/scoop_println`（C），并新增 clang 链接 smoke test 覆盖输出行为。
- 2026-03-25：完成 T0822：LLVM codegen 支持字符串字面量 lowering，并把 sysroot `print/println(String)` 映射到 runtime `scoop_print/scoop_println`；新增 run-pass fixture 覆盖 `println("hello")` 的 stdout。
- 2026-03-25：完成 T0823：LLVM codegen 支持 f-string 插值（Text/Expr 分片拼接，最小支持 `{String}`/`{Int}`），并新增 runtime `scoop_format_{i64,u64}`；新增 run-pass fixture 覆盖 `val s = f"hi {name} {n}"; println(s)` 的 stdout。
- 2026-03-25：完成 T0824：tuple 字段访问语法统一为 `t._0` / `t._1`，并补齐 `print/println(Int)` 的最小 codegen（runtime formatting + `scoop_print/scoop_println`）；新增 run-pass fixture 覆盖 tuple 求和 stdout 与 `t.0` parse compile-fail 回归。
- 2026-03-25：完成 T0825：`when` codegen 支持 or-pattern（`A | B`）与 guard（`pat if cond`），guard 为 false 时会回落到后续分支；新增 run-pass fixture 覆盖两类语义。
- 2026-03-25：完成 T0826：LLVM codegen 支持 `Option<RefType>` niche 表示与 rich enum 的 oversized variant boxing；run-pass fixtures 新增 `RUN-STDERR-CONTAINS`/`RUN-STDOUT-CONTAINS` 子串断言用于稳定验证 lint warning；新增 run-pass fixtures 覆盖 Option niche 与 oversized boxing。
- 2026-03-25：完成 T0828：LLVM codegen 支持 `object` / `companion object` 的单线程 once 初始化（module-local guard）与静态属性访问；新增 run-pass fixture 覆盖 once 初始化与 `ClassName.member`。
- 2026-03-26：完成 T0615：LLVM codegen 补齐 try/catch/finally 的 `finally` 清理语义（正常路径与 raise/unwind 路径都执行一次），并新增 run-pass fixture 覆盖输出顺序。
- 2026-03-26：完成 T0620：新增 `spawn/join` 结构化并发最小模型（`Int` 句柄 + runtime helper），并补齐 typecheck/run-pass fixtures 与 runtime 测试覆盖。
- 2026-03-26：完成 T0622：引入 `Task<T>` 的最小类型/库模型：sysroot 增加 `Task`，`Async.await` 改为 `Task<T> -> T`；typecheck 侧把 `spawn` 返回值与 `await/join` 参数切到 `Task<T>`；HIR/codegen 侧把 `Task` 暂落到 word-sized 句柄并保持 run-pass 可回归。
- 2026-03-26：完成 T0623：支持 `async fun` 降糖到 `Task<T>`：parser 增加 `async` modifier；resolve/index 与 typecheck 侧保证调用点返回 `Task<T>`；`async fun` 函数体内的 `Async` performed effects 不向外层 required effects 传播；HIR lowering 将返回值包装为 task 句柄（`__scoop_task_spawn_int`，early stage）。
- 2026-03-26：完成 T0818：effect codegen（flag-based Raise/try-catch）：支持 `Raise<RuntimeError>` payload 写入/恢复、`EnumName.UnitVariant` 常量生成，并新增 run-pass fixture 回归。
- 2026-03-26：完成 T0630：perform slot ABI 升级为多 word payload（`len + words[8]`），新增读写 API；`Raise.raise` 统一采用 2-word `(kind, value)` 编码并在 handler 边界断言回归，确保 lowering/codegen/runtime 对齐。
- 2026-03-26：完成 T0907：runtime 引入 type descriptor v0（`size_bytes + trace bitmap/trace_fn`），并新增 guard page 集成测试确保扫描按 size 裁剪不越界。
- 2026-03-26：完成 T0908：runtime 对象头（`ScoopGcObjectHeader`）与最小 heap 布局：`scoop_alloc` 初始化 header 字段并固化字段偏移（static asserts），新增对象头集成测试，并同步更新 `Int -> Any` 装箱布局为 `{ header, payload }`。
- 2026-03-26：完成 T0909：GC v0 单线程 shadow stack roots 扫描（visitor API），新增 `scoop_runtime` 集成测试覆盖。
- 2026-03-26：完成 T0910：GC v0 单线程 mark-sweep（手动触发 `scoop_gc_collect`），新增 heap 统计 debug API、runtime 集成测试与 run-pass fixture 覆盖。
- 2026-03-23：完成 T0624：use-site `Type<eff Row>` 的默认化/实例化接入 typecheck，并让名义类型的 `eff` row 参数参与 subeffecting；补齐从 `Type<eff E>` 实参类型推断 `E` 与 required effects 联动的 fixtures 覆盖。
- 2026-03-23：完成 T0626：parser/AST 支持闭合 effect row `E!` 语法（`!` 低于 `+`，作用于整个 row），并新增 parse fixtures 覆盖。
- 2026-03-23：完成 T0627：typecheck 侧为 entry point 补齐闭合 row `Pure!` 的门禁与诊断（显式写 open `/ Pure` 会提示改为 `Pure!`），并新增 `Pure!` + try/catch / unhandled Raise fixtures 覆盖。
- 2026-03-23：T0628（RowExpr 高级语义）跨度较大，已拆分为 T0628a/T0628b，先以 `E + ...` 的实例化/推断为最小可回归落点。
- 2026-03-23：完成 T0628a：typecheck 侧支持 `E + R` 形式的 row（函数类型 `/ Row` 与 use-site `Type<eff Row>`），调用点按 `found - base` 推断并回填实例化结果，新增 infer/effects fixtures 覆盖。
- 2026-03-23：完成 T0628b：引入 `TypeId` 级的 row 替换 plan，支持在 tuple/Option/多层 function type/nominal args 中实例化 `E + ...`，并补齐闭合 row 引用 row 变量（`E!`）的稳定诊断与 fixtures 覆盖。
- 2026-03-23：完成 T0629a：program boundary 的 entry point 引入 cone-aware 规则（仅 consumer cone 的 `main` 视为 entry point），并新增 `typecheck_cone` fixtures runner 与用例覆盖。
- 2026-03-23：完成 T0701：新增 `scoopc::hir` 骨架与最小 lowering，并落地 `scoop dump-hir` 调试输出命令。
- 2026-03-23：完成 T0612：HIR 增加 `perform/handle` 节点与 lowering；MIR 预留对应 terminator；新增 HIR fixtures 回归。
- 2026-03-23：完成 T0704：新增 `scoopc::monomorph::MonomorphKey`（symbol + type args + effect row args）与单测覆盖，用于后续实例缓存。
- 2026-03-24：完成 T0706：AST→HIR lowering 补齐 `member access`（`receiver.member`）节点与解析结果写入，并新增 HIR fixtures/golden 回归覆盖成员访问/成员调用/成员赋值。
- 2026-03-24：完成 T0707：MIR 引入 cleanup/unwind 最小模型（`UnwindAction` + `Terminator.unwind` + `ResumeUnwind`），并新增 MIR 单测覆盖。
- 2026-03-24：完成 T0708：引入最小 MIR lowering（if/when → CFG），新增 `scoop dump-mir` 与 `tests/fixtures/mir/**` golden 回归。
- 2026-03-24：完成 T0709：MIR lowering 补齐 while/break/continue（loop CFG + 跳转目标栈），并新增 `tests/fixtures/mir/while_break_continue.*` 回归。
- 2026-03-24：完成 T0711：捕获闭包（val 捕获）落地 capture set 计算与 env tuple lowering（`MakeTuple`/`TupleGet`），并新增 HIR/MIR fixtures 回归。
- 2026-03-24：完成 T0712：单态化 v0（函数泛型）：typecheck 收集 `MonomorphKey`，并提供 `scoop dump-ir` 输出单态化实例 MIR（`id::<Int>`/`id::<String>` 两实例可回归）。
- 2026-03-24：完成 T0713：MIR lowering 把 HIR `perform/handle` 落到 MIR terminator（`Perform/Handle`），并新增 `tests/fixtures/mir/handle_perform.*` golden 回归。
- 2026-03-24：完成 T0714：捕获闭包对 `var` 引入 CaptureBox 语义（HIR capture 标记 `mutable`；MIR 新增 `CaptureBoxNew/Get/Set` 并在函数内预扫描 closure captures 决定 boxing），新增 `tests/fixtures/{hir,mir}/closure_capture_var.*` 回归覆盖。
- 2026-03-24：完成 T0801：为 `scoopc` 增加 feature-gated `inkwell` 依赖（`llvm` feature 默认关闭）以保持 CI/本地构建可用，并在 `README.md` 说明启用方式与 LLVM/`llvm-config` 前置。
- 2026-03-24：完成 T0802：新增 `scoopc::llvm` 最小 codegen（生成空 `main` 返回 0），并提供 `scoopc --emit-llvm` 写出 `.ll` 用于验证 target triple/pipeline。
- 2026-03-24：完成 T0804：新增 `scoopc --emit-obj` 把最小 LLVM module 编译为 `.o` 并落盘，补齐单测覆盖产物非空，为后续链接（T0806）做准备。
- 2026-03-24：完成 T0805：driver `scoop build` 接入前端 parse/resolve/typecheck，并准备输出路径（当前不做 codegen/链接）。
- 2026-03-24：完成 T0806：driver 在启用 `--features llvm` 时生成 `.o` 并调用 clang 链接早期 C runtime，产出可执行文件（单测覆盖 link 与运行返回 0）。
- 2026-03-24：完成 T0807：driver 实现 `scoop run`（临时目录 build + exec，stdout/stderr 与退出码透传）。
- 2026-03-24：完成 T0112：run-pass fixtures runner 让 `EXPECT-EXIT`/`TIMEOUT` 真正生效，并新增超时/信号终止/退出码不匹配的稳定诊断与 fixtures 覆盖。
- 2026-03-24：完成 T0108：fixtures 支持 `// ENV: KEY=VALUE` 指令，run-pass 执行子进程时注入 env，并新增单测覆盖。
- 2026-03-24：完成 T0808：LLVM codegen v1 支持 Int/Bool 字面量、一元/二元运算（含位运算/移位与 shift count mask）、`val` 局部绑定、`return`/隐式返回，并新增 run-pass fixture 覆盖 `UInt8 >>` 的逻辑右移语义。
- 2026-03-24：完成 T0809：LLVM codegen v2 将 main 内 locals 统一映射为 `alloca` + `load/store`，并支持 `var` 的赋值更新；新增 run-pass fixture 覆盖读写回归。
- 2026-03-24：完成 T0810：LLVM codegen v3 支持顶层函数调用（按简单 C ABI 传参/返回），并且只为 `main` 可达的函数生成/声明，避免未使用的泛型/占位签名影响 codegen；新增 run-pass fixture 覆盖 `add(1,2)`。
- 2026-03-24：完成 T0811：LLVM codegen v4 支持 struct 值类型布局与字段访问：为 struct FQN 生成 named LLVM struct type（opaque + set_body），struct literal 构造用 `insertvalue` 组装 aggregate，字段读取对 `localStruct.field` 走 struct GEP + `load`；新增 run-pass fixture 覆盖 struct literal + 字段读取（当前因 typecheck 对整数字面量推导为 `Int`，fixture 字段类型使用 `Int` 并用 exit code 断言结果）。
- 2026-03-24：完成 T0812：LLVM codegen v5 支持 tuple 值类型布局与元素访问：为 tuple 生成 LLVM struct type，tuple literal 用 `insertvalue` 组装 aggregate；`t._0` / `t._1` 在局部变量路径走 struct GEP + `load`（fallback 用 `extractvalue`）；同时在 typecheck 中支持 tuple 元素访问语义以通过前端检查；新增 run-pass fixture 覆盖 tuple 求和并用 exit code 断言结果。
- 2026-03-24：完成 T0813：LLVM codegen 支持 rich enum 的最小 `{tag, payload}` 表示（payload 为 word-sized int），并在“期望类型语境”下支持 enum variant ctor（含 0-参数 variant 以 `None()` 形式构造），以及 `when` 对 enum 的 tag 判别与 `Some(v)` binder 解构；新增 run-pass fixture 覆盖 `Some(1)`/`None()` + `when` 分支并用 exit code 断言结果。
- 2026-03-24：完成 T0814：LLVM codegen 将 enum/bool 的 `when` 降到 LLVM `switch`（保持“按源码顺序”的首个匹配 arm 语义），并支持 tuple `when` 的字段比较与 binder；新增 run-pass fixture 覆盖 enum/bool/tuple 三类 `when` 并用 exit code 断言结果。
- 2026-03-30：完成 T0813b：支持 value-only enum（`enum E: Int { A = 0, ... }`）端到端链路：parser 增加判别值 AST；typecheck 在 TypeLower 阶段门禁底层类型必须为整型标量；HIR side table 记录 enum repr（tagged-union vs value-only）与判别值；LLVM codegen 将 value-only enum 直接表示为底层整型，并让 `EnumName.Variant` 常量与 `when` 分派按显式判别值工作；新增 parse/typecheck/run-pass fixtures 回归覆盖。
- 2026-03-30：完成 T1314：新增 Kotlin runtime gap 审计文档 `KOTLIN_RUNTIME_GAP_AUDIT.md`，产出 capability matrix（pure_scoop_ok / needs_runtime_lib / needs_new_intrinsic 候选）并给出对 T1315/T1316/T1317 与 T1017/T1018 的落点建议。
- 2026-03-30：完成 T1017：新增 intrinsic gate 审计文档 `RUNTIME_STDLIB_INTRINSIC_AUDIT.md`（结论：std 主线不需要新增 intrinsic），并回写 `KOTLIN_RUNTIME_GAP_AUDIT.md` 3.3 记录落点。
- 2026-03-30：T1315（纯 Scoop 补齐 Kotlin runtime gap）范围过大，已拆分为 T1315a/T1315b/T1315c：本轮先落地 T1315a（`stdlib` prelude 注入 + 可回归的最小可执行 helper），为后续纯 Scoop stdlib/运行库（库层）代码提供稳定落点。
- 2026-03-30：完成 T1315a：新增 `stdlib/prelude.scoop`（`require/check`），并让 `scoop build/run` 自动注入 `stdlib/*.scoop`；后端 HIR lowering 支持 multi-file（并限制非入口文件不得含 source-backed literals），新增 run-pass fixture 回归 `require/check` 可被 try/catch 捕获。
- 2026-03-30：完成 T1315b：补齐 `stdlib` v1 的 Kotlin-like helpers：`requireLazy/checkLazy` 与 `Int.{also,let,run,apply}`；同时补齐 typecheck/HLIR lowering 支撑“sysroot 声明 + stdlib 实现”与 extension call/receiver 的后端可执行链路，并新增 run-pass fixtures 回归覆盖。
- 2026-03-30：完成 T1315c：新增 `IntProgression` 的 sysroot 声明表面，并在 `stdlib` 中提供 `Int.rangeTo/downTo(step)` 与 `IntProgression.forEach` 的纯 Scoop 实现（无新增 intrinsic）；新增 run-pass fixture 覆盖 up/down progression 的构造与迭代回归。
- 2026-03-30：完成 T1316：新增 `STDLIB_DESIGN.md`，给出 `std` 分层（core/alloc/std/platform）、推荐模块树与 capability matrix，并说明各模块对 runtime/platform backends（含 GC backend）的依赖边界；同时在 `README.md` 增加入口链接。
- 2026-03-30：将 `TODO.md` 中 T1317（std v1）拆分为 T1317a~T1317f，以保持“可单独实现 & 单独验证”的粒度；并完成 T1317a：sysroot 增加 `MutableArray<T>` 声明面，typecheck 支持数组字面量 `[...]` 在期望类型语境下推断为 `Array<T>` / `MutableArray<T>`，新增 typecheck fixture 回归覆盖。
- 2026-03-30：完成 T1317b：在 sysroot 中补齐 `Array<T>` / `MutableArray<T>` 的最小公开 API 声明（`size/get/set`），并新增 typecheck fixture 回归覆盖。
- 2026-03-30：完成 T1317c：HIR lowering 支持数组字面量 `[...]`，降为统一的 builder/intrinsics 调用形态，并新增 `tests/fixtures/hir/array_lit_lowering.*` golden 回归覆盖 `Array`/`MutableArray`（含空数组与 call args）。
- 2026-03-30：完成 T1317d：runtime/codegen 增加 `Array`/`MutableArray` 的最小 word-buffer primitive（alloc/len/get/set）与 array literal builder，并新增 run-pass fixture 回归 `size/get/set` 的可执行语义。
- 2026-03-30：完成 T1317e：stdlib 增加 `MutableArray<Int>` 的 `push/pop/insert/removeAt/splice` 扩展（early stage 先对 `Int` 提供落点；不新增 intrinsic；使用 T1317d 的 builder/word-buffer primitive 完成分配与搬移；同时补齐 LLVM codegen 对顶层函数 body `while` 语句的最小支持以保证可执行），并新增 run-pass fixture 回归边界条件与 stdout golden。
- 2026-03-30：完成 T0920：`ScoopTypeDescriptor` 增加可选 `release_fn`，GC sweep 回收对象时在 `free` 前回调一次（用于 FFI-managed 资源释放）；新增 `scoop_runtime` 集成测试回归“仅调用一次”语义。
- 2026-03-25：完成 T0815：生成的 `i32 @main()` 在执行 Scoop `fun main` 前调用 `scoop_runtime_init()`，并更新 LLVM 单测断言 IR 含该调用。
- 2026-03-25：完成 T0901：补齐 C runtime 的 `scoop_runtime_init`（一次初始化标记 + 可选 debug 日志），并新增 `scoop_runtime` 集成测试覆盖可调用性与可观察状态。
- 2026-03-25：完成 T0904：引入 mark-sweep GC 的数据结构骨架（heap/object header/free list）与最小自检，并让 clang 链接覆盖 `runtime/c/*.c`。
- 2026-03-25：完成 T0905：shadow stack `ScoopGcFrame` + TLS 链头（`current_frame`）与 push/pop API，并新增 `scoop_runtime` 集成测试覆盖。
- 2026-03-25：完成 T0816：LLVM codegen 为含 GC 引用的函数插桩 shadow stack frame（push/pop + roots 写入），runtime 新增 debug 扫描计数接口，并新增 run-pass fixture 覆盖。
- 2026-03-25：完成 T0613：补齐 effect runtime 的最小 ABI（active flag + perform slot 的读写 API），并在 LLVM codegen 侧提供 sysroot `__scoop_effect_*` 映射；新增 `scoop_runtime`/`scoopc` 单测与 run-pass fixture 回归覆盖。
- 2026-03-25：完成 T0614：实现 `Raise.raise` 的最小 flag-based unwinding（写 slot+flag、call-site 传播、try/catch 边界消费 slot 并清 flag），并新增 run-pass fixture；同时修复 resolver 对 try/catch lowering 合成 Ident 的 FQN 推导（确保可解析到 `scoop.core.Raise.raise`）。

## 1. 仓库结构与工具链（阶段 0：工程化）

### 1.1 代码结构（Rust workspace 拆分）

- [x] `crates/scoopc/`：编译器前端 + 中端 + LLVM 后端（inkwell）（初始骨架已建立）
- [x] `crates/scoop/`：CLI（`scoop build/run/test`），负责调用 `scoopc`、链接、跑测试（已建立骨架）
- [x] `crates/scoop_runtime/`：早期运行时构建 glue（clang + C runtime）（已建立骨架）
- [x] `runtime/c/`：早期 C 运行时（GC + effect runtime + 线程注册 + once/sync/thread/io/fs/path/process 等平台 glue）（已实现 baseline：mark-sweep + STW + shadow stack roots；T15 将以 LLVM StackMap/statepoint roots 替换并为 moving/compaction 打通更新链路）
- [x] `sysroot/`：`.scoop` 形式的内建 API 声明（已包含 `core/delegates/collections/unsafe/gc` 与 `env/time/fs/path/process/io/sync/thread/channels` 等主线模块；后续继续扩展为更完整的 std capability matrix，并保持平台差异不泄漏到 Scoop 侧）
- [x] `tests/fixtures/`：所有编译期/运行期 fixtures（见 §10）（已建立可扩展 runner：支持 parse/resolve/typecheck/hir/mir/emit/build/run-pass 等 phase，并已具备规模化回归套件）
- [x] `tools/`：辅助脚本（已加入 `tools/scoop_tools`：spec doctest fixtures 抽取/一致性检查；后续扩展 golden 工具）

> 当前已采用 Rust workspace 拆分；后续新增模块优先通过新增 crate 或在现有 crate 内分层来维持依赖方向清晰，避免形成循环依赖。

### 1.2 基础构建与开发体验

- [x] 引入依赖：`clap`、`thiserror`、`miette`（诊断）、`tracing`（后续再引入 `inkwell`）
- [x] 统一日志：`tracing` + `tracing-subscriber`
- [ ] 提供命令行（拆分为可迭代子任务）：
  - [x] `scoop test`（fixtures harness：按 phase 执行并支持 pass/fail/stdout/stderr/exit/timeout/env/stdin 等断言；可在 `--features llvm` 下跑 run-pass）
  - [x] `scoop dump-ast`（AST Debug 输出；用于 parse fixtures/诊断回归）
  - [x] `scoop dump-hir`（HIR Debug 输出；用于后续 lowering/回归）
  - [x] `scoop dump-ir`（单态化实例 MIR Debug 输出；用于回归/调试）
  - [x] `scoop build <main.scoop> -o <bin>`（默认仅做前端检查；启用 `--features llvm` 后生成 `.o` + clang 链接 runtime 输出可执行文件；支持 `--emit-llvm/--emit-obj/--emit-asm`）
  - [x] `scoop run <main.scoop>`（先 build 后 exec；需要启用 `--features llvm`；支持 argv 透传）
- [x] `build.rs`：编译 `runtime/c`（强制 clang；当前通过 `crates/scoop_runtime` 实现）
- [x] CI：最小矩阵（ubuntu）跑 `cargo test --all` + `scoop test`

**本阶段 DoD**
- 能构建出 `scoop` 可执行文件（哪怕只是空壳），`scoop test` 至少能跑通一条最小路径并可扩展为全量回归套件。

---

## 2. 词法/语法/AST（阶段 1：前端可解析）

### 2.1 词法分析（Lexer）

- [x] Token 集：关键字、标识符、数字、字符串、基础运算符、注解（`@`）、泛型尖括号、常用 modifier（`public/internal/private/open/abstract/sealed/inline/override`）等（见 `scoopc::syntax::lexer`）
- [x] 补齐位运算与移位运算符 token：`&` `|` `^` `~` `<<` `>>`（spec §2.3.4 / Appendix B.8）
- [x] 注释：行注释 `//`、块注释 `/* */`（当前实现为**非嵌套**；若后续需要可扩展为嵌套）
- [x] 字符串：
  - 普通字符串（`"..."`）
  - `f` 插值字符串（`f"..."`）（lexer 识别字面量边界；parser 将 f-string token 拆为文本段 + 插值 expr 列表，AST `FStringExpr`/`FStringPart` 已实现）
  - raw 三引号字符串（`""" ... """`）与 `f""" ... """`
  - 大括号转义（`{{` / `}}`）属于字符串内容层语义，lexer 无需特殊处理
- [x] Span（源代码位置）基础设施：`Span` + `SourceFile` 行列映射

### 2.2 语法分析（Parser）

- [x] Parser v0（最小可用）：支持 `package` / `import` / 顶层 `fun` + 基础类型声明（`class/interface/struct/enum/effect`），函数/类型体仅保证 `{ ... }` 括号平衡并记录 span
- [x] fun 签名最小解析：参数列表 + 返回类型（支持 Path/泛型参数列表/tuple/nullable 的 `TypeRef` 子集）
- [x] 工程化：拆分 `scoopc::parser` 为多文件模块（cursor/decls/types/file），避免单文件过长，便于后续语句/表达式迭代
- [ ] Kotlin-like 声明（逐步补齐）：`class/interface/struct/enum/effect/val/var/...`
  - [x] 顶层 `val`/`var`：解析声明头；initializer 暂仅保留 span（不解析表达式）
  - [x] 类型体内部成员声明：`val`/`var`/`fun`/nested type（T0201：TypeBody + Member 建模，parse_type_body 实现）
  - [x] 类型体 `val`/`var` 成员声明头：解析 `val x: T`/`var x: T`，带 pass/fail fixtures 覆盖（T0202）
  - [x] 类型体 `fun` 成员声明头：解析 `fun name(params): Ret { ... }`（body 仍是 span），含 pass/fail fixtures 覆盖（T0203）
  - [x] 类型体嵌套类型声明：class/interface/struct/enum/effect 均可作为成员，支持多层嵌套与修饰符（T0204）
  - [x] 声明修饰符列表：顶层与类型成员支持 `public/internal/private/open/abstract/sealed/inline/override`；AST 保存 `modifiers` 并排序去重（顺序无关）；新增 parse fixture 覆盖（T0245）
  - [x] class/interface 继承列表与主构造头（简化版）：解析 `class Dog(name: String) : Animal(name), IFoo` 的最小语法；AST `TypeDecl` 新增 `primary_ctor`/`supertypes`；新增 pass/fail fixtures 覆盖（T0248）
  - [x] 属性声明与 accessors：`ValDecl` 新增 `accessors: Vec<Accessor>` 字段；`Accessor` 节点支持 `get()`/`set(value)` + 表达式体（`= expr`）或块体（`{ stmts }`）；类型体中 `parse_property_decl` 在 `parse_val_decl` 后探测 `get(`/`set(` 模式并解析 accessor；`get`/`set` 作为上下文关键字（soft keyword），不加入 lexer 关键字表；6 个 pass/fail fixtures + 5 个 unit tests 覆盖（T0234）
  - [x] 委托属性 `by expr`：`ValDecl` 新增 `delegate: Option<Expr>` 字段；`parse_property_decl` 在 `parse_val_decl` 后探测 `by` 上下文关键字并解析委托表达式；`by` 与 accessors 在语法层互斥；支持 `val x: T by lazy { ... }` 等 trailing lambda 形式；2 个 pass/fail fixtures + 3 个 unit tests 覆盖（T0235）
  - [x] Rich enum variant 声明：`Member::Variant(EnumVariant)` 新增 AST 节点；`EnumVariant` 含 `name: Ident` + `params: Vec<Param>`；`parse_type_body` 接收 `TypeKind` 参数，对 `Enum` 类型识别裸标识符作为 variant 开始；`parse_enum_variant` 解析 `Name` / `Name(val field: T, ...)` 形式；variant 参数要求 `val` 关键字 + 类型标注；1 个 pass + 2 个 fail fixtures + 3 个 unit tests 覆盖（T0236）
- [x] `typealias` 声明：解析顶层 `typealias Name = Type` 并纳入 AST（T0251，为 sysroot 标准别名与 Kotlin 兼容铺路）
- [x] Expr/Stmt 最小骨架（T0205）：Ident/IntLit/StringLit/BlockExpr/Missing + Stmt::Expr/Stmt::ValDecl
- [x] val/var initializer 解析为原子表达式（T0206）：`ValDecl.init` 从 `Option<Span>` 升级为 `Option<Expr>`，支持 ident/int/string 原子
- [x] 块表达式解析（T0207）：`parse_block_expr` 解析 `{ stmt* }` 为 `BlockExpr { stmts }`；`FunBody::Block` 改用 `BlockExpr`（含 stmts）替代旧 `Block`（仅 span）；块内支持表达式语句与 val/var 声明
- [x] 块内 val/var 局部绑定（T0208）：`parse_stmt` 已支持 `val x: T = expr`/`val x = expr`/`var x = expr`；新增 pass/fail fixtures 覆盖（含 `val = 1` 缺名报错）
- [x] 函数调用表达式（T0209）：`parse_expr` 引入后缀调用循环，解析 `f(a, b)` 为 `CallExpr { callee, args }`；支持嵌套调用 `f(g(x))`、尾随逗号；`parse_stmt` 和 `ValDecl.init` 改用 `parse_expr`
- [x] 成员访问表达式（T0210）：后缀循环新增 `.` 分支，解析 `a.b` 为 `FieldAccessExpr { receiver, field }`；支持链式 `a.b.c(1)` 与调用组合
- [ ] 语句/表达式（逐步补齐）：lambda
  - [x] Lambda AST 节点：`Expr::Lambda(LambdaExpr)` + `LambdaParam`（T0221）
  - [x] Lambda 表达式解析：`{ params -> body }` / `{ -> body }` 的 lookahead 歧义消解 + 参数列表 + block body 解析；6 个 pass/fail fixtures 覆盖（T0222）
  - [x] Trailing lambda：`f(a, b) { ... }` 与 `expr { ... }` 形式，尾随 lambda 作为最后一个 `CallArg::Positional(Lambda)`；bare `{ body }` 无 `->` 时解析为零参数 lambda；5 个 pass fixtures 覆盖（T0232）
- [x] `when` 表达式解析：`when (subject) { pattern -> body, ... }`（T0215：AST `WhenExpr`/`WhenArm`/`WhenPattern` + parser + pass/fail fixtures）
  - [x] Pattern v0（T0238）：`WhenArm.pattern` 迁移为 `Pattern`，删除 `WhenPattern`；支持 wildcard `_`、int/string/bool 字面量、`is`/`!is` Type、`else`、裸标识符 bind；2 个 pass fixtures + 6 个 unit tests 覆盖
  - [x] Pattern v1 — tuple pattern（T0239）：`parse_when_pattern` 新增 `(` 检测调用 `parse_tuple_pattern()`，解析 `(p1, p2, ...)` 为 `Pattern::Tuple`；支持嵌套 pattern、尾随逗号、空 tuple `()`；`no_call` 标志 + `looks_like_tuple_pattern_ahead()` lookahead 消解 arm body call 与下一 arm tuple pattern 的歧义；1 个 pass + 1 个 fail fixture + 6 个 unit tests 覆盖
  - [x] Pattern v2 — enum variant pattern（T0240）：`parse_when_pattern` 在裸标识符后 peek `(` 调用 `parse_variant_pattern()`，解析 `Name(p1, p2, ...)` 为 `Pattern::Variant`；支持嵌套 variant（`Some(Some(x))`）、空参数（`Point()`）、尾随逗号、wildcard 字段；裸标识符（无括号）保持为 `Bind`（消歧留给 resolve 阶段）；1 个 pass + 1 个 fail fixture + 6 个 unit tests 覆盖
  - [x] Pattern v3 — struct pattern（T0241）：`parse_when_pattern` 在裸标识符后 peek `{` 调用 `parse_struct_pattern()`，解析 `Name { field, field: pattern, ... }` 为 `Pattern::Struct`；支持 shorthand（`x`）、rename（`x: pattern`）、空 struct（`Unit {}`）、尾随逗号、嵌套 pattern（`first: Some(x)`）；1 个 pass + 1 个 fail fixture + 6 个 unit tests 覆盖
  - [x] Pattern v4 — or-pattern（T0242）：`parse_when_pattern` 拆分为 `parse_when_pattern`（含 `|` 循环）+ `parse_when_pattern_atom`（单个 pattern）；`A | B` 解析为左结合 `Pattern::Or`；支持多级 `A | B | C`、嵌套在 tuple/variant/struct 内的 or-pattern、混合 literal/bind/variant/wildcard；1 个 pass + 1 个 fail fixture + 6 个 unit tests 覆盖
  - [x] Pattern v5 — guard `if <expr>`（T0243）：`parse_when_arm` 在 pattern 与 `->` 之间检测 `if` 关键字，解析 guard 表达式并包装为 `Pattern::Guard`；`looks_like_tuple_pattern_ahead` 更新为同时接受 `->` 和 `if` 作为 tuple pattern 判定条件；1 个 pass + 1 个 fail fixture + 6 个 unit tests 覆盖
- [x] `if` 表达式解析：`if (cond) thenExpr else elseExpr`（T0214：AST `IfExpr` + parser + pass/fail fixtures）
- [x] 值类型更新表达式：`expr with { path: value, ... }`（spec §2.6）（T0216：AST `WithExpr`/`WithField` + parser + pass/fail fixtures）
- [x] 运算符优先级（Pratt parser）：二元运算 `+ - * / %`、比较 `< > <= >= == !=`、逻辑 `&& ||`、位运算 `& | ^`、移位 `<< >>`；一元前缀 `- ! ~`（T0252）；括号分组 `(expr)`；`Percent` token 新增
- [x] Elvis `?:` 二元运算（最低优先级）与 not-null 断言 `!!` 后缀运算（T0212）
- [x] 类型判断/转换操作符：`is`/`!is`/`as`/`as?`（与比较运算符同优先级，RHS 为 TypeRef）（T0213）
- [x] 声明处泛型参数列表：`fun id<T>(...)` / `struct Box<T> { ... }` — AST `TypeParam` 节点 + `type_params` 字段 + `parse_type_param_list`（T0218）
- [x] 泛型语法补齐：type args 支持 `*`（star projection），type params 支持 `in/out` 声明处变型（T0249）
- [x] struct literal AST 节点：`Expr::StructLit(StructLitExpr)` + `StructLitField`（T0223）
- [x] struct literal 解析：`TypeName { field: expr, ... }`（T0224）— `looks_like_struct_lit()` lookahead 在 `parse_expr_primary` 中识别 `Ident(.Ident)*(<...>)? { (Ident: | })` 模式，调用 `parse_struct_lit_expr()` + `parse_path_type_inner()` 解析；6 个 pass/fail fixtures 覆盖
- [x] 关键歧义：struct literal vs lambda（对应 spec §12）（T0225）— `looks_like_struct_lit()` 增加 `has_arrow_inside_braces()` 扫描：在 `{ Ident :` 匹配后，前扫顶层 `->` 来排除 lambda with typed params；4 个 pass fixtures 覆盖
- [x] `return` 语句解析：`Stmt::Return(ReturnStmt)` + `parse_return_stmt`（T0226）— 支持 `return` 与 `return expr`；3 个 pass fixtures 覆盖
- [x] 赋值语句解析：`Stmt::Assign(AssignStmt)` + `parse_stmt` 中 `= rhs` 检测（T0227）— 支持 `x = expr` 与 `a.b.c = expr`；2 个 pass + 1 个 fail fixtures 覆盖
- [x] `while` 循环表达式解析：`Expr::While(WhileExpr)` + `parse_while_expr`（T0228）— 支持 `while (cond) body`；`break`/`continue` 作为 `Stmt::Break`/`Stmt::Continue`；lexer 新增 `While`/`Break`/`Continue` 关键字；2 个 pass + 1 个 fail fixtures 覆盖
- [x] 错误恢复：`parse_file_recovering()` 新 API，顶层/块内/类型体三级同步点恢复，收集多个诊断
- [x] safe-call `?.`：`FieldAccessExpr` 与 `CallExpr` 新增 `safe: bool` 标志；postfix 循环处理 `QuestionDot` token，支持 `x?.member` 与 `x?.foo(args)`；2 个 pass + 1 个 fail fixtures 覆盖（T0229）
- [x] 函数参数默认值：`Param` 新增 `default: Option<Expr>` 字段；`parse_param_list` 解析 `= expr`；1 个 pass + 1 个 fail fixtures 覆盖（T0230）
- [x] 命名参数调用：新增 `CallArg` 枚举（`Positional(Expr)` / `Named { name, value }`）；`CallExpr.args` 改为 `Vec<CallArg>`；`parse_call_arg` 通过 lookahead `Ident + =` 区分命名参数与位置参数；2 个 pass fixtures 覆盖（T0231）
- [x] 扩展函数 receiver：`FunDecl` 新增 `receiver: Option<TypeRef>` 字段；`parse_fun_receiver_and_name` 通过 lookahead 识别 `Type.name(...)` / `pkg.Type.name(...)` / `List<T>.name(...)` 模式并拆分 receiver 与函数名；type params 支持 spec 风格 `fun <T> Type.name(...)` 和 Kotlin 风格 `fun name<T>(...)`；resolve 侧同步处理 receiver TypeRef；3 个 pass + 1 个 fail fixtures + 5 个 unit tests 覆盖（T0233）

### 2.3 语法树表示（AST/Parse Tree）

- [ ] 建议区分：
  - `ParseTree`（保留所有 token/节点，利于错误恢复与格式化）
  - `AST`（更语义化的节点，利于后续分析）
- [x] AST（最小骨架）：File/Package/Import/Fun/TypeDecl/Block/Ident/Param/TypeRef，节点带 span 并可回切源文本
- [x] Pattern AST 节点（T0244/T0460）：新增 `Pattern`（Wildcard/Bind/Tuple/Struct/Variant）与 `ValBinding`，用于 block 内 `val` 解构绑定；`when` 分支模式仍使用 `WhenPat`（后续再统一迁移）
- [ ] Parser 收尾补齐：
  - [x] `import foo.bar.Baz as Qux`（Appendix B.7）
  - [x] use-site effect row 实参：`Type<eff Row>`（spec §3.4）
  - [x] pattern rest：`..`（spec §4.2）
  - [x] receiver function type：`T.(A, B) -> R / E`（spec §7.5）
  - [x] 泛型 `where` 子句（spec §3 / Appendix B）
- [ ] Kotlin-like 声明补齐：
  - [x] `init { ... }` blocks（Appendix B.2.2）
  - [x] secondary constructors（Appendix B.2.2）
  - [x] `object` / `companion object` 声明（Appendix B.9）

**本阶段 DoD**
- `scoopc` 能解析大部分 spec 示例，不做类型检查也能 `dump-ast`。

---

## 3. 包与名字解析（阶段 2：可绑定符号）

### 3.1 包系统（Cone 的源级部分）

- [x] `package` 声明、`import`、通配 `*`（已支持解析 + 最小名字绑定：TypeRef 按 import/star import 解析）
- [x] 可见性：`public/internal/private`
- [ ] 作用域：文件级、类/接口/结构体内部、泛型参数作用域（块级局部 `val/var` 已完成，见 T0304）

### 3.2 符号表与解析

- [x] 顶层符号索引（最小子集）：基于 `package + 顶层声明名` 构建 FQN 索引并检测重复定义；索引区分 type/fun/value 命名空间（见 `scoopc::resolve`）
- [x] 类型体成员索引：把 type body 的 fields/methods/nested types 纳入索引并检测同一类型体内重复定义（T0302）
- [x] 两阶段/多阶段解析（T0308）：
  - 先收集声明头（type/function/field signatures）
  - 再解析函数体与初始化表达式
- [x] import 解析与名字绑定（最小子集）：对 fun/val 顶层签名里的 `TypeRef::Path` 做存在性解析（含 star import）
- [x] import 表（T0303）：显式 import 按 type/value 命名空间拆分，并保留 `*` import 前缀（为 expr 解析准备）
- [x] `typealias` 名字解析：alias 作为 type-level symbol 纳入索引；冲突与可见性诊断
- [x] 作用域：块级（函数体/表达式块内局部 `val/var`，含遮蔽）（T0304）
- [x] 表达式裸标识符绑定写回：为 `ExprKind::Ident` 记录其解析到的局部/顶层引用（T0305）
- [x] 调用点候选收集：`Call(Ident)`/成员调用/构造调用写回候选集合 + 调用形状；多候选留给后续 typecheck 决议（T0319）
- [x] 成员访问解析（`.`）：把 `receiver.member` 绑定到类型体字段/方法并写回 `MemberIdent.resolved`（T0310）
- [x] 扩展成员 fallback：member 优先于 extension（同包）且 receiver 类型可匹配（T0312）
- [x] 作用域：泛型参数（声明处 type params 在签名内可解析）（T0309）
- [x] `where` 子句约束解析：约束左侧必须命中 type param scope，右侧 `TypeRef` 按包前缀/import 规则解析（T0320）
- [x] 作用域：`this`（类型体成员/扩展函数体）与主构造参数在成员里可见（T0313）
- [ ] 同名优先级：成员/顶层/扩展（逐步补齐）
- [x] import alias 绑定与冲突规则：`import foo.bar.Baz as Qux`（Appendix B.7）
- [x] class 初始化阶段作用域：property initializer / `init` / secondary constructor（T0316）
- [x] `object` / `companion object` 的名字解析与成员可见性（T0317：支持 `Obj.member` 与 `ClassName.member`）
- [ ] overload set 建模：
  - [x] 索引侧：顶层/成员/扩展函数与构造函数收集为候选集合（T0318）
  - [x] 调用点/构造点：从“唯一 callee”升级为“候选集合 + 调用形状”（T0319）
  - [x] typecheck：普通函数调用最小重载决议（T0453：过滤后唯一/歧义）
  - [x] typecheck：class 构造调用最小重载决议（T0454：primary/secondary + 默认参数）
  - [x] typecheck：扩展函数调用重载决议（T0455：member 优先 + receiver/参数 specificity）
  - [x] typecheck：重载冲突诊断（T0457：重复签名 / 仅返回类型不同 / 默认参数冲突）
  - [x] inference：重载决议与泛型/默认参数/命名参数/`eff` row 推断联动（T0512）
  - [x] inference：most-specific tie-break（T0513：参数/receiver 更具体 + 默认参数更少优先；歧义诊断列出候选签名）
- [ ] 跨包可见性：`public/internal/private` 在 source package / `.cone` 依赖边界上的规则与诊断（拆分为子任务；T0321b 依赖 T1105 `.cone` 读取；T1105 已完成）
  - [x] T0321a：resolver 引入 cone 边界 + source-only 多 cone fixtures
  - [x] T0321b：接入真实 `.cone` 依赖后的可见性过滤（前置 T1105 已完成）
- [ ] 跨包扩展导入：extension 在显式 import / star import / 成员候选之间的可见性、shadowing 与候选收集（依赖 T0321b）

### 3.3 sysroot 注入

- [x] sysroot 文件与 loader 骨架：可发现并解析 `sysroot/*.scoop`（当前实现见 `scoopc::sysroot`）
- [x] 编译流程注入：通过 `scoopc::session::Session` 默认加载 sysroot，并在 `build_top_level_index` 中纳入名字解析环境
- [x] sysroot：补齐内建标量类型的“可见声明”（spec §2.3.4 / runtime §3）
  - `Int/UInt`：word-sized（随 target 指针宽度变化，Swift 约定）
  - 固定位宽整数：`Int8/16/32/64`、`UInt8/16/32/64`
  - 标准别名：`Byte/Short/UShort/Long/ULong`，以及 `UIntPtr = UInt`
  - 说明：这些类型是语言 builtin（布局/语义由编译器固定），但它们的可见声明由 sysroot 提供
  - fixtures：`tests/fixtures/resolve/sysroot_scalar_types_ok.scoop`
- [x] sysroot：运行时错误枚举 `RuntimeError`（`NullAssertionFailed`/`ClassCastFailed`），用于 `Raise<RuntimeError>`（T0419）

**本阶段 DoD**
- 能在无类型检查情况下做 name resolution，并对未定义符号给出准确 span 的错误。

---

## 4. 类型系统（阶段 3：先类型检查再优化）

### 4.1 类型表示（核心）

- [x] 区分引用类型 vs 值类型（spec §2）：内部 `TypeKind::{Ref, Value}` 已落地（T0401）
- [x] 从 sysroot 收集内建类型/效果的声明头（`TypeEnv`：kind + arity），为后续 lowering/typecheck 提供环境起点（T0402）
- [x] TypeEnv：收集 enum variants（tag + payload fields），并检测重复 variant/字段（T0425）
- [x] enum variant ctor：支持 `Some(x)` 风格构造并做参数/类型检查（T0426）
- [x] `TypeRef` → `Type` lowering：支持 `Path`/`Tuple`/`Nullable` + 泛型 arity 检查（T0403）
- [x] Nullability 语法糖：`T?` → `Option<T>`（lowering 阶段 desugar）（T0411）
- [x] 顶层声明头检查：`fun/val/type` 的签名最小约束（类型注解等）（T0404）
- [x] 表达式类型检查 v0：字面量（Int/String/Bool/Unit）（T0405）
- [x] 表达式类型检查 v0：变量引用（局部/参数/顶层）（T0406）
- [x] 表达式类型检查 v0：函数调用（参数数量/类型匹配；无重载/无默认参数）（T0407）
- [x] 表达式类型检查：成员访问（struct 字段 + class 字段/属性最小子集，`p.x` / `this.x`）（T0408/T0438）
- [x] struct 声明最小语义检查：字段重复/`var`/默认值约束（T0409）
- [x] struct literal 类型检查：字段存在性/重复/类型匹配 + 必填字段覆盖（当前：必须显式提供所有字段）（T0423）
- [x] tuple/Unit（0 元 tuple）：tuple 类型与 tuple 字面量 typecheck（T0410）
- [x] 最小子类型规则：`Nothing <: T`（用于 `return`/不可达分支/后续 `Raise.raise`）（T0420）
- [x] `!!` 非空断言：`Option<T>` → `T` 的静态类型规则（T0421a）
- [x] `?.` safe-call 与 Elvis `?:`：`Option<T>` 语法糖的类型规则（`x?.m()` 返回 `R?`；`x ?: y` 返回 `T`）（T0422）
- [ ] 内建整数模型（spec §2.3.4 / runtime §3）
  - （已在 `scoopc::ty` 中建模 `Int/UInt/IntN/UIntN`；运算/布局语义后续补齐）
  - [x] 整数/布尔运算符类型规则：一元 `! - ~`；二元算术/比较/位运算/移位（shift count 固定为 `Int`）与 `&&/||`（T0447）
  - `Int/UInt` 的 bit width = target pointer size
  - 固定位宽整数类型与类型大小/对齐（为 FFI/序列化提供稳定布局）
  - 整数运算语义：wrap-around、算术/逻辑右移、shift count mask（避免 target 相关 UB）
- [x] `typealias` 语义：类型层展开（用于 `Byte/UIntPtr` 等 sysroot 标准别名；循环 alias 报错）（T0446）
- [x] `Unit`、tuple、`Option<T>`（`T?` sugar）：类型表示与格式化输出已完成（语义/typecheck 后续）（T0401）
- [x] 函数类型（含 effect row）：`(A, B) -> T / E`（spec §7.5）— AST `TypeFun`/`RowExpr` + `parse_paren_type`/`parse_row_expr` + pass/fail fixtures（T0219）
- [x] 函数类型（Type 表示 + lowering + 最小子类型规则）：参数逆变/返回协变 + effect row containment（T0435）
- [x] receiver function type：`T.(A, B) -> C / R`（Type 表示 + lowering；receiver 按第一个参数参与逆变比较）（T0435）
- [x] 类型参数（`TypeKind::Param`）与声明处变型（`in/out` + 最小位置规则 + variance 子类型，仅 ref args 生效）（T0437）
- [x] 泛型约束（上界）：`where` 子句语义检查与实例化满足性（T0458）
- [ ] 泛型约束（更复杂形式）：下界/更完整 bound 形式与约束传播（留给推断/求解阶段逐步补齐）

### 4.2 声明类型：class/interface/struct/enum/effect

- [x] class：主构造 `val/var` 参数作为字段/属性 + 成员方法体最小 typecheck（T0438）
- [x] class：继承/override 的最小静态规则（final/open/abstract/sealed + override 检查）（T0439）
- [ ] class：虚表/方法分发与 codegen（先单继承）
- [x] interface：多实现、默认方法（可先限制默认方法 codegen）（T0440）
- [ ] struct：布局（字段顺序/对齐），不可变，值语义
- [x] enum（rich enum）：tag + union 布局 + niche/boxing/lint 元数据（T0449；codegen 另见 §8.2）
- [x] effect：像 interface 一样声明操作签名（T0601）

### 4.3 Boxing 与 Any

- [x] 值类型装箱到 interface/`Any`（spec §2.5）
- [ ] 先实现“语义正确”，性能优化（如 O(n) 显式转换）后置

### 4.4 模式匹配与 smart cast（spec §4）

- [ ] `when` 表达式（穷尽性检查可分阶段做）
  - [x] 分支结果类型（最小 LUB）：一致 → 该类型；不一致 → `Any`（T0414）
  - [x] 分支 pattern 最小类型检查：tuple/variant 限定 + binder 注入分支作用域（T0427）
  - [x] 穷尽性检查 v0：enum/Bool/Option + `else`/`_` 规则（T0428）
  - [x] 穷尽性检查 v1：嵌套组合覆盖（tuple/enum payload 递归）（T0459）
  - [x] guard 分支视为不可覆盖（需 `else`/`_`）（T0429）
- [x] `is` / `!is` + smart cast（T0413：最小子集，仅 `if (x is T)`/`if (x !is T)`；仅参数 + `val`）
- [x] `as` / `as?`：基础类型规则已实现（T0412）；按 spec 的运行时失败路径（`Raise.raise(RuntimeError.ClassCastFailed)`）待 effect 系统（required effect row/try-catch）补齐后接入

### 4.5 值类型更新（`with` 表达式）（spec §2.6）

- [x] 语义：并行更新（静态约束：禁止重复/包含路径）（T0415）
- [x] path 解析：`a.b.c: value`（字段路径必须存在且类型匹配）（T0415）
- 说明：`TODO.md` 中的 T0424 与以上两项重复，已由 T0415 覆盖（本节保持为实现状态来源）。
- [ ] lowering：生成“拷贝 + 覆盖字段”的构造逻辑（对嵌套 path 生成中间拷贝）

### 4.6 变量绑定与解构（spec §9 + Kotlin-like）

- [x] `val`/`var`：
  - 不可变/可变规则（`val` 不可再次赋值；`var` 可）（T0416）
  - `var` 的赋值类型检查：lhs 可写性（局部 `var` / class `var` 属性）+ rhs 可赋值（T0416/T0443）
- [ ] 解构绑定（destructuring）：
  - [x] tuple/struct 的 `val (a, b) = expr` / `val Point { x, y } = expr`（T0430）
  - [x] enum 的 `val Some(x) = expr` / `val Result.Ok(v) = expr`（T0460）
  - [ ] `when` 分支中的解构 pattern
- [ ] 控制流基础：`if/while/for/return/break/continue`（非局部 return 仅允许 inline lambda 实参）
  - [x] `return`：函数内 `return expr?` 返回类型检查与诊断（T0417）
  - [x] `while`：条件必须为 Bool；`break/continue` 仅允许在循环体内（T0442）

### 4.7 属性系统（spec §10）

- [x] 类属性（T0431：typecheck 侧最小规则）：
  - [x] 默认 getter/setter 视为存在（因此可能生成 backing field）
  - [x] `field` 仅在 accessor 内可见；computed 属性引用 `field` 报错
  - [x] backing field 判定 v0：initializer 或默认 accessor
- [x] 值类型属性：
  - [x] computed property 仅允许 getter-only（禁止 setter）
  - [x] computed property 不允许 initializer（避免 backing field）
  - [x] struct/enum 内属性不允许 `var`
- [x] 扩展属性（T0433：解析 + typecheck 侧门禁）：
  - [x] 顶层语法：`val/var ReceiverType.name: Type get()/set()`
  - [x] computed 约束：禁止 initializer / 禁止 `field` / getter 必需 / `var` 需 setter
  - [ ] lowering：编译为静态 getter/setter（receiver 作为第一个参数）
- [ ] 委托属性（delegated properties）：
  - [x] T0434a：`by` 语法 + 最小静态规则（仅 class；检查 `getValue/setValue` 名称存在性）
  - [x] T0434b：对接 `PropertyMeta` 并升级为签名检查（与 §13 comptime/反射联动）
  - [ ] lowering：生成 `$delegate` 字段 + getter/setter 转发到 `getValue/setValue`（T1210）

### 4.8 函数声明细节（spec §7）

- [x] `inline`：non-local return 门禁（lambda 中 `return` 仅允许出现在 inline 调用的 lambda 实参内；T0444）
- [ ] `inline`：实际 inlining/闭包消除等优化（IR/后端阶段）
- [ ] 扩展函数：
  - [x] 解析与分发规则（静态分发、member 优先；typecheck 降糖为 receiver 第一个参数）
  - [ ] codegen：receiver 作为第一个参数的普通函数
- [x] enum 完整语义：niche optimization、oversized variant boxing、variant size disparity lint（spec §2.3.2）（T0449：前端固定元数据；后端待落地）
- [x] pattern rest `..` 的类型检查与绑定规则（spec §4.2）
- [x] class 初始化模型：property initializer、`init` blocks、secondary constructors、初始化顺序（Appendix B.2.2）（T0448：最小 typecheck + delegation 门禁）
- [x] `object` / `companion object`：单例类型、成员访问、伴生对象解析（Appendix B.9）
- [x] 委托属性标准库面：`ReadOnlyProperty` / `ReadWriteProperty` 与 `scoop.delegates`（`lazy`/`observable`/`vetoable`/map-backed）（spec §10.4）
- [ ] 通用重载解析（函数 / 构造函数 / 扩展）：
  - 候选筛选：arity、receiver、可见性、命名参数、默认参数
  - 决议规则：最具体候选（most specific candidate）与稳定歧义诊断
  - [x] enum variant / pattern 在同名跨 enum 时按期望类型或 subject type 消歧

**本阶段 DoD**
- `scoopc` 能对一批无泛型/少量泛型的示例做类型检查（含 struct/enum/Option/when/is/as）。

---

## 5. 类型推断（阶段 4：约束求解）

对齐 spec §14：constraint generation + solving（非 HM W）。

- [ ] 约束表示：`τ1 <: τ2`、相等、行约束（effects）
  - [x] 相等约束 + 推断变量 + 最小 unify 骨架（T0501）
  - [x] 子类型约束 `τ1 <: τ2` 的求解（T0506）
  - [x] effects 行推断入口（T0508：public 强制 Pure、private/internal 可推断；依赖 required effects：T0604 ✅）
  - [x] effects 行参数 `eff` 推断（T0509）
- [x] 局部变量推断：`val x = expr`（T0502）
- [x] LUB（if/when 分支：相同类型 / Any fallback）（T0503）
- [x] 返回类型推断：缺省 return type 从函数体推断（T0507，spec §14.6）
- [x] lambda 推断 v0：参数类型下推（T0504，spec §14.7.2）
- [x] 泛型实参推断 v0：从调用参数推断单一类型参数（T0505）
- [ ] lambda 推断：更完整的返回类型合并与 effect row 推断（后续任务）
- [x] 错误报告：把“推断失败”映射到具体源 span 与最小可读解释（T0510）
- [ ] overload resolution 与推断联动：
  - 泛型实参、lambda expected type、默认参数、命名参数、trailing lambda 共同参与候选决议
  - effect rows / `eff` 参数也必须能参与重载筛选与歧义诊断
- [x] 真正的分支合并类型：LUB / 受限 union 的构造、比较与化简（替代简单 `Any` fallback）（T0514）
- [x] effect row 高级推断 v1：高阶返回透传 + row 归一化（T0515）

**本阶段 DoD**
- 能跑 `tests/fixtures/infer/**`：涵盖 if/when/lambda/泛型调用推断的 compile-pass/compile-fail。

---

## 6. 效果系统（阶段 5：先 `Raise`，再完整三种 arm）

### 6.1 静态层：effect row + 多态 + 推断

 - [x] 语法：
  - [x] 函数声明/函数类型的 `/ RowExpr`（T0603）
  - [x] `handle { ... } with { ... }`（T0605：仅 non-resuming arm `->`；arm 级错误恢复；`finally` 仅语法建模）
  - [x] `eff` 作为上下文关键字：`<eff E = Pure>`、use-site `Type<eff Row>`、function type `/ E`（默认值 + 显式实参 + 推断回填已落地）
  - [x] `+` 并集、`Pure` 空行
  - [x] 闭合行语法：`/ R!`（`!` 后缀作用于整个 row，不与 `+` 右操作数绑定；spec §5.8.4）
 - [ ] 规则：
  - effect operation 调用（T0602）：已支持 `Raise.raise(e)` 的限定名解析与最小类型检查
  - [x] effect operation 调用（泛型 op）：支持 `Async.await<T>` 等带 type params 的 effect op call（以及 handler arm head 的实例化），使 stdlib 可直接表达 `Async.await` 而不需要 `__TaskAwaitInt` 适配层（TODO T0602b）
  - required effects（T0604/T0606：已实现未声明的 effect 报错；支持 non-resuming `handle` 捕获；spec §14.7.1）
  - [x] RowExpr 静态语义：默认 `Pure` + `+` 并集 + containment `R1 ⊆ R2`（T0608）
  - [x] public 默认 `/ Pure` 的强制约束（T0508）
  - [x] private/internal 可推断 effect row（T0508）
  - [x] overriding：`R_over ⊆ R_base`（T0609）
  - [x] entry point 必须 `Pure!`（闭合纯；禁止 open `/ Pure`）（T0627/T0629）
  - [x] Continuation 类型建模与 `k.resume(value)` required effects 传播（T0611；spec §5.5）
  - [x] 闭合行额外约束：所有来源的 effect（含 callback 透传）都不能逃逸出函数边界（spec §5.8.4）
  - [ ] 函数值擦除到 `Any`：仅允许 `(...)->R / Pure!`（effects 不可运行时保真；spec §7.5）
  - 高级 row 语义：高级归一化、泛型 row 变量、必要的高阶 row 运算
 - [x] 语法糖：
  - [x] `try/catch/finally` → `handle { } with { Raise.raise -> } finally { }`（T0607）
  - [x] `!!` 失败 → `Raise.raise(RuntimeError.NullAssertionFailed)`（T0421b：静态 required effects；依赖 try/catch lowering：T0607）
  - [x] `as` 失败 → `Raise.raise(RuntimeError.ClassCastFailed)`（T0445；依赖 T0607）
  - [x] 多个 `catch` arm 与匹配顺序（不只单个 `catch`）

### 6.2 动态层：handler stack dispatch（Appendix A）

- [x] 运行时必须维护 **handler stack**（按“最近匹配 handler”分发）（T0913）
- [x] arm body 在 dispatch scope 之外执行（避免 self-capture）（T0913）

### 6.3 Codegen/Lowering：分三步落地

1) **非恢复 `->`（flag-based unwinding）**
   - [x] TLS：`__scoop_effect_active` + perform slot（T0906）
   - [x] `perform` 写 slot + set flag + return（T0614：先只覆盖 `Raise.raise`）
   - [x] 调用链传播：检查 flag，沿栈向外返回（T0614：先只覆盖顶层函数调用）
   - [x] handler 边界消费 slot 并清 flag，然后执行 arm（T0614）；`finally` 正确执行；必要时 re-raise（T0615）

2) **立即恢复 `-> resume`（栈 state machine）**
   - [x] 把 handle body 分段（v0：仅单个 perform 点）
   - [x] lifted locals（v0：只覆盖必要局部/跨段写回）
   - [x] while-loop 调度 state
   - [x] `resume(value)` 必须恰好一次（v0：运行期 one-shot 断言；违规给出稳定运行期错误，不 `exit/panic`）

3) **逃逸 continuation `, k ->`（堆 state machine + continuation 对象）**
   - [x] continuation 捕获 handler stack（fiber-local 语义）
   - [x] 支持跨线程 `resume`：恢复 captured handler stack 到当前线程 TLS（见 spec §5.5）
   - [x] one-shot：原子状态位保证并发下只能成功一次
   - [ ] one-shot 违规语义：第二次 `k.resume(...)` 通过 `Raise.raise(RuntimeError.ContinuationAlreadyResumed)` 报错（不 `panic/exit`；spec §5.5/§5.7）

- [x] use-site effect row 实参：`Type<eff Row>` 的类型检查（默认值 + 显式实参，纳入 nominal type identity；T0511）
- [x] use-site effect row 实参：由上下文/lambda body 反推的 row 实参推断（T0515）
- [ ] `Task<T>` 与 `async fun` 语义：
  - [x] `async fun foo(): T` desugar 为 `fun foo(): Task<T>`（T0623）
  - [x] 调用者签名不携带 `/ Async`（T0623）
  - [ ] `Task<T>` 懒执行，直到 `await` 或显式启动
- [x] Appendix A 一致性：嵌套 handler 必须支持“最近匹配 handler”分发，不能停留在单层 handler 模型
- [x] program boundary 不只 `main`：库导出入口、多 entry point 与 host/embedded 边界规则（TODO T0629）
  - [x] cone-aware entry point：仅 consumer cone 的 `main` 视为 entry point（TODO T0629a）
  - [x] 库导出入口 + host/embedded entry points（TODO T0629b，依赖 T1107）
- [x] perform slot ABI：从单 slot 扩展到可承载复杂 payload / 多 effect op 的稳定表示（T0630）

**本阶段 DoD**
- compile-pass + run-pass 覆盖 `Raise`、`try/catch/finally`、自定义 effect + handle，以及一个最小 async/await demo（T0619：`tests/fixtures/run-pass/async_await_minimal_int_basic.scoop`）。

---

## 7. 中间表示与单态化（阶段 6：为 LLVM 做准备）

### 7.1 HIR/MIR 设计

- 注：`perform` / `handle` 的 IR 节点（TODO T0612）依赖 HIR/MIR 骨架与 AST→HIR lowering（TODO T0701～T0703），因此在 TODO 中需要排在 T0703 之后，避免出现“首个 TODO 依赖未满足”的顺序问题。
- [ ] HIR：保留大部分结构但已解析/已类型化
  - [x] HIR 骨架 + `dump-hir`（TODO T0701）
  - [x] AST→HIR lowering（声明头 + 简单函数体）：`TypeRef`→`TypeId` + ident→`SymbolId`（TODO T0702）
  - [x] HIR：控制流与语句节点建模（if/when/while/assign/return）（TODO T0705）
- [ ] MIR：显式控制流（基本块）、显式临时变量、显式 drop/cleanup（用于 `finally`/effect unwinding）
  - [x] MIR 骨架：基本块/terminator/locals + CFG 校验（TODO T0703）
  - [x] MIR：cleanup/finally 的最小模型（UnwindAction + ResumeUnwind）（TODO T0707）

### 7.2 泛型单态化（monomorphization）

- [ ] 为每个具体实例生成专用 IR（含 `eff` 参数实例化）
- [ ] 缓存键：符号 + type args + effect row args
- [ ] 支持“预编译常见实例”（对齐 Cone 的 pre-specialize）

### 7.3 闭包与函数值

- [x] lambda → `{ env_struct, fn_ptr }` 形式
- [ ] 捕获分析与 env 布局：为每个 closure 计算捕获集合（immutable/mutable）、生成 env 字段顺序/偏移，并为 capture box 生成稳定布局
- [ ] closure env 走 GC-managed 分配：env 与 capture box 均通过 `scoop_alloc_typed` 分配，并为每个 env 类型生成 type descriptor（trace bitmap 覆盖所有 ref captures）
- [ ] 函数值 ABI：统一 `fun` 值调用约定（receiver/params/return/effects），并规定 safepoint 行为（任意可能触发分配/调用的路径都必须可在 safepoint 停世界）
- [ ] 可变捕获语义：`var` 捕获使用 `CaptureBox<T>`（GC-managed），读写走 `get/set`，并在多线程下提供最小一致性规则（禁止 data race 或通过 `scoop.sync` 明确同步）

**本阶段 DoD**
- 纯子集（无 class 虚分发也可）能 lowering 到 MIR，并能生成可链接 `.o`（下一阶段）。

---

## 8. LLVM 后端（阶段 7：inkwell codegen）

### 8.1 LLVM Module/Pass 管线

- [x] 最小 module + `main`（`ret 0`）IR 输出（T0802）
- [x] 目标三元组与数据布局（target machine）（T0803）
- [ ] 基本优化 pass（O0/O1/O2 可选）
- [ ] 调试信息（源级 DWARF：line table/locals）可后置；但必须保证可展开的 unwind info（如 `.eh_frame`/compact unwind）不被剥离，以支持 GC stack walking 与 non-resuming effect 展开

### 8.2 数据布局与 ABI

- [ ] 值类型（struct/tuple/enum）按 LLVM struct layout 映射
  - [x] struct：布局 + 字段访问（T0811）
  - [x] `@CLayout(aligned, packed)`：GC-free struct 的 C ABI 布局与对齐/pack 控制（用于 FFI 与全局变量 ABI）
  - [x] tuple：布局 + `._0` / `._1` 元素访问（T0812）
  - [x] enum：tagged union 布局（T0813）
  - [x] value-only enum（`enum E: Int { ... }`）：底层整型同布局（无 tag/union），用于 C interop
- [ ] 引用类型统一表示（GC pointer）：
  - 在 LLVM IR 中统一使用 `addrspace(1)` 指针表示 GC-managed object（例如 `i8 addrspace(1)*`），避免与原生/FFI 指针混淆
  - `Option<Ref>` 采用 pointer-niche（NULL → None），并在类型系统与 codegen 中统一规则（含 `T?` desugar）
- [ ] heap 对象模型（Object Model）：
  - 固定对象头布局与字段偏移：`next`（heap 链表）、`type_desc*`、`size_bytes`、`flags`、`mark`
  - payload 紧随对象头；class/array/string/box/closure env 均是“对象头 + payload”
  - 对象头字段与对齐必须用 C `_Static_assert` 固化，并在 Rust/LLVM codegen 侧同步断言（避免 ABI 漂移）
- [ ] type descriptor（运行期类型描述）：
  - 编译器为每个可分配 ref 类型生成一个 `ScoopTypeDescriptor` 常量，包含：对象大小/对齐、trace bitmap/trace_fn、release_fn、RTTI/type id、vtable/itable 元数据
  - 支持 fixed-size 与 variable-size（array/string）两类对象：variable-size descriptor 必须给出“header size + element stride + element trace 规则”
- [ ] 动态分发与 RTTI：
  - class 虚分发：为每个 class 生成 vtable（slot 顺序稳定且含 override），对象通过 `obj.header.type_desc->vtable` 拿到方法指针再调用
  - interface 分发：为每个 class 生成 itable（按 interface id 索引到 method slots），并支持 `is/as` 对 interface 的运行期判定
  - `is/as/as?`：运行期 type test 通过 type id + parent chain（class）与 itable membership（interface）实现；失败路径与 `as?` 的 `None` 语义必须可回归验证

### 8.3 与 GC 的接口（LLVM StackMap + statepoint 精确根集）

- [ ] safepoint 机制（必须可停世界 + 可移动）：
  - 编译器在所有可能触发 GC 的路径上插入 safepoint：分配、回边（loop backedge）、显式 poll、以及可能阻塞的外部调用边界（enter/leave native）
  - safepoint 统一采用 LLVM statepoint 体系：在 IR 中标注 GC-managed pointers，并通过 `rewrite-statepoints-for-gc` 生成 stackmap 记录与 `gc.relocate` 使用点
- [ ] GC roots 枚举（精确）：
  - 运行时以 stackmap 为唯一“栈根集来源”，不依赖 shadow stack
  - **需要扫描整个调用栈（多帧）**：GC 不能只扫描“当前 safepoint 的一帧”，必须对每个 Parked 线程做 stack walking，逐帧读取 `(sp, return_address[, regs])` 并查 stackmap record，累积得到该线程的完整 roots 集合
  - Parked 线程在进入 safepoint runtime helper 时把“可用于 stack walking 的线程上下文”写入 TLS（至少包含：return address、stack pointer、frame pointer、callee-saved regs 的可更新 spill slots）；GC 从该 TLS 上下文出发做 stack walking
  - stack walking 后端以 **runtime/c 的平台层** 实现（优先 `libunwind` 或等价 unwind API）；不得把 unwind/寄存器/ABI 细节泄漏到 Scoop 侧
  - 对于进入 native/extern（可能阻塞）的线程：在 `enter_native` 时把 callsite 对应的 roots 拷贝到 TLS `native_roots` buffer，GC 扫描该 buffer；`leave_native` 清理并恢复线程状态
- [ ] roots 可更新（moving GC 必需）：
  - 移动 GC 在 STW 期间必须原地更新 stackmap 指向的 spill slots（以及 `native_roots` buffer），使得 `gc.relocate` 读取到的新指针与 heap 内修复一致
  - 对于 pinned 对象：禁止移动并在类型/FFI 规则上固定（pin/unpin 语义必须与 relocation 一致）
- [ ] LLVM 管线与产物要求：
  - `scoopc` 的 LLVM pass pipeline 必须包含：statepoint 重写（`rewrite-statepoints-for-gc`）与 stackmap emission（默认随 statepoint lowering 产出）
  - `.o`/最终可执行文件必须携带 stackmap section；链接产物中 stackmap 必须可被 runtime 定位并注册（支持 main binary 与 `.cone` 预编译对象一起链接）

- [ ] `when` lowering：补齐 or-pattern / guard（spec §4.2）
- [x] tuple 字段访问统一为 `._0` / `._1`，并同步修正文档、fixtures、lowering、codegen（spec §2.3.3）
- [ ] enum layout/codegen：补齐 niche optimization 与更完整的 enum ABI 校验（已实现 oversized variant boxing 与 variant size disparity lint；仍需补齐 niche 与更多优化/诊断）（spec §2.3.2）
- [x] `object` / `companion object` codegen：单例存储、一次初始化、静态成员访问（Appendix B.9）
- [x] `trimIndent()`：运行期 fallback 与字符串 API 对接（spec §8.4）

**本阶段 DoD**
- 生成的二进制可运行（至少支持整数运算、函数调用、打印、Option/enum 基本构造）。

---

## 9. 早期运行时（C + clang）（阶段 8：可执行与可观测）

### 9.1 最小运行时组件

- [ ] 启动入口：`main`/平台 glue，初始化 runtime（GC/stackmaps/线程注册/effect TLS）
- [ ] stackmap registry（运行期元数据）：
  - 在进程启动时定位并注册所有已链接 module 的 stackmap section（main binary + 静态库 + `.cone` 产物）
  - 建立 `return_address -> stackmap record` 的查询结构（按 return_address 排序的数组 + 二分查找），并支持多 module 共存
- [ ] 分配器（typed alloc）：
  - `scoop_alloc_typed(type_desc, size_bytes)`：分配并初始化对象头（写入 `type_desc/size/flags/mark`），返回 GC pointer
  - 固定大小对象可走 `scoop_alloc_object(type_desc)` 快路径；变长对象（array/string）走 `scoop_alloc_var(type_desc, size_bytes)`
  - 分配路径必须包含 safepoint（允许触发 GC 并在返回前得到稳定对象指针）
- [ ] GC（StackMap roots + 可移动）：
  - stop-the-world：统一线程状态（Running/Parked/InNative），确保所有线程要么进入 safepoint park，要么进入 native 保护态
  - roots 枚举（完整栈）：Parked 线程对“整个调用栈（多帧）”做 stack walking：逐帧用 `(sp, return_address[, regs]) + stackmap` 枚举 roots；InNative 线程扫描 TLS `native_roots`（由 `enter_native` 预先捕获）
  - heap 扫描：用 `ScoopTypeDescriptor` 的 trace bitmap/trace_fn 扫描对象内引用字段；支持 closure env、box、class fields、array elements
  - relocation：移动/压缩时必须更新 stackmap spill slots、native_roots buffer，以及 heap 内所有引用字段
  - sweep/回收：调用 `release_fn`（若存在）后释放对象；并维护 free list/统计以支持回归与基准
- [ ] 线程注册与 native 过渡：
  - 线程创建后必须注册到 runtime；线程退出必须注销，保证 GC 能枚举线程集合
  - `enter_native/leave_native` API 固化：进入 native 前保存 roots 并切换线程状态；返回后恢复状态并清理 roots
- [ ] `object` / `companion object`：一次初始化原语必须与 GC/stackmap safepoint 兼容（初始化过程中允许 GC，且不会泄漏未初始化对象）

### 9.2 effect runtime（C 或编译器插桩）

- [x] TLS：handler stack 指针、perform slot、flag（T0906/T0913）
- [x] 最小原语：push/pop handler frame、读写 perform slot（T0613/T0913）
- [x] continuation one-shot + resume API（T0914）
- [x] continuation 跨线程 `resume`：安装 captured handler stack，并在返回后恢复原 TLS（T0915a；端到端 fixture 见 T0915b）

### 9.3 与 clang 的构建集成

- [x] `runtime/c` 用 clang 编译成静态库/对象
- [x] `scoopc` 链接时自动把 runtime 拉进来
- [x] fixtures 中提供 `--emit-llvm`/`--emit-obj`/`--emit-asm` 选项方便排查
- [x] effect runtime 必须支持多层 handler stack（最近匹配分发 + arm body 在 dispatch scope 外；Appendix A）
- [x] `Task<T>` / executor 最小 runtime 原语：任务状态、入队/恢复、可选 start（spec §5.7）
- [x] `object` / `companion object` 的 once/init 支持（Appendix B.9）

**本阶段 DoD**
- 有一个“运行期回归套件”（见 §10）能持续压测 GC 与 effect。

---

## 10. Fixtures 与测试体系（贯穿所有阶段，必须先行）

这里的目标是：**任何规范点都有对应的 fixture**，并且 fixtures 能区分：
- 解析是否正确
- 语义/类型/效果是否正确
- 代码生成/运行期行为是否正确

### 10.1 Fixture 目录规划（建议）

```
tests/
  fixtures/
    parse/               # 仅解析：AST snapshot / 语法错误恢复
    resolve/             # 名字解析：import/visibility
    resolve_multi/        # 名字解析：多文件编译单元（目录为 case）
    typecheck/           # 类型检查：compile-pass / compile-fail
    typecheck_multi/      # 类型检查：多文件编译单元（目录为 case）
    infer/               # 推断专项
    effects/             # effect rows / handle / required effects / entrypoint Pure
    codegen/             # 运行输出对比
    runtime_gc/          # GC/alloc/pin/unpin/压力测试
    unsafe_nogc/         # @Unsafe/@NoGC 规则
    language/            # 字符串/with/属性/委托/操作符等语法语义专项（按章节分组也可）
    comptime/            # const fun / comptime / 反射 intrinsics
    cone/                # .cone 打包/消费/单态化缓存
```

当前 runner 约定：fixture 的一级目录名就是 phase（例如 `parse/`、`resolve/`、`typecheck/`）。未实现的 phase 也必须给出清晰诊断，便于先写 fixture 再补实现。

- [x] phase 路由：按 `tests/fixtures/<phase>/**` 目录名决定执行阶段（未实现 phase 返回“未实现”诊断）

默认每个 fixture 采用“单文件 + 注释指令”的形式（类似 LLVM lit 或 Rust compiletest）。
对于需要跨文件验证的规则（例如 `private` 可见性、跨文件引用、sealed 继承等），额外提供 `<phase>_multi/<case>/`：
- `<case>/` 目录内包含 2+ 个 `.scoop` 文件
- runner 先把同一 case 的所有文件作为一个编译单元构建索引，再逐文件执行 `<phase>` 并按各自文件头注释断言 pass/fail

- [x] `// EXPECT: pass|fail`
- [x] `// EXPECT-ERROR: <substring>`（当前为子串匹配；后续可升级为 regex）
- [x] `// EXPECT-AST: <file>`（parse fixtures：AST snapshot / golden）
- [x] `// RUN-STDOUT: <file>`
- [x] `// RUN-STDERR: <file>`
- [x] `// EXPECT-EXIT: <code>`
- [x] `// TIMEOUT: <ms>`
- [x] `// ARGS: ...`

### 10.2 诊断（compile-fail）的 golden 规范

- [x] 诊断必须包含：错误码（稳定 ID）、主消息、关联 span（行列）、可选 note/help（当前 lexer/parser 已提供 code + label span）
- [x] fixtures 断言策略：支持匹配“错误码 + 错误位置（行列）+ 关键片段”（先用文件头注释指令实现；未来可再升级为独立 `.golden`）

推荐模板（compile-fail fixture 文件头）：

```
// EXPECT: fail
// EXPECT-ERROR: <关键片段>
// EXPECT-ERROR-CODE: <稳定错误码>
// EXPECT-ERROR-AT: <line>:<col>
```

### 10.3 spec doctest（强烈建议）

- [x] 工具：从 `SCOOP_FULL_SPEC.md` 抽取包含 `// FIXTURE:` 的 fenced code block，生成 `tests/fixtures/spec_doctest/*`
- [x] 约定：代码块通过注释标记其期望（`// EXPECT:` / `// EXPECT-ERROR:`），`// FIXTURE:` 指定输出路径
- [x] 在 CI 中强制：`cargo run -p scoop_tools -- spec-fixtures check` + `cargo run -p scoop -- test`
- [x] 本地修复：`cargo run -p scoop_tools -- spec-fixtures check --fix`（只写回受影响文件）

### 10.4 运行期 fixtures（run-pass）

- [x] T0106a：fixtures runner 识别 `codegen/`（或 `run-pass/`）phase，并实现 stdout golden 比对（对比逻辑可单测独立验证）
- [x] T0106b：接入 `scoop run`（T0807）真正“编译 + 运行” fixture，并断言 stdout（默认仅在启用 `scoop --features llvm` 时执行）
- [ ] 支持超时、退出码断言（fixtures 指令：`TIMEOUT`/`EXPECT-EXIT`）
- [x] T0111a：支持 stderr golden 断言（对比逻辑 + 稳定诊断，可单测）
- [x] T0111b：新增 run-pass fixtures 覆盖 stderr（需要 T0106b2 真正执行）
- [ ] 对 GC 压测类测试，支持 `SCOOP_GC_STRESS=1` 之类的环境变量切换（让 CI 可控）

### 10.5 Fuzz/性质测试（可选但很有价值）

- [x] lexer/parser fuzz（避免崩溃，保证错误恢复）— 实现为 `crates/scoopc/tests/fuzz.rs`：adversarial + deterministic random + structured fragment 三类测试（5000+ iterations）
- [ ] IR lowering fuzz（随机小 AST → 不崩溃）
- [ ] GC 压测（随机分配/释放/跨线程）

### 10.6 覆盖矩阵（建议维护）

- [x] `cargo run -p scoop_tools -- fixtures-matrix check`：按 phase 目录扫描 fixtures，报告缺少 pass 或 fail 的缺口（见 `tools/scoop_tools/src/fixtures_matrix/`）
- [ ] 后续可细化为按 spec 章节粒度检查（当前为 phase 粒度）

为每个 spec 章节至少准备：
- 1 个 compile-pass
- 1 个 compile-fail（覆盖常见误用）
- 若涉及运行期语义（GC/effect/async），再加 1 个 run-pass

---

## 11. `@NoGC` / `@Unsafe` / `@Extern`（阶段 9：实现“系统编程通道”）

- [ ] 通用注解系统（spec §15）：
  - [x] 解析注解声明（`annotation class`）
  - [x] 解析注解使用（`@Name(...)`）
  - 注解 target（函数/类型/字段/参数/表达式块等）与合法性检查
  - 注解仅编译期存在（不进运行时布局）
  - 内建注解：`@Intrinsic/@Extern/@Inline/@Deprecated`（具体名字按 sysroot 定义）
- [x] T1003：内建注解最小门禁（`@Unsafe/@NoGC/@Extern/@Intrinsic`）
- [x] T1004：`@Unsafe { ... }` 块语法与 unsafe context 传播
- [x] T1005：`@NoGC` 最小静态门禁（保守拒绝可能分配/装箱的路径）
- [x] T1009：最小 unsafe 指针原语（`addrOf/load/store`；后续按 spec §15.9.4 演进为 `addressOf` + `Ptr<T>.load/store` 等）语法落点与门禁（unsafe_nogc fixtures 回归）
- [x] `@Unsafe`（最小落地）：
  - 函数级与块级 `@Unsafe { ... }`
  - 非 unsafe context 禁止：调用 `@Unsafe` 函数/调用 `@Extern`/使用最小 ptr 原语（`addressOf`/`Ptr.load/store` 或等价 sysroot intrinsics）
- [x] `@Safe`：允许在 unsafe context 内显式“收窄”为 safe 区域（禁止 `@Extern`/`@Unsafe` 调用与 unsafe primitives），用于 callback/闭包场景
- [ ] `Ptr<T>` / `UIntPtr` 与指针整数转换（spec §15.9.4 / runtime §4~§5）
  - `UIntPtr` 仅为 `UInt` 的别名（类型本身不 unsafe）
  - 指针 ↔ 整数转换必须在 unsafe context，且通过 sysroot intrinsics（不通过 `as/as?`）
  - `Ptr<T>` 的 `T` 必须是 GC-free value type（不允许直接/间接包含 GC ref）
  - `Ptr<T>` API：`cast/load/store` 与基于元素的 `plus/minus` 指针算术
  - `addressOf(var: T): Ptr<T>`：取局部/全局变量 slot 的地址（lvalue gate）
  - `stackAlloc<T>(): Ptr<T>`：栈上分配 GC-free `T`（生命周期受限于当前函数）
  - [x] `FunPtr<F>`：FFI 函数指针类型（v0：支持 `fp(args...)` / `fp.invoke(args...)`；仅 C ABI）
  - [x] internal atomics（`__AtomicInt/__AtomicLong/...`）：值类型、同底层布局、LLVM IR 原子指令直接生成（用于 FFI + 全局状态）
- [ ] `@NoGC`：
  - 禁止 GC 堆分配；只能调用 `@NoGC` 与 `@Extern`
  - 编译器证明不了“无分配”就必须报错（保守）
- [ ] `@Extern`：
  - 默认视为 `@NoGC`
  - 是否默认 `@Unsafe`：建议 **调用点要求 unsafe context**（更符合“外部世界不可信”）
  - 扩展参数：`@Extern(lib, name)`（`lib` 进入链接参数；符号名可显式指定）
  - 允许 extern 变量声明；`@Extern + @ThreadLocal` 可声明外部 TLS 变量
- [ ] 全局可变变量（GC-free）：仅允许显式标注 `@ThreadLocal` 或 `@Global`；否则 compile-fail（让风险可见）
- [ ] 注解系统补齐：
  - [x] 内建注解：`@TailRec/@AllowIntrinsic/@Suppress/@CLayout/@Target/@Retention`
  - [ ] 内建注解：`@ThreadLocal/@Global`，以及 `@Extern(lib, name)`、`@CLayout(aligned, packed)` 参数语义（`@CallingConvention` 已落地：仅 `c/cdecl`）
  - [x] `AnnotationTarget` enum 与最小 target 合法性检查（非法 target 名）
  - [ ] meta-annotations（拆分）：
    - [x] typecheck enforce `@Target/@Retention`
    - [x] `.cone` 导出策略（cone-preserved 注解下游可见）
- [ ] 注解参数补齐：常量表达式、数组/enum/class-literal 等非纯字面量参数的解析与合法性检查
- [x] 注解 use-site targets：`field:/property:/param:/get:/set:/file:`（spec §15.3，已支持解析与挂载；get/set 仍为纯语法存储）
- [x] namespaced annotations：`@Namespace.Annotation(...)`（spec §15.4）
- [ ] 后期 runtime / std 阶段的 intrinsic 预算规则：
  - 默认不再新增 intrinsic，优先用纯 Scoop 库补 runtime/stdlib 缺口
  - 若审计证明缺少底层 primitive，则单独立项增加最小 intrinsic，并与上层库任务拆开推进
  - 集合特别约束（Array-first，最小 intrinsics）：
    - `Array<T>` / `MutableArray<T>` 允许作为 **唯一** 集合底座引入少量底层 primitive（必要时通过 `@Intrinsic` 落地）
    - `push/pop/insert/remove/splice` 等能力必须作为库 API 支持；其实现 **默认必须** 由纯 Scoop 完成（基于 `get/set` + 容量策略等），只有当审计证明存在底层 blocker 时才允许回流增加最小 intrinsic
    - `Set/Map/List/MutableList/MutableSet/MutableMap` 等上层集合 **不得** 引入新的 intrinsic；性能问题优先通过纯 Scoop 算法/专门化与优化解决

fixtures：
- `tests/fixtures/unsafe_nogc/*` 覆盖所有违规路径（必须 compile-fail）

---

## 12. Cone（包/稳定 IR/分发）（阶段 10：工程化分发）

### 12.1 Scoop IR（scoopir）

- [ ] 定义一个稳定的 IR schema（建议独立文档 + 版本号）
- [ ] `api.scoopir`：仅含 public API（用于类型检查与 IDE）
- [ ] `generics.scoopir`：含泛型/const fun 的可执行 IR（供下游单态化）

### 12.2 `.cone` 归档格式

- [x] archive（v0：tar；包含 `Cone.toml` + `api.scoopir` + `SOURCES_SHA256`，T1104）
- [x] 读取 `.cone`：加载 `api.scoopir` 并注入下游 typecheck（T1105）
- [ ] 读写 `Cone.toml`、依赖解析、目标平台信息
  - [x] 读取 `Cone.toml`（name/version/deps，T1101）
  - [x] 包加载：cone root → `src/**/*.scoop` + `src/main.scoop`（T1102）
  - [ ] `Cone.toml` 平台选择器（`[[select]]`）
    - [x] manifest 解析（T1110）
    - [x] sources include/exclude 应用（T1111）
- [x] 预编译实例（pre-specialize）：cache key 与选择规则（v0：函数实例 + `.cone/PRE_SPECIALIZE.json`）
- [ ] pre-specialize：补齐类型实例（不只函数实例）的打包与消费规则

fixtures：
- `tests/fixtures/cone/*`：
  - 打包后消费编译的 API 兼容性
  - IR 版本兼容（旧版本可读）

---

## 13. 编译期执行与反射（阶段 11：comptime）

- [x] Parser 语法：支持 `const` 修饰符、`comptime { ... }` / `comptime if` / `comptime for`、以及 splice `value.[field]`（见 TODO T0246）
- [x] `const fun` 解释器（先支持 value types/纯计算；`String` 作为特例允许——具有值语义）
  - [x] T1202a：值模型 + 纯表达式求值 v0
  - [x] T1202b：tuple/struct/enum 的值构造与访问
  - [x] T1202c：`const fun` 调用 + `tests/fixtures/comptime` 接入
  - [x] T1203：`comptime { ... }` / `comptime if` 最小语句级执行（含 else-if；未选中分支不求值）
  - [x] T1207：`comptime for` 最小语句级执行（v0：整数范围 `a..b` + tuple/array；break/continue 后置）
- [ ] `const fun` 静态检查：禁止闭包/lambda（捕获环境导致 const 语义难以验证）
- [x] `comptime { ... }` 执行上下文（v0：仅 const 解释器路径；限制 effect：必须 `Pure`）
- [x] 反射 intrinsics v0：`fieldsOf/nameOf/sizeOf`（fieldsOf v0 返回 `FieldMeta` 列表）
- [x] splice operator v0：`value.[field]`（const eval + typecheck 最小语义）
- [x] 反射 intrinsics 补齐：`variantsOf/alignOf/superTypesOf/annotationsOf/paramsOf`（spec §6.4 / §15.6）
- [x] 平台反射：`Platform` struct + `getPlatform(): Platform`（既可在 comptime 求值，也可在 runtime 查询当前执行环境；用于平台选择器等能力）
- [x] 编译期元数据补齐：`VariantMeta/ParamMeta/FunctionMeta/AnnotationMeta/AnnotationArgMeta`（spec §6.4 / §15.6）
- [ ] 编译期注解访问：复杂参数表达式 / 数组 / enum / class-literal 的归一化与读取（不只字面量）
- [x] `trimIndent()`：编译期求值（spec §8.4；运行期 fallback 已由 T0827 完成）
- [ ] sysroot/stdlib：补齐 scope functions（§11）；delegated property API surface 已在 sysroot 落地（spec §10.4）

fixtures：
- `tests/fixtures/comptime/*`：覆盖常量折叠、生成代码（若支持）、错误诊断

---

## 14. Kotlin 语义兼容项（阶段 11+：按需逐步补齐）

spec §16 指出以下功能“遵循 Kotlin 语义”，实现上建议按需求拆分落地，每一项都要配 fixtures：

- [ ] 操作符重载（operator overloading）
  - [x] `a + b` / `a - b` → 绑定到 `plus` / `minus`（T1301）
  - 补齐位运算与移位：`and/or/xor/inv/shl/shr`（Appendix B.8）
  - 运行期与值类型/引用类型的 codegen 覆盖
- [ ] `object` 与 companion object（如需要）
- [x] `typealias`（纯类型层语法糖；当前仅非泛型别名 + 展开 + 循环检测，T0446）
- [ ] Ranges/progressions 与 `for` 迭代协议
- [ ] 基础集合与常用操作（`map/filter/fold` 等更多是库工作，但需要类型推断与泛型单态化支撑）：
  - `Array<T>`：不可变（只读集合）；支持 `get`/`length`/迭代与常用数组操作
  - `MutableArray<T>`：可变；支持 `get/set/push/pop/insert/remove/splice`，并采用容量策略保证 `push/pop` 摊还 O(1)
  - 两者都应支持迭代（`for` 协议/迭代器）与从 iterable 构造（优先 `Array.from(iterable)` / `MutableArray.from(iterable)`）：
    - 允许实现上使用内部 builder（例如内部 `MutableArray`）以获得摊还 O(1) 的增量构造
    - 若需要“零拷贝把 builder 变成不可变 Array”，必须定义**显式且安全**的语义（例如 `freeze`：冻结后任何别名都不可再变更）
    - 在缺少上述语义前，**不要**对外暴露 `MutableArray -> Array` 的零拷贝转换 API（避免把“只读视图”误当成不可变值）
  - 数组字面量 `[...]`：
    - 结果类型按 expected type 选择 `Array<T>` / `MutableArray<T>`（无 expected type 默认 `Array<T>`；空数组 `[]` 需要 expected type）
    - `T` 为元素类型的 LUB（最小公共父类型）
    - 禁止字面量内逐元素隐式装箱：若 `T` 为 `Any` 等引用类型，必须显式写 `as Any`（例如 `[1 as Any, 2 as Any]`）
  - `List<T>`：定义为 `Array<T>` 的别名（`typealias List<T> = Array<T>`；T1317f2 已在 sysroot 落地）
  - `Hashable`：加入 sysroot 并为 primitive types 提供实现；`Set/Map`（含 mutable）全部用纯 Scoop 基于 `Array`/`MutableArray` 实现，不引入 intrinsics
  - `MutableList<T>`：用 `MutableArray` 做 backing pool，以纯 Scoop 实现并追求高效（`push/pop/insert/remove` 摊还 O(1)；T1317f2 已先落地 typealias + `Int.add`）
- [ ] import alias：`import foo.bar.Baz as Qux`（Appendix B.7）
- [ ] `object` / `companion object`：从 parse/resolve 扩展到 typecheck/codegen/初始化语义（Appendix B.9）
- [x] 类初始化语义：property initializer、`init` blocks、secondary constructors、初始化顺序（Appendix B.2.2）（T0448：最小落地）
- [ ] 标准 delegated properties：`lazy`/`observable`/`vetoable`/map-backed（spec §10.4；运行期语义待补齐）
- [ ] Kotlin runtime gap closure（when applicable）：
  - 先审计 Scoop core runtime / stdlib 与 Kotlin runtime 语义缺口
  - 优先用纯 Scoop 补齐；只在审计证明无法表达时回流到 §11 的最小 intrinsic 通道
- [ ] 全量 `std` 库工程：
  - 目标能力与 Rust `std` 同量级、可比较，但不要求 API 一致
  - 建议分层：`core` / `alloc` / `std` / 平台适配层
  - 覆盖 collections、text/regex、iterators、io/fs/path/process/env、time、sync/thread/channels、net、async adapters、test/support utilities 等
  - text 重点：`String` 需要支持零拷贝 slicing/substring（例如 `trimStart/trimEnd/trim`），优先考虑“薄 value struct + 共享 `StringData` backing”的表示以避免频繁分配（见 TODO T1513/T1514）
  - collections 设计约束：以 `Array`/`MutableArray` 为底座；`List<T>` 为 `Array<T>` 别名；`Set/Map`（含 mutable）为纯 Scoop（不新增 intrinsics）
- [ ] Kotlin 风格重载决议兼容：
  - [x] most specific candidate 规则（普通/扩展/构造：T0513 + T1321）
  - [x] 默认参数 / 命名参数 / trailing lambda 与重载集合的交互
  - [ ] 扩展函数、成员函数、构造函数之间的优先级与歧义处理
  - 差异（vs Kotlin）：
    - 当前阶段 resolver 对 `receiver.member(...)` 的 member/extension 优先级仅按“同名存在性”决定，不基于“参数适用性”在 member 不匹配时回退到 extension（完整语义留给 TODO T1508）。
- [x] 默认参数：中间参数省略与命名参数联动
- [ ] 多 trailing lambda：语法、expected type 与重载决议联动
- [ ] varargs spread：集合/序列到 vararg 的桥接规则
- [ ] delegated properties：`lazy`/`observable`/`vetoable` 的线程安全语义与平台 policy
- [ ] 类初始化兼容：复杂继承链与 effect 细节

fixtures：
- `tests/fixtures/language/*` 下为每个特性提供 compile-pass/compile-fail + 必要的 run-pass

---

## 15. C runtime 长期路线（平台隔离 + GC（Immix））（阶段 12：长期稳定）

### 15.1 边界与原则

- **C runtime 是长期方案**：GC/effect runtime/平台系统调用 glue 保持在 `runtime/c/`，不计划迁移到 Scoop。
- **平台依赖只在 C**：OS 差异（POSIX/Windows/WASM/embedded）通过 C 层 backend 选择解决；Scoop 侧（sysroot/stdlib）不出现平台类型泄漏与平台分叉语义。
- **ABI 尽可能小**：对外导出的 runtime 符号要可审计、可稳定版本化；优先复用少量通用 entrypoints，不把 libc/OS API 逐个“直通”暴露给 Scoop。
- **导出符号 allowlist**：使用 `runtime/c/scoop_runtime_api.h` 集中声明导出符号，并在 `crates/scoop_runtime` 单测中用 `nm` 校验“未登记导出会失败”（对应 TODO T1401）。

### 15.2 平台相关代码隔离（建议分层）

建议把 `runtime/c/` 分成两层（或在逻辑上保持边界）：

1) **core（平台无关）**：GC、effect runtime、对象/类型描述、stackmap registry/safepoint、基础分配器、字符串/数组等运行时数据结构与算法。
2) **platform/backends（平台相关）**：env/time/fs/path/io/process/thread/sync/net 等对 OS API 的封装（含线程与同步原语的后端选择）。

最小落地建议：

- `runtime/c/platform/`：放置平台相关实现（先 POSIX/pthread，后续 Windows/其他）。
- `runtime/c/platform.h`（或 `scoop_platform.h`）：定义内部平台层 API；core 层只调用这里，不直接触达 `getenv/open/read/pthread_*` 等。
- sysroot/stdlib 只暴露平台无关的 API surface；平台能力通过 runtime C 符号实现并由 LLVM codegen 映射，不在 Scoop 侧直接调用 OS ABI。

当前进度（截至 2026-04-02）：

- 已落地 `runtime/c/platform/` v0：env/time/io、sync primitives（Mutex/CondVar/Thread self/equal）、thread primitives（spawn/join/yield/sleep/currentId）。
- `runtime/c/scoop_runtime.c`、`runtime/c/scoop_sync.c`、`runtime/c/scoop_channels.c`、`runtime/c/scoop_task_executor.c`、`runtime/c/scoop_thread.c` 已接入 platform API，不再直接触达 OS 调用。

### 15.3 Immix GC 路线（重点：移动/压缩 + 多线程）

- [x] **Immix v0（单线程、非移动）**：作为 baseline mark-sweep 的可选 backend，引入 line/block allocator 与 mark-region 基本流程。
  - [x] T1406a：backend 选择 + capability matrix（`gc-immix` feature / C 编译单元接入）
  - [x] T1406b：allocator v0（line/block allocator，单线程）
  - [x] T1406c：mark-region v0（单线程、非移动）
  - [x] T1406d：microbench/fixtures（碎片化与吞吐对比）
- [x] **移动与压缩（moving/compaction）**：
  - [x] T1407：选择性 block evacuation + forwarding pointer + roots 更新；pin policy：pinned objects 不移动。
- [ ] **多线程正确性**：
  - [ ] T1408a：协作式 STW + 多线程 shadow stack roots 枚举/更新（先 correctness，可用全局锁）
  - [ ] T1408b：与 stackmap/statepoint 的 STW 协议统一（对齐 T1505，避免两套协议长期分叉）
  - 线程本地分配与 GC 元数据的并发安全与可回归验证（性能优化在 T1409）
- [ ] **多线程性能**：
  - thread-local allocation（TLAB/per-thread blocks）、全局 block 池的低争用策略；
  - 并行标记/并行 sweep 的渐进引入；增加基准与 stress 测试（碎片化、并发分配、跨线程引用）。
- [x] **GC backend capability matrix v0**：固化 STW/多线程 roots 枚举/moving/精确 roots 更新/shadow stack roots 等能力，并用于测试 gating（T1405b）。
- [ ] **GC backend 可替换**：编译期可选 baseline/Immix/embedded/minimal/hosted(adapter)，并维护 capability matrix（尤其是 WASM/embedded）。

fixtures / tests（建议）：
- `tests/fixtures/runtime_gc/*`：基础分配与回收、碎片化、pin/unpin、moving 后指针更新。
- `tests/fixtures/run-pass/*`：多线程分配/跨线程引用 + GC stress（可用 `--gc-stress`/`--threads` 指令驱动）。

---

## 16. 风险点与建议的优先级

- **高风险/高复杂度**：effect（尤其 `, k ->` + 跨线程）、GC（移动/压缩 + pin/unpin）、类型推断（subtyping + effect rows）
- **建议优先级**：
  1) 先把 fixtures 与诊断体系立住（否则后期难以迭代）
  2) 先做“语义正确”的实现（优化后置）
  3) effect 先 `Raise`/`->`，再扩展 `-> resume`、`, k ->`
  4) GC 先非移动，再移动（pin/unpin 在移动 GC 上才真正有意义）
