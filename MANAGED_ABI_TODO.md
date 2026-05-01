# TODO（Managed ABI 与 native callable ABI 收口）

> 生成时间：2026-04-30  
> 历史归档：暂无（本轮首次建档）  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 本轮主线：按 [`MANAGED_ABI.md`](./MANAGED_ABI.md) 把 external callable surface 从“入口驱动的局部 lowering”收口为“ABI identity 驱动的统一 lowering”；先统一 native `@Extern` / `FunPtr` 的 ABI contract，再落地 `ExternAbi::Scoop` 与 `string cone` 试点。

## 全局约束

- [`MANAGED_ABI.md`](./MANAGED_ABI.md) 是本轮设计基线；若实现改变主张，必须先回写该文档，再继续实现。
- [`MANAGED_ABI_PLAN.md`](./MANAGED_ABI_PLAN.md) / 当前 `MANAGED_ABI_TODO.md` 是本轮唯一计划记录；不回写旧 round 文档。
- 本轮禁止为单个 failing fixture 写 adhoc patch。
  - 一切改动都必须收口到 callable ABI identity、native ABI-safe contract、或 `ExternAbi::Scoop` 的正式设计上。
- `FunctionType` 不得再被默认为“已知 ABI 的 callable”。
- direct `@Extern` 与 native `FunPtr` 必须共享同一套 native ABI classifier。
- `GC-free` 不是 `C ABI-safe` 的充分条件。
  - typecheck / lowering / fixtures 都必须按 ABI-safe surface 重新收口。
- native surface v1 优先 correctness 与跨平台一致性，不优先保留宽松 aggregate 支持。
- `FunPtr` 不能绕过 native ABI contract。
  - 不能出现 `@Extern` 被拒绝、但等价签名的 `FunPtr<F>` 却仍可调用的旁路。
- 每个实现任务后必须紧跟 review 任务。
- 跨平台是本轮验收的一部分。
  - 至少要求 `linux/amd64` 与 `macos/aarch64` 的 direct/indirect parity 有明确验证。
- 若任务改变 public surface / ABI 合同 / fixture 假设，必须同步 `MANAGED_ABI.md`、必要注释与相关 fixtures。

## T6001：Managed ABI 与 native callable ABI 收口

### [TODO] T6001a 固化 native surface 基线，盘清 ABI 身份丢失点与现有错误假设
- 范围：
  - 盘清当前 external callable surface 的主路径，至少覆盖：
    - direct `@Extern` declaration / call；
    - `FunPtr<F>` direct call / `.invoke(...)`；
    - `funPtrToUIntPtr` / `uintPtrToFunPtr`；
    - aggregate param / return；
    - receiver；
    - calling convention；
    - `enter_native/leave_native` 与 `gc-leaf-function`。
  - 明确当前 ABI identity 在哪一层被擦除，哪些 lowering 规则只存在于 direct extern 路径，哪些在 `FunPtr` 路径被局部重建。
  - 列出当前建立在错误 ABI 假设上的 fixtures / test helper，尤其是 native aggregate-return 相关项。
- 验收：
  - 形成一份可直接指导后续任务的 baseline：能明确回答“ABI 身份 today 丢在哪里”“native classifier 缺失在哪些入口”“哪些 fixture/helper 需要迁移”。
  - baseline 回写到 `MANAGED_ABI.md` 的相关章节，若发现新设计约束也一并补充。
- 依赖：无

### [TODO] T6001aR Review：确认 baseline 足以支撑后续 ABI 收口顺序
- 重点：
  - 是否已覆盖 direct extern、native `FunPtr`、token round-trip、aggregate/native return 四类关键热点；
  - 是否已经能支持“先 ABI identity / native classifier，再 `ExternAbi::Scoop`”的实现顺序；
  - 是否仍遗漏会直接影响跨平台 ABI 正确性的入口。
- 验收：
  - baseline 可作为 `T6001b+` 的统一前提，不需要每轮重新定位 native surface 问题。
- 依赖：T6001a

### [TODO] T6001b 建立 callable ABI identity 的内部表示，切开签名与 ABI 身份
- 范围：
  - 在前端/HIR/LLVM lowering 使用的内部表示中，明确引入 callable ABI identity。
  - 至少区分：
    - ordinary managed callable；
    - native C ABI callable；
    - managed external callable（`ExternAbi::Scoop`）。
  - 明确 `FunctionType` 只表示源码级签名，不再暗含 ABI。
  - 为 direct extern、native `FunPtr`、未来 `ExternAbi::Scoop` 共享 classifier 建立统一输入结构。
- 验收：
  - 后续 lowering 不再需要“先看到入口，再猜 ABI”；
  - 能在 declaration path 与 callsite path 使用同一个 ABI identity 源。
- 依赖：T6001aR

### [TODO] T6001bR Review：确认 ABI identity 已是一等 contract，而不是局部标签
- 重点：
  - ABI identity 是否真正独立于源码级 `FunctionType`；
  - direct/indirect/native/managed external 三类 callable 是否已可被统一建模；
  - 是否还残留靠“入口形状”推 ABI 的路径。
