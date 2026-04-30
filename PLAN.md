# Scoop：下一轮计划（Root Map 抽象与 explicit root frame 落地）

> 生成时间：2026-04-29  
> 历史归档：`docs/archive/plans/PLAN-7.md` / `docs/archive/plans/TODO-7.md`  
> 本轮主题：按 [`ROOT_FRAME_REFACTOR.md`](./ROOT_FRAME_REFACTOR.md) 的设计基线，把 managed roots 的统一抽象收口为围绕 `void** slot` 的 root map，并将 `explicit root frame` 落地为新的默认 explicit mode；默认 explicit mode 完成切换后，不再使用、也不再生成 LLVM stackmap，stackmap 退回未来可选优化路径。

## 0. 工作原则

- 本轮严格按 `TODO.md` 的顺序推进，不跨条目并行实现。
- [`ROOT_FRAME_REFACTOR.md`](./ROOT_FRAME_REFACTOR.md) 是本轮设计基线；若实现过程中改变主张，必须先回写该文档，再继续实现。
- root map 的统一抽象必须围绕 `void** slot` 枚举与更新建立，不能继续把上层接口绑定在 `return_address -> stackmap record` 上。
- 默认 explicit mode 的 source of truth 必须切到 explicit root frame。
  - 默认 explicit mode 下不再使用 LLVM stackmap 作为 managed roots 来源。
  - 默认 explicit mode 下不再生成 `.llvm_stackmaps` / `__llvm_stackmaps` section 及相关 records。
- correctness-first，优化 second。
  - 第一阶段先保证 roots 可见、可更新、safepoint 后可 reload；不先追求最小 live-set 扫描开销。
- 不做“半 stackmap、半 explicit”的同路径混搭。
  - 同一条 lowering 主线内，managed frame roots 只能有一套 source of truth。
- explicit frame 只表示 stack-backed managed root home slots。
  - 它不是“函数所有 locals 的大 struct”。
  - heap-backed traced fields、effect/continuation heap frame 字段不进入 explicit frame。
- 对包含 ref 的 aggregate，frame layout 必须按 ref leaf slots 建模。
  - 非 ref 字段不进入 descriptor。
  - post-safepoint 若要继续复制/传参，必须先从最新 home slots 刷新或重组 fresh aggregate。
- safepoint 是 GC ref 的 clobber 边界。
  - safepoint 后不得继续信任 safepoint 前的 GC SSA / register 值。
  - post-safepoint 使用必须从 explicit frame 的 home slot reload。
- NULL discipline 是 explicit mode 正确性的组成部分。
  - entry 时初始化为 `NULL`。
  - dead / inactive slot 必须及时清回 `NULL`。
- `native_roots` 继续只服务 native 边界，不再承担“帮 explicit mode 找回更高层 managed frames”的职责。
- `mem2reg` / SSA promotion 不是本轮主线。
  - 仅在 explicit root frame 稳定后，作为单独优化任务重新评估。
- 若实现改变公开语义或运行时合同，必须同步 `SCOOP_RUNTIME.md`、必要实现注释与相关 fixture。
- 本轮后续所有步骤都必须拆成可验证的子任务。
  - 每个实现步骤都要能对应到定向验证命令、LLVM 回归、runtime test 或最小 fixture；
  - 不接受“先做完大块重构，最后一次性验证”的不可拆分推进方式。
- 每个复杂逻辑都必须有相应 fixture（或最小等价回归）验证正确性。
  - 特别是 state-machine local home / flush-back、continuation handle owner、sync sidecar、class init cleanup、task handoff、boxed payload resume 等路径，都必须有最小可复现覆盖。
- 最终 full review 不是“抽样看几个 fixture”，而是要在 `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1` 条件下完整验证所有相关 fixture 集合。

## 1. 顺序总览

1. 先建立 runtime 侧统一 root map / slot visitor 抽象，把 GC 与 roots 来源解耦。
2. 再引入 explicit root frame 的 runtime 结构与 TLS frame chain，让 GC 能在 explicit mode 直接枚举 managed frame roots。
3. 随后在编译器侧规划 per-function frame layout、descriptor 与 activation frame object，建立 entry/push、exit/pop、slot init/clear 的基础路径。
4. 在 runtime/编译器骨架就位后，再收紧 safepoint clobber / reload contract，并补齐 aggregate refresh/rebuild 语义。
5. 然后把默认 explicit mode 切到 explicit root frame，停止默认 explicit mode 的 stackmap 生成与使用。
6. 最后做全量回归、GC stress、verify-roots 与尺寸观察，确认 stackmap 已退回可选优化实现，而不再承担默认 correctness 路线。
7. `mem2reg` allowlist/denylist、stackmap selective comeback 等优化项只在上述主线稳定后再单独评估，不与 correctness 落地主线交错推进。

## 2. 分阶段目标

### P0. 基线收口与实现边界确认

- 先把当前 roots 入口、runtime 枚举路径、编译器 root slot 生成点、`extra_gc_root_slots`、`native_roots`、stackmap registry 依赖面重新盘清。
- 目标不是再写一版设计，而是形成可执行 baseline：
  - 哪些路径 today 仍直接依赖 stackmap；
  - 哪些 runtime visitor 已天然按 `void** slot` 工作；
  - 哪些 lowering 路径会生成跨 safepoint 的源码级 roots 与编译器内部 roots；
  - 哪些退出路径、resume 路径、state-machine 路径后续必须一起接入 push/pop 与 reload contract。
- `T5001a` 完成后，baseline 已固化到 `ROOT_FRAME_REFACTOR.md` 的“4.4 当前实现基线”一节。
  - runtime 现状：managed frame roots 仍以 `stackmap + unwind ctx` 为主，`InNative` 线程额外叠加 `native_roots`；pinned / handles / globals / heap object trace 已天然围绕 `void** slot` visitor。
  - 编译器现状：ordinary safepoint 依赖 `with_conservative_gc_local_root_spills(...)`；`extra_gc_root_slots`、hidden sret spill、indirect aggregate spill、ordinary resume 临时槽位是后续 explicit frame layout 必须吸收的 stack-backed roots。
  - effect/state-machine 现状：长生命周期状态主要位于 heap-backed frame / continuation object，不属于 activation explicit frame 要接管的那部分 roots。
- `T5001aR` 复核结论：baseline 已覆盖后续切换顺序需要的四类关键热点。
  - runtime roots 入口：`scoop_gc_stackmap_visit_roots_from_ctx(...)`、`scoop_gc_native_roots_visit_slots(...)`、`scoop_runtime_init()` 中的 stackmap registry init、globals/handles/pins/heap trace 入口均已在 baseline 中定位。
  - 编译器 roots 热点：ordinary safepoint、`@Extern` native 边界、`extra_gc_root_slots` / hidden sret / indirect aggregate spill、ordinary resume 与 effect/state-machine replay 边界均已纳入 baseline。
  - 顺序判断：`T5001b` 可以先只处理 runtime root source 抽象，不需要在抽象前额外回头补做新的基线盘点。

### P1. 统一 runtime root map 抽象

