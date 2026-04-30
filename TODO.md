# TODO（Scoop：Root Map 抽象与 explicit root frame 落地）

> 生成时间：2026-04-29  
> 历史归档：`docs/archive/plans/TODO-7.md` / `docs/archive/plans/PLAN-7.md`  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 本轮主线：按 [`ROOT_FRAME_REFACTOR.md`](./ROOT_FRAME_REFACTOR.md) 把 runtime/GC 的 managed roots 统一抽象为 root map slot visitor，并将 `explicit root frame` 落地为新的默认 explicit mode；默认 explicit mode 完成切换后，不再使用、也不再生成 LLVM stackmap，stackmap 仅保留为未来可选优化模式。

## 全局约束

- [`ROOT_FRAME_REFACTOR.md`](./ROOT_FRAME_REFACTOR.md) 是本轮设计基线；若实现改变主张，必须先回写该文档，再继续实现。
- `PLAN.md` / 当前 `TODO.md` 是本轮唯一计划记录；`docs/archive/plans/*` 只作历史归档，不回写旧 round。
- 本轮先收口 correctness，再讨论优化。
  - roots 必须可见、可更新、post-safepoint 可 reload；live-set 精度、扫描成本与 mem2reg 不是先决条件。
- root map 的统一抽象必须围绕 `void** slot` visitor 建立。
  - 不允许继续把上层 GC 入口设计成 `return_address -> stackmap record` 专用接口。
- 默认 explicit mode 完成切换后：
  - managed roots 统一来自 explicit root frame；
  - runtime 不再以 LLVM stackmap 作为默认 explicit mode 的 roots 输入；
  - 编译器不再为默认 explicit mode 生成 `.llvm_stackmaps` / `__llvm_stackmaps`。
- 不做“半 stackmap、半 explicit”的同路径混搭。
  - 同一条 lowering 主线内，managed frame roots 只能有一套 source of truth。
- explicit frame 只表示 stack-backed managed root home slots。
  - 它不是“函数所有 locals 的大 struct”；
  - heap-backed traced fields、heap object 自身引用字段、effect/continuation heap frame 字段不进入 explicit frame。
- 对含 ref 的 aggregate，frame layout 必须按 ref leaf home slots 建模。
  - 非 ref 字段不进 descriptor；
  - safepoint 后若要按值复制/传参，必须先从 home slots 刷新或重组 fresh aggregate。
- safepoint 是 GC ref 的 clobber 边界。
  - safepoint 后不得继续信任 safepoint 前的 GC SSA / register 值；
  - post-safepoint 使用必须来自 home slot reload。
- NULL discipline 是 correctness 合同的一部分。
  - entry 初始化为 `NULL`；
  - dead / inactive slot 必须及时清回 `NULL`。
- `native_roots` 可以保留，但只服务 native 过渡边界；不再承担 explicit mode 找回更高层 managed frames 的职责。
- `mem2reg` / SSA promotion 不是本轮主线。
  - 只能在 explicit root frame 稳定后，作为单独优化任务按 allowlist/denylist 重新评估。
- 每个实现任务后必须紧跟 review 任务。
  - review 重点是 source-of-truth 是否收口正确、是否仍残留旧 stackmap 假设、以及是否破坏 safepoint / aggregate / state-machine 合同。
- 若任务改变公开语义或运行时合同，必须同步 `SCOOP_RUNTIME.md`、相关实现注释与必要 fixture。
- `TODO.md` 中的每一步都必须是可验证的。
  - 不接受“先做一大坨实现，最后再整体看”的不可拆分任务描述；
  - 每个步骤都要明确对应的定向验证命令、LLVM 断言、runtime test 或 fixture。
- 每个复杂逻辑都必须有相应 fixture（或最小等价回归）验证正确性。
  - 特别是 effect/state-machine、continuation resume、outer mutable local writeback、sync sidecar、GC debug helper、class init cleanup 等复杂路径，不能只靠已有大 fixture 偶然覆盖。
- 最终 review / full verification 必须在以下环境下完整执行 fixture：
  - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1`
  - 且必须完整覆盖 `run-pass`、`runtime_gc`、`build` 等相关 fixture 集合，而不是只跑最小 smoke。

## T5001：Root Map 抽象与 explicit root frame 落地

### [DONE] T5001a 建立当前 roots 主线基线，盘清 stackmap / native_roots / extra root slots / effect 路径
- 范围：
  - 盘清 runtime 当前 managed roots 枚举、moving update、verify-roots、stackmap registry、`native_roots`、pinned/global roots 的入口与依赖面。
  - 盘清编译器当前会生成 GC roots 的主要 lowering 路径，至少覆盖：
    - ordinary call / safepoint；
    - `@Extern` / `enter_native` / `leave_native`；
    - `extra_gc_root_slots` 与其它编译器内部临时 roots；
    - sret / indirect arg / return alloca；
    - effect / continuation / resume / state-machine 路径。
  - 形成一份最小但可复验的 baseline，明确哪些位置后续必须接入 explicit frame push/pop、slot init/clear 与 post-safepoint reload。
- 验收：
  - 能明确回答“runtime 当前哪些路径仍直接依赖 stackmap”“哪些 roots 已天然按 `void** slot` visitor 工作”“哪些 lowering 路径会生成必须进入 explicit frame 的 roots”。
  - 后续任务可直接引用这份 baseline，不再在实现中临时摸索入口。
- 完成记录：baseline 已固化到 `ROOT_FRAME_REFACTOR.md` 的“4.4 当前实现基线（T5001a，2026-04-29）”。
- 依赖：无

### [DONE] T5001aR Review：确认 baseline 足以支撑后续切换顺序
- 重点：
  - 是否已覆盖 runtime roots 入口、stackmap 依赖面、native 边界、effect/state-machine 特殊路径四类关键热点；
  - 是否已经能支持“先抽 root map，再上 runtime substrate，再接编译器”的顺序；
  - 是否仍遗漏会直接影响 explicit frame push/pop 或 reload contract 的结构性入口。
- 验收：
  - baseline 可作为后续 `T5001b+` 的统一前提，不需要每轮重新定位 roots 来源。
- 完成记录：已对照 `ROOT_FRAME_REFACTOR.md` 第 4.4 节与当前代码入口复核 runtime/编译器热点；确认 `T5001b` 可直接围绕 stackmap roots、`native_roots`、globals/handles/pins 抽象统一 slot visitor，而 effect/state-machine 相关 heap-backed traced fields 仍可留在现有 heap trace 合同内。
- 依赖：T5001a

### [DONE] T5001b 抽出统一 runtime root map / slot visitor 抽象
- 范围：
  - 把 runtime 当前“managed frame roots 来源”的入口从 stackmap 专用逻辑中抽离，统一到围绕 `void** slot` 的 visitor 接口。
  - 让 mark/update/verify-roots 只依赖统一 visitor，而不是分别嵌着 stackmap 细节。
  - 为 stackmap root map 与 explicit-frame root map 预留并列实现边界。
  - 保持现有行为不变，不在本任务中切换默认模式。
- 验收：
  - runtime 上层逻辑已不再直接依赖 stackmap record 解释细节；
  - 新接口已经足以承接后续 explicit frame 实现，而不是继续把 explicit mode 伪装成 stackmap 特例。
- 完成记录：新增 `runtime/c/scoop_gc_root_map_internal.h`，以 `ScoopGcManagedRootMap + scoop_gc_root_map_visit_slots(...)` 收口 managed roots 入口；`scoop_gc_backend_immix.c` 与 `scoop_gc.c` 的 verify-roots、mark、major/minor roots update 以及 stackmap smoke 测试已统一改走该抽象，stackmap 细节退回 root-map 实现内部，并预留 `SCOOP_GC_MANAGED_ROOT_MAP_EXPLICIT_FRAME` 边界。
- 依赖：T5001aR

### [DONE] T5001bR Review：确认 runtime 上层已围绕 slot visitor 收口
- 重点：
  - GC/verify-roots 是否已改为统一消费 `void** slot` visitor；
  - stackmap 细节是否已退到具体 root-map 实现内部；
  - 是否还残留“给我 return address，我给你 roots”的上层接口假设。
- 验收：
  - 后续 explicit root frame 接入时，不需要再先拆一轮 runtime 上层入口。
- 完成记录：已复核 `runtime/c/scoop_gc_root_map_internal.h` 与 `runtime/c/scoop_gc*.c`。runtime 上层的 managed roots 枚举、moving update 与 verify-roots 已统一经由 `ScoopGcManagedRootMap + scoop_gc_root_map_visit_slots(...)` 消费 `void** slot` visitor；`scoop_stackmap_registry_lookup(...)` 与 `scoop_stackmap_record_visit_root_slots(...)` 仅残留在 root-map 实现内部及专用测试中，未再作为 GC 上层接口泄漏。并已通过 `cargo test --all`、`cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`、`cargo clippy --all-targets -- -D warnings` 验证。
- 依赖：T5001b

### [DONE] T5001c1 引入 explicit root frame 的 runtime 数据结构与 TLS frame chain
- 范围：
  - 在 runtime 中引入 `ScoopRootFrameDesc`、`ScoopRootFrameHeader` 与 TLS `__scoop_explicit_root_frame_top`。
  - 建立按 `header -> desc -> offsets` 解释 explicit frame 的基础能力。
  - 明确并实现 `hdr` 必须是 frame object 首字段的 layout contract。
  - 为 `slot_count == 0` 的函数定义清晰行为：允许首阶段不构造 frame，但不得让 runtime 进入未定义状态。
- 验收：
  - runtime 已具备显式 frame descriptor + header + TLS top 的基础表示；
  - 后续编译器只需按合同发射 frame object / descriptor，即可被 runtime 扫描。
- 完成记录：新增 `runtime/c/scoop_root_frame.h`，固化 `ScoopRootFrameDesc` / `ScoopRootFrameHeader`、`__scoop_explicit_root_frame_top` TLS 符号，以及 `scoop_root_frame_visit_slots(...)` 的 `header -> desc -> offsets` 解释 helper；`runtime/c/scoop_gc_root_map_internal.h` 已补上 explicit-frame root map visitor，并明确 `slot_count == 0` frame 合法但不访问任何 slot；另通过 `scoop_test_explicit_root_frame_*` 与 `crates/scoop_runtime/tests/explicit_root_frame.rs` 锁定 TLS top 清零、zero-slot frame 与 descriptor walk 行为。
- 依赖：T5001bR

### [DONE] T5001c1R Review：确认 explicit frame substrate 边界成立
- 重点：
  - header / desc / offset 的职责边界是否清晰；
  - runtime 是否真正按 frame base + offset 恢复 `void** slot`，而不是重新依赖 SP/FP/栈布局猜测；
  - 零 roots 函数路径是否明确定义，没有留下灰色行为。
- 验收：
  - 后续编译器发射 descriptor 时不需要再调整 runtime 数据结构语义。
- 完成记录：已复核 `runtime/c/scoop_root_frame.h`、`runtime/c/scoop_gc_root_map_internal.h`、`runtime/c/scoop_runtime.c` 与 `crates/scoop_runtime/tests/explicit_root_frame.rs`。确认 `ScoopRootFrameHeader`/`ScoopRootFrameDesc` 的边界已固定为“frame base + descriptor offsets -> void** slot”，runtime 未重新依赖 SP/FP 猜测；`top == NULL` 与 `slot_count == 0` 的语义也已通过定向 smoke test 固化。另补齐 `SCOOP_RUNTIME.md` 的 explicit root frame ABI 说明，并通过 `cargo test -p scoop_runtime --test explicit_root_frame`、`cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 复核。
- 依赖：T5001c1

