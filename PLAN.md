# P2：LIR 内部重设计（自包含 `LirProgram`）

> 设计依据：[`FACT_REFACTOR.md`](./FACT_REFACTOR.md) §13（新 LIR 草案）、§2.7（统一身份模式）、§14（P1 已建 handoff）。
> 上一阶段：P1（LIR↔codegen handoff）已完成 → [`PLAN-1.md`](./PLAN-1.md) / [`TODO-1.md`](./TODO-1.md)。
> 总路线（后→前）：P1✓ → **P2（本阶段）** → P3 effect-lowering/facts → P4 MIR → P5 HIR → P6 跨 cone 接口面。

## 0. 目标

收口 P1 留下的两个过渡字段——删掉 `LirArtifact.mir: Option<MaterializedMir>` 与 `LirArtifact.facts: LirFacts`，使 LIR 成为**单一、引用闭合、自包含**的 `LirProgram`：codegen 只 walk 它，**无 MIR overlay、无平行 flat facts、无字符串 live key**。这是 LIR 这一段「fallback 不可表示」真正成立的一步。

## 1. 现状（P1 末，已核对）

- `LirArtifact { cone, program: LateLoweredProgram, facts: LirFacts, base_context, mir: Option<MaterializedMir>, object_files }`（`crates/scoopc/src/pipeline/lir_artifact.rs`）。
- **overlay 仍在**：`crates/scoopc_lir/src/effect_lowered/ir.rs:343` `type LateLoweredSourceBody = crate::mir::Body`；`LateLoweredStateSlice`(:3591) 按 `start/end_statement_index` 切回 MIR；codegen 走「state → MIR slice」。
- **身份是两 String**：`StableLirCallableKey { canonical_text, readable_path }`（`crates/scoopc_ids/src/lib.rs:260`），生产代码 **139 处**（含 BTreeMap key 与 owner/target/candidate 跨引用）。
- **`LirFacts` 18 组平表**（`crates/scoopc_lir_facts/src/lib.rs:24`）与 program 并列。

## 2. 子阶段（按序：地基 → 大头）

- **P2a — 身份地基**（统一身份模式 §2.7）：引入 `LirCallableId`（= `program.callables` 下标）+ `LirCallableHash`（跨 cone/序列化）；`StableLirCallableKey.readable_path` 降为仅调试；139 处 live 引用迁到 `LirCallableId`。**详见 `TODO.md`。**
- **P2b — 折叠 `LirFacts`**：按 fold map 把 per-callable / per-call-site / per-nominal 的 fact 挂到对应节点；删 `LirArtifact.facts`；程序级组（`summary`/`opt_pipeline`/`type_context`/step-schema 类）保留为 program 字段。
- **P2c — lift 指令、消除 overlay**（最大）：定义 LIR 自有指令集（覆盖 MIR 的 4 statements / 24 rvalues / 8 terminators / 6 call kinds + transport metadata），lowering 产出 LIR 指令，codegen 改走 LIR 指令；删 `LateLoweredStateSlice` / `LateLoweredSourceBody` 与 `LirArtifact.mir`。

> P2a 是 P2b/P2c 的前提（节点先有密集句柄身份）。本阶段 `TODO.md` 三个子阶段都列出任务，但 P2c 的指令集细节在进入 P2c 时按 mir 指令逐条定稿。

## 3. 完成标志

`LirArtifact = { cone, program: LirProgram, object_files }`（`base_context` 的类型/布局并入 `program`）；无 `mir`/`facts`/overlay/字符串 live key；codegen 单结构 walk；全套 fixture 端到端绿。

## 4. 验证基线（每任务收尾）

```
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test --all --all-targets
cargo build -p scoop -p scoopc
python3 tools/dependency_gate.py
python3 tools/spec_fixtures.py check
python3 tools/run_fixtures.py
```
