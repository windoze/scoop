# 当前执行记录（本轮）

## 分析摘要

- 最新提交是 `28dd8897811c42212003fc6231bb08551daa1391`（`[T4016b4b] Remove pure continuation shorthand and note GC blocker`）。
- 该提交没有正文，也没有额外列出新的遗留 issue；但提交标题与 `TODO.md` / `PLAN.md` 一致地表明，当前真实前置问题是 `T4016b4b0`：修复 GC stress 下 cross-thread escaped continuation resume 的 runtime 崩溃。
- `TODO.md` 中最前面的 `[TODO]` 条目包含总纲任务与拆分任务；按当前顺序，首个可执行的未完成叶子任务是 `T4016b4b0`，不是总纲 `T4016` / `T4016b` / `T4016b4`。
- 本轮目标因此明确为：先完整修复 `T4016b4b0`，验证通过后更新 `TODO.md` / `PLAN.md` / `memory/claude_plan.md` 并提交；本轮不继续推进 `T4016b4b`。

## 执行计划

1. 复现 `T4016b4b0` 描述的 runtime 崩溃，确认当前失败形态、stdout/stderr、退出码，以及是否能稳定重现。
2. 阅读与该路径相关的实现：
   - fixture：`tests/fixtures/run-pass/gc_continuation_multi_thread_concurrent_alloc_resume.scoop`
   - continuation runtime / GC / 线程注册代码
   - 如有必要，查找 cross-thread resume、GC root、frame liveness、continuation 状态迁移相关测试与实现。
3. 定位根因：
   - 判断是 thread registration、GC root 缺失、escaped continuation frame 生命周期、resume 状态机、还是 stress collect 路径上的竞态/断言问题。
   - 如果发现当前任务依赖另一个更基础且未被追踪的真实缺陷，则停止直接修复，按规则先把前置任务写入 `TODO.md` / `PLAN.md` 并提交。
4. 在不引入 workaround 的前提下实现修复，并补必要测试/回归。
5. 运行充分验证，至少覆盖：
   - 定向 build + `SCOOP_GC_STRESS=1` 运行该 fixture；
   - 相关 runtime / run-pass / 单元测试；
   - `cargo test --all`；
   - `cargo clippy --all-targets -- -D warnings`。
6. 更新文档与任务状态：
   - 将 `T4016b4b0` 标记为完成；
   - 更新 `PLAN.md` 的当前状态与下一步；
   - 继续记录本文件中的结果与残余风险。
7. 提交变更并停止，不继续处理后续任务。

## 当前进度

- 已确认仓库工作树干净，上一轮任务已提交完成。
- 已确认本轮目标是 `T4016b4b0`。
- 已完成复现：
  - `cargo run -p scoop -- build tests/fixtures/run-pass/gc_continuation_multi_thread_concurrent_alloc_resume.scoop -o /tmp/gc_continuation_multi_thread_concurrent_alloc_resume.out`
  - `SCOOP_GC_STRESS=1 /tmp/gc_continuation_multi_thread_concurrent_alloc_resume.out`
  - 现象是在输出 `workerA_resuming` 后以 `SIGSEGV` 退出。
- 已找到更小复现：
  - `tests/fixtures/run-pass/effect_escape_continuation_resume_cross_thread.scoop`
  - 它在 `SCOOP_GC_STRESS=1` 下也会于 worker 线程 resume escaped continuation 时崩溃。
- 已基本锁定根因：
  - continuation 对象本身仍然活着，但 `Continuation.state` 指向的 effect frame 在 GC stress 触发的收集中没有被正确保活，随后被字符串分配重用。
  - 当前最可疑的缺口不在线程注册或 join 路径，而在 LLVM/codegen 的 safepoint / statepoint root 暴露：某些局部虽然底层实际表示是 `ptr addrspace(1)` 的 GC 指针，但未进入 `gc-live` 根集合。
