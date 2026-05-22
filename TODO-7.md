# TODO-7：Stage crate split + per-cone build artifact

> 生成时间：2026-05-22
> 计划基线：[`PLAN.md`](./PLAN.md) §4/P9-P10
> 设计基线：[`PIPELINE_REFACTOR.md`](./PIPELINE_REFACTOR.md)
> 审计基线：[`PIPELINE-CLEANUP.md`](./PIPELINE-CLEANUP.md)
> 索引：[`TODO.md`](./TODO.md)
> 顺序约束：严格按本文件任务顺序推进；每个实现任务后必须执行紧随其后的 review 任务。
> 本包目标：把当前驻留在 `scoopc` 单 crate 里的 stage 实现按 PLAN.md §1.2 的依赖方向拆分为独立 crate，并以此为基础落地 per-cone build artifact，让下游 cone 能消费上游已发布的 facts/IR/objs 而不是重扫上游源码。

## 范围

- 把 AST、HIR（含 resolve / typecheck / infer）、MIR（含 monomorph / opt / 去虚化 / 共享 RTTI/vtable/itable/intrinsics 的归属）、effect facts builder、LIR（effect_lowered）、LLVM codegen 拆为独立 stage crate；驱动 facade 留在 `scoopc` 或拆为独立 driver crate。
- 把 cone 模块按"数据 vs 操作"两层拆分：`Cone.toml` / `SourceConeGraph` / `ConeManifest` 等纯数据 + 文件系统 adapter 进 `scoopc_project_model`；archive / scoopir export / consume / annotations / visibility / pre-specialize 这一层进新 crate `scoopc_cone`。
- 扩展 `tools/scoop_tools` 的 `dependency-gate` 以覆盖新增 stage / cone crate，禁止反向边。
- 设计并落地 per-cone build artifact：每个 cone 在 `build/<profile>/cones/<cone>@<version>/` 下输出 `hir_facts.bin` / `mir_facts.bin` / `effect_facts.bin` / `lir_facts.bin` / `lir_program.bin` / 必要 layout / .o / `inputs.fingerprint` / `outputs.fingerprint`。
- 解决跨进程 `TypeId` / type identity stable wire format（P7-T04-a 完成记录里显式推迟到本包）。
- 改 `run_frontend` 按 cone DAG 拓扑顺序运行；下游 cone 注入上游 cone 的 facts artifact 而不是吃源码；替换现有 `cone::consume::inject_cone_dependency_public_api` 路径。
- 加 per-cone inputs/outputs fingerprint chain，让 cache hit 跳过整个上游 cone 的重 lower。

## 进入门禁

- P0-P8 全部完成（含 P7-T04 / P7-T05 全包清场和 P8-T01/T02 final verification）。
- LLVM backend 已经只消费 `LIR + LIR facts + base context`，且 `LirStageOutput` 不再保留 LLVM residual pass-view accessor。
- `dependency_gate` 已在 P0-P8 期间记录所有 base crate 与 fact crate 的边界；本包负责把同样的 gate 向 stage crate 边界扩展。
- 仓库不再保留 comptime 残留、HIR/codegen 层去虚化或 stage output 嵌套上游整包。
- P7-T04-a 完成记录中关于"cross-process stable wire format 推迟到 P8/per-cone build artifact serialization"的承诺由本包正式接手。

## 细化原则

- **P9 与 P10 严格串行**：P9 在交付前 stage crate 边界必须由 cargo build 强制；P10 才允许新增 per-cone artifact 持久化与 frontend 重构。
- **P9 内每一步只拆一个 stage crate**，互相不允许交错。每拆一个就让 `cargo check --workspace` 与 `cargo run -p scoop_tools -- dependency-gate` 全绿，再进入下一个。
- **P9 内允许保留 `scoopc` umbrella crate 的 façade re-exports**（`pub use scoopc_hir as hir;` 这种），让下游驱动/测试代码无需大规模改 import 路径；但 stage crate 内部一律不能 `use scoopc::`。
- **P10 不重新设计 fact schema**：直接让现有 `scoopc_hir_facts` / `scoopc_mir_facts` / `scoopc_effect_facts` / `scoopc_lir_facts` 加 `Serialize`/`Deserialize`（解决 TypeId stable representation 后），不另起新数据模型。
- **per-cone artifact 不引入 tar 归档**：磁盘布局直接是 `build/<profile>/cones/<cone>/` 下的目录树，不复活 `.cone` archive；`scoop package` 仍保持停用状态。
- **不引入 generic body wire format（HIR/MIR template）**：本包只解决"下游不再过上游源码做 frontend"，跨 cone 重单态化的 generic 仍在下游 re-lower（容许的损失，由 facts + LIR 已能覆盖大部分场景）。如果实测显示该损失不可接受，再单独立项。
- **任何后向边或 cycle 必须本任务内消除**，不得新增 façade hack 或临时再导出绕开 cargo 报错。

## 触碰面基线

本节是 `TODO-7-INIT` 的仓库搜索记录，后续 P9/P10 任务应优先从这些位置开始，不再重复做开放式搜索。

### 当前 `scoopc/src/` 主模块体量

