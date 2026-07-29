# Effect Lowering：MIR 内本地状态机方案

> 状态：实现计划
> 目标：在 scoop2_mir 内实现完整的 effect lowering，把 Perform/Handle/Resume 消除为状态机 + Step tagged union + continuation 对象。
> 完成后，产出的 MIR IR 中不再有任何 effect 相关的控制流结构（Perform/Handle 终结符），effect row 仅作为诊断元数据保留。

## 0. 前提与约束

### 0.1 规范变更（本次一并实现）

- **二次 resume 直接 panic**：第二次调用 `k.resume(...)` 不再 perform `Raise<RuntimeError>`，而是直接 panic（终止性，返回 `Nothing`，不可恢复）。
- 需同步更新：
  - sysroot `Continuation.resume` 签名：`/ (E + Raise<RuntimeError>)` → `/ E`
  - HIR typecheck `record_continuation_resume_effects`：移除无条件 `Raise` 记录
  - SCOOP_FULL_SPEC.md §5.5 相关描述

### 0.2 运行时约束

- C runtime 不提供任何 effect 分发/handle/resume 函数
- 唯一可用的 runtime 入口：`scoop_panic`（`@Extern(name = "scoop_panic")`），用于二次 resume 的终止
- 所有 effect 语义由编译器生成的状态机代码实现
- **语言中无异常/unwinding 概念**：try/catch 已 desugar 为 handle/on，finally 是普通的顺序控制流（在退出 handle 前执行），不是 unwinding cleanup

### 0.3 现状诊断

当前 scoop2_mir 的 effect 处理有三个断层（这不是"改造"而是"从占位到真实"）：
1. **Perform→Handle 无连接**：`Perform` 跳到 `resume_target`，但没有机制把 perform 路由到 handler arm
2. **`arms: Vec::new()`**：`Handle` 终结符的 arms 字段始终为空
3. **resume 是普通 interface 调用**：`k.resume(v)` 走 `CallKind::Interface`，无 MIR 级 resume 结构
4. **`ResumeMetadata` / `CallKind::Resume` 不存在于 scoop2_mir**（仅在旧管线 scoopc_mir 中）

### 0.4 需要处理的所有代码形状

**Arm 形状（2 种）：**
- Non-resuming `E.op(args) -> expr`：不绑定 continuation，body 被放弃
- Resuming `E.op(args), k -> expr`：绑定 `Continuation<Resume, Answer, eff E>`，可 `k.resume(v)`

**Perform 场景：**
- 任意用户自定义 effect 操作（不止 Raise）
- 一个 handler body 可含多个 perform
- 无匹配 arm 的 perform 向外层传播

**Resume 场景：**
- `k.resume(payload)` 单值/Unit/tuple
- 二次 resume → panic（新规范）
- resume 后的计算可再次 perform → 生成新 continuation

**Handle 结构：**
- body + 零或多个 arm + 可选 finally
- finally 在正常完成、arm 触发时都运行（语言中无 unwinding/异常概念）
- 嵌套 handle：内层未匹配的 perform 传播到外层

---

## 1. 总体架构

### 1.1 管线位置

```
lower (AST→MIR，产出含 Perform/Handle 的 generic Module)
  → materialize (BFS 单态化)
    → devirtualize
    → [新增] effect_lower (本方案)
    → compute_public_stable_keys
```

**关键决策**：effect lowering 放在 devirtualize 和 inline **之后**。
- devirt 之后：devirt 产生的 Direct 调用更利于 effect lowering 分析
- **inline 之后（核心）**：inline 会把 effectful HOF（如 `forEach`/`map`/`filter`）内联到调用者。这些 HOF 本身只是传递闭包参数的 effect（effect-transparent），内联后调用者直接执行闭包 body，很多情况下消除了 effect 边界，不再需要状态机。如果 effect lowering 在 inline 之前运行，这些 HOF 会先生成状态机（ABI 完全改变），inline 就无法再内联它们。
- inline 的门控需要放宽：当前 inline 拒绝含 `Perform`/`Handle` 终结符的函数（`is_safe_terminator`）。但 effect-transparent HOF（如 `forEach`）体内含 Perform 但其 effect 只是从闭包参数转发——这类函数**必须能在 effect lowering 之前被内联**。需要修改 `is_safe_terminator`：允许内联含 Perform/Handle 的 effect-transparent 函数（多块 + effect 转发检测已实现），而不是一律拒绝。