- 下一步是直接检查 codegen 中 statepoint / extern call / print / string allocation 的 live-root 收集逻辑，优先修补“按高层 `CgTy` 判断导致漏掉 niche-optimized enum pointer local”的路径。

## 已确认的技术结论

- 崩溃点位于 resumed state machine 正常完成后的 outer-scope writeback 路径；frame 中保存的写回目标地址槽已经失效，因此最终在 `mov %rcx,(%rax)` 处对空地址写入。
- 更早的根因是：主线程在 `println("after handle")` 触发 GC stress 后，`saved: Continuation<Int, Unit>?` 仍持有 continuation，但其 `state` 所指的 effect frame 已被回收，并被新的 `ScoopString("after handle")` 对象复用原地址。
- 因此问题不是 continuation resume 语义本身，而是 safepoint/GC root 可见性错误：GC 没有沿着 `saved -> continuation -> state(frame)` 这条链保活 frame。
- 当前实现里已有类似风险信号：
  - `needs_write_barrier_for_value_ty()` 的注释已经指出，`Option<Ref>` 等 `CgTy::Enum` 可能 niche-opt 成 `ptr addrspace(1)`，若只看高层 `CgTy` 会漏掉 GC 相关处理。
  - 本次崩溃与这一类“表面是 enum，底层是 GC pointer”的 local 在 statepoint live-root 收集中漏记高度一致。

## 下一步执行顺序

1. 检查 `crates/scoopc/src/llvm/codegen/mod.rs`、`gc.rs` 与 effect/state machine 路径中 statepoint live-root 的收集规则。
2. 为“底层 LLVM 表示实际是 GC pointer 的 local 栈槽”补保守 root 暴露逻辑，不再仅依赖 `CgTy::Ref | CgTy::String`。
3. 优先覆盖当前最容易触发问题的 safepoint：
   - 字符串字面量分配
   - `println` / print 相关路径
   - 普通 direct call / extern call 包装层
4. 补回归测试，至少锁住最小 cross-thread continuation + GC stress 复现。
5. 复验最小复现、目标 fixture、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
6. 若修复成立，再更新 `TODO.md` / `PLAN.md` 并提交本轮任务。

## 约束与停机条件

- 不接受通过关闭 GC stress、规避 cross-thread resume、修改 fixture 语义、降低验证范围等方式“绕过”问题。
- 若修复过程中暴露出新的、更基础的规范/实现缺口，且它是 `T4016b4b0` 的真实前置依赖，则必须先更新 `TODO.md` / `PLAN.md`，提交任务重排后停止。
- 本轮完成标准是：`T4016b4b0` 完整修复、测试通过、文档状态同步、已提交。
## 2026-04-21 当前续做记录

### 说明
- 按用户要求先更新本文件，再继续执行任何检查/构建/测试命令。
- 这里记录的是可审计的判断摘要、执行计划与进度，不包含逐字内部思维展开。

### 当前任务
- `TODO.md` 当前首个未完成叶子任务仍是 `T4016b4b0`：
  修复 `GC stress` 下 cross-thread escaped continuation resume 的 runtime 崩溃。

### 已知状态摘要
- 最新提交 `28dd8897 [T4016b4b] Remove pure continuation shorthand and note GC blocker` 已检查过，没有新的 commit message issue 需要先修。
- 第一阶段修复已经落地：
  - ordinary 函数 safepoint/call 前后对 GC local root 做保守 keepalive + relocated 写回。
  - 最小复现 `effect_escape_continuation_resume_cross_thread.scoop` 已在 `SCOOP_GC_STRESS=1` 下恢复正常。
- 当前剩余 blocker：
  - 目标 fixture `gc_continuation_multi_thread_concurrent_alloc_resume.scoop` 仍在 `workerAFn` 中崩溃。
  - GDB 栈显示崩于 `scoop_gc_write_barrier -> scoop_gc_immix_block_from_object`。
  - 现有线索更指向 write barrier 调用参数或 `Option<Continuation>`/field store lowering 形状错误，而不是类字段 trace bitmap 漏记。