- 验收：
  - `T6001c/T6001d` 可直接围绕 ABI identity 推进，不再补新的旁路类型信息。
- 依赖：T6001b

### [TODO] T6001c 收紧 `ExternAbi::C` typecheck 合同，从 GC-free 升级为 ABI-safe native value surface
- 范围：
  - 把 `@Extern` 的 native surface typecheck 从 “GC-free” 收紧为“ABI-safe”。
  - 明确 v1 允许集，至少覆盖：
    - 标量整数/布尔/字符/浮点；
    - `UIntPtr`；
    - `Ptr<T>`；
    - `@CLayout` 且已证明 ABI-safe 的值类型。
  - 明确 v1 禁止集，至少覆盖：
    - tuple；
    - 普通 nominal value type；
    - enum；
    - 需要 hidden sret 的 aggregate。
  - 同步把 native `FunPtr<F>` 纳入同一 ABI-safe 约束，不允许 `FunPtr` 侧门绕过 `@Extern` 限制。
- 验收：
  - typecheck 能稳定拒绝“GC-free 但非 ABI-safe”的 native surface；
  - direct extern 与 native `FunPtr` 在 surface 约束上保持一致。
- 依赖：T6001bR

### [TODO] T6001cR Review：确认 native surface 的门禁已经从“GC-free 近似”收口到“ABI-safe contract”
- 重点：
  - 是否仍有 tuple / enum / 普通 nominal value type 通过 native surface 混入；
  - `FunPtr<F>` 是否仍保留绕过 `@Extern` typecheck 的路径；
  - 诊断文案是否清楚表达“为什么不能过 C ABI”。
- 验收：
  - `T6001d` 可以在已收紧的 surface 上建立统一 classifier，而不是继续为非法形状兜底。
- 依赖：T6001c

### [TODO] T6001d 建立单一 native ABI classifier，统一 declaration / direct call / indirect call lowering
- 范围：
  - 建立 single-source native ABI classifier，并让以下入口统一复用：
    - direct `@Extern` declaration；
    - direct `@Extern` call；
    - native `FunPtr` indirect call；
    - `.invoke(...)` lowering。
  - classifier 必须整体决定：
    - 参数 lowering；
    - receiver lowering；
    - 返回值 lowering；
    - aggregate return 是 direct 还是 indirect；
    - calling convention；
    - `enter_native/leave_native`；
    - `gc-leaf-function`。
  - 清理当前“参数 native、返回 ordinary”“direct/indirect 分别决定 hidden sret”的分裂实现。
- 验收：
  - 不再出现 direct extern 与 native funptr 对同一目标签名产生不同 ABI lowering；
  - `macos/aarch64` 与 `linux/amd64` 的差异只会体现在 classifier 的目标平台结果，而不会体现在入口分裂。
- 依赖：T6001cR

### [TODO] T6001dR Review：确认 native ABI 已按 classifier 收口，而不是入口驱动
- 重点：
  - declaration path 与 callsite path 是否共享同一套 native ABI 决策；
  - direct extern 与 native `FunPtr` 是否仍有局部特判分流；
  - aggregate/native return 是否已不再偷套 ordinary hidden sret。
- 验收：
  - native surface 的 lowering 已可按 ABI family 解释，而不是按入口形状解释。
- 依赖：T6001d

### [TODO] T6001e 迁移错误 native 假设的 fixtures / test helper，并建立 direct/indirect parity 回归
- 范围：
  - 清理当前把 ordinary hidden sret helper 当作 native aggregate return 验收依据的测试方式。
  - 对不符合 v1 `ExternAbi::C` contract 的 fixtures，明确迁移策略：
    - 改成 `@CLayout` / `Ptr<T>` / `UIntPtr` 形式；
    - 或改为未来 `ExternAbi::Scoop` / managed external 场景；
    - 或删除错误前提的测试并用 parity 回归替代。
  - 建立 native surface parity 回归，至少覆盖：
    - direct extern vs 取地址后的 `FunPtr`；
    - token round-trip；
    - `linux/amd64` 与 `macos/aarch64`。
- 验收：
  - native surface 的测试 helper 尽量贴近真实 C ABI；
  - direct/indirect parity 不再依赖错误的 hidden sret 假设；
  - 现有唯一失败项对应的问题被系统性消化，而不是以局部修补保留。
- 依赖：T6001dR

### [TODO] T6001eR Review：确认测试与 helper 已反映真实 native contract
- 重点：
  - 是否还残留用 ordinary ABI 冒充 native ABI 的测试 helper；
  - native surface 回归是否已经覆盖 direct/indirect parity 与平台矩阵；
  - 是否仍保留与 v1 `ExternAbi::C` 设计冲突的 fixture 前提。
- 验收：
  - native surface 可以在跨平台矩阵下稳定作为后续重构的基线。
- 依赖：T6001e