### [DONE] T5001c2 让 explicit mode 的 managed roots 枚举走 TLS frame chain，并收窄 `InNative` 依赖
- 范围：
  - 在 explicit mode 下，把 managed frame roots 枚举切到 TLS explicit frame chain。
  - `native_roots` 保留给 native 边界临时根；不再要求 `enter_native` 为找回更高层 caller managed frames 捕获 unwind ctx。
  - 保持 stackmap mode 仍可用，但使其退回并列实现，而不是默认显式路径前提。
- 验收：
  - explicit mode 的 managed roots 枚举不再依赖 unwind ctx + stackmap registry lookup；
  - `InNative` 协议已简化到只处理 native 边界临时 roots，而不是继续承担 managed caller frame 回溯。
- 完成记录：`runtime/c/scoop_gc.c` 与 `runtime/c/scoop_gc_backend_immix.c` 现已在 STW park 和 `enter_native` 时把 `explicit_root_frame_top` 快照进线程记录，并在 managed roots 枚举/更新/verify 时优先走 explicit-frame root map，仅在没有 explicit frame snapshot 时退回 stackmap ctx；`native_roots` 保留为 native 边界临时 roots。另新增 `scoop_test_explicit_root_frame_enter_native_smoke` 与 `crates/scoop_runtime/tests/explicit_root_frame.rs` 回归，锁定 `enter_native` 不再为 explicit-frame 路径捕获 unwind ctx，同时通过 `cargo test --all`、`cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`、`cargo clippy --all-targets -- -D warnings` 验证。
- 依赖：T5001c1R

### [DONE] T5001c2R Review：确认 explicit mode 已从 stackmap roots lookup 解耦
- 重点：
  - explicit mode 下是否还有 runtime 代码默认假定 stackmap registry 可用；
  - `native_roots` 职责是否已收窄，没有继续承载 caller managed frame 枚举；
  - stackmap mode 是否仍保留为清晰的可选实现，而不是被顺手删坏。
- 验收：
  - 后续编译器切默认 explicit mode 时，不会被 runtime roots lookup 再次阻塞。
- 完成记录：已复核 `runtime/c/scoop_gc.c`、`runtime/c/scoop_gc_backend_immix.c`、`runtime/c/scoop_gc_root_map_internal.h` 与 `crates/scoop_runtime/tests/explicit_root_frame.rs`。确认 explicit mode 下 runtime 在 STW park、`enter_native`、verify-roots、moving update 与 mark 阶段均优先消费 `explicit_root_frame_top -> explicit-frame root map`，只有在没有 explicit frame snapshot 时才退回 stackmap ctx；`native_roots` 继续仅承载 native 边界临时 roots，未再负责 caller managed frame 回溯；stackmap mode 仍通过并列 root-map 实现与现有 smoke/registry 测试保留。另修正了 `runtime/c/scoop_gc_backend_immix.c` 中两处已过期注释，并通过 `cargo test --all`、`cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`、`cargo clippy --all-targets -- -D warnings` 验证。
- 依赖：T5001c2

### [DONE] T5001d1 规划 per-function explicit frame layout，并生成 descriptor / offset table
- 范围：
  - 为每个 managed 函数规划固定 explicit frame layout。
  - descriptor 只记录 root home slots 的固定 offsets，不记录机器 SP/FP 偏移。
  - 对含 ref 的 aggregate，按 ref leaf slots 展开，而不是把整个 aggregate 直接放进 frame。
  - 明确哪些内部 roots 要进入 frame layout，至少覆盖 `extra_gc_root_slots`、hidden sret result roots、call/effect lowering 产生的临时 GC roots。
- 验收：
  - 每个 managed 函数都能得到可审计的 frame layout 与 descriptor；
  - explicit frame 的建模粒度已经与 runtime `void** slot` 语义对齐，而不是“把 locals 打包成大 struct”。
- 完成记录：编译器现已在各类 managed body lowering（top-level/HIR、raw MIR、closure、object init、effect-call wrapper、callee resume entry）开始时建立函数级 explicit-frame layout 规划，并在函数收尾时统一发射 `ScoopExplicitRootFrame$...` 结构类型、`__scoop_explicit_root_offsets__*` offset table 与 `__scoop_explicit_root_desc__*` descriptor。layout 以 GC leaf slot 为粒度：entry allocas 会按 LLVM storage type 展开 ref leaves；ordinary indirect GC aggregate params 也会额外纳入同一规划，因此 `Named(String, Int)` 这类 aggregate 只生成 `String` leaf slot，而不会把整个 aggregate 打进 descriptor。另新增三条 `crates/scoopc/src/llvm/tests.rs` 回归，分别锁定 direct ref local、indirect aggregate flattening 与 hidden-sret caller temp 三类路径，并已通过 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 验证。
- 依赖：T5001c2R

### [DONE] T5001d1R Review：确认 frame layout 语义与 leaf-slot 展开规则成立
- 重点：
  - descriptor 是否仅描述 stable home slots；
  - aggregate flattening 是否按 ref leaf slots 建模；
  - 是否仍有 heap-backed 字段、非 root 普通局部或机器栈偏移误入 descriptor。
- 验收：
  - 后续 frame 发射与 runtime 更新可直接依赖该 layout 语义，不需要补额外特判。