- 把 runtime 中“managed frame roots 来源”的入口从 stackmap 专用逻辑抽出来。
- 上层 GC/verify-roots/更新逻辑只消费统一的 `visitor(void** slot)` 接口。
- stackmap root map 与 explicit-frame root map 成为同层实现，而不是让 explicit mode 继续伪装成 stackmap 特例。
- 当前状态（T5001b，2026-04-29）：已新增内部头文件 `runtime/c/scoop_gc_root_map_internal.h`，定义 `ScoopGcManagedRootMap`、`ScoopGcRootMapVisitResult` 与统一入口 `scoop_gc_root_map_visit_slots(...)`；默认实现为 stackmap root map，并预留 `SCOOP_GC_MANAGED_ROOT_MAP_EXPLICIT_FRAME` kind。`scoop_gc_backend_immix.c` 与 baseline `scoop_gc.c` 的 verify-roots、major/minor mark、roots update 和 stackmap smoke 测试均已切到该抽象，上层不再直接展开 stackmap record 遍历。
- Review 状态（T5001bR，2026-04-29）：已复核 runtime 上层调用点，确认 `scoop_stackmap_registry_lookup(...)` / `scoop_stackmap_record_visit_root_slots(...)` 不再被 GC 上层直接调用；stackmap 细节已收缩到 `scoop_gc_root_map_internal.h` 内部实现，GC 上层对 managed roots 的入口已稳定为 `scoop_gc_root_map_visit_slots(...)`。配套验证已通过 `cargo test --all`、`cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc` 与 `cargo clippy --all-targets -- -D warnings`。

### P2. explicit root frame runtime substrate

- 引入 `ScoopRootFrameDesc`、`ScoopRootFrameHeader` 与 TLS `__scoop_explicit_root_frame_top`。
- 让 runtime 能从 TLS top 沿 `prev` 链遍历 explicit frame chain，并按 `header -> desc -> offsets` 恢复每个 `void** slot`。
- `InNative` 线程在 explicit mode 下不再依赖 captured unwind ctx 回找 caller managed frames；`native_roots` 只保留 native 边界临时根语义。
- 当前状态（T5001c1，2026-04-29）：已新增 `runtime/c/scoop_root_frame.h`，定义 `ScoopRootFrameDesc`、`ScoopRootFrameHeader`、TLS 符号 `__scoop_explicit_root_frame_top`，以及基础 helper `scoop_root_frame_visit_slots(...)`；该 helper 已明确 `top == NULL` 为合法空链，`slot_count == 0` frame 为合法 frame（计入 frame walk，但不访问 slot），并在 `runtime/c/scoop_gc_root_map_internal.h` 中接入为 explicit-frame root map 的并列实现。当前尚未把 GC 默认 managed roots 枚举切到 TLS frame chain；该切换仍留给 `T5001c2`。
- Review 状态（T5001c1R，2026-04-29）：已复核 explicit frame substrate 的职责边界，确认 runtime 只通过 `frame base + descriptor offsets` 恢复 `void** slot`，没有重新引入 SP/FP 或栈布局猜测；`top == NULL` 与 zero-slot frame 均有明确 contract，并已由 `crates/scoop_runtime/tests/explicit_root_frame.rs` 与 runtime smoke helper 覆盖。另已把该 ABI/TLS contract 补记到 `SCOOP_RUNTIME.md`，避免后续编译器发射 descriptor 时依赖隐式约定。
- 当前状态（T5001c2，2026-04-29）：baseline/Immix runtime 现已在 STW park 和 `enter_native` 时把 `explicit_root_frame_top` 快照进 `ScoopGcThreadRecord`，managed roots 枚举/更新/verify 统一优先消费 explicit-frame root map，仅在没有 explicit frame snapshot 时退回 stackmap ctx；因此 explicit-frame 路径已不再依赖 unwind ctx + stackmap lookup。`native_roots` 继续保留，但职责已收窄为 native 边界临时根，而不是 caller managed frame 回溯入口。为锁定该 contract，新增 `scoop_test_explicit_root_frame_enter_native_smoke` 和 Rust 侧 `explicit_root_frame_enter_native_uses_saved_tls_chain` 回归，并在修复 `gc_verify_roots` 的 stackmap-mode `InNative` 回归后，通过 `cargo test --all`、`cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc` 与 `cargo clippy --all-targets -- -D warnings` 复核通过。
- Review 状态（T5001c2R，2026-04-29）：已复核 `runtime/c/scoop_gc.c`、`runtime/c/scoop_gc_backend_immix.c` 与 `runtime/c/scoop_gc_root_map_internal.h` 在 STW park、`enter_native`、verify-roots、moving update、major/minor mark 五类调用点上的 managed roots 来源选择。确认 explicit mode 统一优先走 `explicit_root_frame_top -> explicit-frame root map`，未再默认假定 stackmap registry 必须可用；`native_roots` 只保留 native 边界临时 slots，未再承担 caller managed frames 枚举；stackmap mode 仍作为并列 root-map 实现保留，相关 stackmap smoke / registry 测试未受损。期间还修正了 `runtime/c/scoop_gc_backend_immix.c` 两处与现状不符的旧注释，并再次通过 `cargo test --all`、`cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc` 与 `cargo clippy --all-targets -- -D warnings` 验证。

### P3. 编译器发射 explicit frame object 与 descriptor

- 为每个 managed 函数规划固定 frame layout，并为其生成函数级 descriptor / offset table。
- 每个 activation 的 explicit frame object 必须是 entry-block alloca，且 header 位于首字段。
- entry 时完成：frame alloca、slot `NULL` 初始化、`hdr.desc` / `hdr.prev` 安装、TLS push。
- 所有退出路径都必须完成 TLS pop。
- 无 roots 函数可在第一阶段跳过 frame 构造，但该选择必须是显式且可审计的，而不是偶然漏发射。
- 当前状态（T5001d1，2026-04-29）：编译器已具备“函数级 explicit frame layout 规划事实”。`MainCodegen` 现在会在 top-level/HIR body、raw MIR body、closure body、object init、effect-call wrapper 与 callee resume entry 进入时启动 layout 收集，并在函数收尾统一发射：
  - 具名 frame 结构类型 `scoop.runtime.ScoopExplicitRootFrame$...`；
  - `__scoop_explicit_root_offsets__*` 常量 offset table；
  - `__scoop_explicit_root_desc__*` 常量 descriptor。
- 当前布局规划仍是 correctness-first 的静态 superset：
  - 所有 entry alloca 都按其 LLVM storage type 展开 GC leaf pointer fields；
  - ordinary indirect GC aggregate params 额外按 pointee storage type 展开并纳入同一 frame layout；
  - 因此 descriptor 已明确围绕 leaf-slot，而不是“整个 aggregate/local 大字段”。
- 已锁定的回归：
  - direct ref local 会得到 descriptor + 单槽 offsets；
  - `Named(String, Int)` 这类 indirect aggregate param 仅生成一个 `String` leaf slot；
  - hidden-sret caller temp 会进入 descriptor 规划。
