# GC-FIX-TODO：彻底移除 Shadow Stack，StackMap-only 精确 GC（以正确性/完备性为第一目标）

> 目标：当本文档中的全部步骤完成后，GC 子系统应在 **不依赖 shadow stack 的前提下**，仅基于 **LLVM statepoint + StackMap** 完整枚举与更新 roots，并且不存在任何“当前限制/未来再做”的缺口（性能优化不在本轮目标内）。

---

## 0. 背景与现状（为什么现在不能直接删 shadow stack）

当前实现处于“过渡期双轨”：

- 运行时仍维护 `ScoopGcFrame` 链（shadow stack），并在 GC mark / moving roots update 时 **必扫**（即使 stackmap 命中 record）。
- stackmap roots 枚举已接入（statepoint pipeline + runtime stackmap registry + stack walking），但仍存在多处 best-effort/启发式与子集支持：
  - `return_address -> record` 有近似匹配窗口；
  - FP 基址可能需要从 CFA 猜测；
  - stackmap locations 仅支持 pointer-sized 的 Direct/Indirect（其余跳过）；
  - managed frames 的边界策略是“首次命中后遇到 miss 即停止”；
  - 为避免崩溃存在 membership 过滤/保守跳过策略；
  - Rust 侧测试大量通过手工 `ScoopGcFrame` push/pop 来模拟 roots（因为 Rust 代码本身不会产生 statepoint stackmaps）。

因此，“直接删 shadow stack”会把 correctness 押在若干不变式（record 命中、location 完整性、unwind 可靠性、测试根集来源）上；这些不变式目前尚未被系统性固化为强保证。

---

## 1. 总目标（What “Done” looks like）

### 1.1 功能目标（必须全部满足）

1) **无 shadow stack**
- 编译器不再生成 `ScoopGcFrame` / `scoop_gc_frame_push/pop` 插桩。
- 运行时不再维护 TLS `gc_current_frame`，不再提供 `scoop_gc_shadow_stack_*` / `scoop_gc_debug_count_roots_*` 等 API。
- GC mark / moving/compaction roots update 代码路径中不再出现 shadow stack 分支或回退扫描。

2) **StackMap-only roots（managed code）**
- 所有 managed 线程在 stop-the-world 下都能通过 stack walking + stackmap lookup 枚举其 **全部 live GC pointers**。
- moving/compaction 必须对 stackmap roots 槽位执行 **原地写回更新**，保证 `gc.relocate`/后续 loads 读取到新地址。

3) **Native roots（non-managed code）明确、完备、可更新**
- 对于不产生 statepoint stackmaps 的代码（例如 Rust 测试、C runtime 自身），roots 必须通过明确机制提供：
  - `enter_native/leave_native`（native roots slots）或
  - stable handles / pin 表等。
- moving/compaction 期间这些 roots 也必须被更新（slot 原地写回 / handle 表更新等）。
- 不允许“未注册线程/未暴露 roots 但仍持有 GC 指针”的 silent correctness hole。

4) **无“当前限制/未来再做”**
- 不再依赖 `__clang__/__GNUC__` 的 GNU label address smoke；
- Windows/COFF/remote-unwind 等平台支持应进入“可用且可回归”的状态；
- 不再存在“某 backend 在多线程场景下退化为 no-op（宁可泄漏）”之类的功能缺口（除非明确移除该 backend）。

### 1.2 退出条件（Definition of Done）

完成本文档所有步骤后，应满足：

- `cargo test --all` 通过（Rust 测试无 shadow stack 依赖）。
- `cargo run -p scoop -- test` 通过（fixtures 全部更新为 stackmap/statepoint 路线）。
- `cargo run -p scoop_tools -- spec-fixtures check` 通过（若 spec fixtures 涉及 GC 章节/示例）。
- `scoop dump-stackmaps <bin>` 能输出稳定、可诊断的“roots 位置”信息，并可用于定位任意 GC 相关 bug。
- 所有对外 ABI/API 文档与 sysroot API 与实现一致（不包含 shadow stack）。