| 模块 | 文件数 | 行数 | 后续归属 |
| --- | --- | --- | --- |
| `ast/` | 1 | ~2.7k | `scoopc_ast` |
| `parser/` | 9 | ~9k | `scoopc_ast` |
| `syntax/` | 7 | ~1.9k | `scoopc_ast` |
| `hir/` | 30 | ~30k | `scoopc_hir` |
| `resolve/` | 3 | ~6k | `scoopc_hir` |
| `typecheck/` | 39 | ~42k | `scoopc_hir` |
| `infer/` | 1 | ~0.6k | `scoopc_hir` |
| `mir/` | 39 | ~38k | `scoopc_mir` |
| `monomorph/` | 2 | ~1k | `scoopc_mir` |
| `rtti/` | 2 | ~3k | `scoopc_mir`（数据结构） / `scoopc_hir_facts`（dispatch facts 已有） |
| `effect/` | 12 | ~10k | `scoopc_effect_stage` |
| `effect_facts/` | 7 | ~9k | `scoopc_effect_stage`（builder） / `scoopc_effect_facts`（已是独立 crate，仅需吸收 builder 端契约） |
| `effect_lowered/` | 16 | ~25k | `scoopc_lir` |
| `llvm/` | 125 | ~82k | `scoopc_codegen_llvm` |
| `pipeline/` | 10 | ~18k | 留在 umbrella `scoopc`（driver 编排） |
| `cone/` | 13 | ~5k | 数据/FS 层进 `scoopc_project_model`；操作层进 `scoopc_cone` |

### 阻塞 stage crate split 的后向边（P9-T01 必须在第一步消除）

| 位置 | 当前用法 | 处置 |
| --- | --- | --- |
| `crates/scoopc/src/hir/lower/util/mod.rs:11` | `use crate::mir::{InstanceKey, TemplateKey}` | 把 `InstanceKey` / `TemplateKey` 移到 `scoopc_ids`，HIR/MIR 都引用 base 即可。 |
| `crates/scoopc/src/typecheck/annotations.rs:24` | `use crate::hir::ExternAbi` | 把 `ExternAbi` 移到 `scoopc_types` 或 `scoopc_ids`。 |
| `crates/scoopc/src/effect_facts/builder.rs:11,21` | `use crate::resolve::{FunOverload, Index}` + `crate::typecheck::{TypeEnv, TypeLowering, TypeSymbol}` | builder 和 effect 一并归 `scoopc_effect_stage`，但它依赖 HIR resolver/typecheck；按 PLAN §1.2 这是合法（前一阶段 crate 依赖），无需消除，仅需在 P9-T06 时正确放置。 |
| `crates/scoopc/src/cone/scoopir/`、`annotations.rs`、`visibility.rs`、`pre_specialize.rs` | 直接调用 parser / typecheck / hir / mir | 操作层归 `scoopc_cone`，依赖所有 stage crate；按 PLAN §1.2 这是合法（cone 在 stage 之上）。 |
| `crates/scoopc/src/cone/consume.rs` | 注入 `Index`/`TypeEnv` | 同上，归 `scoopc_cone`，依赖 `scoopc_hir`。 |
| `crates/scoopc/src/llvm/**` | `use crate::hir` 8 处 + `use crate::mir` 9 处 | P7-T03 / P7-T04 已经/将会清干净；P9 开始前应已为 0。若 P9 启动时仍有残留，先回 P7 收尾。 |

### 跨切面 helper 的归属决策（P9-T05 处理）

| 文件 | 当前用法 | P9 后归属 |
| --- | --- | --- |
| `crates/scoopc/src/stable_id.rs` | 32 处使用，基础 stable key 工厂 | 拆为两段：纯 stable key 部分到 `scoopc_ids`（已经在迁移中），符号 mangler 到 `scoopc_mir`。 |
| `crates/scoopc/src/intrinsics.rs` | 6 处使用，sysroot intrinsic 注册 | `scoopc_hir`（typecheck 阶段消费）。 |
| `crates/scoopc/src/itable.rs`（1k 行）/ `vtable.rs`（300 行） | dispatch slot 计算 | `scoopc_hir`（slot 表是 HIR 阶段事实，且已有 `DispatchTableFact` in `scoopc_hir_facts`）。 |
| `crates/scoopc/src/devirtualize.rs`（110 行） | 0 处使用 | 删除（P3 后已被 MIR pass 取代）。 |
| `crates/scoopc/src/expr_facts.rs` | 3 处使用 | `scoopc_hir`（typecheck-owned）。 |
| `crates/scoopc/src/opt.rs` | 18 处使用，`OptLevel` enum + 解析 | `scoopc_project_model`（opt level 是 manifest 字段，已经 re-export 在那里）。 |
| `crates/scoopc/src/stackmap.rs` | 1 处使用 | `scoopc_codegen_llvm`（仅 backend 需要）。 |
| `crates/scoopc/src/sysroot/` | sysroot loader | `scoopc_project_model`（与 cone graph 紧耦合）。 |

### 当前 `cone/` 内的两层

| 文件 | 行数 | stage 依赖 | 归属 |
| --- | --- | --- | --- |
| `cone/manifest.rs` | 77 | 无 | `scoopc_project_model` |
| `cone/package.rs` | 696 | 无 | `scoopc_project_model` |
| `cone/graph.rs` | 791 | 无（仅 sysroot） | `scoopc_project_model` |
| `cone/archive.rs` | 276 | 调用下面四个 | `scoopc_cone` |
| `cone/scoopir/` | — | parser + typecheck + ast | `scoopc_cone` |
| `cone/annotations.rs` | 224 | parser + typecheck | `scoopc_cone` |
| `cone/visibility.rs` | 272 | parser + typecheck | `scoopc_cone` |
| `cone/pre_specialize.rs` | 874 | parser + typecheck + hir + mir | `scoopc_cone` |
| `cone/consume.rs` | 778 | resolve + typecheck + hir | `scoopc_cone` |

### Per-cone build artifact 现状基线