- 完成记录：已复核 `crates/scoopc/src/llvm/codegen/mod.rs`、`call/abi.rs`、`effect/state_machine_emitter.rs` 与 LLVM 回归。确认 descriptor 仍只由 tracked entry-home slots 构成，aggregate 继续按 GC leaf slots 展开，未把 heap-backed traced fields 或机器栈偏移塞进 descriptor；同时补上两处遗漏的 managed function 覆盖面：顶层不可变值初始化函数，以及 effect state-machine 的 `step/dispatch` 托管函数。其返回/result 临时槽位现也改走 tracked entry alloca，从而不会在后续 `T5001d2` 中漏掉 descriptor/TLS 接线。另新增 LLVM 回归锁定上述函数会发 explicit-frame descriptor，并通过 `cargo test -p scoopc --lib`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop -- test --fixtures tests/fixtures/build` 验证。
- 依赖：T5001d1

### [DONE] T5001d2 发射 activation frame object，接入 entry alloca、TLS push/pop 与 slot NULL discipline
- 范围：
  - 为 managed 函数发射 entry-block alloca 的 frame object，并把 header 放在首字段。
  - entry 时完成：slot `NULL` 初始化、`hdr.desc` / `hdr.prev` 安装、TLS push。
  - 所有返回、early return、error/raise、effect propagation、resume dispatch 等离开 activation 的路径都必须完成 TLS pop。
  - dead / inactive slot 必须显式清回 `NULL`，避免长期残留旧对象地址。
- 验收：
  - frame 地址在整个 activation 内稳定；
  - push/pop 与 NULL discipline 在所有退出路径上一致成立；
  - 没有“普通 return 正常、异常/恢复边界漏 pop”的残留路径。
- 完成记录：编译器现在会在每个 managed function 的 entry 先预留稳定的 activation frame storage，并在函数收尾把其补成 `ScoopExplicitRootFrame$...` 对象：header 固定在首字段、descriptor/offset table 与 frame slot 地址统一按 `header + pointer-size slot` 解释。entry 路径会安装 `hdr.prev` / `hdr.desc`、把所有 frame slots 初始化为 `NULL` 并 push 到 `@__scoop_explicit_root_frame_top`；所有 `ret` 终结点前都会先清空 frame slots 并恢复上一层 TLS top，因此 ordinary return、top-level/object-init 提前返回、closure、resume wrapper/entry 与 effect state-machine step/dispatch 的退出路径都统一走同一 pop 合同。现有保守 safepoint spill 也已接上 frame mirror：调用前把 live GC slot 写入 explicit frame，调用后写回 spill slot 并把 frame slot 清回 `NULL`，避免 inactive slot 长期保活。另新增 LLVM 回归锁定 TLS push/pop 与 safepoint slot clear，并通过 `cargo test --all`、`cargo clippy --all-targets -- -D warnings` 与 `cargo run -p scoop -- test --fixtures tests/fixtures/build` 验证。
- 依赖：T5001d1R

### [DONE] T5001d2R Review：确认 frame 生命周期与 NULL discipline 成立
- 重点：
  - frame 是否统一是 entry-block alloca；
  - push/pop 是否覆盖 ordinary return、effect propagation、continuation resume、state-machine runtime function 等全部边界；
  - dead/inactive slots 是否真正清零，而不是只在入口初始化一次。
- 验收：
  - 后续 post-safepoint reload 与 moving update 可以把 frame fields 当成唯一可信 source of truth。
- 完成记录：已复核 `crates/scoopc/src/llvm/codegen/mod.rs`、`gc.rs`、`call/resume.rs`、`effect/state_machine_emitter.rs` 与 LLVM 回归，确认 managed body 的 explicit frame storage 统一经 `begin_function_explicit_frame_layout(...)` 在 entry-block 预留，top-level/object-init/closure/callee-resume/effect state-machine/dispatch 等入口都在函数收尾统一执行 `finish_function_explicit_frame_layout(...)`。review 期间发现并修复了一处现存缺口：此前 teardown 仅插入到 `ret` 终结点，导致 `CgTy::Never` 共享返回块发射 `unreachable` 时会漏掉 frame slot 清零与 TLS pop；现已改为同时覆盖 `ret` / `unreachable` 终结点，并新增 LLVM 回归 `never_returning_managed_function_pops_explicit_root_frame_before_unreachable`。另复核 safepoint spill mirror 路径仍会在调用后把 explicit frame mirror slot 清回 `NULL`，普通函数退出也会在 teardown 中统一清槽。已通过 `cargo test -p scoopc --lib`、`cargo run -p scoop -- test --fixtures tests/fixtures/build` 与 `cargo clippy --all-targets -- -D warnings` 验证。
- 依赖：T5001d2

### [DONE] T5001d3 把源码级 roots 与编译器内部临时 roots 全部收敛到 frame home slots
- 范围：
  - 把跨 safepoint 的源码级 refs、aggregate ref leaves、内部临时 roots、hidden sret / indirect arg / return scratch roots 统一映射到 frame fields。
  - 清理仍依赖“运行时动态注册某个 root slot id”或“事后让 LLVM spill 出可更新位置”的旧路径。
  - 让 effect / state-machine / continuation lowering 产生的临时 GC roots 同样走固定 home slot 合同。
- 验收：
  - 任何跨 safepoint 存活的 GC ref 都能指出对应的 stable home slot；
  - runtime moving update 后，不再存在必须依赖旧 SSA / register / 临时数组镜像的 source-of-truth。
- 完成记录：编译器现已把 explicit frame 从“按需 safepoint mirror”收口为稳定 home-slot 合同。`store_local_value_exact(...)` 会在 stack-backed store 后同步写入 explicit frame leaf home slots；ordinary indirect GC aggregate params 现会在绑定时建立 incoming slot -> frame slot 持久映射并把 ref leaves 预热到 frame；deferred spill / call-arg spill 等内部临时 roots 不再走 `extra_gc_root_slots` + root-slot-id 动态注册，而是直接跟踪固定 spill/home-slot 并在消费后清回 `NULL`。ordinary safepoint 的 keepalive 源也已切到 explicit frame home slots，调用后同时写回原 spill/local 与 frame home slot；hidden sret 结果则在 call 返回后立即同步进 frame，并在 load 后清理其 home slot。另新增 LLVM 回归锁定 indirect aggregate param 会在 safepoint 前写入 explicit frame home slot，并更新既有 safepoint/statepoint 断言以匹配“frame home slot 持续为 source of truth、仅在 activation teardown 时清零”的新合同。已通过 `cargo test --all`、`cargo clippy --all-targets -- -D warnings` 与 `cargo run -p scoop -- test --fixtures tests/fixtures/build` 验证。
- 依赖：T5001d2R

### [DONE] T5001d3R Review：确认“所有跨 safepoint roots 都有 stable home slot”
- 重点：
  - 是否仍残留某些 temporaries / hidden roots 未进入 frame；
  - `extra_gc_root_slots` 等旧机制是否只是换名保留，而未真正收口到 frame fields；
  - effect / continuation / state-machine 临时 roots 是否与 ordinary 路径一致遵守同一合同。
- 验收：
  - 后续 safepoint reload contract 可以默认“home slot 是唯一 source of truth”。
- 完成记录：已复核 `crates/scoopc/src/llvm/codegen/mod.rs`、`gc.rs`、`mir_body.rs`、`call/abi.rs`、`call/dispatch.rs`、`effect/mod.rs` 与 `effect/state_machine_emitter.rs`。确认 `extra_gc_root_slots` / root-slot-id 动态注册路径已从当前 lowering 中移除，ordinary locals、indirect aggregate params、hidden sret、deferred spill / call-arg spill 以及 effect/state-machine/continuation lowering 产生的 stack-backed temporaries 都已统一映射到 explicit frame home slots。review 期间发现一处真实缺口：共享 `Continuation.resume` runtime helper `resume_continuation_with_encoded_payload(...)` 之前直接发射 `scoop_continuation_resume_with` 调用，没有走 `build_call_preserving_gc_local_roots(...)`，导致 replay / tail-resume / ordinary resume 路径未完全遵守与 ordinary safepoint 一致的 keepalive + home-slot write-back 合同；现已修复为统一经该 helper 包装，并新增 LLVM 回归锁定调用窗口必须出现 GC keepalive、explicit frame home-slot 参与以及调用后 write-back 痕迹。另已通过 `cargo test -p scoopc state_machine_multi_payload_perform_uses_tuple_transport`、`cargo test -p scoopc when_arm_try_resume_nested_handle_ir_keeps_binder_scope_for_inner_resume`、`cargo test -p scoopc --lib` 与 `cargo clippy -p scoopc --all-targets -- -D warnings` 验证。
- 依赖：T5001d3

### [DONE] T5001e1 收紧 safepoint clobber / reload contract，打通 post-safepoint ref 使用主线
- 范围：
  - 明确所有 safepoint 都是 GC ref clobber 边界。
  - post-safepoint 的 ref 使用必须从 explicit frame home slot reload。
  - 清理仍沿用 safepoint 前 SSA / register 值的路径，至少覆盖 ordinary call、runtime helper call、effect boundary 与 resume 后继续执行路径。
  - 必要时补充更强的 lowering / memory-clobber 语义，避免 LLVM 把 safepoint 前的 load/CSE 结果错误复用到 safepoint 后。
- 验收：
  - post-safepoint ref 使用路径可审计地来自 home slot reload；
  - 不再存在“GC 已更新 slot，但后续仍从旧 SSA 值继续运行”的 correctness 缺口。
- 完成记录：已为单槽 pointer-shaped GC 值补上统一的 explicit-frame reload helper，并让 `local_ptr_for_use(...)`、`materialize_deferred_cg_value(...)` 与 call-arg materialize 路径在 explicit frame 已启用时优先从 home slot reload，而不再从原 local/spill slot 取回 post-safepoint 值；这覆盖了 ordinary local 读取、runtime helper / ordinary call 之后的 deferred scalar GC 值消费，以及由 effect/resume lowering 复用 `local_ptr_for_use(...)` 的 direct ref 路径。另新增两条 LLVM 回归，分别锁定 direct local safepoint 后 reload 与“先求值 call arg 遇到后续 safepoint”两类窗口，并通过 `cargo test -p scoopc --lib`、`cargo run -p scoop -- test --fixtures tests/fixtures/build`、`cargo clippy -p scoopc --all-targets -- -D warnings` 验证。
- 依赖：T5001d3R

### [DONE] T5001e1R Review：确认 safepoint 已成为真实的 clobber 边界
- 重点：
  - 是否还残留 post-safepoint 直接复用旧 SSA / register 的路径；
  - LLVM 是否仍可能把 safepoint 前值 CSE 到 safepoint 后；
  - ordinary call、effect boundary、resume replay 等路径的 reload 语义是否一致。
- 验收：
  - safepoint clobber / reload contract 已能作为 aggregate refresh 之前的稳定前提。
- 完成记录：已复核 `crates/scoopc/src/llvm/codegen/mod.rs`、`call/abi.rs`、`mir_body.rs`、`effect/mod.rs`、`effect/state_machine_emitter.rs` 与 LLVM 回归。确认 HIR/direct-ref 路径、deferred spill/call-arg 路径，以及 effect/resume/state-machine 中复用 `local_ptr_for_use(...)` / `materialize_deferred_cg_value(...)` 的读取点，已统一把 explicit frame home slot 作为 post-safepoint reload source-of-truth。review 期间发现一处真实缺口：production MIR bridge 的 `load_mir_local(...)` 仍直接从 `slot.ptr` 读取，导致 raw/materialized MIR body 在 safepoint 后可能继续复用旧 local 槽位；现已修复为同样经 `local_ptr_for_use(...)` 走 reload helper，并新增 LLVM 回归 `production_mir_function_reloads_direct_gc_local_from_explicit_frame_after_safepoint` 锁定 ordinary managed call 之后的 MIR local reload 行为。另已通过 `cargo test -p scoopc --lib`、`cargo run -p scoop -- test --fixtures tests/fixtures/build`、`cargo clippy -p scoopc --all-targets -- -D warnings` 验证。
- 依赖：T5001e1

### [DONE] T5001e2 补齐 aggregate refresh / rebuild contract，覆盖 args、returns、payload transport
- 范围：
  - 对含 ref 的 aggregate，建立“reload 最新 ref 字段 + 复用非 ref 字段 + 重组 fresh aggregate”的 lowering contract。
  - 覆盖 direct arg、indirect arg、sret result、return alloca、effect payload、continuation payload、state-machine transport 等路径。
  - 杜绝盲目复制 pre-safepoint 旧 aggregate 副本或用旧镜像 `memcpy` 继续传播。
- 验收：
  - 含 ref 的 aggregate 在 safepoint 后继续复制、传值、返回、transport 时，不再依赖 stale 副本；
  - 相关 ABI 路径与 runtime payload 路径都能统一解释为基于最新 home slots 刷新后的值。
- 完成记录：LLVM codegen 新增基于 explicit-frame leaf home slots 的 aggregate rebuild helper：对带 GC leaf 的 storage slot，会按“ref leaf 从 frame home slot reload、非 ref leaf 从原 storage slot 读取”重建 fresh aggregate，并在需要地址的场景落到临时 rebuild alloca。`local_ptr_for_use(...)`、deferred call-arg materialize、indirect aggregate call-arg pointer materialize 与 hidden-sret result load 已统一改走该 contract，因此 direct/indirect args、hidden sret returns、effect boxed payload transport 以及复用这些入口的 continuation/state-machine transport 不再整体复用 stale local/spill/sret 镜像。另新增三条 LLVM 回归，分别锁定 aggregate call arg、hidden-sret aggregate result 与 boxed effect payload 都会从 explicit-frame home slots 重建 fresh aggregate，并已通过 `cargo test -p scoopc --lib`、`cargo run -p scoop -- test --fixtures tests/fixtures/build`、`cargo test --all` 与 `cargo clippy -p scoopc --all-targets -- -D warnings` 验证。
- 依赖：T5001e1R

### [DONE] T5001e2R Review：确认 aggregate 不再持有 post-safepoint 的旧 source-of-truth
- 重点：
  - 是否仍有 aggregate copy / arg / return 路径直接复用旧镜像；
  - 非 ref 字段与 ref 字段的来源是否清晰分离；
  - effect / continuation / state-machine payload 是否也遵守同一 refresh/rebuild 合同。
- 验收：
  - 切默认 explicit mode 前，aggregate 相关 correctness 缺口已被系统性封住。
- 完成记录：已复核 `crates/scoopc/src/llvm/codegen/mod.rs`、`call/abi.rs`、`call/resume.rs`、`effect/mod.rs`、`effect/state_machine_emitter.rs` 与 LLVM 回归。确认 aggregate 读取主线已统一经 `storage_slot_for_use(...)` / rebuild helper 收口：direct/indirect call args、hidden-sret aggregate result、effect boxed payload，以及复用 `encode_effect_transport_value(...)` 的 continuation / state-machine payload transport，都会按“GC ref leaf 从 explicit-frame home slot reload、非 ref leaf 从原 storage 或 heap field 读取”的合同重建 fresh aggregate；未再发现直接传播 post-safepoint stale aggregate 镜像的现存路径。另通过 `cargo test -p scoopc --lib`、`cargo run -p scoop -- test --fixtures tests/fixtures/build` 与 `cargo clippy -p scoopc --all-targets -- -D warnings` 复核。
- 依赖：T5001e2

### [DONE] T5001f 切换默认 explicit mode 到 explicit root frame，并停止默认路径的 stackmap 生成与使用
- 范围：
  - 默认 explicit mode 的 managed roots 完全切到 explicit root frame。
  - runtime 默认 explicit mode 不再读取 stackmap registry；编译器默认 explicit mode 不再生成 stackmap sections / records。
  - stackmap mode 保留为未来可选实现，但不能再成为默认 explicit mode 的 correctness 前提。
  - 补充定向 build/fixture 断言，锁定默认 explicit mode 产物中不再出现 `.llvm_stackmaps` / `__llvm_stackmaps`。
- 验收：
  - 默认 explicit mode 已可独立运行并通过回归，不依赖 stackmap；
  - 默认 explicit mode 的产物已不再生成 stackmap section。
- 完成记录：默认 explicit mode 现已全面切到 explicit root frame。编译器侧已移除托管函数、closure/object-init/raw-MIR/effect state-machine/callee-resume 以及 synthetic `main` 的 `gc "statepoint-example"` 标记，默认 lowering 不再生成 statepoint/stackmap intrinsics；同时让 synthetic `main` 也走 explicit frame layout，并修复其 frame storage alloca 必须固定插在 entry alloca 区的 dominance 缺口。runtime 侧 `scoop_runtime_init()` 不再默认注册当前进程 stackmap registry，GC 在 `InNative` 线程上也允许“只有 native_roots、没有 managed frame root map”的默认 explicit-mode 场景，不再把 stackmap ctx 视为必备前提。测试侧删除了默认矩阵里仅服务 stackmap 默认路径的 dump/registry fixture，新增 `tests/fixtures/build/explicit_root_frame_default_mode_no_stackmaps.scoop`，并把 LLVM/object/runtime 断言统一改为锁定“默认产物无 `.llvm_stackmaps` / `__llvm_stackmaps`、无 `gc.statepoint`、stackmap registry 仅在手动注册时可用”的合同；另同步更新了 `extern_enter_native_no_statepoint_writeback` 与 `thread_join` 相关断言，使其匹配 explicit-frame home-slot source-of-truth。已通过 `cargo test -p scoopc minimal_main_obj_omits_stackmap_section_by_default`、`cargo test -p scoopc minimal_main_obj_with_live_gc_roots_still_omits_stackmap_section`、`cargo test -p scoopc default_explicit_mode_omits_statepoint_intrinsics_and_gc_strategy`、`cargo test -p scoopc thread_join_preserves_live_gc_locals_via_explicit_root_frame`、`cargo test -p scoopc effect_runtime_functions_use_explicit_root_frame_without_statepoints`、`cargo test -p scoop_runtime`、`cargo run -p scoop -- test --fixtures tests/fixtures/build` 与 `cargo clippy --all-targets -- -D warnings` 验证。
- 依赖：T5001e2R

### [DONE] T5001fR Review：确认默认 correctness 路线已真正切到 explicit root frame
- 重点：
  - 默认 explicit mode 下是否还有隐含 stackmap 依赖；
  - stackmap 是否已退到可选优化实现，而不是默认路径必需物；
  - build/fixture 断言是否真正锁住“不再生成 stackmap section”的合同。
- 验收：
  - 可以明确声称：默认 explicit mode 的 source of truth 已是 explicit root frame，而非 stackmap。
- 完成记录：已复核 `runtime/c/scoop_runtime.c`、`runtime/c/scoop_gc*.c`、`crates/scoopc/src/llvm/mod.rs`、`crates/scoopc/src/llvm/codegen/gc.rs`、`runtime_abi.rs`、`sysroot/core.scoop` 与现有 LLVM/object/registry 回归，确认默认 `scoop_runtime_init()` 不再自动注册 stackmap registry，GC 在默认 explicit mode 下会优先以 explicit root frame 作为 managed roots source-of-truth，而默认 LLVM 产物继续锁定“无 `gc "statepoint-example"`、无 `llvm.experimental.gc.statepoint`、无 stackmap section”。review 期间发现并修复一处真实回归：保留中的显式 stackmap smoke helper `__scoop_stackmap_statepoint_smoke()` 在默认移除 GC strategy 后已无法再为调用点产出真实 record；现已改为仅对显式调用该 helper 的函数恢复 statepoint GC strategy，从而把 stackmap 保持为按需 opt-in 的可选实现边界，而不重新污染默认 correctness 路线。另新增两条 LLVM 回归，分别锁定该 helper 会重新进入 statepoint pipeline、并可按需产出 stackmap section；已通过 `cargo test -p scoopc --lib`、`cargo run -p scoop -- test --fixtures tests/fixtures/build` 与 `cargo clippy -p scoopc --all-targets -- -D warnings` 验证。
- 依赖：T5001f

### [DONE] T5001f1 修复 async/task waiting path 的 transport-await regression
- 范围：
  - 修复 `await` internal handler 与 `Task.step()` waiting path 在 run-pass/runtime 下的真实回归：当前最小复现中，外层 task 首次 `step()` 返回 `Pending` 后，再次 drive 会卡住，`async_await_minimal_int_basic.scoop`、`async_await_string_basic.scoop`、`async_fun_task_runtime_basic.scoop` 与 `task_step_manual_basic.scoop` 均受影响。
  - 精确查清并修复 `Async.await` 降低出来的 awaited-task transport 合同，确保 waiting task 驱动的 source-of-truth 与 `Continuation<(Int, Any), __TaskStepResult<T>>` resume payload contract 一致，不能继续依赖当前会把 post-await drive 卡死的错误路径。
  - 为最小 await/task-step 路径补定向回归，至少覆盖：
    - `async { 41 }` 被另一 task `await` 后可完成；
    - `Task.step()` 在首个 `Pending` 后再次 drive 能返回 `Ready`；
    - `handled Async.await(...) -> __task_join(...)` 主线恢复通过。
- 验收：
  - `task_step_manual_basic.scoop` 恢复输出 `step1=ready` / `step2=ready`；
  - `async_await_minimal_int_basic.scoop`、`async_await_string_basic.scoop`、`async_fun_task_runtime_basic.scoop` 重新稳定通过；
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 不再在这些 async/task fixtures 上卡住或失败。
- 完成记录：`crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 的 await waiting-path 现已在把 escaped continuation 从 effect frame 取出后，统一经 `store_local_value_exact(...)` 写回 continuation local，并同步刷新 explicit-frame home slot，再调用 `__task_step_pending(...)`；从而 `Task.step()` 的 Waiting transport 不再把 `null` continuation 记进 waiting state，post-await drive 能正确恢复外层 continuation。另新增 LLVM 回归 `async_task_pending_path_stores_escape_continuation_before_waiting_helper` 锁定 `load_continuation -> local store -> explicit-frame store -> __task_step_pending(...)` 序列，并已通过 `cargo test -p scoopc async_task_pending_path_stores_escape_continuation_before_waiting_helper -- --nocapture`、`cargo run -p scoop -- run tests/fixtures/run-pass/task_step_manual_basic.scoop`、`cargo run -p scoop -- run tests/fixtures/run-pass/async_await_minimal_int_basic.scoop`、`cargo run -p scoop -- run tests/fixtures/run-pass/async_await_string_basic.scoop`、`cargo run -p scoop -- run tests/fixtures/run-pass/async_fun_task_runtime_basic.scoop`、`cargo test -p scoopc --lib` 与 `cargo clippy -p scoopc --all-targets -- -D warnings` 验证。
- 依赖：T5001fR

