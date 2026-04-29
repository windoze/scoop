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

### P6. 默认 explicit mode 切换与 stackmap 退居可选优化

- 默认 explicit mode 的 managed roots 完全由 explicit root frame 支撑。
- runtime 默认 explicit mode 不再读取 stackmap registry。
- 编译器默认 explicit mode 不再生成 stackmap sections / records。
- stackmap 路径保留为未来可选模式，但必须从“默认 correctness 依赖”降级为“可审计的优化后端”。

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