- 现有 `tools/scoop_tools/src/dependency_gate.rs` 的 `FORBIDDEN_WORKSPACE_CRATES` 已预留 `scoopc_ast` / `scoopc_ast_facts` / `scoopc_hir` / `scoopc_mir` / `scoopc_effect_facts_stage` / `scoopc_lir` / `scoopc_codegen_llvm` / `scoopc_codegen_c`，本包按这些命名实际创建 crate。
- 现有 `crates/scoop/src/commands/build/incremental.rs` 的 fingerprint 是单一全项目 SHA-256（Cone.toml + sources + native + sysroot + runtime + toolchain）；本包要把它替换为 per-cone fingerprint chain。
- 现有 `cone/archive.rs` 的 `.cone` tar 归档保持停用（`scoop package` 仍 return `ARCHIVE_UNSUPPORTED_MESSAGE`）；本包不复活归档，磁盘格式直接是目录。
- 现有 4 套 fact crate (`scoopc_hir_facts` / `scoopc_mir_facts` / `scoopc_effect_facts` / `scoopc_lir_facts`) 中的 `FactIdentity` 已经携带 `StableConeKey`，按 cone 切片本来就是 schema 内置能力。
- `MirFacts.snapshots.canonical` 已有 canonical (pre-opt) MIR 快照锚点；本包暂不持久化 generic body 本体，但保留这个 anchor 以便后续单独立项。
- `LirFacts.global_init.cone_init_routines` / `final_entry_init_order` 已经按 cone 标记 init routine 入口符号；per-cone artifact 直接复用，不需要新设计。

## [TODO] TODO-7-INIT：初始化并细化本任务包

- 目标：
  - 分析 `PLAN.md` §4/P9-P10、`PIPELINE_REFACTOR.md` 与当前 stage crate 拆分阻塞、cone 两层归属、per-cone build artifact 真实需求；
  - 生成本任务包的详细任务列表，覆盖 stage crate split、cone two-layer split、per-cone artifact 设计、TypeStore stable wire format、per-cone frontend 编排、fingerprint cache；
  - 更新 `TODO.md` 的具体任务索引，把 `TODO-7-INIT` 所在行替换为本包细化后的任务列表。
- 必须实现的内容：
  1. 复核"触碰面基线"是否覆盖 P9-T01..T08、P10-T01..T06 的所有起点。
  2. 把 P9 / P10 拆成数量适中的实现小阶段，每个阶段必须有明确目标、修改范围、验证命令和完成条件。
  3. 在每个实现小阶段后插入独立 review 任务，review 任务必须复审前一阶段是否满足 PLAN §1.2 的 crate DAG 约束或 per-cone artifact 边界约束。
  4. 同步更新 `TODO.md` 中的具体任务索引，确保任务 ID、状态和顺序与本文件一致。
- 完成条件：
  - `TODO-7.md` 不再只是范围说明，而是包含可直接执行的详细任务列表；
  - `TODO.md` 的具体任务索引已经同步反映 `TODO-7.md` 的新任务和 `[TODO]` 状态；
  - 本任务完成记录说明 P9/P10 的拆分依据与未展开的风险。
- 依赖：P8-T02R
- 完成记录：
  - 待填写。

---

## P9：Stage crate split

将 PLAN.md §1.2 要求的 stage / fact crate DAG 用 cargo crate 边界硬化。

### [TODO] P9-T01：消除阻塞 stage crate split 的后向边

- 参考：本文件"阻塞 stage crate split 的后向边"。
- 目标：
  - 把 `InstanceKey` / `TemplateKey` 与 `ExternAbi` 等被反向引用的小型 stable 数据迁到 base crate；
  - 删除 P3 残留的 `crates/scoopc/src/devirtualize.rs`（0 处使用）；
  - 验证 P7 已经清掉所有 `llvm/** -> hir/mir` import；若仍有残留，回写 P7-T05 修复，不在本任务绕开。
- 必须修改的主要位置：
  - `crates/scoopc/src/hir/lower/util/mod.rs`
  - `crates/scoopc/src/typecheck/annotations.rs`
  - `crates/scoopc/src/mir/materialize/mod.rs`（搬出 `InstanceKey` / `TemplateKey`）
  - `crates/scoopc_ids/`、`crates/scoopc_types/`
  - `crates/scoopc/src/devirtualize.rs`（删除）
- 必须实现的内容：
  1. `InstanceKey` / `TemplateKey` 移到 `scoopc_ids`，HIR / MIR 改 import。
  2. `ExternAbi` 移到 `scoopc_types` 或 `scoopc_ids`，HIR / typecheck 改 import。
  3. 删除 `devirtualize.rs` 与对应 `pub(crate) mod devirtualize;`。
  4. 全仓搜索 `use crate::mir` 在 `scoopc/src/hir`、`use crate::hir` 在 `scoopc/src/typecheck`、`scoopc/src/llvm`、`scoopc/src/effect_lowered` 内的命中，确认归零；若 LLVM 还有 hir/mir import，回 P7-T05 修复。
- 验证：
  1. `cargo fmt`
  2. `cargo build --workspace`
  3. `cargo test --all --all-targets`
  4. `cargo run -p scoop_tools -- dependency-gate`
  5. 残余搜索：`rg -n "use crate::mir" crates/scoopc/src/hir crates/scoopc/src/typecheck`；`rg -n "use crate::hir" crates/scoopc/src/typecheck crates/scoopc/src/llvm crates/scoopc/src/effect_lowered`
  6. `git diff --check`
- 完成条件：
  - 反向边搜索无生产命中；
  - `devirtualize.rs` 删除；
  - `cargo build --workspace` 与 `cargo test --all --all-targets` 通过。
- 依赖：TODO-7-INIT

### [TODO] P9-T01R：Review 后向边消除结果

- 参考：P9-T01。
- 重点：
  - `InstanceKey` / `TemplateKey` / `ExternAbi` 是否落到正确 base crate；
  - 残余反向边搜索结果；
  - 是否在 P9 启动前 LLVM 已无 hir/mir import 残留。
- 验证：重新运行 P9-T01 的所有验证。
- 完成条件：
  - 后续 stage crate 抽取不会再因为反向边失败；
  - 若发现 P7 仍有残留，本 review 内回写 P7-T05 而非在 P9 内绕开。
- 依赖：P9-T01

### [TODO] P9-T02：抽出 `scoopc_ast` crate