### [DONE] T5001f1R Review：确认 await/task waiting transport 合同重新闭合
- 重点：
  - awaited task drive 是否真正与 `(Int, Any)` transport / continuation resume contract 对齐；
  - 是否还残留“首次 `Pending` 成功、后续 drive 卡死”或 resume 重放 await site 的路径；
  - run-pass 回归是否已覆盖最小 await、manual `Task.step()` 与 handled `Async.await` 三类主线。
- 验收：
  - `T5001g` 可在不再被 async/task runtime regression 阻塞的前提下继续做全量验收。
- 完成记录：已复核 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 与相关 LLVM/fixture 回归。确认 await waiting path 在从 effect frame 取出 escaped continuation 后，会先经 `store_local_value_exact(...)` 写回 continuation local 与 explicit-frame home slot，再调用 `__task_step_pending(...)`；因此 awaited task 的 waiting transport 已重新与 `(Int, Any)` / continuation resume payload 合同对齐，不会再把 `null` continuation 记进 waiting state。另已通过 `cargo test -p scoopc async_task_pending_path_stores_escape_continuation_before_waiting_helper -- --nocapture`、`cargo test -p scoopc async_task_resume_ir_does_not_replay_original_await_site -- --nocapture`、`cargo test -p scoopc single_file_minimal_ir_supports_handled_async_await -- --nocapture`、`cargo run -p scoop -- run tests/fixtures/run-pass/task_step_manual_basic.scoop`、`cargo run -p scoop -- run tests/fixtures/run-pass/async_await_minimal_int_basic.scoop`、`cargo run -p scoop -- run tests/fixtures/run-pass/async_await_string_basic.scoop`、`cargo run -p scoop -- run tests/fixtures/run-pass/async_fun_task_runtime_basic.scoop` 与 `cargo clippy -p scoopc --all-targets -- -D warnings` 验证。review 期间确认 run-pass 已越过原 async/task waiting regression；当前剩余阻塞已切换为独立的 `class_init_order_primary_secondary_basic.scoop` 类初始化顺序问题，因此在 `T5001g` 前新增 `T5001f2/T5001f2R`。
- 依赖：T5001f1