- 这一阶段尚未开始把这些逻辑 slots 绑定到真实 activation frame object/TLS push-pop；该部分仍留给 `T5001d2`。
- Review 状态（T5001d1R，2026-04-29）：已复核 `mod.rs`、`call/abi.rs`、`effect/state_machine_emitter.rs` 与 LLVM 回归，确认当前 descriptor 仍只描述 tracked stable home slots，aggregate flattening 继续按 GC leaf slots 建模，没有把 heap-backed traced fields、普通非 root 局部或机器栈偏移混入 descriptor。review 期间还修复了两处覆盖面缺口：顶层不可变值初始化函数此前虽设置 GC strategy，但未开启/结束 explicit frame layout；effect state-machine 的 `step/dispatch` 托管函数及其返回/result 临时槽位也未统一走 tracked entry alloca。现这两类路径都已补齐 descriptor 发射，并新增 LLVM 回归锁定。配套验证已通过 `cargo test -p scoopc --lib`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 与 `cargo run -p scoop -- test --fixtures tests/fixtures/build`。
- 当前状态（T5001d2，2026-04-29）：编译器现已把函数级 layout 真正落到 activation frame lifecycle 上。`begin_function_explicit_frame_layout(...)` 会在 entry 先发射原始 frame storage alloca，tracked entry allocas / hidden sret / indirect aggregate spill / `extra_gc_root_slots` 会同步预留 explicit-frame mirror slots；`finish_function_explicit_frame_layout(...)` 在 descriptor 发射后再回填 frame storage 大小，并统一注入 entry setup 与 return-time teardown。entry setup 会：写入 `hdr.prev` / `hdr.desc`、把所有 frame slots 置 `NULL`、并 push 到 `@__scoop_explicit_root_frame_top`。函数所有 `ret` 终结点前都会插入对称 teardown：先把各 frame slots 清回 `NULL`，再把 TLS top 恢复到 `hdr.prev`。ordinary safepoint 这边，`with_conservative_gc_local_root_spills(...)` 也已在调用前把 live stack-backed GC roots 镜像到 explicit frame，调用后写回原 spill slot 并把对应 frame slot 清零，从而让 inactive slot 不会在 safepoint 之后长期残留旧对象地址。closure、object init、resume wrapper/entry、top-level immutable init、effect state-machine step/dispatch 等 managed body 入口均已接上同一 lifecycle。新增 LLVM 回归锁定 TLS push/pop 与 safepoint slot clear，配套验证已通过 `cargo test --all`、`cargo clippy --all-targets -- -D warnings` 与 `cargo run -p scoop -- test --fixtures tests/fixtures/build`。
- Review 状态（T5001d2R，2026-04-29）：已复核 `mod.rs`、`gc.rs`、`call/resume.rs`、`effect/state_machine_emitter.rs` 与 LLVM 回归，确认 managed body 的 explicit frame storage 统一经 entry-block alloca 建立，top-level/object-init/closure/callee-resume/effect state-machine step/dispatch 等路径都通过同一个 `finish_function_explicit_frame_layout(...)` 收尾。review 期间发现一处真实缺口：teardown 先前只插入到 `ret` 终结点，导致 `CgTy::Never` 共享返回块上的 `unreachable` 会漏掉 slot 清零和 TLS pop；现已修复为同时覆盖 `ret` / `unreachable` 终结点，并新增 LLVM 回归 `never_returning_managed_function_pops_explicit_root_frame_before_unreachable` 锁定。结合既有 safepoint spill mirror clear 回归，可确认当前阶段的 NULL discipline 已覆盖 entry init、ordinary safepoint epilogue 与函数退出 teardown 三个关键边界。配套验证已通过 `cargo test -p scoopc --lib`、`cargo run -p scoop -- test --fixtures tests/fixtures/build` 与 `cargo clippy --all-targets -- -D warnings`。

### P4. 把所有跨 safepoint roots 收敛到 stable home slots

- 源码级 locals、aggregate 内 ref leaf、sret/indirect arg scratch、effect/state-machine lowering 生成的临时 GC roots、`extra_gc_root_slots` 一类内部根，都必须映射到 explicit frame 的固定 fields。
- descriptor 记录 slot offset；frame field 本身保存对象头指针值。
- 不允许继续依赖“LLVM 之后会帮我 spill 成可更新 location”这一旧合同。
- 当前状态（T5001d3，2026-04-29）：compiler 侧现已把 explicit frame home slot 变成真正的长期 source of truth，而不再只是 ordinary safepoint 的临时 mirror。
  - `store_local_value_exact(...)` 对 stack-backed managed locals / aggregates 的写入后，会同步把 GC leaf refs 写进对应 explicit frame home slots；因此源码级 local root 与 aggregate ref leaf 已不再只停留在原始 alloca/spill 里。
  - ordinary indirect GC aggregate params 现在会在参数绑定时建立 incoming slot -> explicit frame slot 的持久映射，并在 entry 立刻把 ref leaves 同步进 frame，补上了此前“descriptor 已预留，但 incoming aggregate roots 没有真正进入 frame”的缺口。
  - `DeferredGcSensitiveSpill`、call-arg spill 等编译器内部临时根不再走 `extra_gc_root_slots` / root-slot-id 动态注册；已改为直接跟踪固定 spill/home-slot，并在值 materialize/消费后清掉对应 frame home slot，避免 inactive root 长期残留。
  - ordinary safepoint 的 keepalive 源也已切到 explicit frame home slots：调用前从 frame home slot load，调用后同时写回原 spill/local 与 frame home slot；因此 moving update 后不会再依赖“先前 spill/local 还是旧 source-of-truth，frame 只是临时数组镜像”的旧路径。
  - hidden sret 结果现在会在 call 返回后立即同步进 frame home slot，并在 load 结果后清理该 home slot；function-value/closure/vtable/itable/raw-MIR/callee-resume 等 hidden-sret 路径均已接入同一合同。
  - `emit_enter_native_for_extern_call_impl(...)` 在 explicit frame 已启用时也改为暴露 home-slot 指针，而不再偏向旧 local/spill 槽位形态。
  - 配套 LLVM 回归已新增/更新：一条新断言锁定 indirect aggregate param 会在 safepoint 前写入 explicit frame home slot；既有 safepoint/statepoint 断言则已更新为“home slot 在 safepoint 后保持最新 relocated 值，仅在 activation teardown 时清零”的合同。验证已通过 `cargo test --all`、`cargo clippy --all-targets -- -D warnings` 与 `cargo run -p scoop -- test --fixtures tests/fixtures/build`。
- Review 状态（T5001d3R，2026-04-29）：已复核 `mod.rs`、`gc.rs`、`mir_body.rs`、`call/abi.rs`、`call/dispatch.rs`、`effect/mod.rs` 与 `effect/state_machine_emitter.rs` 的跨 safepoint root 来源，确认 `extra_gc_root_slots` / root-slot-id 动态注册已从当前 lowering 主线移除，ordinary locals、indirect aggregate params、hidden sret、deferred spill / call-arg spill 与 effect/state-machine/continuation 产生的 stack-backed temporaries 都已统一收口到 explicit frame home slots。review 期间发现并修复了一处 effect/resume 边界缺口：共享 `Continuation.resume` runtime helper `resume_continuation_with_encoded_payload(...)` 此前直接发射 `scoop_continuation_resume_with` 调用，没有走 `build_call_preserving_gc_local_roots(...)`，会让 replay / tail-resume / ordinary resume 路径绕开与 ordinary safepoint 一致的 keepalive + home-slot write-back 合同；现已统一改走该 helper 包装，并新增 LLVM 回归锁定调用窗口必须出现 GC keepalive、explicit frame home-slot 参与及调用后 write-back。配套验证已通过 `cargo test -p scoopc state_machine_multi_payload_perform_uses_tuple_transport`、`cargo test -p scoopc when_arm_try_resume_nested_handle_ir_keeps_binder_scope_for_inner_resume`、`cargo test -p scoopc --lib` 与 `cargo clippy -p scoopc --all-targets -- -D warnings`。