- 参考：本文件"当前 `scoopc/src/` 主模块体量"。
- 目标：
  - 把 `crates/scoopc/src/{ast,parser,syntax}` 整体迁到新 crate `scoopc_ast`；
  - `scoopc` 通过 `pub use scoopc_ast as ast;` / `pub use scoopc_ast::parser as parser;` 等 façade re-export 维持下游兼容。
- 必须修改的主要位置：
  - 新建 `crates/scoopc_ast/`（Cargo.toml + src 树）
  - `Cargo.toml` workspace 加入新 crate
  - `crates/scoopc/src/lib.rs` 的 `pub mod ast;` / `parser` / `syntax` 改 façade
  - `tools/scoop_tools/src/dependency_gate.rs`：把 `scoopc_ast` 从 `FORBIDDEN_WORKSPACE_CRATES` 名单激活为正式存在 crate
- 必须实现的内容：
  1. `scoopc_ast` 仅依赖 base crates（span / source / ids / types）。
  2. 删除 `scoopc/src/ast/`、`parser/`、`syntax/` 的实际定义，原文件路径替换为 façade re-export。
  3. workspace 测试与 fixture 不需要改 import（依赖 façade）。
  4. dependency_gate 增加 `scoopc_ast` 的 base-only 依赖检查。
- 验证：
  1. `cargo fmt`
  2. `cargo build --workspace`
  3. `cargo test --all --all-targets`
  4. `cargo run -p scoop_tools -- dependency-gate`
  5. `git diff --check`
- 完成条件：
  - `cargo tree -p scoopc_ast` 只显示 base crates；
  - `scoopc` umbrella 仍提供 `ast` / `parser` / `syntax` 模块路径。
- 依赖：P9-T01R

### [TODO] P9-T02R：Review `scoopc_ast` 抽取

- 参考：P9-T02。
- 重点：
  - `scoopc_ast` 是否真的只依赖 base crates；
  - façade re-export 是否覆盖所有公共 API；
  - 是否有重复定义（迁移残留）。
- 验证：重新运行 P9-T02 的所有验证。
- 依赖：P9-T02

### [TODO] P9-T03：抽出 `scoopc_codegen_llvm` crate

- 参考：本文件"当前 `scoopc/src/` 主模块体量"。
- 目标：
  - P7 完成后 LLVM 已经只读 `LIR + LIR facts + base context`，本任务把 `crates/scoopc/src/llvm` 整体迁到 `scoopc_codegen_llvm`；
  - 顺带把 `crates/scoopc/src/stackmap.rs` 一起搬过去（仅 backend 用）。