### [DONE] T5001f2 修复 class init order regression，解除 run-pass 全量验收阻塞
- 范围：
  - 修复 `tests/fixtures/run-pass/class_init_order_primary_secondary_basic.scoop` 当前回归：程序目前仅输出 `start` / `Primary.a` 后即提前异常退出，未继续执行 `println(this.x)`、后续 primary init steps 与 secondary ctor body。
  - 查清 primary ctor 参数属性、property initializer、`init {}` 与 secondary ctor body 的 lowering / runtime 执行顺序及 `this` 可见性合同，按 Appendix B.2.2 恢复实现。
  - 为 primary/secondary ctor 初始化顺序补最小定向回归，至少覆盖 primary 参数属性在 property initializer / init block 中可见，以及 secondary ctor body 晚于 primary init steps。
- 验收：
  - `class_init_order_primary_secondary_basic.scoop` 恢复输出预期 14 行；
  - `cargo run -p scoop -- test` 不再在该 fixture 处失败。
- 完成记录：`crates/scoopc/src/llvm/codegen/class_ctor.rs` 现已把 ctor-inline `this` local 的初始化从裸 `store` 收口为 `store_local_value_exact(...)`，确保 `create_entry_alloca(...)` 为 `this` 预留的 explicit-frame home slot 会在进入 property initializer / init block 前同步写入。这样当 `Primary.a` 内的 `println("Primary.a")` 先触发 safepoint 后，后续 `this.x` 读取会按既有 post-safepoint contract 从 explicit frame reload，而不会再从未同步的 `this` 局部槽位取到空值并在运行期段错误。另新增 LLVM 回归 `class_ctor_this_local_reloads_from_explicit_frame_after_safepoint`，锁定 ctor-inline `this` 在 safepoint 后必须走 explicit frame home slot reload；并已通过 `cargo test -p scoopc class_ctor_this_local_reloads_from_explicit_frame_after_safepoint -- --nocapture` 与 `cargo run -p scoop -- run tests/fixtures/run-pass/class_init_order_primary_secondary_basic.scoop` 验证。继续执行 `cargo run -p scoop -- test` 时，suite 已越过该 fixture，当前新的既有阻塞已切换为 `effect_escape_continuation_gc_stress_multi_string.scoop`，因此按要求在本条后插入 `T5001f3/T5001f3R`。
- 依赖：T5001f1R

### [DONE] T5001f3 修复 effect escape continuation GC-stress golden regression，解除 run-pass 后续阻塞
- 范围：
  - 修复 `tests/fixtures/run-pass/effect_escape_continuation_gc_stress_multi_string.scoop` 当前回归：在 `T5001f2` 验证过程中，`cargo run -p scoop -- test` 已越过 class-init fixture，但在该 effect/continuation GC-stress fixture 处出现 stdout 与 golden 不一致。
  - 查清 escaped continuation、effect transport、GC-stress 驱动与 golden 预期之间的真实 source-of-truth 偏差，不能通过放宽 fixture 或修改 golden 来回避实现问题。
  - 为最小 effect escape continuation GC-stress 路径补定向回归，至少覆盖 multi-string payload 在 escape/resume 后的输出顺序与值保持正确。
- 验收：
  - `effect_escape_continuation_gc_stress_multi_string.scoop` 恢复与 golden 一致；
  - `cargo run -p scoop -- test` 不再在该 fixture 处失败。
- 完成记录：`crates/scoopc/src/llvm/codegen/effect/mod.rs` 现已把 fresh `Continuation.resume(...)` receiver 收口为与普通 GC-sensitive call arg 一致的 contract：receiver 先经 `defer_gc_sensitive_cg_value(...)` spill 成 tracked root，再在 payload materialize 完成后通过 `continuation_resume_receiver_reload` reload，最后才调用 `scoop_continuation_resume_with(...)`。这样 `k1.resume("alpha")` 这类“先读 continuation、后分配 String payload”的路径在 `SCOOP_GC_STRESS=1` 下不再把 stale continuation 指针传进 runtime，也不会错误触发 `ContinuationAlreadyResumed`。另新增 LLVM 回归 `continuation_resume_reloads_receiver_after_gc_sensitive_payload_materialization`，锁定 payload 分配后必须 reload continuation receiver 的 IR 顺序；并已通过 `cargo test -p scoopc continuation_resume_reloads_receiver_after_gc_sensitive_payload_materialization -- --nocapture`、`cargo test -p scoopc when_arm_try_resume_nested_handle_ir_keeps_binder_scope_for_inner_resume -- --nocapture`、`SCOOP_GC_STRESS=1 cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_gc_stress_multi_string.scoop` 与 `cargo clippy -p scoopc --all-targets -- -D warnings` 验证。继续执行 `cargo run -p scoop -- test` 时，suite 已越过该 fixture，当前新的既有阻塞已切换为 `gc_cross_function_class_object_graph.scoop`，因此按要求在本条后插入 `T5001f4/T5001f4R`。
- 依赖：T5001f2

