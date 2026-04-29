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

### P2. explicit root frame runtime substrate

- 引入 `ScoopRootFrameDesc`、`ScoopRootFrameHeader` 与 TLS `__scoop_explicit_root_frame_top`。
- 让 runtime 能从 TLS top 沿 `prev` 链遍历 explicit frame chain，并按 `header -> desc -> offsets` 恢复每个 `void** slot`。
- `InNative` 线程在 explicit mode 下不再依赖 captured unwind ctx 回找 caller managed frames；`native_roots` 只保留 native 边界临时根语义。

### P3. 编译器发射 explicit frame object 与 descriptor

- 为每个 managed 函数规划固定 frame layout，并为其生成函数级 descriptor / offset table。
- 每个 activation 的 explicit frame object 必须是 entry-block alloca，且 header 位于首字段。
- entry 时完成：frame alloca、slot `NULL` 初始化、`hdr.desc` / `hdr.prev` 安装、TLS push。
- 所有退出路径都必须完成 TLS pop。
- 无 roots 函数可在第一阶段跳过 frame 构造，但该选择必须是显式且可审计的，而不是偶然漏发射。

### P4. 把所有跨 safepoint roots 收敛到 stable home slots

- 源码级 locals、aggregate 内 ref leaf、sret/indirect arg scratch、effect/state-machine lowering 生成的临时 GC roots、`extra_gc_root_slots` 一类内部根，都必须映射到 explicit frame 的固定 fields。
- descriptor 记录 slot offset；frame field 本身保存对象头指针值。
- 不允许继续依赖“LLVM 之后会帮我 spill 成可更新 location”这一旧合同。

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