- 必须修改的主要位置：
  - 新建 `crates/scoopc_codegen_llvm/`
  - `crates/scoopc/src/lib.rs` 的 `pub mod llvm;` 改 façade（保持 `feature = "llvm"` 门）
  - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs` 改 import
  - `tools/scoop_tools/src/dependency_gate.rs` 激活 `scoopc_codegen_llvm`
- 必须实现的内容：
  1. `scoopc_codegen_llvm` 依赖 `scoopc_lir`（在 P9-T07 之前需要先把 `effect_lowered` re-export 暴露好；本任务允许暂时直接依赖 umbrella `scoopc` 的 façade，但必须在完成记录里登记，等 P9-T07 完成后立即切换）。
  2. inkwell / LLVM feature 门保持原样。
  3. dependency_gate 增加 `scoopc_codegen_llvm` 的允许依赖白名单（base + lir + lir_facts）。
- 验证：
  1. `cargo fmt`
  2. `cargo build --workspace --features llvm`
  3. `cargo test --all --all-targets --features llvm`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  5. `cargo run -p scoop_tools -- dependency-gate`
  6. `git diff --check`
- 完成条件：
  - `cargo tree -p scoopc_codegen_llvm` 不显示 `scoopc_hir` / `scoopc_mir` / `scoopc_effect_facts` 任何 stage crate（只能通过 façade `scoopc` 间接，且必须在 P9-T07 完成后切到 `scoopc_lir` 直依赖）；
  - run-pass fixtures 全部通过。
- 依赖：P9-T02R

### [TODO] P9-T03R：Review `scoopc_codegen_llvm` 抽取

- 参考：P9-T03。
- 重点：
  - 是否仍存在 LLVM crate 反向使用 HIR/MIR 的迹象；
  - feature gate 是否正确传播；
  - 完成记录是否登记了 P9-T07 后的依赖切换义务。
- 验证：重新运行 P9-T03 的所有验证。
- 依赖：P9-T03

### [TODO] P9-T04：抽出 `scoopc_hir` crate

- 参考：本文件"跨切面 helper 的归属决策"。
- 目标：
  - 把 `hir/` + `resolve/` + `typecheck/` + `infer/` + `intrinsics.rs` + `expr_facts.rs` + `vtable.rs` + `itable.rs` 一起迁到 `scoopc_hir`；
  - 这是按 PLAN §4/P2 已经存在的"前端语义屏障"概念整合，而非进一步细分。
- 必须修改的主要位置：
  - 新建 `crates/scoopc_hir/`
  - `crates/scoopc/src/lib.rs` façade
  - `crates/scoopc/src/pipeline/{ast_stage,hir_stage,...}.rs` 改 import
  - dependency_gate 激活 `scoopc_hir`
- 必须实现的内容：
  1. `scoopc_hir` 依赖：base crates、`scoopc_ast`、`scoopc_hir_facts`。
  2. resolve / typecheck / infer / intrinsics / expr_facts / vtable / itable 的 `pub` API 收口在 `scoopc_hir`。
  3. façade re-export 维持 `scoopc::{hir, resolve, typecheck, infer}` 路径不变。
- 验证：
  1. `cargo fmt`
  2. `cargo build --workspace`
  3. `cargo test --all --all-targets`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  5. `cargo run -p scoop_tools -- dependency-gate`
  6. `git diff --check`
- 完成条件：
  - `cargo tree -p scoopc_hir` 只显示 base + ast + hir_facts；
  - 全套测试与 run-pass fixture 通过。
- 依赖：P9-T03R

### [TODO] P9-T04R：Review `scoopc_hir` 抽取

- 参考：P9-T04。
- 重点：
  - 是否有反向边逃逸到 `scoopc_mir` / `scoopc_codegen_llvm`；
  - vtable/itable 是否真的归属在前端而非后端；
  - 残留 `use crate::*` import 是否全部转为 crate-local 或 base 引用。
- 验证：重新运行 P9-T04 的所有验证。
- 依赖：P9-T04

### [TODO] P9-T05：抽出 `scoopc_mir` crate

- 参考：本文件"跨切面 helper 的归属决策"。
- 目标：
  - 把 `mir/` + `monomorph/` + `opt.rs` + `rtti/` + `stable_id` 中的 mangler 段一起迁到 `scoopc_mir`；
  - 决定 RTTI 数据结构归属：纯数据进 `scoopc_hir_facts`（dispatch facts 已有先例），算法/lower 进 `scoopc_mir`。
- 必须修改的主要位置：
  - 新建 `crates/scoopc_mir/`
  - `crates/scoopc/src/lib.rs` façade
  - `crates/scoopc/src/pipeline/mir_stage.rs` 改 import
  - dependency_gate 激活 `scoopc_mir`
- 必须实现的内容：
  1. `scoopc_mir` 依赖：base + `scoopc_ast` + `scoopc_hir` + `scoopc_hir_facts` + `scoopc_mir_facts`。
  2. monomorph / opt（普通 backend-neutral pass pipeline）/ devirtualization MIR pass / escape analysis / inlining 全部归 `scoopc_mir`。
  3. 决定 cross-cutting helper：`stable_id.rs` 的 mangler 部分进 `scoopc_mir`，纯 stable key 部分确认仍在 `scoopc_ids`。
- 验证：
  1. `cargo fmt`
  2. `cargo build --workspace`
  3. `cargo test --all --all-targets`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  5. `cargo run -p scoop_tools -- dependency-gate`
  6. `git diff --check`
- 完成条件：
  - `cargo tree -p scoopc_mir` 不显示 `scoopc_effect_*` / `scoopc_lir` / `scoopc_codegen_llvm`；
  - MIR pass pipeline 在新 crate 下行为不变。
- 依赖：P9-T04R

### [TODO] P9-T05R：Review `scoopc_mir` 抽取

- 参考：P9-T05。
- 重点：
  - RTTI 归属决策是否落实；
  - opt pipeline 是否完整迁移；
  - mangler 段是否切干净。
- 验证：重新运行 P9-T05 的所有验证。
- 依赖：P9-T05

### [TODO] P9-T06：抽出 `scoopc_effect_stage` 与 `scoopc_lir` crate

- 参考：本文件"当前 `scoopc/src/` 主模块体量"。
- 目标：
  - `effect/` + `effect_facts/builder.rs` 进 `scoopc_effect_stage`（与已有的 data crate `scoopc_effect_facts` 区分；前者是 builder + stage，后者是 fact 数据）；
  - `effect_lowered/` 进 `scoopc_lir`。
- 必须修改的主要位置：
  - 新建 `crates/scoopc_effect_stage/` 与 `crates/scoopc_lir/`
  - `crates/scoopc/src/lib.rs` façade
  - `crates/scoopc/src/pipeline/{effect_facts_stage,effect_lowering_stage,lir_facts_builder}.rs` 改 import
  - dependency_gate 激活两个 crate
- 必须实现的内容：
  1. `scoopc_effect_stage` 依赖：base + `scoopc_ast` + `scoopc_hir` + `scoopc_hir_facts` + `scoopc_mir` + `scoopc_mir_facts` + `scoopc_effect_facts`。
  2. `scoopc_lir` 依赖：base + `scoopc_mir` + `scoopc_mir_facts` + `scoopc_effect_facts` + `scoopc_lir_facts`（按 PLAN §1.2，LIR 不应再依赖 hir 或 ast）。
  3. P9-T03 中登记的"`scoopc_codegen_llvm` 暂走 façade"义务在本任务完成后切换：`scoopc_codegen_llvm` 改为直接 `use scoopc_lir::...`。
- 验证：
  1. `cargo fmt`
  2. `cargo build --workspace`
  3. `cargo test --all --all-targets`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  5. `cargo run -p scoop_tools -- dependency-gate`
  6. `git diff --check`
- 完成条件：
  - `cargo tree -p scoopc_lir` 不显示 `scoopc_hir` / `scoopc_ast`；
  - `cargo tree -p scoopc_codegen_llvm` 直接显示 `scoopc_lir`，不再绕 façade。
- 依赖：P9-T05R

### [TODO] P9-T06R：Review `scoopc_effect_stage` 与 `scoopc_lir` 抽取

- 参考：P9-T06。
- 重点：
  - LIR crate 是否摆脱 HIR；
  - `scoopc_codegen_llvm` 的依赖方向是否切换为 `scoopc_lir` 直依赖；
  - effect stage builder 是否在新 crate 内仍能正常构造 facts。
- 验证：重新运行 P9-T06 的所有验证。
- 依赖：P9-T06

### [TODO] P9-T07：cone 两层拆分（`scoopc_project_model` 扩展 + 新 `scoopc_cone`）

- 参考：本文件"当前 `cone/` 内的两层"。
- 目标：
  - 把 `cone/manifest.rs` / `package.rs` / `graph.rs` 与 `sysroot/` 全部迁到 base crate `scoopc_project_model`（这些只依赖 base）；
  - 把 `cone/archive.rs` / `scoopir/` / `annotations.rs` / `visibility.rs` / `pre_specialize.rs` / `consume.rs` 迁到新 crate `scoopc_cone`（操作层，依赖所有 stage crate）。
- 必须修改的主要位置：
  - `crates/scoopc_project_model/` 扩展
  - 新建 `crates/scoopc_cone/`
  - `crates/scoopc/src/lib.rs` 把 `pub mod cone;` 改 façade
  - `crates/scoopc/src/frontend.rs` 改 import（`cone::ConeManifest` 等改路径）
  - dependency_gate 增加 `scoopc_cone`：禁止任何 stage crate 反向依赖它
- 必须实现的内容：
  1. `scoopc_project_model` 吸收 manifest / package / graph / sysroot 后仍只依赖 `scoopc_span` / `scoopc_source` / `scoopc_ids` / `scoopc_types`。
  2. `scoopc_cone` 依赖：所有 stage crate + 所有 fact crate + project_model。
  3. `inject_cone_dependency_public_api` / scoopir export / annotations / visibility / pre_specialize 等 public API 在 `scoopc_cone` 内重新暴露；façade `scoopc::cone::*` 保持向后兼容。
  4. dependency_gate 加入 `scoopc_cone`：在 `FORBIDDEN_WORKSPACE_CRATES` 中保留它，禁止任何 stage / base / fact crate 反向依赖。
- 验证：
  1. `cargo fmt`
  2. `cargo build --workspace`
  3. `cargo test --all --all-targets`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  5. `cargo run -p scoop_tools -- dependency-gate`
  6. `git diff --check`
- 完成条件：
  - `cargo tree -p scoopc_project_model` 仍只有 base crates；
  - `cargo tree -p scoopc_cone` 显示对所有 stage crate 的依赖；
  - 任何 stage crate 都不依赖 `scoopc_cone`（dependency_gate 强制）。
- 依赖：P9-T06R

### [TODO] P9-T07R：Review cone 两层拆分

- 参考：P9-T07。
- 重点：
  - cone 数据层是否真的只在 base；
  - cone 操作层是否独立成 crate 且无 stage crate 反向依赖；
  - sysroot loader 归属决策是否落地。
- 验证：重新运行 P9-T07 的所有验证。
- 依赖：P9-T07

### [TODO] P9-T08：`scoopc` umbrella crate 收尾 + dependency_gate 全面强化

- 参考：本文件"细化原则"。
- 目标：
  - `scoopc` umbrella crate 只剩 façade re-exports + `pipeline/` 编排 + driver helper（`session/` / `target/` / `warnings.rs` 等）；
  - dependency_gate 把所有 stage / cone crate 边界都纳入硬检查，使后续违反 PLAN §1.2 的 PR 在 CI 上立即失败。
- 必须修改的主要位置：
  - `crates/scoopc/src/lib.rs`
  - `crates/scoopc/Cargo.toml`
  - `tools/scoop_tools/src/dependency_gate.rs`
  - `crates/scoopc/src/pipeline/`（如不需要拆 driver crate，留在 umbrella）
- 必须实现的内容：
  1. `scoopc` 的 `Cargo.toml` 仅依赖 base + 所有 fact crate + 所有 stage crate + cone crate；不再含 `inkwell` 等 backend 私有依赖（若 `--features llvm` 仍需要传递，由 `scoopc_codegen_llvm` 自己拉）。
  2. `scoopc/src` 下不再有任何"实际定义文件"（除 `pipeline/` / `session/` / `frontend.rs` / driver helper）；其余文件全部是 façade re-export。
  3. dependency_gate 扩展：每个新 stage crate 都加白名单 + 反向依赖禁令；为 cone 加专门规则（cone 可以依赖 stage，stage 不能依赖 cone）。
  4. README + crate overview 更新（指向新 crate 结构）。
- 验证：
  1. `cargo fmt`
  2. `cargo build --workspace`
  3. `cargo test --all --all-targets`
  4. `cargo run -p scoop -- test`（完整 fixture suite，至少 30 分钟 timeout）
  5. `cargo run -p scoop_tools -- dependency-gate`
  6. `cargo clippy --all-targets -- -D warnings`
  7. `git diff --check`
- 完成条件：
  - workspace 编译加测试全绿；
  - dependency_gate 输出列出所有新 crate 的依赖关系都符合 PLAN §1.2；
  - README 与 crate overview 反映最终结构。
- 依赖：P9-T07R

### [TODO] P9-T08R：Review umbrella 收尾

- 参考：P9-T08。
- 重点：
  - umbrella `scoopc` 是否仅剩 façade + driver；
  - dependency_gate 是否覆盖全部 stage / cone crate 反向边；
  - 文档（README / crate overview）是否同步。
- 验证：重新运行 P9-T08 的所有验证。
- 依赖：P9-T08

### [TODO] P9-T09：P9 全包清场、文档同步与依赖审计

- 目标：搜索确认 P9 完成后没有 stage crate 互相反向依赖、没有 façade 兜底逃避边界、没有遗留的 `crate::` cross-stage import。
- 必须修改的主要位置：`TODO.md`、`TODO-7.md`、`PIPELINE-CLEANUP.md`、`PIPELINE_REFACTOR.md`（仅在设计边界实际变化时）、`README.md`。
- 必须实现的内容：
  1. 搜索确认每个 stage crate 的 `Cargo.toml` 依赖与 PLAN §1.2 一致；
  2. 搜索确认 `crates/scoopc/src/` 下没有 stage 实际定义遗留；
  3. 文档明确"任何后续 stage 改动必须在自己的 crate 内完成，不能跨 crate"。
- 验证：
  1. `cargo run -p scoop_tools -- dependency-gate`
  2. `cargo build --workspace`
  3. `cargo test --all --all-targets`
  4. `git diff --check`
- 完成条件：crate DAG 与 PLAN §1.2 一致；TODO 与文档同步。
- 依赖：P9-T08R

### [TODO] P9-T09R：Review P9 全包完成度

- 参考：P9-T09。
- 重点：是否所有 stage crate 都按 DAG 方向依赖；是否还残留 `scoopc/src/` 下的旧实现；P10 是否可以从干净的 crate 边界开始。
- 验证：重新运行 P9-T09 的所有验证。
- 依赖：P9-T09

---

## P10：Per-cone build artifact

利用 P9 已经独立出来的 stage / fact crate 边界，让每个 cone 的编译产物落地到磁盘，下游 cone 通过反序列化 fact 注入而不是吃源码。

### [TODO] P10-T01：解决 `TypeId` cross-process stable wire format

- 参考：
  - 本文件"进入门禁"
  - `TODO-6.md` P7-T04-a 完成记录："cross-process stable wire format 本轮显式推迟到 P8/per-cone build artifact serialization"
  - `TODO-6.md` P7-T04 第 6 条
- 目标：
  - 决定并落实跨进程 stable type identity 表示，使 fact / LIR 序列化到磁盘后下游进程能正确恢复；
  - 避免后续 P10 任务每个都要单独处理 TypeId 失效问题。
- 候选方案（在本任务内做出选择并记录）：
  - **方案 A**：fact crate / LIR 中所有 `TypeId` 字段替换为 `scoopc_ids::CanonicalTextKey` 等 stable key；序列化用 stable key，进程内查询时按需 re-intern 到本地 TypeStore。
  - **方案 B**：保留 `TypeId`，给 `TypeStore` 设计 portable serialization（按拓扑序输出 TypeKind + 子 TypeId 引用 → 反序列化时按相同顺序 intern，得到 ID 重映射表，外部使用方拿映射表走重映射）。
  - **方案 C**：混合 —— fact crate 的"声明性身份"用 stable key，LIR body 内部仍用 TypeId，反序列化时通过 TypeStore 重建 + 局部映射。
- 必须修改的主要位置：
  - `crates/scoopc_types/`
  - `crates/scoopc_ids/`
  - `crates/scoopc_hir_facts/` / `mir_facts` / `effect_facts` / `lir_facts` 的 type 字段
  - `crates/scoopc_lir/` 的 body 序列化接口
  - 新增 wire format 测试 fixtures
- 必须实现的内容：
  1. 选择方案并在完成记录里给出依据；
  2. fact / LIR / 必要 base 类型实现 `Serialize` + `Deserialize`（默认 bincode 二进制）+ schema version；
  3. 新增 round-trip 测试：序列化 → 反序列化 → 验证 fact 完全等价；
  4. 文档明确"任何新增 fact 字段必须遵守同样的 wire format 约束"。
- 验证：
  1. `cargo fmt`
  2. `cargo build --workspace`
  3. `cargo test -p scoopc_types`
  4. `cargo test -p scoopc_hir_facts -p scoopc_mir_facts -p scoopc_effect_facts -p scoopc_lir_facts`
  5. `cargo test --all --all-targets`
  6. `git diff --check`
- 完成条件：
  - 4 套 fact + LIR program 能往返序列化；
  - schema version 字段就位；
  - 选择的方案在完成记录里有明确依据。
- 依赖：P9-T09R

### [TODO] P10-T01R：Review TypeStore wire format

- 参考：P10-T01。
- 重点：
  - 选择的方案是否真的解决跨进程问题；
  - schema version 是否覆盖所有 fact crate；
  - round-trip 测试是否覆盖 generic / effect / continuation 等复杂类型。
- 验证：重新运行 P10-T01 的所有验证。
- 依赖：P10-T01

### [TODO] P10-T02：定义 per-cone build artifact 磁盘布局与 `scoopc_cone` 读写 API

- 参考：本文件"Per-cone build artifact 现状基线"。
- 目标：
  - 在 `scoopc_cone` 中定义 `ConeArtifact` 结构与磁盘布局；
  - 实现 `ConeArtifact::write(&self, dir: &Path)` 与 `ConeArtifact::read(dir: &Path) -> Result<Self>`；
  - 与现有 `cone::consume::inject_cone_dependency_public_api` 路径并存（不强制下游切换）。
- 磁盘布局：

  ```text
  build/<profile>/cones/<cone-name>@<version>/
    manifest.json           # cone identity + schema versions
    hir_facts.bin
    mir_facts.bin
    effect_facts.bin
    lir_facts.bin
    lir_program.bin         # LIR body 本体
    objs/
      scoop.o               # 本 cone Scoop 源 emit 的 obj
      native_*.o            # [native-build] 的 obj
    inputs.fingerprint
    outputs.fingerprint
  ```

- 必须修改的主要位置：
  - `crates/scoopc_cone/`
  - 新增 fixtures：至少 1 个上游 lib cone + 1 个 consumer cone
- 必须实现的内容：
  1. `ConeArtifact` 数据结构与 IO；
  2. `manifest.json` 包含 cone name / version / scoopc compiler version / 各 fact crate schema version；
  3. 单测：写 → 读 → 校验 fact 等价；
  4. 文档说明每个文件的内容与版本兼容策略（不兼容时整 cone 重编）。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc_cone`
  3. `cargo test --all --all-targets`
  4. `git diff --check`