---

## 2. 设计原则（保证“完备性 + 正确性”，性能后置）

1) **宁可 fail-fast，不允许 silent mis-collection**
- 纯 stackmap 模式下：任何“stackmap record 未命中 / locations 无法解析 / roots 更新不全”的情况都必须可诊断，并在默认配置下 fail-fast（避免静默悬挂指针/误回收）。

2) **根集来源必须可枚举、可更新**
- moving/compaction 的正确性要求：每个 live pointer 必须能映射到可写回位置（slot 或等价机制），否则不能宣称“完成”。

3) **系统性回归：每个缺口都要有测试**
- 每项“过去的限制/启发式”都应落在某个回归测试或 tooling 断言里，避免回退。

---

## 3. 分阶段实施计划（Step-by-step）

> 说明：每一步都应包含：
> - 改动范围（哪些 crate / 哪些 runtime 文件）
> - 不变量（该步骤后系统必须满足什么）
> - 回归/验收（新增或修改哪些测试、跑哪些命令）

### Phase A：把 StackMap 路线做成“强保证”（仍可暂时保留 shadow stack 作为冗余，直到最后一刀切）

#### A1) 固化 statepoint/stackmap 的语义契约（编译器 ↔ 运行时）
- 明确“哪些 stackmap locations 是 GC roots”的可计算规则（不依赖 heap membership 过滤）。
  - 需要决定：使用 LLVM statepoint 的哪一段（deopt args vs gc-live args）作为 roots；并保证 IR 构造一致。
- 为运行时提供可验证的 metadata：
  - 例如：每个 statepoint 的 patchpoint_id / extra metadata 能让 runtime 确认 location 列表与 roots 的对应关系。
- 验收：
  - `scoopc` LLVM 单测：生成一个最小 module，断言 stackmap records 可被解析且“roots locations 数量/类型”符合契约。
  - `scoop dump-stackmaps`：新增 `--verify-roots` 模式，失败时给出精确诊断（哪个 record、哪个 location 不符合契约）。

#### A2) StackMap registry/lookup：从“近似命中”升级为“无歧义命中”
- 目标：managed frame 的 return address lookup 必须稳定、可证明不误配。
- 具体任务：
  - 统一 “record key 的定义”（callsite vs return address）与 platform/unwind 提供的 `ra` 语义；
  - 处理 instruction_offset 的边界（call 指令长度、-1/+N 偏移）时，不再依赖小窗口碰运气；
  - 若存在多个候选 record（冲突/近似）必须诊断并 fail-fast（纯模式下）。
- 验收：
  - 新增回归：构造多个相邻 records 的场景，确保 lookup 不会误命中；
  - 在 macOS/Linux/Windows 各至少一个可执行文件上回归通过（CI 允许分平台 gating，但实现不应永久缺失）。

#### A3) Platform unwind：为 GC 提供“可回归、可解释、跨平台”的 stack walking 输入
- 目标：对于被 park 的线程，能提供稳定的 `(sp/cfa, ra, fp)`（或等价信息），且足以计算 stackmap slots。
- 具体任务：
  - POSIX：从 `_Unwind_Backtrace` 采样升级为“可验证的帧序列”（必要时引入更强的上下文捕获）。
  - Windows：补齐 `unwind_win32.c`，实现 ctx capture + frame walk（至少能服务 stackmap roots 枚举与更新）。
  - 明确 FP 的来源：不允许长期依赖“从 CFA 猜 FP”（纯模式下应作为错误/不可接受降级）。
- 验收：
  - `gc_stack_walking_unwind` / `unwind_capture` 类测试扩展为多平台回归；
  - 新增一个“帧校验”工具输出：每帧打印 ra/sp/fp 与是否命中 stackmap record。

#### A4) StackMap location 支持从“子集”升级为“完备或可证明不需要”
当前实现仅支持 pointer-sized 的 Direct/Indirect，并跳过 Register/Constant 等。