### 1.2 函数 ABI 分类

lowering 后，函数分为两种 ABI：
- **Plain**：effect_row 为 Pure 或 Pure-compatible（无未捕获 perform）。保持普通 ABI `(args) -> R`。
- **EffectStep**：含有未本地捕获的 perform。ABI 变为 `step(frame, resume_payload?) -> Step`。

判断标准：函数体内是否存在未被本地 Handle 捕获的 Perform。

### 1.3 dump-mir 可见性

当前 `dump-mir` 输出 generic 模板模块（`lower_result.module`），不显示 materialize 后的结果。
effect lowering 在 materialize 内部运行，因此 **dump-mir 不可见**。
需要新增 dump 选项或在 verify 阶段检查 lowering 产物。

---

## 2. 核心数据结构

### 2.1 合成类型

effect lowering 需要在 TypeStore 中引入合成类型（synthetic nominal）：

**Frame 结构体**（每个 effectful 函数一个）：
```
FQN: "<owner_fqn>$frame"
TypeKind::Value(Nominal { fqn, args: [], eff: None })
字段：所有跨挂起点存活的 locals + state(u32) + result(AnswerT)
```

**Step tagged union**（每个 effectful 函数一个）：
```
FQN: "<owner_fqn>$step"
TypeKind::Value(Nominal { fqn, args: [], eff: None })
变体：
  - Complete(AnswerT)
  - <EffectOp>(payload, continuation)  // 每个未捕获的 effect 操作一个变体
```

**Continuation 对象**（每个 resuming arm 的 perform 点一个）：
```
FQN: "<owner_fqn>$cont<EffectOp>"
TypeKind::Ref(Nominal { fqn, args: [ResumeT, AnswerT], eff: Some(E) })
字段：frame_ptr, resume_state(u32), resume_fn(fn ptr), resumed(bool)
```

这些合成类型需要：
- 在 TypeStore 中通过 `value_nominal` / `ref_nominal` 构造
- 发出对应的 `Item::Metadata(MetadataRoot { kind: Struct/Enum })` 供后端 layout

### 2.2 FunDecl 扩展

在 `FunDecl` 新增字段：
```rust
/// 是否为 effect step 函数（ABI: step(frame, resume?) -> Step）。
/// None = Plain 函数（普通 ABI）。
/// Some = EffectStep 函数，携带 step 类型信息和 frame schema。
pub effect_abi: Option<EffectStepAbi>,
```

```rust
pub struct EffectStepAbi {
    /// Step tagged union 的 TypeId
    pub step_ty: TypeId,
    /// Frame 结构体的 TypeId
    pub frame_ty: TypeId,
    /// 所有 outward case（未捕获的 effect 操作 FQN）
    pub outward_cases: Vec<String>,
    /// resume 入口状态编号列表（每个 Perform 的 resume_target 对应一个）
    pub resume_states: Vec<u32>,
}
```

### 2.3 调用点 ABI 元数据

利用已有的 `CallAbiHandoffMetadata`（transport.rs:249-255）scaffolding：
- Plain→Plain 调用：保持 `plain_no_outward()`
- Plain→EffectStep 调用：需构造 frame + 调用 step 函数 + 处理返回的 Step
- EffectStep 内部的调用：按 callee 的 ABI 处理

新增构造器 `CallAbiHandoffMetadata::effect_step(cases)`。

---

## 3. 实现步骤

### 步骤 0a：移除 unwind 机制（前置清理）

**背景**：语言中没有异常，try/catch 已 desugar 为 handle/on，不存在 unwinding 概念。当前 scoop2_mir 中的 unwind 机制完全是死代码：
- `is_cleanup` 从未设为 true
- `ResumeUnwind` 从未发射
- cleanup scope 虽有 push/pop，但产出的 `Cleanup{target}` 会被 verify 拒绝（目标块不是 cleanup 块）
- finally 实际上只是顺序的 goto 块