### [DONE] T5001f4 修复 cross-function class object graph GC-stress regression，解除 run-pass 后续阻塞
- 范围：
  - 修复 `tests/fixtures/run-pass/gc_cross_function_class_object_graph.scoop` 当前回归：`cargo run -p scoop -- test` 已越过 `effect_escape_continuation_gc_stress_multi_string.scoop`，但在该 fixture 处失败；默认环境单跑可通过，而 suite 使用的 `SCOOP_GC_STRESS=1` 下会卡住/异常退出。
  - 查清跨函数传递的 `Node`/`String`/`Node?` object graph 在 GC-stress 下的真实 source-of-truth 缺口，重点覆盖 factory return、setter 调用、对象字段写入与 transitive readback 主线，不能通过放宽 fixture 或修改 golden 来回避实现问题。
  - 为最小 cross-function class object graph 路径补定向回归，至少覆盖 class->class、class->String 引用跨 `makeNode` / `setLeft` / `setRight` / `printNode` 边界后在 `SCOOP_GC_STRESS=1` 下仍保持正确。
- 验收：
  - `gc_cross_function_class_object_graph.scoop` 恢复通过；
  - `cargo run -p scoop -- test` 不再在该 fixture 处失败。
- 完成记录：`crates/scoopc/src/llvm/codegen/class_ctor.rs` 现已把 freshly allocated class object 本身也收口为 tracked GC root：class ctor call 在 `scoop_alloc_typed(...)` 返回后，会先经 `defer_gc_sensitive_cg_value(...)` 把新对象 spill 到 `class_ctor_obj_root`，随后在 ctor 实参求值前、进入 ctor init 前以及 factory return 前都通过 explicit-frame-backed reload 重新取回当前对象指针，而不再继续依赖 pre-GC 的 `%rt_alloc_class` SSA。这样 `makeNode("root", 1)` 这类“先分配 class object、再分配 `String` ctor arg / 执行 ctor init”的路径在 `SCOOP_GC_STRESS=1` 下不会再把 stale object 指针传进字段初始化、setter/readback 或最终 return，`gc_cross_function_class_object_graph.scoop` 的首个 `__scoop_gc_collect()` 也不再卡住。另新增 LLVM 回归 `class_ctor_factory_keeps_allocated_object_rooted_across_gc_sensitive_arg_eval`，锁定 factory ctor path 会把 `rt_alloc_class` 先落到 tracked root，并在 GC-sensitive ctor arg eval/ctor return 之前从 explicit frame reload；并已通过 `cargo test -p scoopc class_ctor_factory_keeps_allocated_object_rooted_across_gc_sensitive_arg_eval -- --nocapture`、`cargo test -p scoopc class_ctor_this_local_reloads_from_explicit_frame_after_safepoint -- --nocapture`、`SCOOP_GC_STRESS=1 cargo run -p scoop -- run tests/fixtures/run-pass/gc_cross_function_class_object_graph.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/gc_cross_function_class_object_graph.scoop` 与 `cargo clippy -p scoop -p scoopc --all-targets -- -D warnings` 验证。继续执行 `cargo run -p scoop -- test` 时，suite 已越过该 fixture，当前新的既有阻塞已切换为 `higher_order_aggregate_return_struct_mapper.scoop`，因此按要求在本条后插入 `T5001f5/T5001f5R`。
- 依赖：T5001f3

### [DONE] T5001f5 修复 higher-order aggregate return struct mapper 的 GC-stress regression，解除 run-pass 后续阻塞
- 范围：
  - 修复 `tests/fixtures/run-pass/higher_order_aggregate_return_struct_mapper.scoop` 当前回归：`cargo run -p scoop -- test` 已越过 `gc_cross_function_class_object_graph.scoop`，但在该 fixture 处失败；`SCOOP_GC_STRESS=1` 下 direct higher-order 调用 `mapper("go")` 目前会错误输出 `!!` / `2`，而不是 golden 中的 `go!` / `3`。
  - 查清带 `String` 字段的 `Labelled` struct 经由 higher-order 间接调用返回后，在后续分配、再次 higher-order 调用与最终 direct readback 链路中的真实 source-of-truth 缺口，重点覆盖 aggregate return、GC ref field refresh/rebuild 与 higher-order call-result materialize 主线，不能通过放宽 fixture 或修改 golden 来回避实现问题。
  - 为最小 higher-order aggregate return 路径补定向回归，至少覆盖 direct `mapper(...)` 返回值与 `runMapper(...)` 内的二次 higher-order 调用在 `SCOOP_GC_STRESS=1` 下都能保持 `Labelled.text` / `Labelled.score` 正确。
- 验收：
  - `higher_order_aggregate_return_struct_mapper.scoop` 恢复通过；
  - `cargo run -p scoop -- test` 不再在该 fixture 处失败。
- 完成记录：`crates/scoopc/src/llvm/codegen/intrinsics/builtin.rs` 现已把 builtin `String` method receiver 也纳入现有 GC-sensitive defer/reload 合同：在 `codegen_string_method(...)` 中，receiver 会先经 `defer_gc_sensitive_cg_value(...)` spill 成 tracked root；对 `concat`、`replace`、`compareTo`、`repeat`、`charAt`、`getByte`、`unsafeSliceBytes` 以及同一 lowering 中其它会在后续参数求值后才消费 receiver 的路径，均在真正使用 receiver 前从 explicit-frame-backed home slot materialize/reload，而不再继续依赖参数求值前的旧 SSA。这样 higher-order closure 中的 `input.concat("!")` 即使先分配 `"!"` 并触发 moving GC，runtime `scoop_string_concat(...)` 也会吃到 relocate 后的 receiver，`mapper("go")` 返回的 `Labelled.text/score` 不会再在 direct higher-order aggregate return 起点就损坏。另新增 LLVM 回归 `higher_order_aggregate_return_reloads_string_receiver_after_gc_sensitive_arg_eval`，锁定 closure-lowered `String.concat` 会在 `"!"` 分配后从 explicit frame reload receiver，再进入 `scoop_string_concat(...)`；并已通过 `cargo test -p scoopc higher_order_aggregate_return_reloads_string_receiver_after_gc_sensitive_arg_eval -- --nocapture`、`env SCOOP_GC_STRESS=1 cargo run -p scoop -- run tests/fixtures/run-pass/higher_order_aggregate_return_struct_mapper.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/higher_order_aggregate_return_struct_mapper.scoop` 与 `cargo clippy -p scoop -p scoopc --all-targets -- -D warnings` 验证。继续执行 `cargo run -p scoop -- test` 时，suite 已越过该 fixture，当前新的既有阻塞已切换为 `tests/fixtures/runtime_gc/task_step_cross_thread_sequential_handoff_gc_stress.scoop`，因此按要求在本条后插入 `T5001f6/T5001f6R`。
- 依赖：T5001f4

### [DONE] T5001f6 修复 cross-thread sequential task handoff 的 runtime GC-stress regression，解除后续验收阻塞
- 范围：
  - 修复 `tests/fixtures/runtime_gc/task_step_cross_thread_sequential_handoff_gc_stress.scoop` 当前回归：`cargo run -p scoop -- test` 已越过 `higher_order_aggregate_return_struct_mapper.scoop`，但在该 runtime_gc fixture 处失败；直接运行当前程序只输出到 `inner-ready / inner / 41`，缺失 golden 里的 `outer-after-await`、`worker-ready` 与 `main-final-ready` 后续 handoff/readback。
  - 查清 `Task.step()` 顺序跨线程 handoff 在 moving GC + verify-roots + stress 模式下的真实 source-of-truth 缺口，重点覆盖 worker thread 接手 `outer.step()`、awaited payload 从 inner -> outer 的 transport、completed cache 以及 `worker.join()` 后的最终 main-thread readback，不能通过放宽 fixture 或修改 golden 来回避实现问题。
  - 为最小顺序跨线程 handoff 路径补定向回归，至少覆盖首次 `Pending` 后由另一线程继续 drive 并返回 `Ready`，以及主线程之后再次 `outer.step()` 仍能稳定读回相同完成值。
- 验收：
  - `task_step_cross_thread_sequential_handoff_gc_stress.scoop` 恢复通过；
  - `cargo run -p scoop -- test` 不再在该 fixture 处失败。