为了“无任何限制”，需要二选一（推荐先做 1，再决定是否还要 2）：

1) **编译器保证法（推荐）**：保证 GC roots 在 safepoint 上总能以可写回的 stack slot 形式出现（Direct/Indirect，base=SP/FP 可解析）。
- 通过 LLVM pipeline/约束，使 gc-live values 在 statepoint 周围稳定 spill 到内存槽位；
- 并在 tooling 中加入硬断言：若出现寄存器 roots/不可写回 location，直接拒绝构建或 fail-fast。

2) **运行时寄存器更新法（更难）**：支持 Register locations 的 roots 枚举与 moving 更新。
- 需要 platform 层提供：park 点捕获可修改的寄存器上下文，并在恢复执行前写回；
- 需要 STW 协议允许“修改被 park 线程的上下文”。

验收：
- 无论选择哪条路，最终必须做到：GC roots 枚举与更新 **对所有程序都完备**，不存在“某些 roots 位置永远扫不到/更不回去”的情况。

---

### Phase B：把 GC 本体切到 StackMap-only（并清理所有 shadow stack 相关实现）

#### B1) 线程模型与 STW 协议：消除 shadow stack 字段与语义
- 修改 `ScoopGcThreadRecord`：
  - 移除 `current_frame_slot`（以及任何依赖 `ScoopGcFrame**` 的注册协议）；
  - 明确线程的 roots 来源仅为：
    - `stack_walking_ctx`（Parked / managed）
    - `native_roots`（InNative）
    - global roots / handle / pin / runtime-internal roots（见后续步骤）
- 修改 `scoop_thread_register/unregister`：不再传入/维护 `gc_current_frame`。
- 验收：
  - STW 回归测试：多线程 park/恢复稳定；不会因缺少 frame slot 崩溃或死锁。

#### B2) GC roots 枚举：只走 stackmap/native/handle/pin/global，不允许 shadow stack fallback
- baseline 与 immix 两个 backend 都需要完成同一套逻辑（或合并实现，避免分叉）：
  - mark roots：
    - initiator：捕获并 walk 自己的 ctx（必须覆盖完整 managed 栈）；
    - parked threads：使用其 park 前捕获的 ctx；
    - in-native threads：扫描 `native_roots` slots；
    - stable handles、pinned objects、以及（若存在）全局 roots 表。
  - roots update（moving/compaction）：
    - 对 stackmap spill slots 与 native_roots slots 执行原地写回；
    - 更新 handle 表（`handle->obj`）；
    - 修复 heap 内字段（type descriptor trace）；
    - 不再扫描 shadow stack。
- 关键点：
  - 需要把“stackmap 中哪些 locations 是 roots”变成精确规则（见 A1），否则会一直依赖 membership 过滤而无法宣称完备性。
- 验收：
  - 端到端：moving/compaction 下多线程 + cross-thread refs 的压力测试稳定通过；
  - 新增“强校验模式”：GC 结束后对所有 roots 进行一致性验证（可用慢路径/额外扫描；性能不要求）。

#### B3) runtime API 清理：移除所有 shadow stack 导出符号与 sysroot 接口
- 移除（或彻底废弃并在 ABI allowlist 中剔除）：
  - `scoop_gc_current_frame`
  - `scoop_gc_frame_push/pop`
  - `scoop_gc_shadow_stack_visit_roots_*`
  - `scoop_gc_debug_count_roots_current_thread`
  - 以及 sysroot 中对应的测试辅助 API（例如 core.scoop 里的 shadow stack 调试函数）
- 同步更新：
  - `runtime/c/scoop_runtime_api.h` allowlist
  - 文档与 audit 文件（如 `RUNTIME_STDLIB_INTRINSIC_AUDIT.md`、`STDLIB_DESIGN.md` 等）
- 验收：
  - 全仓 `rg "shadow_stack|ScoopGcFrame|gc_frame_push"` 仅保留历史文档（或完全消失，视策略而定）。