### P5. safepoint clobber / reload 与 aggregate contract 收紧

- 明确所有 safepoint 都是 GC ref clobber 边界。
- post-safepoint 的 ref 使用必须从 home slot reload。
- 对含 ref 的 aggregate：
  - 旧 aggregate 副本不再是 source of truth；
  - direct call arg、indirect arg、sret result、effect payload、continuation payload 等路径，必须按“reload 最新 ref 字段 + 复用非 ref 字段 + 重组 fresh aggregate”的 contract lowering。
- 当前状态（T5001e1，2026-04-29）：pointer-shaped GC 值的 post-safepoint reload 主线已收紧到 explicit frame home slot。compiler 现已新增单槽 GC pointer 的统一 reload helper，并让 `local_ptr_for_use(...)`、`materialize_deferred_cg_value(...)` 与 `materialize_deferred_cg_value_for_call_arg_impl(...)` 在 explicit frame 已启用时优先从 home slot 取回值，因此 ordinary local 读取、runtime helper / ordinary call 后的 deferred scalar GC 值消费，以及 effect/resume lowering 中复用 `local_ptr_for_use(...)` 的 direct ref 路径，均不再默认回落到原 local/spill slot。配套新增两条 LLVM 回归，分别锁定 direct local safepoint 后 reload 与 deferred call arg 经后续 safepoint 后的 reload source，并通过 `cargo test -p scoopc --lib`、`cargo run -p scoop -- test --fixtures tests/fixtures/build`、`cargo clippy -p scoopc --all-targets -- -D warnings` 验证。
- Review 状态（T5001e1R，2026-04-29）：已复核 `mod.rs`、`call/abi.rs`、`mir_body.rs`、`effect/mod.rs`、`effect/state_machine_emitter.rs` 与对应 LLVM 回归，确认 HIR/direct-ref 读取点、deferred spill / call-arg materialize，以及 effect/resume/state-machine 中复用 `local_ptr_for_use(...)` / `materialize_deferred_cg_value(...)` 的 direct GC use，已统一把 explicit frame home slot 作为 post-safepoint reload source-of-truth。review 期间发现并修复了一处 production MIR bridge 漏洞：`load_mir_local(...)` 仍直接从 `slot.ptr` 加载，raw/materialized MIR body 在 safepoint 后可能继续读取旧 local 槽位；现已改为同样经 `local_ptr_for_use(...)` 走 single-slot reload helper，并新增 `production_mir_function_reloads_direct_gc_local_from_explicit_frame_after_safepoint` 回归锁定 ordinary managed call 后的 MIR local reload。配套验证已通过 `cargo test -p scoopc --lib`、`cargo run -p scoop -- test --fixtures tests/fixtures/build` 与 `cargo clippy -p scoopc --all-targets -- -D warnings`。
- 当前状态（T5001e2，2026-04-29）：aggregate refresh / rebuild contract 已补齐到 explicit-frame source-of-truth。compiler 现已新增通用 aggregate rebuild helper：对带 GC leaf 的 stack-backed storage slot，不再整体 `load` 旧 local/spill/sret 镜像，而是按“GC ref leaf 从 explicit-frame home slot reload、非 ref leaf 从原 storage slot 读取”的规则重建 fresh aggregate；需要地址的入口则额外落到临时 rebuild alloca。`local_ptr_for_use(...)` 因此不再只覆盖 direct ref，也能为 tuple/struct/tagged-union enum 等 aggregate 读路径提供 fresh storage；`materialize_deferred_cg_value(...)`、`materialize_deferred_cg_value_for_call_arg_impl(...)`、`deferred_gc_spill_slot_for_call_arg_impl(...)` 与 `load_hidden_sret_result_from_ptr(...)` 也已统一改走该 contract，从而 direct/indirect call args、hidden sret returns、effect boxed payload transport，以及复用同一 materialize/arg 入口的 continuation/state-machine payload transport，均不再继续传播 stale aggregate 副本。新增三条 LLVM 回归分别锁定 aggregate call arg rebuild、hidden-sret aggregate result rebuild 与 boxed effect payload rebuild，并已通过 `cargo test -p scoopc --lib`、`cargo run -p scoop -- test --fixtures tests/fixtures/build`、`cargo test --all` 与 `cargo clippy -p scoopc --all-targets -- -D warnings` 验证。
- Review 状态（T5001e2R，2026-04-29）：已复核 `mod.rs`、`call/abi.rs`、`call/resume.rs`、`effect/mod.rs` 与 `effect/state_machine_emitter.rs` 的 aggregate use-site，重点确认是否还有绕开 `storage_slot_for_use(...)` 的 post-safepoint stale aggregate 传播路径。结论是：stack-backed aggregate 的 direct/indirect args、hidden-sret result 与 boxed effect payload 已统一经 explicit-frame rebuild helper 收口；continuation / state-machine payload transport 继续复用 `encode_effect_transport_value(...)`，因此也遵守“GC ref leaf 从 explicit-frame home slot reload、非 ref leaf 从原 storage 或 heap field 读取”的同一合同。review 期间未发现新的 aggregate correctness 缺口，并已通过 `cargo test -p scoopc --lib`、`cargo run -p scoop -- test --fixtures tests/fixtures/build` 与 `cargo clippy -p scoopc --all-targets -- -D warnings` 再次验证。

### P6. 默认 explicit mode 切换与 stackmap 退居可选优化

- 默认 explicit mode 的 managed roots 完全由 explicit root frame 支撑。
- runtime 默认 explicit mode 不再读取 stackmap registry。
- 编译器默认 explicit mode 不再生成 stackmap sections / records。
- stackmap 路径保留为未来可选模式，但必须从“默认 correctness 依赖”降级为“可审计的优化后端”。
- 当前状态（T5001f，2026-04-30）：默认 explicit mode 已切换完成。compiler 现已移除默认托管函数与 synthetic `main` 的 `gc "statepoint-example"` 标记，closure/object-init/raw-MIR/effect runtime/callee-resume 等托管入口也不再进入 LLVM statepoint rewrite；synthetic `main` 同时接入 explicit root frame lifecycle，并修复了 frame storage alloca 必须固定在 entry alloca 区的 dominance 缺口。runtime 现已停止在 `scoop_runtime_init()` 时默认注册当前进程 stackmap registry，`InNative` 线程上的 managed roots 枚举/更新/mark 也允许“仅 native_roots、无 managed frame root map”的默认 explicit-frame 场景，因此 stackmap 已不再是默认路径的 runtime 前提。
- 已锁定的回归：
  - LLVM/object 断言默认产物不再出现 `gc "statepoint-example"`、`llvm.experimental.gc.statepoint`、`llvm.experimental.stackmap` 与 stackmap section；
  - `stackmap_registry` runtime 测试改为断言默认 init 不自动注册，但手动注册当前进程 stackmaps 仍可用；
  - build fixtures 删除了只服务旧默认 stackmap 路径的 dump/registry smoke，并新增 `explicit_root_frame_default_mode_no_stackmaps.scoop` 锁定 synthetic `main` 也走 explicit root frame。