- 完成记录：本轮先从当前未提交变更与 fresh LLVM IR 复核 root source-of-truth，确认 `Task.step()` 顺序 handoff 的 runtime GC-stress 回归并非仅限 cross-thread，而是暴露出一组更普遍的 post-safepoint stale SSA 路径。`crates/scoopc/src/llvm/codegen/mod.rs` 现已抽出统一的 GC-sensitive fresh-ref defer/reload helper；`effect/state_machine_emitter.rs` 把 fresh effect frame 自身先落到 tracked root，再在 seed / entry-state / dispatch / result-read 各窗口从 explicit-frame-backed home slot reload；`intrinsics/containers.rs` 为 array builder receiver 补上 defer/reload；`call/dispatch.rs` 为 function-value call、vtable 与 itable receiver 补上 legacy boundary / arg-eval 后的 reload；`closure/mod.rs`、`mir_body.rs` 与 `effect/mod.rs` 也分别为 closure object、MIR closure/capture box、effect transport box 接上同一 contract，并新增 LLVM 回归锁定 `TaskStep` return slot 初始化、closure call receiver reload、virtual/interface receiver reload、`Continuation.resume` boxed payload reload 等顺序。定向验证已通过 `cargo test -p scoopc async_task_effect_return_slots_start_null_before_resume_writes -- --nocapture`、`cargo test -p scoopc array_of_string_uses_ref_element_runtime_apis_without_ptr_to_u64 -- --nocapture`、`cargo test -p scoopc closure_call_with_real_outward_effect_uses_explicit_outcome_boundary -- --nocapture`、`cargo test -p scoopc virtual_call_with_real_outward_effect_uses_explicit_outcome_boundary -- --nocapture`、`cargo test -p scoopc interface_call_with_real_outward_effect_uses_explicit_outcome_boundary -- --nocapture`、`cargo test -p scoopc continuation_resume_boxed_payload_reloads_box_object_before_runtime_call -- --nocapture`、`cargo test -p scoopc continuation_resume_reloads_receiver_after_gc_sensitive_payload_materialization -- --nocapture`、`env SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- run tests/fixtures/runtime_gc/task_step_manual_gc_aggregate_transport_basic.scoop`、`env SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- run tests/fixtures/runtime_gc/task_step_cross_thread_sequential_handoff_gc_stress.scoop`，以及两条 fixture harness：`cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/task_step_manual_gc_aggregate_transport_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/task_step_cross_thread_sequential_handoff_gc_stress.scoop`。继续额外排查 `Continuation.resume` boxed payload / verify-roots 路径时，又暴露出新的既有 blocker `continuation_resume_struct_with_ref.scoop`，因此按要求在本条后插入 `T5001f7/T5001f7R`，并把 `T5001f6R` 顺延到其后。
- 依赖：T5001f5

### [DONE] T5001f7 修复 Continuation.resume consumed-root verify-roots regression，解除 `T5001f6R` / 后续验收阻塞
- 范围：
  - 修复 `tests/fixtures/run-pass/continuation_resume_struct_with_ref.scoop` 在 `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1` 下的新暴露回归：当前程序已越过 `after_handle / alice`，但随后会报 explicit-frame invalid root，说明 `Continuation.resume(...)` 相关 consumed continuation roots 在 moving GC + verify-roots 组合下仍有 stale source-of-truth 或漏更新路径。
  - 查清问题究竟来自 `when` binder / subject roots、resume receiver capture、state-machine outer-slot seed / writeback，还是 runtime 对 consumed continuation 生命周期与 root 更新的契约缺口；不能通过关闭 verify-roots、缩小 fixture、或假定 consumed continuation “用户本就不该再持有” 来回避实现问题。
  - 为最小 `Continuation<Named>` / boxed payload 路径补定向回归，至少覆盖 `when (saved) { Some(k) -> k.resume(Named { ... }) }` 后继续触发 GC collect / print 的窗口。
- 验收：
  - `env SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_struct_with_ref.scoop` 恢复通过；
  - `cargo run -p scoop -- test` / `T5001f6R` 不再被该 `Continuation.resume` verify-roots 回归阻塞。
- 完成记录：本轮先沿 verify-roots 把坏 slot 精确定位到 resumed outer handle 的 `step/dispatch` frame slot0，再对照 runtime `Continuation` ABI 收口到真正的长期 owner：`runtime/c/scoop_runtime.c` 里的 `ScoopContinuation.state`。此前 continuation 只把 effect frame state 当作普通 raw 指针保存；一旦后续 GC 搬迁该 frame，未来 `Continuation.resume(...)` 重新进入 `step/dispatch` 时就会把 stale `k->state` 写进两层 explicit frame slot0，并在 `continuation_resume_struct_with_ref.scoop` 的 resumed-body `println` 窗口触发 verify-roots invalid root。现已把 continuation state 与 continuation 生命周期绑定为 pinned：`scoop_continuation_alloc(...)` 在写入 `k->state` 后保留一个长期 pin，`scoop_continuation_release(...)` 在 continuation 释放时对称 `unpin`。这样 resumed body 期间的 raw `step(state, ...)` / `dispatch(state, ...)` 指针窗口不再依赖 moving update。定向验证已通过 `cargo build -p scoop_runtime`、`env SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_struct_with_ref.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/continuation_resume_struct_with_ref.scoop` 与 `cargo clippy --all-targets -- -D warnings`。继续执行 `cargo run -p scoop -- test` 时，suite 已越过该 fixture，新的首个失败切换为 `tests/fixtures/run-pass/class_init_raise_cleanup_init_block_gc_basic.scoop`，因此按要求在本条后插入 `T5001f8/T5001f8R`。
- 依赖：T5001f6

### T5001f8 将 state-machine 的 frame-backed locals 收口为“稳定执行期 local home + 统一 flush-back”设计（已拆分）
- 背景：
  - 当前 state-machine 仍会把 heap frame field 的 GEP 直接放进 env 当 `CgLocal.ptr`；一旦 state/arm body 中发生分配并触发 moving GC，这些预先算好的 slot pointer 会整体 stale，后续对 local/outer slot 的读写都会落到旧 frame 地址。
  - outer mutable local 的 caller-side source-of-truth 也还未完全收口：即使 frame 已记录 backing slot 指针，后续 writeback 若绕开 caller 当前稳定 local home / explicit-frame home slot，同样会让 handle 之后的 caller 读取继续看到旧值。
  - 当前 blocker 仍以 `continuation_resume_enum.scoop` 与 `effect_multi_escape_indirect_direct_while.scoop` 为主，故先拆成顺序子任务逐步收口。

### [DONE] T5001f8a0 修复 state-body 预填充 frame-slot locals 的 stale heap-slot pointer，解除 `T5001f8a` 阻塞
- 范围：
  - 修复 `populate_frame_slots_in_env(...)` 当前把 frame slot 的 heap-field GEP 直接塞进 env 当 `CgLocal.ptr` 的设计，至少先覆盖 outer mutable locals 在 state body 中的主线。
  - 让 `continuation_resume_enum.scoop` 第一段 handle 的 state-body `saved = Some(k)` 在 GC env 下也走稳定执行期 local home，而不是继续在 `Some(k)` 构造/分配后把写入落到 stale heap-slot pointer。
  - 与当前已完成的 caller-side writeback/readback 加固衔接：先保证 state body 产出的 frame slot 内容在 GC env 下是正确的，再继续做 `T5001f8a` 的 caller-return source-of-truth 收口。
- 验收：
  - `env SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_enum.scoop` 恢复输出 `ok / 42` 与 `err2 / 99`，不再出现 `missing1/missing2`；
  - 至少补一条最小 LLVM/fixture 回归，锁定 outer mutable local 在 state body 中不再长期直接使用 heap-frame GEP 作为执行期 local home。
- 完成记录：修复根因不是 frame slot 值本身，而是 state/arm body 内对 outer mutable local 的 `store` 仍直接使用 `populate_frame_slots_in_env(...)` 预先塞进 env 的 heap-frame field GEP 指针；RHS 求值触发 moving GC 后，这些 slot pointer 会 stale，导致写入落到旧 frame 地址。
  - compiler：在 `store_local_value_exact(...)` 写入前统一调用 `rematerialize_ptr_in_current_block(...)` 重建指针链，确保 heap-frame GEP 的 base（explicit-frame-backed frame ptr load）会在当前 block 重新 load，从而把 store 重新指向 relocated heap frame slot。
  - fixture：新增 `tests/fixtures/runtime_gc/effect_outer_mutable_state_body_writeback_basic.scoop`（固定 `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1`），锁定 arm 内对 outer `saved` 的赋值在 GC stress 下仍能在 handle-return 后读回。
  - 验证：
    - `env SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_enum.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/effect_outer_mutable_state_body_writeback_basic.scoop`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
- 依赖：T5001f7

### [TODO] T5001f8a0R Review：确认 state-body outer mutable local 已不再直接依赖 stale heap-slot pointer
- 重点：
  - `populate_frame_slots_in_env(...)` 是否已停止把 outer mutable local 直接暴露成 heap-frame GEP env local；
  - `continuation_resume_enum.scoop` 在 GC env 下恢复是否来自新的稳定执行期 local home，而不是偶然规避；
  - 新增回归是否真正锁住 state-body 写回窗口。
- 验收：
  - `T5001f8a` 可在不再被 state-body stale heap-slot pointer 阻塞的前提下继续推进。
- 依赖：T5001f8a0

### [TODO] T5001f8a 修复 outer mutable local 在原 caller handle-return 路径的稳定 writeback / readback 合同
- 范围：
  - 收紧 `write_back_outer_scope_frame_slots(...)`：当 handle 在原 caller activation 内完成时，outer mutable local 必须优先写回 caller 当前稳定 backing slot / local home，而不是只经由 frame 中记录的裸 storage pointer 回写。
  - 确保 caller handle-return 后立即读取 outer mutable local 时，读到的是刚写回的新值，而不是 caller explicit frame 中的旧 home-slot 镜像。
  - 以最小修复解除当前首个 blocker：
    - `tests/fixtures/run-pass/continuation_resume_enum.scoop`
  - 新增最小回归，单独锁定“escape-continuation arm 把 outer mutable local 写成 `Some(k)` 后，handle 结束返回 caller，caller 立刻读取能看到最新值”。