---

### Phase C：编译器侧彻底移除 shadow stack 插桩，确保所有 roots 由 statepoint/stackmap 覆盖

#### C1) LLVM codegen：删除 `GcFrameState` 与所有 frame/slot 维护代码
- 删除 `setup_gc_frame*`、`store_gc_root_slot_value`、以及 class ctor 临时 frame 等逻辑。
- 确保所有 GC 相关 safepoint 都通过 statepoint 管线可见：
  - 分配：`scoop_alloc_typed`（或统一的 alloc helper）必须是 statepoint safepoint；
  - 其它可能触发 GC 的边界（显式 `GC.collect()`、effect/exception/unwind 边界、FFI 边界等）要有明确策略：
    - 要么保证这些边界不会在“未 statepoint 化的位置”触发移动；
    - 要么把 safepoint 设计成只发生在 statepoint 可见位置。
- 验收：
  - `scoopc --features llvm`：生成 IR 断言不包含任何 `scoop_gc_frame_*` 调用；
  - stackmap dump：每个预期 safepoint 均能定位 record，并能枚举到 live roots。

#### C2) 彻底消除“statepoint 重写崩溃/特殊绕路”的遗留
- 目前有一些“为避免 rewrite-statepoints-for-gc 崩溃而绕开的路径”（例如某些临时对象/addrspacecast 场景）。
- 目标：这些绕路不应作为长期限制存在。
- 具体任务：
  - 统一 ref 指针类型策略（`addrspace(1)`）；
  - 禁止 ptr<->int 的编码/逃逸；
  - 让 println/字符串等路径也能在 statepoint 下稳定工作（必要时拆分为 runtime helper，确保 IR 合法）。
- 验收：
  - 新增 fixtures 覆盖：字符串/数组/closure/接口分发等场景下触发 GC，仍可正确运行且 stackmaps 可解析。

---

### Phase D：Rust 测试全面去 shadow stack（并新增 stackmap-only 语义回归）

> 关键现实：Rust 测试代码本身不会产生 statepoint stackmaps，因此不能指望“靠 stackmap 扫 Rust 栈”。
> 正确做法是：Rust 测试必须通过 **native_roots / handle / pin** 等显式机制参与 roots 枚举与更新。

#### D1) 删除/替换 `crates/scoop_runtime/tests/shadow_stack.rs`
- 该文件将不再有意义（shadow stack 已移除）。
- 用以下内容替代（任选组合）：
  - `native_roots` 的行为/更新测试（moving 下 slot 写回）；
  - stackmap registry + walk 的纯 smoke（尽量不依赖 GNU label address）。

#### D2) 把所有手工 `ScoopGcFrame` 的 Rust 测试改为显式 roots
逐个替换（示例策略）：

- `gc_mark_sweep.rs`：
  - 用 `scoop_handle_new` 或 `scoop_enter_native(&mut obj as slot)` 保活对象；
  - 再 drop handle / leave_native 后验证可回收。

- `gc_stop_the_world.rs`：
  - worker 与 main 都通过 `enter_native` 暴露各自的 `obj` 局部变量槽位；
  - worker 仍在 `scoop_gc_safepoint(_poll)` 循环里参与协作式 STW；
  - GC 后验证对象均未被回收；退出后再验证可回收。

- `type_descriptor_release_callback.rs`：
  - 通过 handle 或 native_roots 保持对象一次 GC 周期；
  - 释放后验证 release callback 恰好一次。

- `gc_immix_compaction.rs` / `gc_immix_parallel_mark*.rs` / `gc_immix_parallel_mark_sweep_stress.rs`：
  - 去掉 `ScoopGcFrame` roots；
  - 使用 `enter_native` 暴露 roots slots（这样 moving/compaction 可写回更新 Rust 变量）；
  - 若需要验证“根槽位被更新”，直接断言 `obj` 变量在 GC 后变为新地址并且数据/引用关系正确。