- 配套验证已通过 `cargo test -p scoopc minimal_main_obj_omits_stackmap_section_by_default`、`cargo test -p scoopc minimal_main_obj_with_live_gc_roots_still_omits_stackmap_section`、`cargo test -p scoopc default_explicit_mode_omits_statepoint_intrinsics_and_gc_strategy`、`cargo test -p scoopc thread_join_preserves_live_gc_locals_via_explicit_root_frame`、`cargo test -p scoopc effect_runtime_functions_use_explicit_root_frame_without_statepoints`、`cargo test -p scoop_runtime`、`cargo run -p scoop -- test --fixtures tests/fixtures/build` 与 `cargo clippy --all-targets -- -D warnings`。
- Review 状态（T5001fR，2026-04-30）：已复核 runtime init、GC managed-root-map 选择、默认 LLVM codegen 与现有“无 stackmap”断言，确认默认 explicit mode 的 source-of-truth 现已真正收口到 explicit root frame：`scoop_runtime_init()` 默认不再注册 stackmap registry，GC 在 managed path 上只把 explicit frame 视为默认 roots 输入，而默认编译产物继续锁定无 `gc "statepoint-example"`、无 statepoint/stackmap intrinsics、无 stackmap section。review 期间发现一处真实边界回归：保留中的显式 stackmap smoke helper `__scoop_stackmap_statepoint_smoke()` 在默认移除 GC strategy 后失去了生成真实 record 的能力；现已修复为仅对显式调用该 helper 的函数恢复 LLVM statepoint GC strategy，从而把 stackmap 保持为按需 opt-in 的可选实现，而不是默认 correctness 依赖。新增 LLVM 回归锁定该 helper 会重新进入 statepoint pipeline 并按需产出 stackmap section；并已通过 `cargo test -p scoopc --lib`、`cargo run -p scoop -- test --fixtures tests/fixtures/build` 与 `cargo clippy -p scoopc --all-targets -- -D warnings` 验证。

### P7. 稳定化、回归与后续优化入口

- 全量回归要覆盖：`cargo test --all`、fixture runner、moving GC、stress GC、verify-roots、effect/continuation/resume 路径、native 边界路径。
- 需要补充最小定向回归，锁定：
  - explicit frame push/pop 与 TLS chain；
  - dead slot 清零；
  - post-safepoint reload；
  - aggregate refresh/rebuild；
  - 默认 explicit mode 不再产出 stackmap section。
- 在 correctness 稳定后，才单独启动两类后续优化任务：
  - stackmap selective comeback；
  - `mem2reg` allowlist/denylist rollout。
- 当前状态（2026-05-01）：本轮已把最初那串 async/task/class-init/effect/GC-stress/sync regressions 逐步收口，并把同步原语改成了 “GC 壳对象 + unmanaged sidecar” 设计；当前仍未闭合的 correctness 缺口，已经不再是零散单点 bug，而是两类需要系统收口的设计问题：
  - state-machine 仍会把 heap frame field 的 GEP 直接放进 env 作为 local home；一旦 state/arm body 内发生分配并触发 moving GC，这些 slot pointer 会整体 stale，导致 `saved = Some(k)` / `cell.k = Some(k)` 一类写入看似执行但最终没有生效；
  - continuation / replay-state 的长期 owner 仍有一部分依赖长期 pin 或裸指针语义，尚未统一收口为 stable GC handle。
- 顺序判断：接下来不再继续追加 ad hoc 补丁，而是先完成两步设计性收口，再做 full review：
  1. 把 state-machine frame-backed locals 统一改成“稳定执行期 local home + 状态终结点统一 flush-back”；
  2. 把 continuation / replay-state 的长期 owner 全部改成 stable handle，只保留短窗口动态 pin；
  3. 每一步都补对应最小 fixture/LLVM/runtime 回归；
  4. 完成后再重新执行三项 GC env 全开的全量 fixture 扫描与最终验收。
- 任务拆分更新（2026-05-01）：执行 `T5001f8` 前先复现当前 blocker，确认 `continuation_resume_enum.scoop` 仍在默认环境与 GC env 下输出 `missing1/missing2`，而 `effect_multi_escape_indirect_direct_while.scoop` 与 `class_init_raise_cleanup_init_block_gc_basic.scoop` 当前单跑可通过。进一步查看 LLVM IR 后确认，当前最前置缺口是 outer mutable local 在原 caller handle-return 路径上的 source-of-truth 没有闭合：`write_back_outer_scope_frame_slots(...)` 通过 frame 中保存的裸 storage pointer 回写 backing alloca 时，无法再把 caller explicit-frame home slot 同步成最新值，导致 handle 之后的 caller 立刻读取仍看到旧镜像。为避免把 `T5001f8` 做成不可审计的大块重构，现已将其拆为：
  1. `T5001f8a`：先修复 outer mutable local 在原 caller handle-return 路径的稳定 writeback / readback 合同，并用最小 escape-continuation fixture 锁定；
  2. `T5001f8b`：再把 state/arm body 内长期暴露给 env 的 heap-frame GEP 收口为稳定执行期 local home；
  3. `T5001f8c`：最后统一 suspend / return / arm-exit / cleanup 的 mutable-local flush-back，并复核 direct/indirect mixed 回归。
  当前本轮只执行新的首个子任务 `T5001f8a`。
- 阻塞更新（2026-05-01，本轮执行中）：在实现 `T5001f8a` 的 caller-side writeback/readback 加固后，`continuation_resume_enum.scoop` 已在默认环境恢复，但在 `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1` 下仍输出 `missing1/missing2`。重新检查最新 LLVM IR 后确认，新的真实前置 blocker 位于更早的 state-body local home：`populate_frame_slots_in_env(...)` 仍把 outer mutable local 的 heap-frame field GEP 直接塞进 env，导致第一段 handle 的 state-body `saved = Some(k)` 在 GC env 下会在 payload/enum 构造触发分配后继续写入 stale heap-slot pointer。按阻塞规则，现已在 `TODO.md` 中把该问题前移为新的前置任务 `T5001f8a0/T5001f8a0R`，并把原 `T5001f8a` 顺延到其后。
- 当前已落地但不足以单独收口 `T5001f8a` 的改动：
  - `write_back_outer_scope_frame_slots(...)` 现在在 handle 仍位于原 caller activation 时，会优先经 caller 当前稳定 backing slot 回写，从而同步 caller-facing explicit-frame home slots，而不是只经由 frame 中保存的裸 storage pointer；
  - 新增最小 fixture `tests/fixtures/run-pass/effect_escape_continuation_outer_mutable_writeback_basic.scoop`，锁定“escape-continuation arm 把 outer mutable local 写成 `Some(k)` 后，handle 返回 caller，caller 立即读取能看到最新值”的默认环境窗口；
  - 这些改动已通过默认环境定向验证，但尚不足以消除 GC env 下由 state-body stale heap-slot pointer 导致的同一主线 blocker，因此本轮按规则停止在任务重排，并把后续实现转交给新的 `T5001f8a0`。