### [TODO] T6001f 扩展前端/HIR，正式引入 `ExternAbi::Scoop` 声明路径
- 范围：
  - 扩展 `ExternAbi`、annotation lowering、HIR side table 与相关诊断，支持 `ExternAbi::Scoop`。
  - 建立 `ExternAbi::Scoop` v1 declaration contract：
    - external linkage；
    - ordinary param ABI；
    - ordinary return / hidden sret；
    - 不插 `enter_native/leave_native`；
    - 不标记 `gc-leaf-function`；
    - 初版只支持 `Pure`、顶层函数、非泛型、无 closure/function-value crossing。
- 验收：
  - `ExternAbi::Scoop` 已能作为独立 ABI family 稳定进入 lowering side table；
  - 不会与现有 `ExternAbi::C` 语义混淆。
- 依赖：T6001eR

### [TODO] T6001fR Review：确认 `ExternAbi::Scoop` 的前端表示边界清晰
- 重点：
  - `ExternAbi::Scoop` 是否只是“多一个名字”，还是已经具备独立 ABI 身份；
  - annotation / HIR / 诊断是否与 `ExternAbi::C` 明确区分；
  - v1 限制是否已被门禁锁定。
- 验收：
  - `T6001g` 可在不引入语义混乱的前提下接 call lowering。
- 依赖：T6001f

### [TODO] T6001g 接通 `ExternAbi::Scoop` 的 call lowering，走 ordinary managed call 主线
- 范围：
  - 把 imported `ExternAbi::Scoop` 调用接到 ordinary managed call lowering：
    - ordinary param ABI；
    - ordinary return / hidden sret；
    - conservative root spill；
    - statepoint / safepoint correctness；
    - 不走 native leaf path。
  - 补齐 declaration/callsite 的 IR 回归，锁定：
    - 无 `enter_native/leave_native`；
    - aggregate return 可 hidden sret；
    - GC roots 在调用点前后正确保留。
- 验收：
  - imported `ExternAbi::Scoop` 顶层函数已能像 ordinary managed helper 一样正确工作；
  - external linkage 下的 managed helper 不再需要名字驱动 special-case。
- 依赖：T6001fR

### [TODO] T6001gR Review：确认 managed external call 已与 native surface 正式分流
- 重点：
  - `ExternAbi::Scoop` 是否真正走 ordinary managed call 主线；
  - 是否还残留被当作 native leaf 的 managed external path；
  - declaration / callsite / runtime 合同是否一致。
- 验收：
  - `string cone` 可在此基础上作为首个试点落地。
- 依赖：T6001g

### [TODO] T6001h 用 `string cone` 做 tracer bullet，验证编译器已按 ABI 而不是按 helper 名字工作
- 范围：
  - 先选择一个最小 string helper 作为 `ExternAbi::Scoop` tracer bullet，建议从 `Int.toString` 开始。
  - 再扩展到 `Bool/Char/Float.toString` 与少量 `String` helper。
  - 清理与这些 helper 相关的 FQN-based compiler special-case，并用 ABI-driven declaration/call path 替代。
- 验收：
  - `string cone` 试点可运行；
  - 对应 helper 不再依赖 compiler dispatch 中的名字 special-case；
  - IR 与语义回归都能证明它们是经 `ExternAbi::Scoop` 进入 ordinary managed call 主线。
- 依赖：T6001gR

### [TODO] T6001hR Review：确认试点已经证明 ABI 驱动架构成立
- 重点：
  - helper 是否已按 ABI，而不是按 FQN 被 special-case；
  - runtime 是否只保留真正 substrate；
  - `string cone` 是否足以作为后续领域迁移模板。
- 验收：
  - 本轮可以明确宣称：Managed ABI 与 native callable ABI 的设计已从文档进入实现主线。
- 依赖：T6001h

### [TODO] T6001i 做全量稳定化与跨平台验收，收尾文档与迁移说明
- 范围：
  - 全量回归至少覆盖：
    - `cargo test --all`；
    - `cargo run -p scoop -- test`；
    - native surface direct/indirect parity；
    - `linux/amd64` 与 `macos/aarch64`；
    - `ExternAbi::Scoop` IR 与语义回归。
  - 收尾更新：
    - `MANAGED_ABI.md`；
    - `SCOOP_RUNTIME.md` 中相关 ABI 约束；
    - fixture / helper 注释；
    - 必要迁移说明。
- 验收：
  - native surface 与 managed external surface 已有清晰、可回归、跨平台的 contract；
  - 本轮所有主张都已在设计文档与回归中闭环。
- 依赖：T6001hR

### [TODO] T6001iR Review：确认本轮收口状态与后续扩展边界
- 重点：
  - callable ABI identity 是否已真正成为 source of truth；
  - native surface 与 managed external surface 是否已清晰分层；
  - 后续 v2+（managed function pointer、generics、outward effect ABI）是否已拥有稳定前提。
- 验收：
  - 可以在不回退本轮 contract 的前提下继续推进 v2+ 扩展。
- 依赖：T6001i