- 完成条件：
  - artifact IO 单测通过；
  - 磁盘布局文档化。
- 依赖：P10-T01R

### [TODO] P10-T02R：Review per-cone artifact schema

- 参考：P10-T02。
- 重点：磁盘布局是否覆盖 PLAN §1.2 要求的 stage / fact 完整集合；版本兼容策略是否合理。
- 验证：重新运行 P10-T02 的所有验证。
- 依赖：P10-T02

### [TODO] P10-T03：`run_frontend` 改造为按 cone DAG 拓扑顺序运行

- 参考：本文件"Per-cone build artifact 现状基线"。
- 目标：
  - 把 `crates/scoopc/src/frontend.rs::run_frontend` 从"扁平 build_closure_sources 循环"改为"按 cone 拓扑跑"；
  - 每个 cone 跑完 frontend 后产生本 cone 的 fact set；下游 cone 通过 fact 注入接口获取上游已发布内容；
  - 不再让下游遍历上游源 AST。
- 必须修改的主要位置：
  - `crates/scoopc/src/frontend.rs`
  - `crates/scoopc/src/pipeline/`
  - `crates/scoopc_cone/`（提供 fact 注入接口）
  - `crates/scoopc_hir/resolve/`、`typecheck/`（提供从 fact 注入 Index/TypeEnv 的接口）