- 验收：
  - `continuation_resume_enum.scoop` 恢复输出 `ok / 42` 与 `err2 / 99` 主线，不再出现 `missing1/missing2`；
  - 至少新增/更新一条最小 fixture，单独锁定 outer mutable local 的 caller-side writeback/readback；
  - 当前修复不依赖放宽 fixture、关闭 explicit-frame reload，或让 caller 退回读取旧 local slot。
- 依赖：T5001f8a0R

### [TODO] T5001f8aR Review：确认 outer mutable local 的 caller-side source-of-truth 已闭合
- 重点：
  - handle 完成并返回原 caller 时，writeback 是否优先经过 caller 当前稳定 backing slot / local home；
  - caller 紧随 handle 之后的读取是否已经看到最新值，而不是 caller explicit frame 中的旧镜像；
  - 新增最小回归是否真正锁住了“handle-return 后立刻读回”的窗口。
- 验收：
  - `T5001f8b` 可在不再被 outer mutable local caller-side writeback/readback 缺口阻塞的前提下继续推进。
- 依赖：T5001f8a

### [TODO] T5001f8b 把 state/arm body 中的 frame-backed locals 从 heap-frame GEP 收口为稳定执行期 local home
- 范围：
  - 为 handle body locals、arm binder、capture locals、escape continuation binder 建立统一 contract：进入 state/arm 时从 heap frame 读出，落到稳定的 entry alloca / scratch local home，body 内后续读写只操作该稳定 local home。
  - 清理 `populate_frame_slots_in_env(...)`、`emit_bind_local_to_frame(...)`、`emit_read_local_from_frame(...)` 与 arm capture/binder 恢复里长期把 heap frame GEP 暴露给 env 的设计。
  - 重点解除“state/arm body 发生分配或 moving GC 后继续读写 stale heap-slot pointer”的剩余 correctness 缺口。
- 验收：
  - IR 中不再把 heap frame field GEP 长期作为 env local home 暴露给后续会分配/GC 的 state/arm body；
  - 至少补一条最小 LLVM 回归，锁定 state/arm body local 会先落到稳定执行期 local home。
- 依赖：T5001f8aR

### [TODO] T5001f8bR Review：确认 state/arm body 已不再长期持有 stale heap-slot pointer
- 重点：
  - handle body locals、arm binder、capture locals、escape continuation binder 是否都已收口到稳定执行期 local home；
  - 是否还残留“env local 直接指向 heap frame field GEP”的路径；
  - LLVM 回归是否锁住了新的 local-home contract。
- 验收：
  - `T5001f8c` 可在不再被 stale heap-slot pointer 设计阻塞的前提下继续推进。
- 依赖：T5001f8b

### [TODO] T5001f8c 在 suspend / return / arm-exit / cleanup 边界统一 flush mutable locals 回 frame，并补齐 direct/indirect mixed 回归
- 范围：
  - 在 suspend / return / arm-exit / cleanup edge 统一 flush mutable locals 回 heap frame，使 frame 成为跨 resume / cleanup 的稳定持久化 source-of-truth。
  - 以系统性设计修复当前剩余 blocker：
    - `tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
  - 为 direct escape、indirect ordinary suspend、outer mutable local writeback 三类窗口补最小 fixture/LLVM 回归。
- 验收：
  - `effect_multi_escape_indirect_direct_while.scoop` 恢复 golden，不再出现 `missing1..missing4` 或顺序错乱；
  - mutable local 的 frame flush-back contract 已覆盖 suspend / return / arm-exit / cleanup 四类边界。
- 依赖：T5001f8bR

### [TODO] T5001f8R Review：确认 state-machine local-home / flush-back 合同已经真正取代 stale heap-slot pointer 设计
- 重点：
  - state/arm body 内是否仍残留“直接把 heap frame field GEP 绑定到 env local”这种 stale pointer 设计；
  - outer mutable local、arm binder、capture local、escape continuation binder 是否统一经稳定 local home + flush-back 路径收口；
  - `continuation_resume_enum.scoop`、`effect_multi_escape_indirect_direct_while.scoop` 是否锁住了 direct escape、indirect ordinary suspend、outer var writeback 三类关键窗口。
- 验收：
  - `T5001f9` / `T5001g` 可在不再被 state-machine stale heap-slot pointer 设计阻塞的前提下继续推进。
  - review 阶段必须用 `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1` 重跑相关 direct/indirect/outer-writeback fixture。
- 依赖：T5001f8c

### [TODO] T5001f9 将 continuation / replay-state 的长期对象 owner 从长期 pin 收口为 stable GC handle
- 范围：
  - 清理 runtime 中所有“长期 raw owner 只能靠长期 pin 保住对象地址”的 continuation 相关路径，明确哪些所有权是短窗口（可继续动态 pin），哪些是跨 resume / 跨线程 / 跨 handle-return 的长期 owner（必须用 stable handle）。
  - 至少覆盖：
    - `Continuation.state`
    - `Continuation.captured_callee_suspend_state`
    - `ContinuationResumeReplayState.pending_continuation`
    - `ContinuationResumeReplayState.prev_callee_suspend_state`
  - 确保 release_fn / discard / successful resume / replay-state 替换路径对 handle 生命周期与 drop 时机是对称的，并避免在 GC release 上下文里重入 GC 锁。
  - 保持 TLS `__scoop_callee_suspend_state` 只是短期 scratch transport，而不是新的长期 owner。
- 拆分为可验证步骤：
  1. `Continuation.state` 改为 stable handle，并验证 direct resumed body 在 GC env 下不再依赖长期 pin。
  2. `Continuation.captured_callee_suspend_state` 改为 stable handle，并验证 indirect ordinary resume / replay 主线。
  3. `ReplayState.pending_continuation` / `prev_callee_suspend_state` 改为 stable handle，并验证 replacement/release 路径不再 stale/self-deadlock。
  4. 清理 release_fn 上下文中的 handle drop 语义，确保不会重入 GC 锁。
  5. 为 direct resume、indirect resume、cross-thread resume、boxed payload resume、mixed direct/indirect escape 各补最小 fixture/回归。
- 验收：
  - 运行时不再依赖长期 `pin` 维持 continuation/replay-state 的长期所有权；
  - `continuation_resume_struct_with_ref.scoop`、`continuation_resume_enum.scoop`、`effect_escape_continuation_multi_perform_cross_thread.scoop`、`task_step_cross_thread_sequential_handoff_gc_stress.scoop` 在 GC env 下继续稳定通过；
  - runtime 侧 handle release / discard / replay-state release 不再出现 release_fn 自锁或 stale raw owner。
- 依赖：T5001f8R

### [TODO] T5001f9R Review：确认 continuation/replay-state 的长期 owner 已稳定收口到 handle，而不是 pin 或裸指针
- 重点：
  - continuation / replay-state 的长期 owner 是否已全部 handle 化；
  - 剩余 pin 是否都只是短窗口（如 `step_fn` 执行期、payload runtime call 窗口、native wait 窗口）；
  - release_fn / discard / cross-thread worker / replay-state replacement 是否仍存在锁重入、handle 泄漏或过早 drop。
- 验收：
  - `T5001g` 可在不再被 continuation 长期 owner 设计阻塞的前提下继续做全量验收。
  - review 阶段必须在三项 GC env 全开条件下重跑所有 continuation/effect/task handoff 邻近 fixture，而不是只看单条 smoke。
- 依赖：T5001f9

### [TODO] T5001g 全量回归、GC stress、verify-roots 与文档收尾
- 范围：
  - 运行并整理最小验收矩阵，至少覆盖：
    - `cargo test --all`
    - `cargo run -p scoop -- test`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/build`
  - 补最小定向回归，锁定：
    - explicit frame push/pop 与 TLS chain；
    - dead slot 会清零；
    - post-safepoint reload；
    - aggregate refresh/rebuild；
    - 默认 explicit mode 不再生成 stackmap section。
  - 同步 `SCOOP_RUNTIME.md` 与必要实现注释，说明默认 explicit mode 现已采用 explicit root frame。
  - 记录 object / binary size 与 steady-state / GC pause 的观察结果，但不把性能调优作为本任务 blocker。
  - 使用单个 fixture 顺序执行的方式，在 `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1` 条件下完整验证：
    - `tests/fixtures/run-pass/**`
    - `tests/fixtures/runtime_gc/**`
    - `tests/fixtures/build/**`
  - 任何复杂逻辑若在本轮实现中新增了独立设计点，但还没有最小 fixture 覆盖，必须在进入 `T5001gR` 前补齐。
- 验收：
  - 全量回归与定向回归都能支撑“默认 explicit mode 已切换成功”的结论；
  - 文档、实现注释与实际行为已对齐。
- 依赖：T5001f9R

### [TODO] T5001gR Review：确认本轮 correctness 收口完成，并为后续优化划清边界
- 重点：
  - 是否还残留默认 explicit mode 的 correctness 缺口；
  - regression 是否已覆盖 push/pop、NULL discipline、reload、aggregate rebuild 与“无 stackmap section”五类核心合同；
  - stackmap selective comeback、`mem2reg` allowlist/denylist 是否已被明确留到后续独立任务，而不是混入本轮验收。
- 验收：
  - 本轮结论可明确表述为：默认 explicit mode 已切到 explicit root frame；stackmap 已退到可选优化路线；`mem2reg` 仍是后续单独任务。
- 依赖：T5001g