- 进展更新（2026-05-01）：`T5001f8a0` 已完成。已在 compiler 侧为 `store_local_value_exact(...)` 写入前统一补上 `rematerialize_ptr_in_current_block(...)`，确保 state/arm body 内对 outer mutable local 的 store 不会在 RHS 分配触发 moving GC 后继续写入 stale heap-slot pointer；并新增 runtime_gc fixture `effect_outer_mutable_state_body_writeback_basic.scoop` 锁定该窗口。下一步进入 `T5001f8a0R` 复核后再继续推进 `T5001f8a`。
- 当前状态（T5001f1，2026-04-30）：已定位并修复 await waiting transport 的 source-of-truth 漏写。此前 `effect/state_machine_emitter.rs` 在 escaped continuation waiting-path 中从 effect frame 读出 `%continuation_val` 后，直接把 continuation local 暴露给 `__task_step_pending(...)`，却没有通过普通 local-store 路径同步 explicit-frame home slot，导致 Waiting state 记录到 `null` continuation，二次 drive 在 `scoop_continuation_resume_with(...)` 处以空 continuation 崩溃。现已统一改为经 `store_local_value_exact(...)` 写回 continuation local/home slot，再构造 `__task_step_pending(...)` 调用；新增 LLVM 回归 `async_task_pending_path_stores_escape_continuation_before_waiting_helper` 锁定 `load_continuation -> local store -> explicit-frame store -> pending helper` 序列，并已定向复验 `task_step_manual_basic.scoop`、`async_await_minimal_int_basic.scoop`、`async_await_string_basic.scoop`、`async_fun_task_runtime_basic.scoop`、`cargo test -p scoopc --lib` 与 `cargo clippy -p scoopc --all-targets -- -D warnings` 通过。继续尝试 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 时，suite 已越过原 async blocker，但目前会在新的独立 fixture `class_init_order_primary_secondary_basic.scoop` 处失败；该问题不属于本次 await/task waiting transport 修复本身。
- Review 状态（T5001f1R，2026-04-30）：已复核 `effect/state_machine_emitter.rs` 中 escaped continuation waiting-path 与相关 LLVM/fixture 回归，确认 await waiting transport 已重新闭合到 continuation resume 合同：escaped continuation 会先经 `store_local_value_exact(...)` 写回 ordinary local 与 explicit-frame home slot，再交给 `__task_step_pending(...)` 进入 Waiting state，因此 awaited task 的后续 drive 不会再丢失 continuation，也不会重放原 await site。定向验证已通过 `cargo test -p scoopc async_task_pending_path_stores_escape_continuation_before_waiting_helper -- --nocapture`、`cargo test -p scoopc async_task_resume_ir_does_not_replay_original_await_site -- --nocapture`、`cargo test -p scoopc single_file_minimal_ir_supports_handled_async_await -- --nocapture`、`cargo run -p scoop -- run tests/fixtures/run-pass/task_step_manual_basic.scoop`、`cargo run -p scoop -- run tests/fixtures/run-pass/async_await_minimal_int_basic.scoop`、`cargo run -p scoop -- run tests/fixtures/run-pass/async_await_string_basic.scoop`、`cargo run -p scoop -- run tests/fixtures/run-pass/async_fun_task_runtime_basic.scoop` 与 `cargo clippy -p scoopc --all-targets -- -D warnings`。review 同时确认 run-pass 已越过原 async/task waiting regression；为遵守 `PROMPT.md` 的“先修 blocker 再前进”要求，现已把新暴露出来的 `class_init_order_primary_secondary_basic.scoop` 类初始化顺序回归提炼为前置任务 `T5001f2/T5001f2R`，放在 `T5001g` 之前处理。
- 当前状态（T5001f2，2026-04-30）：已定位并修复类初始化顺序回归的真实根因，不是 property/init 顺序调度本身，而是 ctor-inline `this` local 没有同步 explicit-frame home slot。此前 `class_ctor.rs` 在 `codegen_class_ctor_invoke_inner(...)` 中为 `this` 创建 entry alloca 后直接做裸 `store`，导致 `Primary.a` property initializer 里先执行 `println("Primary.a")` 触发 safepoint 后，随后 `this.x` 的 post-safepoint 读取会从未同步的 `this` local/home-slot 路径取到空值并在运行期段错误。现已统一改为经 `store_local_value_exact(...)` 写入 `this`，让 ctor-inline `this` 与其他 GC local 一样遵守 explicit-frame source-of-truth / post-safepoint reload contract；并新增 LLVM 回归 `class_ctor_this_local_reloads_from_explicit_frame_after_safepoint` 锁定该行为。定向验证已通过 `cargo test -p scoopc class_ctor_this_local_reloads_from_explicit_frame_after_safepoint -- --nocapture` 与 `cargo run -p scoop -- run tests/fixtures/run-pass/class_init_order_primary_secondary_basic.scoop`，fixture 现已恢复完整 14 行输出。
- 新阻塞（2026-04-30）：继续执行 `cargo run -p scoop -- test` 做 run-pass 验证时，suite 已越过 `class_init_order_primary_secondary_basic.scoop`，但随后在 `tests/fixtures/run-pass/effect_escape_continuation_gc_stress_multi_string.scoop` 处失败，表现为 stdout 与 golden 不一致。该问题不属于 class init 顺序本身，而是新暴露出来的既有 effect/continuation GC-stress 回归。按“遇到既有问题必须先修或先排前置任务”的规则，现已新增 `T5001f3/T5001f3R`，并把 `T5001f2R` 顺延到其后，再继续后续 review / 全量验收。
- 当前状态（T5001f3，2026-04-30）：已定位并修复 effect escape continuation GC-stress 回归的真实根因。问题不在 golden/fixture，而在 fresh `Continuation.resume(...)` lowering：此前会先把 continuation receiver 读成原始 `%load_ref`，随后才去 materialize `String`/boxed payload；在 `SCOOP_GC_STRESS=1` 下，这个 payload 分配可触发 moving GC，导致 `scoop_continuation_resume_with(...)` 收到 stale continuation 指针，并在第一次 resume 就误报 `ContinuationAlreadyResumed`。现已在 `crates/scoopc/src/llvm/codegen/effect/mod.rs` 中把 receiver 收口为普通 GC-sensitive call arg 合同：先经 `defer_gc_sensitive_cg_value(...)` spill 成 tracked root，再在 payload materialize 完成后通过 `continuation_resume_receiver_reload` reload，最后才调用 runtime resume helper。另补充 LLVM 回归 `continuation_resume_reloads_receiver_after_gc_sensitive_payload_materialization`，锁定“payload 分配后 reload receiver，再进入 `scoop_continuation_resume_with(...)`”的 IR 顺序。定向验证已通过 `cargo test -p scoopc continuation_resume_reloads_receiver_after_gc_sensitive_payload_materialization -- --nocapture`、`cargo test -p scoopc when_arm_try_resume_nested_handle_ir_keeps_binder_scope_for_inner_resume -- --nocapture`、`SCOOP_GC_STRESS=1 cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_gc_stress_multi_string.scoop` 与 `cargo clippy -p scoopc --all-targets -- -D warnings`；继续执行 `cargo run -p scoop -- test` 时，suite 已越过该 fixture。
- 新阻塞（2026-04-30）：在 `T5001f3` 修复后继续执行 `cargo run -p scoop -- test`，新的首个失败已切换为 `tests/fixtures/run-pass/gc_cross_function_class_object_graph.scoop`。该 fixture 默认环境单跑可通过，但 suite 所用 `SCOOP_GC_STRESS=1` 下会失败/卡住，说明 cross-function class object graph 在 GC-stress 下仍有真实 correctness 缺口。按同一规则，现已新增 `T5001f4/T5001f4R` 作为 `T5001f3R` 之前的前置任务，再继续后续 review / 全量验收。
- 当前状态（T5001f4，2026-04-30）：已定位并修复 cross-function class object graph GC-stress 回归的真实根因。问题不在 setter/readback fixture 本身，而在 factory ctor path：此前 `class_ctor.rs` 会在 `scoop_alloc_typed(...)` 返回后把新分配的 class object 只保留在 `%rt_alloc_class` 这类 SSA 指针里，随后才去求值会再次分配 `String` 的 ctor args / property init 输入；在 `SCOOP_GC_STRESS=1` 下，这个窗口内的 moving GC 会更新 spill/home slots，却不会更新裸 SSA，因此后续 ctor init / field write / return 会继续使用 stale object pointer，并在 `gc_cross_function_class_object_graph.scoop` 的首个 `__scoop_gc_collect()` 后卡住。现已把 freshly allocated class object 本身也收口为 tracked GC root：对象分配后立即经 `defer_gc_sensitive_cg_value(...)` spill 成 `class_ctor_obj_root`，并在 ctor arg eval 前、进入 ctor init 前以及 factory return 前都从 explicit-frame-backed home slot reload 当前对象指针。新增 LLVM 回归 `class_ctor_factory_keeps_allocated_object_rooted_across_gc_sensitive_arg_eval` 锁定该 contract；定向验证已通过 `cargo test -p scoopc class_ctor_factory_keeps_allocated_object_rooted_across_gc_sensitive_arg_eval -- --nocapture`、`cargo test -p scoopc class_ctor_this_local_reloads_from_explicit_frame_after_safepoint -- --nocapture`、`SCOOP_GC_STRESS=1 cargo run -p scoop -- run tests/fixtures/run-pass/gc_cross_function_class_object_graph.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/gc_cross_function_class_object_graph.scoop` 与 `cargo clippy -p scoop -p scoopc --all-targets -- -D warnings`。另外已顺手扩展 `scoop test --fixtures`，现在既可接受目录，也可直接接受单个 fixture 文件，便于后续逐个复现阻塞项。
- 新阻塞（2026-04-30）：在 `T5001f4` 修复后继续执行 `cargo run -p scoop -- test`，suite 已越过 `gc_cross_function_class_object_graph.scoop`，新的首个失败切换为 `tests/fixtures/run-pass/higher_order_aggregate_return_struct_mapper.scoop`。该 fixture 也在 `SCOOP_GC_STRESS=1` 下触发真实回归：direct higher-order 调用 `mapper("go")` 当前输出 `!!` / `2`，而不是 golden 的 `go!` / `3`，说明 higher-order aggregate return 的 `String` field / aggregate source-of-truth 仍存在 correctness 缺口。按同一规则，现已新增 `T5001f5/T5001f5R` 作为 `T5001f4R` 之前的前置任务，再继续 review / 全量验收。
- 当前状态（T5001f5，2026-04-30）：已定位并修复 higher-order aggregate return struct mapper 的 GC-stress 回归。问题不在 hidden-sret aggregate return 自身，而在 closure 内的 builtin `String.concat` lowering：此前 `codegen_string_method(...)` 会先把 receiver 读成 `%load_str` 一类 SSA，然后才去求值后续参数 `"!"`；在 `SCOOP_GC_STRESS=1` 下，这个字符串字面量分配可触发 moving GC，导致 `scoop_string_concat(...)` 继续消费 stale receiver 指针，`mapper("go")` 返回的 `Labelled.text` / `Labelled.score` 在 direct higher-order aggregate return 起点就已经错误。现已把 builtin string-method receiver 也纳入 GC-sensitive defer/reload 合同：receiver 先经 `defer_gc_sensitive_cg_value(...)` spill 成 tracked root，再在真正调用 `concat` / `replace` / `compareTo` / `repeat` / `charAt` / `getByte` / `unsafeSliceBytes` 等路径前从 explicit-frame-backed home slot materialize/reload。另新增 LLVM 回归 `higher_order_aggregate_return_reloads_string_receiver_after_gc_sensitive_arg_eval`，锁定 closure-lowered `String.concat` 会在 `"!"` 分配后 reload receiver 再进入 `scoop_string_concat(...)`。定向验证已通过 `cargo test -p scoopc higher_order_aggregate_return_reloads_string_receiver_after_gc_sensitive_arg_eval -- --nocapture`、`env SCOOP_GC_STRESS=1 cargo run -p scoop -- run tests/fixtures/run-pass/higher_order_aggregate_return_struct_mapper.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/higher_order_aggregate_return_struct_mapper.scoop` 与 `cargo clippy -p scoop -p scoopc --all-targets -- -D warnings`。
- 新阻塞（2026-04-30）：在 `T5001f5` 修复后继续执行 `cargo run -p scoop -- test`，suite 已越过 `higher_order_aggregate_return_struct_mapper.scoop`，新的首个失败切换为 `tests/fixtures/runtime_gc/task_step_cross_thread_sequential_handoff_gc_stress.scoop`。当前程序只输出到 `inner-ready / inner / 41`，缺失 golden 中的 `outer-after-await`、`worker-ready` 与 `main-final-ready`，说明 `Task.step()` 顺序跨线程 handoff 在 moving GC + verify-roots + stress 组合下仍有真实 runtime correctness 缺口。按同一规则，现已新增 `T5001f6/T5001f6R` 作为 `T5001f5R` 之前的前置任务，再继续 review / 全量验收。
- 当前状态（T5001f6，2026-04-30）：已把本轮 blocker 收敛为“fresh GC object / GC receiver 在 safepoint 后继续读旧 SSA”的一组系统性漏洞，而不只是单个 cross-thread handoff 特例。compiler 侧新增统一 fresh-ref defer/reload helper，并把 effect frame、effect transport box、array builder receiver、function-value call closure object、vtable/itable receiver、closure/MIR closure object、MIR capture box 等路径纳入同一 contract：凡是 `scoop_alloc_typed(...)` 或 GC-sensitive receiver 会在后续分配/legacy boundary/dispatch 之后再次被消费，都必须先落到 explicit-frame-backed tracked root，再在真正使用前 reload。与此同时，effect state-machine return slots 改为 entry 零初始化，避免 `TaskStep` / `__TaskStepResult` 的 GC leaf 从 `undef` 同步进 explicit roots。新增 LLVM 回归覆盖 `async_task_effect_return_slots_start_null_before_resume_writes`、`closure_call_with_real_outward_effect_uses_explicit_outcome_boundary`（现同时断言 `closure_call_obj_reload`）、`virtual_call_with_real_outward_effect_uses_explicit_outcome_boundary`（现同时断言 `vtable_call_receiver_reload`）、`interface_call_with_real_outward_effect_uses_explicit_outcome_boundary`（现同时断言 `itable_call_receiver_reload`）、`continuation_resume_boxed_payload_reloads_box_object_before_runtime_call` 与既有 `continuation_resume_reloads_receiver_after_gc_sensitive_payload_materialization`。定向验证已通过 `cargo test -p scoopc async_task_effect_return_slots_start_null_before_resume_writes -- --nocapture`、`cargo test -p scoopc array_of_string_uses_ref_element_runtime_apis_without_ptr_to_u64 -- --nocapture`、`cargo test -p scoopc closure_call_with_real_outward_effect_uses_explicit_outcome_boundary -- --nocapture`、`cargo test -p scoopc virtual_call_with_real_outward_effect_uses_explicit_outcome_boundary -- --nocapture`、`cargo test -p scoopc interface_call_with_real_outward_effect_uses_explicit_outcome_boundary -- --nocapture`、`cargo test -p scoopc continuation_resume_boxed_payload_reloads_box_object_before_runtime_call -- --nocapture`、`cargo test -p scoopc continuation_resume_reloads_receiver_after_gc_sensitive_payload_materialization -- --nocapture`、`env SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- run tests/fixtures/runtime_gc/task_step_manual_gc_aggregate_transport_basic.scoop`、`env SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- run tests/fixtures/runtime_gc/task_step_cross_thread_sequential_handoff_gc_stress.scoop`，以及两条 harness：`cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/task_step_manual_gc_aggregate_transport_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/task_step_cross_thread_sequential_handoff_gc_stress.scoop`。这表明 `T5001f6` 的主 handoff regression 已闭合，suite 也不再卡在该 runtime_gc fixture。
- 新阻塞（2026-04-30）：在完成 `T5001f6` 后继续按“顺手排查同类旧 SSA 路径”验证 `Continuation.resume` boxed payload / verify-roots 组合时，发现 `tests/fixtures/run-pass/continuation_resume_struct_with_ref.scoop` 在 `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1` 下仍会于 `after_handle / alice` 之后报 explicit-frame invalid root。当前 IR 已确认：`when_subject` root 会在 arm 绑定后清空，`Continuation.resume` receiver reload 与 boxed payload object reload 也都已落位，但 consumed continuation 在 `when` binder / nested handle / resumed-body 后续 GC 窗口里仍残留一组 stale explicit roots，说明还有一个独立的 `Continuation.resume` consumed-root correctness 缺口。按同一规则，现已把该问题提炼为前置任务 `T5001f7/T5001f7R`，并把 `T5001f6R` 顺延到其后，再继续 review / 全量验收。
-- 当前状态（T5001f7，2026-04-30 / 2026-05-01 扩展）：`Continuation.resume` / escaped continuation 主线已经确认不只是“某个 receiver/payload 没 reload”的局部问题，而是暴露了两个更深层的 source-of-truth 缺口：
  - heap-backed handle frame 的 field GEP 被直接当作执行期 local home 使用；
  - continuation / replay-state 的长期 owner 还没有完全和 stable handle 对齐。
  这两类缺口目前直接体现在 `continuation_resume_enum.scoop` 与 `effect_multi_escape_indirect_direct_while.scoop` 上：前者表现为 outer mutable local `saved` 写回后仍读到 `None`，后者表现为 direct/indirect mixed escape arm 中 `cell.k` / resumed path 仍会丢 continuation。后续任务已从“继续单点修 fixture”提升为两个显式设计任务（见 `TODO.md`：`T5001f8` / `T5001f9`），并以它们作为进入 `T5001g` 之前的最终 correctness 前置条件。