- `gc_hosted_multi_thread_collect_noop.rs`：
  - 若最终决定“hosted/minimal 也要无功能缺口”，则该测试需要重写或删除；
  - 若决定移除 hosted/minimal backend，则删除该测试并同步更新 build/features 文档。

#### D3) 更新 capability matrix 相关测试
- `gc_capabilities.rs`：
  - `shadow_stack_roots` 应删除或改为 `stackmap_roots`（以及 `native_roots` 等能力标识）。
- 任何依赖 `shadow_stack_roots: true` 的断言都必须更新。

验收：
- `cargo test --all`：Rust tests 全部通过，且全仓不再引用 `ScoopGcFrame` / `scoop_gc_frame_push/pop`。

---

### Phase E：fixtures / 工具链回归（Scoop 侧端到端证明“无需 shadow stack”）

#### E1) 移除 shadow stack fixtures，新增 stackmap/statepoint fixtures
- 删除或改写：
  - `tests/fixtures/run-pass/gc_shadow_stack_instrumentation_basic.*`
  - 任何依赖 sysroot “shadow stack debug helper” 的用例
- 新增：
  - “stackmap roots keepalive” run-pass：在 Scoop 程序里制造多帧 live roots，触发 GC（含 moving）并断言行为正确；
  - “FFI/native roots” run-pass：通过 `@Extern` 路径覆盖 `enter_native/leave_native` 的 roots 更新。

#### E2) `scoop dump-stackmaps` 升级为 GC 调试主工具
- 输出应能定位：
  - 每个 record 对应的 function/offset/patchpoint_id；
  - roots locations 的类型/基址/offset；
  - 是否满足“可写回 roots slot”的要求；
  - 若不满足，给出明确错误（纯 stackmap 模式下应阻止继续）。

验收：
- `cargo run -p scoop -- test` 全绿；
- `dump-stackmaps` 输出可被 fixtures 断言（稳定、不依赖平台偶然性）。

---

## 4. 关键风险与对策（确保最终能“彻底移除限制”）

1) **寄存器 roots 更新**
- 对策优先级：
  1) 编译器保证 roots 可写回（最现实、跨平台成本最低）；
  2) 若仍出现寄存器 roots，则必须扩展 platform/unwind 与 park 协议以支持寄存器写回（工程量大，但可作为“真正无死角”的最终形态）。

2) **RA 语义不一致导致 record 命中不稳定**
- 对策：统一 key 语义、去掉近似匹配或将近似匹配升级为“可证明不误配”的多约束匹配，并加入强诊断。

3) **Rust 测试无法靠 stackmap 扫栈**
- 对策：Rust tests 全部改走 native_roots/handle/pin；并新增“native_roots 在 moving 下必须写回更新”的专用回归。

4) **多 backend 分叉导致长期不一致**
- 对策：能合并的逻辑尽量合并（stackmap roots enum / roots update / STW 状态机），避免 baseline 与 immix 在语义上长期漂移。

---

## 5. 实施顺序建议（最短 correctness 路径）

建议按以下顺序落地（每步都有可回归增量）：

1) A1/A2（语义契约 + lookup 无歧义）  
2) A3（unwind 可靠，尤其是 FP）  
3) A4（roots locations 完备：至少做到“可证明不需要寄存器 roots 更新”）  
4) D2（Rust tests 全面改为 native_roots/handle/pin）  
5) B1/B2（GC 核心切 stackmap-only，移除 shadow stack fallback）  
6) C1/C2（编译器彻底移除 shadow stack 插桩；消除 statepoint 绕路）  
7) B3/E1/E2（清理 ABI/文档/fixtures/tooling，保证无遗留）  

---

## 6. 验收命令清单（每个大阶段都应跑）

- `cargo test --all`
- `cargo run -p scoop -- test`
- `cargo run -p scoop_tools -- spec-fixtures check`
- （LLVM 路线相关）`cargo test -p scoopc --features llvm`
- （调试）`cargo run -p scoop -- dump-stackmaps <bin>`