**移除内容**：
- `UnwindAction` 枚举（mod.rs:695-706）及 `Terminator.unwind` 字段（mod.rs:613）
- `BasicBlock.is_cleanup` 字段（mod.rs:261）
- `TerminatorKind::ResumeUnwind`（mod.rs:660-661）
- `FnLowering.cleanup_scopes` 栈及相关方法（builder.rs:35-37, 61-73, 112-140）
- `build_unwind()` / `enter_cleanup_scope()` / `exit_cleanup_scope()`
- verify.rs 中 cleanup 目标检查（verify.rs:312-328）
- dump.rs 中 unwind/is_cleanup 渲染
- inline.rs 中 unwind/is_cleanup 拷贝
- 所有 `build_unwind()` 调用点（stmt.rs:37, expr.rs:681, expr.rs:1823, builder.rs:725）——直接移除
- 所有 `UnwindAction::NoUnwind` / `Propagate` 字面量——移除

**影响范围**：~30 处引用，但全部是机械删除，不改变任何实际 CFG。
finally 块保持顺序 goto 语义（本就如此）。

**验证**：所有现有测试通过（无行为变化）。

### 步骤 0b：放宽 inline 门控，允许 effect-transparent HOF 内联

**背景**：effect lowering 放在 inline **之后**，目的是让 effect-transparent HOF（如 `forEach`/`map`/`filter`）在内联后才做状态机转换。但当前 inline 的 `is_safe_terminator`（inline.rs:227）拒绝任何含 `Perform`/`Handle` 终结符的函数。effect-transparent HOF 体内含 Perform（转发闭包参数的 effect），所以会被拒绝内联——这就失去了 inline-before-effect-lowering 的意义。

**改动**：
- `is_safe_terminator`：对 effect-transparent HOF（`is_effect_transparent` 返回 true），允许含 `Perform` 终结符的函数被内联。
  - Perform 终结符在多块内联时需要正确处理：把 `Perform { resume_target, .. }` 的 `resume_target` 通过 block_map 重定向（与 Goto/CondBr 同理）。
  - Handle 终结符仍需谨慎：如果 handle 的 dispatch 逻辑在 inline 时不做特殊处理，可能需要继续拒绝（或做完整的多块+dispatch 重定向）。保守策略：先只允许 Perform（effect-transparent HOF 的典型形态），Handle 暂不允许。
- `rename_terminator`（inline.rs）：新增 `Perform` 分支，重命名 `resume_target`（通过 block_map 重定向）。

**文件**：`crates/scoop2_mir/src/mir/inline.rs`

**验证**：effect-transparent HOF（如 `forEach`）可被内联到调用者；内联后调用者的 Perform 直接出现在调用者体内。

### 步骤 1：规范同步（sysroot + HIR typecheck）

**文件**：
- `sysroot/lib/scoop.core/src/core.scoop:1479-1481`：`Continuation.resume` 签名 `/ (E + Raise<RuntimeError>)` → `/ E`
- `crates/scoop2_hir/src/typecheck/expr.rs` `record_continuation_resume_effects`（~4149-4211）：移除无条件 `Raise` 记录；只记录 continuation 的 `eff E`
- `SCOOP_FULL_SPEC.md` §5.5：更新二次 resume 语义

**验证**：HIR typecheck 测试通过；现有 effect fixture 的 effect row 推断结果正确（resume 不再加 Raise）。

### 步骤 2：填充 Handle arms 契约

**文件**：`crates/scoop2_mir/src/mir/lower/expr.rs` `lower_handle`（~2137-2215）

当前 `arms: Vec::new()`。改为为每个 arm 构建 `HandlerArm`：
- `op_fqn`：从 arm 的 HandleOp 构建（`{effect_path}.{op}`）
- `binder_locals`：arm 的 binder 对应的 LocalId 列表
- `continuation_local`：resuming arm 的 `escape_continuation` binder 对应的 LocalId（None for non-resuming）
- `handled_effect_ty`：effect 类型 TypeId
- `payload_component_tys`：binder 类型列表
- `body_ty`：arm body 的类型
- `kind`：`NonResuming` 或 `EscapeContinuation`