- 必须实现的内容：
  1. 新接口 `import_upstream_artifacts(&[&ConeArtifact], &mut Index, &mut TypeEnv) -> Result<()>` 在 `scoopc_cone`；
  2. `run_frontend` 按 `SourceConeGraph::compilation_units()` 顺序循环；每个 cone 完成后通过新接口给下游注入；
  3. 现有 `inject_cone_dependency_public_api`（吃 ScoopIR JSON）与新路径并存，但默认走新路径；
  4. fixture 验证：跨 cone fixture（`tests/fixtures/run_pass_cone/source_path_dependency_public_call`）下游 typecheck 不读上游 source。
- 验证：
  1. `cargo fmt`
  2. `cargo build --workspace`
  3. `cargo test --all --all-targets`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  6. `git diff --check`
- 完成条件：
  - 跨 cone fixture 通过；
  - 下游 cone 编译过程中不再 parse 上游 cone 源文件（用 trace 或断言验证）。
- 依赖：P10-T02R

### [TODO] P10-T03R：Review per-cone frontend orchestration

- 参考：P10-T03。
- 重点：
  - 下游 cone 是否真的不再读上游源；
  - fact 注入是否覆盖 Index / TypeEnv / annotations / visibility 等所有现有 ScoopIR 注入点；
  - 测试是否能稳定证明跳过上游 source。