## 3. 本轮关键判断

- 本轮不是“给现有 stackmap 路线打补丁”，而是要把 managed roots 的默认 correctness 路线切到 explicit root frame。
- root map 的统一点是 slot visitor，不是 stack walking 细节。
- explicit frame 的价值不在于自动解决 register root，而在于强制建立稳定、可写回的 home slot source of truth。
- safepoint reload contract 与 aggregate rebuild contract 是这次 refactor 的核心难点；如果只完成 frame 链与 descriptor，而没有真正收紧 post-safepoint 语义，correctness 仍然不成立。
- `native_roots` 会保留，但职责会收窄；它不再是默认 explicit mode 找回更高层 managed frames 的关键机制。
- 去掉默认 stackmap 后，二进制尺寸可能获得收益，但这不是本轮的主验收；主验收仍是 correctness 与路径收口。

## 4. 主要风险与应对

### 4.1 safepoint 后旧值被 LLVM 复用

- 即使源码层面显式 reload，也要防止 LLVM 把 safepoint 前的 load / SSA 结果 CSE 到 safepoint 后。
- 本轮要把“safepoint 是 clobber 边界”的 lowering 语义写实，而不是仅靠编码习惯暗示。

### 4.2 aggregate 路径改动面大

- 含 ref 的 aggregate 复制、传参、返回、payload transport 与 state-machine replay 都可能隐含旧镜像复用。
- 因此 aggregate contract 要单独成阶段推进，避免在 frame 发射刚落地时就过早切默认模式。