**验证**：dump-mir 的 Handle 终结符显示 arms 信息。

### 步骤 3：识别 resume 调用 + 添加 CallKind::Resume

**文件**：`crates/scoop2_mir/src/mir/mod.rs` + `lower/expr.rs`

当前 `k.resume(v)` 走 `CallKind::Interface`。需要：
- 在 `CallKind` 新增 `Resume { continuation: Operand, resume_value: Operand, metadata: ResumeMetadata }`
- 在 lowering 中识别 `Continuation.resume` 调用（检查 owner_fqn == Continuation interface）并发射 `CallKind::Resume`
- 填充 `ResumeMetadata`（已有结构定义在 transport.rs:338，当前未使用）

**验证**：dump-mir 显示 Resume 调用 kind。

### 步骤 4：实现 liveness 分析

**新文件**：`crates/scoop2_mir/src/mir/effect_lower/liveness.rs`

对每个含 Perform 的函数体，计算每个挂起点的 live-out locals 集合：
- 挂起点 = Perform / Resume / 可能传播 effect 的 Call
- 对每个挂起点 P，live-out(P) = 从 P 的 resume_target 可达的所有块中，在 P 之前定义且在 resume_target 之后使用的 locals
- 这些 locals 需要保存到 frame

算法：标准 backward dataflow liveness analysis（每个基本块计算 use/def 集合，反向不动点迭代）。

### 步骤 5：实现状态分割

**新文件**：`crates/scoop2_mir/src/mir/effect_lower/segment.rs`

遍历函数体 CFG，在挂起点切割：
1. 从 entry 开始 BFS/DFS
2. 遇到 Perform/Resume/传播-Call 时，当前段结束（成为状态 S_i），resume_target 开始新段（成为状态 S_{i+1}）
3. 遇到 Handle 时，记录 handle 的 dispatch 信息，body/arm/finally/exit 各成为独立状态组
4. 为每个状态分配编号

输出：`StateMap { block_id → state_id, state_list: Vec<State> }`

### 步骤 6：实现 Frame/Step/Continuation 类型生成

**新文件**：`crates/scoop2_mir/src/mir/effect_lower/types.rs`

为每个 EffectStep 函数生成：
- Frame 结构体类型（含 state + result + 所有 live-out locals）
- Step tagged union 类型（含 Complete + 每个 outward case）
- Continuation 对象类型（每个 resuming perform 点）
- 对应的 `Item::Metadata` 声明

### 步骤 7：实现 Perform 重写

**新文件**：`crates/scoop2_mir/src/mir/effect_lower/lower_perform.rs`

每个 `Perform { op_fqn, args, resume_local, resume_target }` 重写为：
1. 保存 live-out locals 到 frame（`StoreMember` 到 frame 的对应字段）
2. 设置 `frame.state = <resume_target 的状态编号>`
3. 构造 Step payload（op 的参数）
4. 构造 continuation 对象（如果是 resuming handle 的 perform）
5. `return Step::<op_fqn>(payload, continuation)`

注意：lowering 后的 Perform 变为一个块的终结符——返回 Step（通过 `Return` 终结符返回 Step 值）。

### 步骤 8：实现 Handle dispatch 重写

**新文件**：`crates/scoop2_mir/src/mir/effect_lower/lower_handle.rs`

每个 `Handle { body_target, arm_targets, finally_target, exit_target, arms }` 重写为 dispatch 逻辑：

1. **进入 body**：从 body_target 开始执行（goto body_target）。
2. **Perform 被此 handle 匹配**（op_fqn 匹配某 arm）：
   - 当 Perform 重写为返回 Step 时，如果该 perform 在此 handle 的 body 内且匹配某 arm，则不返回 Step，而是跳到 arm：
     - Non-resuming arm：丢弃 continuation，执行 arm body，goto finally/exit
     - Resuming arm：绑定 continuation 对象到 `continuation_local`，执行 arm body
3. **Perform 不匹配任何 arm**：Perform 正常返回 Step（向上传播）
4. **body 正常完成**：goto finally（若有）→ goto exit
5. **arm 正常完成**：goto finally（若有）→ goto exit