- 验证：重新运行 P10-T03 的所有验证。
- 依赖：P10-T03

### [TODO] P10-T04：per-cone fingerprint cache + 增量 build

- 参考：本文件"Per-cone build artifact 现状基线"。
- 目标：
  - 替换 `crates/scoop/src/commands/build/incremental.rs` 的整项目 SHA-256 fingerprint 为 per-cone inputs/outputs fingerprint chain；
  - 上游 cone outputs.fingerprint 不变 → 下游 cone 用上游已落盘 artifact，跳过上游所有 stage；
  - 用户 cone outputs.fingerprint 不变 + 入口 entrypoint 不变 → 跳过整个 build。
- 必须修改的主要位置：
  - `crates/scoop/src/commands/build/incremental.rs`
  - `crates/scoopc_cone/`（fingerprint 计算与读写）
- 必须实现的内容：
  1. inputs.fingerprint = SHA-256(本 cone Cone.toml + 本 cone 源文件 + opt level + toolchain version + 每个直接依赖 cone 的 outputs.fingerprint)；
  2. outputs.fingerprint = SHA-256(本 cone artifact 各文件)；
  3. cache hit：从磁盘加载 artifact；cache miss：跑 stage 后写盘；
  4. 测试：(a) 修改用户 cone 源 → 仅用户 cone 重 build；(b) 修改库 cone 源 → 库 + 用户重 build；(c) 修改 toolchain → 全重 build。
- 验证：
  1. `cargo fmt`
  2. `cargo test --all --all-targets`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone`
  4. 手工时序测试（在完成记录里给出具体数据）：fresh build vs cache hit 的耗时对比
  5. `git diff --check`
- 完成条件：
  - 三种 cache 场景测试通过；
  - 完成记录给出 fresh build vs cache hit 的实测耗时。
- 依赖：P10-T03R

### [TODO] P10-T04R：Review per-cone fingerprint cache

- 参考：P10-T04。
- 重点：
  - fingerprint chain 是否能正确传播到下游；
  - cache hit 的耗时是否实际下降到目标量级（之前调研数据：fingerprint scan 是 4s 启动开销的主因）；
  - sysroot 改动是否会触发可控范围的失效，而不是全量。
- 验证：重新运行 P10-T04 的所有验证。
- 依赖：P10-T04

### [TODO] P10-T05：P10 全包清场、文档同步与依赖审计

- 目标：
  - 搜索确认 frontend 不再读上游源；
  - 更新 README / SCOOP_RUNTIME 等文档体现新的 per-cone build；
  - 把"将来可能要做"的 generic body wire format 列为后续单独 milestone 的入口，不再在本包内闷头扩展。
- 必须修改的主要位置：`TODO.md`、`TODO-7.md`、`PIPELINE-CLEANUP.md`、`README.md`、相关 fixture 的 README。
- 必须实现的内容：
  1. 搜索 `inject_cone_dependency_public_api` 是否仅作为兼容路径保留；
  2. 搜索 frontend 内是否仍存在跨 cone 平铺 source loop；
  3. 文档明确"如果未来需要跨 cone generic body 复用，应单独立项 P11"；
  4. 把整个 build closure 的 SHA fingerprint 路径标记为废弃。
- 验证：
  1. `cargo fmt`
  2. `cargo run -p scoop_tools -- dependency-gate`
  3. `cargo test --all --all-targets`
  4. `cargo run -p scoop -- test`（完整 fixture suite，至少 30 分钟 timeout）
  5. `cargo clippy --all-targets -- -D warnings`
  6. `git diff --check`
- 完成条件：
  - per-cone build 在所有 fixture 上稳定；
  - 文档冻结新边界。
- 依赖：P10-T04R

### [TODO] P10-T05R：Review P10 全包完成度

- 参考：P10-T05。
- 重点：
  - 下游 cone 是否真的不再读上游源；
  - per-cone fingerprint cache 的命中行为是否符合预期；
  - 是否还有遗漏的 path 上游源加载路径需要清理。
- 验证：重新运行 P10-T05 的所有验证。
- 依赖：P10-T05
