# P-CG：codegen 直接消费 LIR（去 MIR + 去 FQN + never-fail）

> 设计依据：[`FACT_REFACTOR.md`](./FACT_REFACTOR.md) §1.7/§1.8（责任划分、下游 never-fail）、§13（LIR）、§2.7（句柄）。
> 前序：P1（handoff）见 `PLAN-1.md`；P2（LIR 内部重设计：P2a 身份 / P2b 折叠 facts 已完成，P2c lift/overlay 未完）见 `PLAN-3.md`。**本阶段 subsume P2 未完的 T2-08/T2-09**。

## 0. 为什么单列、为什么是一组

调研确认 codegen 是全管线最脏的一层（HEAD：`panic!` 583 / `.expect(` 491 / `.unwrap()` 232；`_fqn` 1283；整个 `mir_body/` 子树直接 walk `mir::Body`）。且 **codegen-walk-LIR、LIR-lift-完成、删-overlay 三者互相依赖**——不能分开做（上次 agent 试图分开，结果造了 LIR→MIR 反向 shim + 句柄转回 FQN + 满地 `Result`/`panic`，已打回）。故合并为一个阶段，端到端完成「LIR 指令落地 → codegen 消费 LIR → 删 overlay」。

## 1. 现状（post-revert，HEAD 49639d4a）

- **两条 body 发射路径**（`body/main_entry.rs:codegen_program_bodies` 按 `plain_abi().is_some()` 分流）：
  - **plain 路径** `mir_body/`：直接 walk 原始 `mir::Body.blocks[].stmts[]`（`main_entry.rs:591-623`），不消费 LIR（除 FQN 查找）。
  - **effect-step 路径** `effect_lowered/body/`：已 walk LIR `state_graph().states()`（StateId 句柄），但**语句仍经 source-slice 切回 MIR**（`lower_source.rs`/`value.rs`）。
- **身份**：~1283 处 `_fqn`——callee/符号/布局/路由都按 FQN 字符串查（`callable_lookup.rs`、`identity.rs`、`call/lowering.rs`）。
- **overlay 仍在**：`LateLoweredSourceBody = crate::mir::Body`、`LateLoweredStateSlice`；`LirArtifact.mir` 过渡字段。
- **错误**：`LlvmEmitError` 约 60-70% 是「输入失败」变体（`Frontend`/`MissingEntryMain`/`EffectLoweringUnsupported`/`BackendGate`/`AmbiguousEntryMain`/`InvalidLiteral`），其余为真后端（`Target`/`Builder`/`Instruction`/`ModuleVerification`/`RunPasses`）+ IO（`Write*`）。
- T2-07/T2-08A 已提交：LIR 指令**类型**与 executable body 容器已存在；但 lift 未把指令填满、codegen 未消费。

## 2. 目标（完成标志）

- codegen 两条路径都 **walk LIR 指令/句柄**，`mir_body/` 不再 match `mir::Statement/Rvalue/Terminator/Operand`；`effect_lowered/body` 语句来自 LIR 而非 MIR slice。
- callee/符号/布局引用全用 `LirCallableId`/`NominalId` 句柄，不再按 FQN 字符串查。
- 删除 `LateLoweredSourceBody`/`LateLoweredStateSlice`/`LirArtifact.mir`；`LateLoweredProgram` 不再引用 `crate::mir::Body`。
- `LlvmEmitError` 只剩真后端 + IO 变体；**输入失败变体全部上移**（§1.8）；codegen 对输入 never-fail，残留 `panic!`/`expect` 仅为引用闭合下**结构不可达**的 `unreachable!`/`debug_assert`。

## 3. 子阶段（一组任务，按序；详见 `TODO.md`）

- **TC-01 lift 落地（producer）**：LIR lift 全函数（§1.8），为**所有** callable body（plain + effect-step）填满 LIR 指令；拒绝占位的 guard 上移到 MIR 输出。
- **TC-02 plain 路径消费 LIR**：`mir_body/` 改 walk LIR 指令，去 `mir::*` match。
- **TC-03 effect 路径语句消费 LIR**：`lower_source.rs`/`value.rs` 语句来自 LIR，不再 MIR slice。
- **TC-04 FQN→句柄**：callee/符号/布局查找改 `LirCallableId`/`NominalId`。
- **TC-05 删 overlay**：删 `LateLoweredSourceBody`/`LateLoweredStateSlice`/`LirArtifact.mir`。
- **TC-06 never-fail 收口**：上移输入失败错误变体；panic/expect on input → 结构不可达。

## 4. 硬纪律（贯穿，违反即 review 打回）

§1.8：下游对输入 never-fail——**无 `Result` 输入错误出口、无 `Todo`/`Unsupported`/escape 变体、无 `_ => {}` no-op 容忍、无句柄→FQN 反转、无 LIR→MIR 反向 shim**。任何「失败可能」上移为上游输出保证。绝不留 placeholder。

## 5. 验证基线（每任务收尾）

```
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test --all --all-targets
cargo build -p scoop -p scoopc
python3 tools/dependency_gate.py
python3 tools/spec_fixtures.py check
python3 tools/run_fixtures.py
```