实现方式：状态分割时，每个 handle 的 body/arm/finally/exit 被分配到不同的状态组。Perform 的 Step 返回被拦截：如果 perform 在某 handle 的 body 内且匹配该 handle 的 arm，则跳到 arm 状态而非返回 Step。

### 步骤 9：实现 Resume 重写

**新文件**：`crates/scoop2_mir/src/mir/effect_lower/lower_resume.rs`

每个 `CallKind::Resume { continuation, resume_value, metadata }` 重写为：
1. 检查 `continuation.resumed`：若 true → `StatementKind::Panic { message: "ContinuationAlreadyResumed" }`（终止性）
2. 设置 `continuation.resumed = true`
3. 调用 `continuation.resume_fn(continuation.frame, resume_value)` → 得到 Step
4. 直接传播返回的 Step（`return step`）——resume 后的 effects 通过 escape 已体现在函数 ABI 中

### 步骤 10：实现 Finally 重写

**文件**：`crates/scoop2_mir/src/mir/effect_lower/lower_handle.rs`（与步骤 8 集成）

finally 块是一段在退出 handle 前必须执行的控制流（不是 unwinding cleanup——语言中无异常/unwinding 概念）。通过 frame 中的 `pending_completion` 字段路由退出路径：
- 添加 frame 字段 `pending_completion: u32`（枚举：Normal=0, ArmResult=1, PropagateStep=2）
- body/arm 正常完成 → 设置 pending_completion → goto finally → finally 结束后按 pending_completion 路由到正确出口
- PropagateStep 时还需保存 pending Step 到 frame
- finally 本身的 performs 按正常 effect 规则处理（在 handle 的 effect 上下文中）

### 步骤 11：实现 step 入口函数构造

**新文件**：`crates/scoop2_mir/src/mir/effect_lower/step_entry.rs`

为每个 EffectStep 函数生成 step 入口函数 `f$step`：
1. 参数：`frame: &mut Frame_f`, `resume_payload: Option<ResumeT>`（首次调用为 None）
2. 函数体：`match frame.state { ... }`（用 CondBr 链实现 switch）
3. 每个状态对应一个基本块组
4. 状态 0 = 初始入口（执行原函数体的状态分割结果）
5. 状态 N = 第 N 个 Perform 的 resume 续点

原函数 `f` 变为 wrapper：构造 frame + 调用 `f$step` + 处理返回的 Step。

### 步骤 12：实现普通调用适配

**文件**：`crates/scoop2_mir/src/mir/effect_lower/lower_call.rs`

当 Plain 函数调用 EffectStep 函数时：
1. 构造 frame
2. 调用 step 函数
3. 收到 Step：
   - Complete → 提取结果，继续执行
   - <EffectOp> → 需要处理：如果调用者有匹配的 handle → 处理；否则传播（调用者本身也变成 EffectStep）

当 EffectStep 函数内部调用其他函数时：
- Plain callee：普通调用
- EffectStep callee：构造 frame + 调用 step + 处理 Step（匹配则处理，不匹配则包装为自己的 Step 向上传播）

### 步骤 13：主 pass 编排

**新文件**：`crates/scoop2_mir/src/mir/effect_lower/mod.rs`

```rust
pub fn lower_effects(module: &mut Module, interner: &Interner) {
    // 1. 分类每个函数：Plain vs EffectStep
    // 2. 对每个 EffectStep 函数：
    //    a. liveness 分析
    //    b. 状态分割
    //    c. 生成 Frame/Step/Continuation 类型
    //    d. 重写 Perform/Handle/Resume
    //    e. 构造 step 入口函数
    // 3. 适配所有调用点
    // 4. 验证：无 Perform/Handle/Resume 残留
}
```

接入 materialize pipeline（`materialize/mod.rs`，devirtualize 之后、inline 之前）。

### 步骤 14：清理 vestigial 结构

effect lowering 完成后：
- `TerminatorKind::Perform` / `TerminatorKind::Handle` 不再出现在 IR 中（可保留枚举变体但添加文档标注"仅 lowering 前"）
- `Rvalue::PerformResult` 移除（当前已是死代码）
- `ResumeMetadata` 被 lowering 消费后可清理
- `HandlerArm` 被 lowering 消费后可清理
- effect_row 保留在 FunDecl 中作诊断元数据