### 本轮执行计划
1. 重新读取 `TODO.md` / `PLAN.md` / 当前工作区状态，确认仍应继续 `T4016b4b0`，并避免覆盖已有未提交修改。
2. 精确定位 `workerAFn` 中崩溃对应的 `scoop_gc_write_barrier` 调用点：
   - 检查生成 IR / 目标汇编 / 反汇编；
   - 如有必要，增加最小且可保留的运行时防御/断言或编译期检查，确认传入的 `slot_addr` / `value` 哪一个失真。
3. 回到 LLVM codegen 的 heap field store / enum niche lowering 路径，重点检查：
   - `needs_write_barrier_for_value_ty`
   - heap field store 使用的 slot 地址计算
   - `Option<Continuation>` 在 `Some/None`、load/store、coerce/cast 路径中的实际 LLVM 表示
4. 完成修复后，先跑定向复现与回归：
   - 两个相关 run-pass fixture 在 `SCOOP_GC_STRESS=1` 下通过；
   - 补最小必要回归测试，锁住此次 bug。
5. 再跑全量质量门：
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
6. 全部通过后再更新：
   - `TODO.md`
   - `PLAN.md`
   - 本文件进度记录
   - git commit

### 本轮重点判断
- 优先验证 `workerAFn` 中某次 barrier 调用是否把“非 heap slot 地址”或“非对象头指针的 niche 值”传给了 runtime。
- 如果确认这是更基础的 codegen/runtime 缺陷，且无法在本轮内以规范实现完成，则按要求先在 `TODO.md` / `PLAN.md` 中显式建前置任务并停止。

### 2026-04-21 进展更新：已确认更基础 blocker，转为任务重排
- `workerAFn` 中崩溃的第一条 `scoop_gc_write_barrier` 已对上机器码 call site：
  - 崩溃点是 `workerAFn` 里把 `cell.k` 读出后写入新 effect frame 槽位的那次 barrier；
  - 崩溃时 `rdi/rsi` 同时为 `0x9`，不是 barrier 内部自行算坏，而是调用前读取到的全局/字段值已经错误。
- 继续向前追到主线程 `cellA.k = Some(k)` 的 lowering：
  - 在 `scoop.effect.step.cf70b2f` 对应的 barrier call site，`CellA` 对象指针与 continuation 指针都正常；
  - 说明 continuation 写入 `CellA.k` 时值是好的，坏值在后续 GC / roots 处理阶段产生。
- 关键现场：
  - 在 `gc_before_workers` 之后、worker 线程刚进入 `workerAFn` 还没读取字段前，GDB 已显示：
    - `__scoop_object_prop__Shared.cellA` 指向的不是原 `CellA` 对象，而是一个 `ScoopString("taskA_arm")` 对象；
    - 即被弄坏的是模块级全局槽 `__scoop_object_prop__Shared.cellA` 本身，不只是其中的 `k` 字段。
- 推断结论：
  - 这不是单纯的 cross-thread continuation frame liveness 问题，而是更基础的 GC roots 合同缺口：
    - object property globals（`__scoop_object_prop__*`）
    - 同类 top-level immutable backing globals
    当前没有被 GC 作为永久 roots 稳定扫描/更新。
  - 因而显式 GC 之后，模块级 GC 指针全局值可能悬挂或错指；当前 fixture 只是最先把该问题暴露成 continuation crash。

### 决策
- 按用户规定，发现更基础且未显式跟踪的 spec/runtime blocker 后，不能继续带着该缺口实现 `T4016b4b0`。
- 已改为先在 `TODO.md` / `PLAN.md` 中新增前置任务 `T4016b4a0`：
  - 先补齐 object property / top-level immutable backing globals 的永久 GC roots 合同；
  - 再回到 `T4016b4b0` 修复 cross-thread continuation / GC stress 的剩余 runtime 问题。
- 本轮不继续做生产代码修复；完成任务重排、记录证据并提交后停止。