### 4.3 effect / continuation / resume / state-machine 路径

- 这些路径存在多种非普通退出与恢复边界，是 push/pop、NULL discipline 与 post-safepoint reload 最容易漏掉的区域。
- 本轮必须把它们作为一等验收对象，而不是只靠 ordinary call / return 通过回归。

### 4.4 NULL discipline 回退成“安全但过度保活”

- 若 dead slot 未清零，可能短期“看起来没错”，但会掩盖 frame layout 与 liveness contract 的真实问题。
- 因此需要专门的 regression 锁定 dead/inactive slot 会回到 `NULL`。

### 4.5 `mem2reg` 误被当成本轮自然副产物

- explicit root frame 只是为重新评估 mem2reg 创造前提，不等于本轮就能安全默认放开 promotion。
- 本轮文档与 TODO 必须明确：`mem2reg` 是后续单独任务。

## 5. 预期收口状态

- runtime 对 managed roots 的统一入口已经收口为 root map / slot visitor。
- 默认 explicit mode 下，managed frames roots 全部来自 explicit frame chain，而不是 stackmap registry。
- 编译器对跨 safepoint refs、aggregate ref leaves 与内部临时 roots 都已建立 stable home slot contract。
- safepoint 后的 ref 使用路径可解释、可审计、可回归验证。
- `.llvm_stackmaps` / `__llvm_stackmaps` 不再属于默认 explicit mode 的产物。
- stackmap 仍保留，但已经退到“未来可选优化实现”而不是默认 correctness 路线。