### 步骤 15：验证 + fixture

**验证**：
- `verify` 新增检查：effect lowering 后的模块不含 Perform/Handle/Resume 终结符
- effect lowering 后的模块通过 `verify_materialized_with_external`

**Fixture 检查与新增**：
- 检查现有 `tests/fixtures/mir2/effect_handle.scoop`（唯一的新管线 effect fixture）：确认 lowering 后的 MIR 正确
- 新增 fixture 覆盖所有形状：
  - `mir2/effect_non_resuming.scoop`：non-resuming arm（现有 effect_handle 即此类型）
  - `mir2/effect_resuming.scoop`：resuming arm + resume
  - `mir2/effect_nested.scoop`：嵌套 handle
  - `mir2/effect_finally.scoop`：handle with finally
  - `mir2/effect_propagate.scoop`：无匹配 arm 传播
  - `mir2/effect_multi_perform.scoop`：多个 perform 在一个 body
  - `mir2/effect_resume_panic.scoop`：二次 resume → panic（新规范）
- 参考 `tests/fixtures/effect_lowered/`（旧管线 10 个 fixture）的源码形状，但 golden 对应新管线输出
- **注意**：旧管线 fixture 的 golden 不可直接复用（IR 不同），但源码形状可参考

---

## 4. 实现顺序与依赖

```
步骤 0a（移除 unwind 机制）   ← 无依赖，可先做（纯死代码清理）
步骤 0b（放宽 inline 门控）   ← 依赖步骤 0a（unwind 移除后终结符结构更简单）
    ↓
步骤 1（规范同步）           ← 无依赖，可与步骤 0a/0b 并行
    ↓
步骤 2（填充 arms 契约）      ← 依赖步骤 1（effect row 正确）
步骤 3（识别 resume + CallKind::Resume）  ← 依赖步骤 1
    ↓
步骤 4（liveness 分析）       ← 无依赖（可与 2、3 并行）
步骤 5（状态分割）            ← 依赖步骤 2、3（需要完整的 Perform/Handle/Resume 信息）
    ↓
步骤 6（类型生成）            ← 依赖步骤 5（需要状态分割结果 + liveness）
步骤 7（Perform 重写）        ← 依赖步骤 4、5、6
步骤 8（Handle dispatch）     ← 依赖步骤 5、6、7
步骤 9（Resume 重写）         ← 依赖步骤 6
步骤 10（Finally 重写）       ← 依赖步骤 8
    ↓
步骤 11（step 入口函数）      ← 依赖步骤 7-10
步骤 12（调用适配）           ← 依赖步骤 11
    ↓
步骤 13（主 pass 编排）       ← 依赖步骤 4-12
步骤 14（清理）               ← 依赖步骤 13
步骤 15（验证 + fixture）     ← 依赖步骤 13、14
```

## 5. 风险与注意事项

1. **合成类型的 layout**：Frame/Step/Continuation 是合成 nominal，无 HIR 背景。后端 codegen 需要能处理这些类型。需确认 TypeStore + MetadataRoot 机制足够。

2. **递归 effect 函数**：如果函数递归调用自身且自身是 EffectStep，step 函数的 frame 需要独立分配（不能栈上）。简单方案：所有 frame 堆分配。

3. **跨函数 continuation**：continuation 对象可能跨函数传递（如 fixture `continuation_resume_runtime_error_boundary`）。resume_fn 通过函数指针调用，frame 指针跨函数有效（堆分配）。

4. **effect row 与实际 perform 的一致性**：函数声明的 effect_row 可能与实际 perform 不完全一致（声明是上界）。Step 的 outward case 应基于实际 perform，而非声明。

5. **dump-mir 不可见性**：effect lowering 在 materialize 内部，dump-mir 输出 generic 模板。需要新增 `dump-effect-lowered` 命令或在 materialize 后输出调试 dump。

6. **Fixture 正确性**：用户明确提醒"fixture 的正确性不能完全保证"。实现时需对每个 fixture 的源码做语义分析，不能盲信 golden。
