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
| `effect/` | 12 | ~10k | `scoopc_effect_facts_stage` |
| `effect_facts/` | 7 | ~9k | `scoopc_effect_facts_stage`（builder） / `scoopc_effect_facts`（已是独立 crate，仅需吸收 builder 端契约） |
| `effect_lowered/` | 16 | ~25k | `scoopc_lir` |
| `llvm/` | 125 | ~82k | `scoopc_codegen_llvm` |
| `pipeline/` | 10 | ~18k | 留在 umbrella `scoopc`（driver 编排） |
| `cone/` | 13 | ~5k | 数据/FS 层进 `scoopc_project_model`；操作层进 `scoopc_cone` |

### 阻塞 stage crate split 的后向边（P9-T01 必须在第一步消除）

| 位置 | 当前用法 | 处置 |
| --- | --- | --- |
| `crates/scoopc/src/hir/lower/util/mod.rs:11` | `use crate::mir::{InstanceKey, TemplateKey}` | 把 `InstanceKey` / `TemplateKey` 移到 `scoopc_ids`，HIR/MIR 都引用 base 即可。 |
| `crates/scoopc/src/typecheck/annotations.rs:24` | `use crate::hir::ExternAbi` | 把 `ExternAbi` 移到 `scoopc_types` 或 `scoopc_ids`。 |
| `crates/scoopc/src/effect_facts/builder.rs:11,21` | `use crate::resolve::{FunOverload, Index}` + `crate::typecheck::{TypeEnv, TypeLowering, TypeSymbol}` | builder 和 effect 一并归 `scoopc_effect_facts_stage`，但它依赖 HIR resolver/typecheck；按 PLAN §1.2 这是合法（前一阶段 crate 依赖），无需消除，仅需在 P9-T06 时正确放置。 |
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

## [DONE] TODO-7-INIT：初始化并细化本任务包

- 目标：
  - 分析 `PLAN.md` §4/P9-P10、`PIPELINE_REFACTOR.md` 与当前 stage crate 拆分阻塞、cone 两层归属、per-cone build artifact 真实需求；
  - 生成本任务包的详细任务列表，覆盖 stage crate split、cone two-layer split、per-cone artifact 设计、TypeStore stable wire format、per-cone frontend 编排、fingerprint cache；
  - 更新 `TODO.md` 的具体任务索引，把 `TODO-7-INIT` 所在行替换为本包细化后的任务列表。
- 必须实现的内容：
  1. 复核"触碰面基线"是否覆盖 P9-T01..T08、P10-T01..T05 的所有起点。
  2. 把 P9 / P10 拆成数量适中的实现小阶段，每个阶段必须有明确目标、修改范围、验证命令和完成条件。
  3. 在每个实现小阶段后插入独立 review 任务，review 任务必须复审前一阶段是否满足 PLAN §1.2 的 crate DAG 约束或 per-cone artifact 边界约束。
  4. 同步更新 `TODO.md` 中的具体任务索引，确保任务 ID、状态和顺序与本文件一致。
- 完成条件：
  - `TODO-7.md` 不再只是范围说明，而是包含可直接执行的详细任务列表；
  - `TODO.md` 的具体任务索引已经同步反映 `TODO-7.md` 的新任务和 `[TODO]` 状态；
  - 本任务完成记录说明 P9/P10 的拆分依据与未展开的风险。
- 依赖：P8-T02R
- 完成记录：
  - 2026-05-23：完成 P9/P10 任务包细化复核；本文件已包含 P9-T01..T09、P10-T01..T05 及每个实现阶段后的 review 任务，根索引 `TODO.md` 同步列出这些任务。
  - 拆分依据：P9 严格先硬化 stage/codegen/cone crate DAG，再由 P10 处理 `TypeId` stable wire format、per-cone artifact IO、DAG frontend 编排与 fingerprint cache，避免在 crate 边界未收口前引入持久化格式。
  - 同步修正：统一 effect facts stage crate 名为 `scoopc_effect_facts_stage`（与 `PIPELINE_REFACTOR.md` 和现有 `dependency_gate` 预留名一致），并把 P9-T03 登记的 LLVM 临时 façade 依赖切换点明确为 P9-T06。
  - 未展开风险：跨 cone generic body wire format 不纳入本包，若后续实测下游 re-lower generic 成本不可接受，应在 P10 清场后另立 P11；P10 也必须在 P10-T01 先解决 `TypeId` 跨进程稳定表示后才能落盘 facts/LIR。

---

## P9：Stage crate split

将 PLAN.md §1.2 要求的 stage / fact crate DAG 用 cargo crate 边界硬化。

### [DONE] P9-T01-a：修复 P9-T01 前置的 LLVM/HIR-MIR residual baseline

- 阻塞原因：
  - 执行 P9-T01 指定的残余搜索前置检查时，当前树仍在 `crates/scoopc/src/llvm` 命中 `use crate::hir`，并且 LLVM codegen 路径仍大量通过 `crate::mir` 消费 raw MIR/LIR source-payload 兼容类型。
  - P9-T01 的目标明确要求验证 P7 已经清掉 `llvm/** -> hir/mir` import；若仍有残留，必须先回到 P7 cleanup 边界修复，不能在 P9 stage split 中用 façade 或改窄搜索范围绕开。
  - 这些残留会阻塞后续 `scoopc_codegen_llvm` 独立 crate，因为该 crate 不能依赖 `scoopc_hir` 或 raw `scoopc_mir` production API。
- 目标：
  - 在继续 P9-T01 之前，修复 LLVM/effect-lowered 对 HIR/MIR 的 residual baseline；
  - 将仍属 production 的 HIR/MIR 依赖迁到正确 owner（LIR/LIR facts/base context 窄合同或 base crate），删除真正死代码；
  - 只允许测试或明确 LIR-owned payload helper 保留必要引用，并在完成记录中说明分类依据。
- 必须修改的主要位置：
  - `crates/scoopc/src/llvm/**`
  - `crates/scoopc/src/effect_lowered/**`
  - `crates/scoopc_lir_facts/`（如需发布缺失合同）
  - `crates/scoopc/src/pipeline/lir_facts_builder.rs`（如需填充 LIR facts）
  - `tools/scoop_tools/src/dependency_gate.rs`（如需补强 source boundary）
- 必须实现的内容：
  1. 审计 `rg -n "use crate::hir|crate::hir::|use crate::mir|crate::mir::" crates/scoopc/src/llvm crates/scoopc/src/effect_lowered` 的命中，区分 production、测试和已文档化窄合同。
  2. 对 production residual 做 owner 修复：codegen-neutral 小合同进 base/LIR facts，LIR source payload 类型进 LIR owned surface，死的 raw MIR fallback 删除；不得新增 `scoopc` façade re-export 或仅改 import 形状来骗过搜索。
  3. 补强 `dependency_gate`，防止本任务清除的 LLVM HIR/MIR residual 回归。
  4. 更新 P7/P9 相关完成记录说明剩余允许命中的分类；若不能完全清除生产命中，必须继续保持 P9-T01 未完成。
- 验证：
  1. `cargo fmt`
  2. `cargo build --workspace`
  3. `cargo test --all --all-targets`
  4. `cargo run -p scoop_tools -- dependency-gate`
  5. 残余搜索：`rg -n "use crate::hir|crate::hir::" crates/scoopc/src/llvm crates/scoopc/src/effect_lowered`；`rg -n "use crate::mir|crate::mir::" crates/scoopc/src/llvm`
  6. `git diff --check`
- 完成条件：
  - P9-T01 可以在不绕开、不缩窄搜索范围的情况下继续执行；
  - LLVM production codegen 不再依赖 `scoopc_hir` 或 raw `scoopc_mir` API；
  - 所有剩余测试/窄合同命中均有明确 owner 和 dependency-gate 覆盖。
- 依赖：TODO-7-INIT
- 完成记录：
  - 2026-05-23：修复 P9-T01 前置 residual baseline。`llvm/frontend.rs` 的 single-file handoff 改为承载 `CodegenLoweringOutput`，`LlvmEmitError` 不再直接保存 HIR lowering error variant；pipeline 内需要转换的 HIR stage error 统一映射为 LLVM frontend-stage diagnostic。
  - 发布 LIR-owned source payload namespace：`effect_lowered::source` 承载 HIR-shaped source payload，`effect_lowered::mir_source` 承载 MIR-shaped source payload，避免同名 `CallArg` 等类型混淆；LLVM production 中直接 `crate::hir` 命名已清到测试和该 LIR-owned namespace 边界内，非 `mir_body` source-body helper 的直接 `crate::mir` 命名也已切到 `mir_source`。
  - dependency gate 补强：新增递归 source-boundary 规则，覆盖 `crates/scoopc/src/llvm` production 直接 HIR residual，以及排除测试和 `codegen/mir_body/` 后的直接 MIR residual；规则同时预防未来 split 后的 `scoopc_hir` / `scoopc_mir` / `scoopc::hir` / `scoopc::mir` 回归。
  - residual 搜索分类：`rg -n "use crate::hir|crate::hir::" crates/scoopc/src/llvm crates/scoopc/src/effect_lowered` 只剩 `effect_lowered::source` 的 LIR-owned HIR payload re-export 与 LLVM 测试命中；`rg -n "use crate::mir|crate::mir::" crates/scoopc/src/llvm` 剩余命中位于测试和 `llvm/codegen/mir_body/**` LIR-owned source-body helper，后者是已发布 source callable/body payload 的 lowering surface，不是 pass-view/body discovery fallback。
  - 验证通过：`cargo fmt`；`cargo build --workspace`；`cargo test --all --all-targets`；`cargo run -p scoop_tools -- dependency-gate`；上述两条 residual 搜索；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。

### [DONE] P9-T01：消除阻塞 stage crate split 的后向边

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
- 依赖：P9-T01-a
- 完成记录：
  - 2026-05-23：`InstanceKey` / `TemplateKey` 从 `mir::materialize` 下沉到 `scoopc_ids`，`mir` 仅保留 façade re-export；HIR lowering 中的实例 key 引用已改为 base crate，`crates/scoopc/src/hir` 对 `crate::mir` 的直接引用归零。
  - `ExternAbi` 下沉到 `scoopc_types`，`hir` 保留兼容 re-export；`typecheck::annotations` 改为引用 type base crate，`typecheck` 对 `use crate::hir` 的命中归零。
  - 删除 root-level `crates/scoopc/src/devirtualize.rs` 与 `pub(crate) mod devirtualize;`；仍被 MIR pass 使用的 dispatch 去虚化 helper 迁入 MIR-owned `dispatch_devirtualize`，测试中的 helper 调用改走 `crate::mir`。
  - 残余搜索结果：`use crate::mir` 在 `scoopc/src/hir` 与 `scoopc/src/typecheck` 内无命中；`use crate::hir` 在 `scoopc/src/typecheck` 内无命中，LLVM 仅剩 `llvm/tests/mod.rs` 测试命中，`effect_lowered` 仅剩 P9-T01-a 已记录的 LIR-owned source payload namespace。
  - 验证通过：`cargo fmt`；`cargo build --workspace`；`cargo test --all --all-targets`；`cargo run -p scoop_tools -- dependency-gate`；指定残余搜索；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。

### [DONE] P9-T01R：Review 后向边消除结果

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
- 完成记录：
  - 2026-05-23：复核 P9-T01 后向边消除结果。`InstanceKey` / `TemplateKey` 已由 `scoopc_ids` 拥有，HIR 与 MIR 通过 base identity 使用；`ExternAbi` 已由 `scoopc_types` 拥有，typecheck 不再通过 HIR 读取 ABI 数据；`devirtualize.rs` 已删除。
  - Review 修正：发现 `typecheck/lower.rs` 仍通过 `crate::hir::EFFECT_ROW_PARAM_DECL_FILE` 读取 effect-row 参数内部标记。已将该标记下沉到 `scoopc_types`，同步切换 typecheck 与 MIR materialize 调用点，HIR 仅保留 crate-internal re-export 供自身 lowering 模块使用。
  - 防回归：`dependency_gate` 新增 source-tree 规则，覆盖 `crates/scoopc/src/hir` 对 MIR 的直接 residual 与 `crates/scoopc/src/typecheck` 对 HIR 的直接 residual，包含当前 monolith 路径和未来 split 后的 `scoopc_*` / `scoopc::` façade 路径。
  - 残余搜索结果：`use crate::mir` 在 `scoopc/src/hir` 与 `scoopc/src/typecheck` 内无命中；`use crate::hir` 在 `scoopc/src/typecheck` 内无命中；LLVM 的 `use crate::hir` 仅剩 `llvm/tests/mod.rs` 测试命中；`effect_lowered` 的 `use crate::hir` 仍是 P9-T01-a 已记录的 LIR-owned source payload namespace。
  - 验证通过：`cargo fmt`；`cargo build --workspace`；`cargo test --all --all-targets`；`cargo run -p scoop_tools -- dependency-gate`；指定残余搜索；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。

### [DONE] P9-T02：抽出 `scoopc_ast` crate

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
- 完成记录：
  - 2026-05-23：新建 `crates/scoopc_ast` 并把 `crates/scoopc/src/{ast,parser,syntax}` 整体迁入该 crate；`scoopc_ast` root re-export AST 节点，内部通过 base adapter 使用 `scoopc_source` / `scoopc_span` / `scoopc_types`。
  - `scoopc` umbrella 已切换为 `pub use scoopc_ast as ast;`、`pub use scoopc_ast::parser;` 与 `pub use scoopc_ast::syntax;`，现有 `scoopc::{ast,parser,syntax}` 路径保持可用；抽取后暴露 `File::safe_member_access_resolved_entries()` 作为只读快照，避免跨 crate 测试直接读取 AST side-table 字段。
  - `dependency_gate` 激活 `scoopc_ast` 的 base-only stage 检查，并把已迁移的 lexer/parser source-boundary 路径更新到 `crates/scoopc_ast/src/**`；gate 输出确认 `scoopc_ast [base-only-stage]` 的 workspace base 依赖仅为 `scoopc_source`、`scoopc_span`、`scoopc_types`。
  - 验证通过：`cargo fmt`；`cargo build --workspace`；`cargo test --all --all-targets`；`cargo run -p scoop_tools -- dependency-gate`；`cargo clippy --all-targets -- -D warnings`；`cargo tree -p scoopc_ast`；`git diff --check`。

### [DONE] P9-T02R：Review `scoopc_ast` 抽取

- 参考：P9-T02。
- 重点：
  - `scoopc_ast` 是否真的只依赖 base crates；
  - façade re-export 是否覆盖所有公共 API；
  - 是否有重复定义（迁移残留）。
- 验证：重新运行 P9-T02 的所有验证。
- 依赖：P9-T02
- 完成记录：
  - 2026-05-23：复核 `scoopc_ast` 抽取结果。`crates/scoopc/src/{ast,parser,syntax}` 实体目录已不存在，`scoopc` umbrella 通过 `pub use scoopc_ast as ast;`、`pub use scoopc_ast::parser;`、`pub use scoopc_ast::syntax;` 保持原公共入口；`scoopc_ast` crate root 继续 `pub use ast::*`，覆盖原 `scoopc::ast::*` AST 节点入口。
  - 依赖边界：`cargo tree -p scoopc_ast` 显示 workspace 依赖仅为 `scoopc_source`、`scoopc_span`、`scoopc_types`；`dependency-gate` 将 `scoopc_ast` 作为 `base-only-stage` 检查通过，且 lexer/parser source-boundary 规则已指向 `crates/scoopc_ast/src/**`。
  - 迁移残留：复核 `crates/scoopc/src` 下已无 `pub mod ast/parser/syntax` 或实体模块定义，未发现重复定义；历史文档中的旧路径仅为归档/完成记录引用，不构成 production 残留。
  - 验证通过：`cargo fmt`；`cargo build --workspace`；`cargo test --all --all-targets`；`cargo run -p scoop_tools -- dependency-gate`；`cargo clippy --all-targets -- -D warnings`；`cargo tree -p scoopc_ast`；`git diff --check`。

### [DONE] P9-T03：抽出 `scoopc_codegen_llvm` crate

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
  1. `scoopc_codegen_llvm` 依赖 `scoopc_lir`（在 P9-T06 之前需要先把 `effect_lowered` re-export 暴露好；本任务允许暂时直接依赖 umbrella `scoopc` 的 façade，但必须在完成记录里登记，等 P9-T06 完成后立即切换）。
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
  - `cargo tree -p scoopc_codegen_llvm` 不显示 `scoopc_hir` / `scoopc_mir` / `scoopc_effect_facts` 任何 stage crate（只能通过 façade `scoopc` 间接，且必须在 P9-T06 完成后切到 `scoopc_lir` 直依赖）；
  - run-pass fixtures 全部通过。
- 依赖：P9-T02R
- 完成记录：
  - 2026-05-24：新建 `crates/scoopc_codegen_llvm` 并把 `crates/scoopc/src/llvm` 与 backend-only `stackmap.rs` 物理迁入该 crate 路径；`scoopc` 的 `llvm` / `stackmap` 入口改为 `#[path]` façade，保持 `scoopc::llvm` 与 `scoopc::stackmap` 原路径可用。
  - 临时依赖登记：由于 P9-T06 前 `scoopc_lir` 尚未抽出，若让 `scoopc` 直接依赖 `scoopc_codegen_llvm` 且 backend crate 再依赖 `scoopc` façade 会形成 Cargo cycle。本任务采用 staged extraction：`scoopc_codegen_llvm` 暂时通过 `scoopc` façade re-export backend API，`llvm = ["scoopc/llvm"]` 保持原 LLVM feature 门；P9-T06 完成 `scoopc_lir` 抽取后必须切换为 direct `scoopc_lir` / `scoopc_lir_facts` 输入。
  - dependency gate 已激活 `scoopc_codegen_llvm` 的 codegen-stage 检查，并把 LLVM source-boundary 规则迁到 `crates/scoopc_codegen_llvm/src/llvm/**`；用户可见 failure policy 与 stable-id audit 的源路径也同步到新位置。
  - `cargo tree -p scoopc_codegen_llvm` 显示当前直接依赖为临时 `scoopc` façade；没有直接 `scoopc_hir` / `scoopc_mir` stage crate，后续 LIR 抽取完成后按本记录移除 façade 依赖。
  - 验证通过：`cargo fmt`；`cargo check --workspace --features llvm`；`cargo build --workspace --features llvm`；`cargo test --all --all-targets --features llvm`（用 30 分钟超时重跑通过；单独复现 `once_guard_cross_dylib` 1.22s 通过，确认此前 20 分钟超时是整条测试命令总时长不足）；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`；`cargo run -p scoop_tools -- dependency-gate`；`cargo clippy --all-targets --features llvm -- -D warnings`；`cargo tree -p scoopc_codegen_llvm`；`git diff --check`。

### [DONE] P9-T03R：Review `scoopc_codegen_llvm` 抽取

- 参考：P9-T03。
- 重点：
  - 是否仍存在 LLVM crate 反向使用 HIR/MIR 的迹象；
  - feature gate 是否正确传播；
  - 完成记录是否登记了 P9-T06 后的依赖切换义务。
- 验证：重新运行 P9-T03 的所有验证。
- 依赖：P9-T03
- 完成记录：
  - 2026-05-24：复核 `scoopc_codegen_llvm` 抽取结果。当前 crate root 仍按 P9-T03 记录进行 staged extraction：`llvm = ["scoopc/llvm"]` 传播原 LLVM feature gate，`src/lib.rs` 临时 re-export `scoopc::llvm` / `scoopc::stackmap`，实际 backend 源码由 `scoopc` 通过 `#[path]` 编译；P9-T06 后切换到 direct `scoopc_lir` / `scoopc_lir_facts` 输入的义务已在 P9-T03 完成记录和 crate root 注释中登记。
  - 反向边复核：`cargo tree -p scoopc_codegen_llvm` 显示直接 workspace 依赖仅为临时 `scoopc` façade；`cargo tree -p scoopc_codegen_llvm --no-default-features` 不拉入 LLVM/inkwell 依赖。残余搜索中，非测试 LLVM HIR residual 无命中；排除测试与 `codegen/mir_body/**` 后的 LLVM MIR residual 无命中。剩余测试命中与 `mir_body` source-body helper 命中仍是 P9-T01-a/P9-T03 记录的允许分类。
  - 验证通过：`cargo fmt`；`cargo build --workspace --features llvm`；`cargo test --all --all-targets --features llvm`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`；`cargo run -p scoop_tools -- dependency-gate`；`cargo clippy --all-targets --features llvm -- -D warnings`；`cargo tree -p scoopc_codegen_llvm`；`cargo tree -p scoopc_codegen_llvm --no-default-features`；LLVM HIR/MIR residual 搜索；`git diff --check`。

### [DONE] P9-T04：抽出 `scoopc_hir` crate

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
- 完成记录：
  - 2026-05-24：新建 `crates/scoopc_hir` 并把 `hir/`、`resolve/`、`typecheck/`、`infer/`、`intrinsics.rs`、`expr_facts.rs`、`vtable.rs`、`itable.rs` 迁入该 crate；`scoopc` 通过 façade re-export 保持 `scoopc::{hir, resolve, typecheck, infer}` 以及相关前端 helper 路径可用。
  - 前端辅助面：为避免 `scoopc_hir` 反向依赖 umbrella crate，随 HIR 迁入当前 HIR/typecheck API 必需的 `session`、`sysroot`、`target`、`warnings`、`stable_id`、`dump_support` 与 monomorph request key 数据；`scoopc::monomorph` 仍保留 MIR dump wrapper 并 re-export HIR-owned request key。`sysroot` 迁入 HIR 后，P9-T07 仍负责最终 cone 数据/FS 层与 project-model 归属收口。
  - dependency gate 已激活 `scoopc_hir` 的 HIR-stage crate 检查，允许依赖 base crates、`scoopc_ast` 与 `scoopc_hir_facts`，并把 HIR/typecheck source-boundary 扫描路径迁到 `crates/scoopc_hir/src/**`。
  - 测试整理：原 HIR lowering 中依赖 umbrella pipeline stage dump 或 via-MIR/codegen request-root collection 的 tests 移到 `scoopc::pipeline` 测试模块；HIR crate 自身测试继续覆盖纯前端 lowering、resolve、typecheck、sysroot/session helper。
  - 验证通过：`cargo fmt`；`cargo check --workspace`；`cargo build --workspace`；`cargo test --all --all-targets`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`；`cargo run -p scoop_tools -- dependency-gate`；`cargo clippy --all-targets -- -D warnings`；`cargo tree -p scoopc_hir`；`git diff --check`。

### [DONE] P9-T04R：Review `scoopc_hir` 抽取

- 参考：P9-T04。
- 重点：
  - 是否有反向边逃逸到 `scoopc_mir` / `scoopc_codegen_llvm`；
  - vtable/itable 是否真的归属在前端而非后端；
  - 残留 `use crate::*` import 是否全部转为 crate-local 或 base 引用。
- 验证：重新运行 P9-T04 的所有验证。
- 依赖：P9-T04
- 完成记录：
  - 2026-05-24：复核 `scoopc_hir` 抽取结果。`scoopc_hir` 的 workspace 依赖保持在 base crates、`scoopc_ast` 与 `scoopc_hir_facts` 范围内；`cargo tree -p scoopc_hir` 未显示 `scoopc_mir` 或 `scoopc_codegen_llvm`。
  - 反向边检查：`crates/scoopc_hir/src` 未发现 `scoopc_mir`、`scoopc_codegen_llvm`、`scoopc::mir`、`scoopc::llvm`、`crate::mir` 或 `crate::llvm` 命中；`use crate::*` / `pub use crate::*` 也无命中，剩余 `crate::` 引用均为 `scoopc_hir` crate-local 模块或 base façade。
  - vtable/itable 归属：`vtable.rs` 与 `itable.rs` 只存在于 `crates/scoopc_hir/src/`，由 HIR lowering/typecheck 前端路径收集并发布表数据；LLVM backend 仍只消费该前端产物，不拥有 vtable/itable 构建逻辑。
  - 验证通过：`cargo fmt`；`cargo build --workspace`；`cargo test --all --all-targets`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`；`cargo run -p scoop_tools -- dependency-gate`；`cargo clippy --all-targets -- -D warnings`；`cargo tree -p scoopc_hir`；`git diff --check`。

### [DONE] P9-T05：抽出 `scoopc_mir` crate

- 参考：本文件"跨切面 helper 的归属决策"。
- 目标：
  - 把 `mir/` + `monomorph/` + `rtti/` + `stable_id` 中的 mangler 段一起迁到 `scoopc_mir`；
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
- 完成记录：
  - 2026-05-24：新建 `crates/scoopc_mir` 并把 `mir/`、`monomorph/`、`rtti/` 迁入该 crate；`scoopc` umbrella 改为 façade re-export `scoopc_mir::{mir, monomorph, rtti, stable_id}`，工作区注册新 crate，`scoopc` 的 `llvm` feature 同步传递到 `scoopc_mir/llvm`。
  - HIR facts 构建面从 umbrella pipeline 下沉到 `scoopc_hir::stage`，删除 `scoopc/src/pipeline/{hir_stage,hir_completeness}.rs` 的旧实际定义，消除 `scoopc_mir -> scoopc` 后向边；`pipeline/mir_stage.rs` 与现有 effect/LIR/codegen 消费面改为跨 crate 可见的 MIR 查询 API。
  - MIR-owned pass pipeline、dispatch devirtualization、escape analysis、inlining、materialized pass view、RTTI/type descriptor helper 均随 `scoopc_mir` 编译；RTTI 运行期算法/descriptor 生成留在 `scoopc_mir::rtti`，纯 facts schema 未迁出 `scoopc_hir_facts`；shared stable key 与 ABI/private mangler primitive 继续由 `scoopc_ids` 拥有，`scoopc_mir::stable_id` 作为 MIR-facing symbol facade 暴露当前 materialization/codegen 所需 surface，避免后续 codegen 直接依赖 HIR stable-id façade。
  - `dependency_gate` 激活 `scoopc_mir` stage 检查：允许 base + `scoopc_ast` + `scoopc_hir` + `scoopc_hir_facts` + `scoopc_mir_facts`，禁止 façade/effect/LIR/codegen 后向依赖；`cargo tree -p scoopc_mir` 未显示 `scoopc_effect_*`、`scoopc_lir` 或 `scoopc_codegen_llvm`。
  - 验证通过：`cargo fmt`；`cargo build --workspace`；`cargo test --all --all-targets`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`；`cargo run -p scoop_tools -- dependency-gate`；`cargo clippy --all-targets -- -D warnings`；`cargo tree -p scoopc_mir`；`git diff --check`。

### [DONE] P9-T05R：Review `scoopc_mir` 抽取

- 参考：P9-T05。
- 重点：
  - RTTI 归属决策是否落实；
  - opt pipeline 是否完整迁移；
  - mangler 段是否切干净。
- 验证：重新运行 P9-T05 的所有验证。
- 依赖：P9-T05
- 完成记录：
  - 2026-05-24：复核 `scoopc_mir` 抽取结果。`crates/scoopc/src/{mir,monomorph,rtti}` 与旧 `stable_id.rs` 实体已不存在，`scoopc` umbrella 仅通过 `pub use scoopc_mir::{mir, monomorph, rtti, stable_id}` 保留兼容入口。
  - RTTI 归属：`dump-rtti` / type descriptor 的运行期算法与 descriptor 生成实现位于 `scoopc_mir::rtti`；source-semantic dispatch table facts 仍由 `scoopc_hir_facts` 拥有，未发现 `scoopc_mir` 复制 fact schema 或依赖 effect/LIR/codegen crate。
  - MIR opt pipeline 归属：`pass_pipeline.rs`、dispatch devirtualization、summary-driven inlining、escape analysis 与 closure simplification 均在 `crates/scoopc_mir/src/mir/**` 下运行，materialization 结束后由 `run_mir_pass_pipeline` 统一调度。
  - Mangler / stable-id 复核：共享 `AbiMangler` / `PrivateSymbolMangler` primitive 继续由 base `scoopc_ids` 拥有；`scoopc_mir::stable_id` 作为 MIR-facing stable symbol surface 暴露 materialization/codegen 所需 key，materialized MIR 负责发布 stable exported symbol surface。Review 中同步澄清了 generic MIR 与 materialized exported symbol 的模块注释边界。
  - 依赖边界：`cargo tree -p scoopc_mir` 未显示 `scoopc_effect_*`、`scoopc_lir` 或 `scoopc_codegen_llvm`；`dependency-gate` 将 `scoopc_mir` 作为 MIR-stage 检查通过。
  - 验证通过：`cargo fmt`；`cargo build --workspace`；`cargo test --all --all-targets`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`；`cargo run -p scoop_tools -- dependency-gate`；`cargo clippy --all-targets -- -D warnings`；`cargo tree -p scoopc_mir`；`git diff --check`。

### [DONE] P9-T06-a：收窄 LIR 的 HIR/AST source payload 边界

- 阻塞原因：
  - P9-T06 要求 `scoopc_lir` 不再依赖 HIR/AST，并在抽取后让 LLVM codegen 直接依赖 `scoopc_lir` / `scoopc_lir_facts`，不能继续通过 `scoopc` façade 或 `scoopc_mir` façade 隐藏 HIR/AST payload。
  - 当前 LIR 实现仍在 `crates/scoopc/src/effect_lowered/ir.rs` 通过 `source { pub use crate::hir::*; }` 发布 HIR-shaped source payload，并在 `crates/scoopc/src/effect_lowered/builder.rs` 的 class ctor init 构建路径直接匹配 `crate::ast::CtorDelegationKind`。
  - `cargo tree -p scoopc_mir --depth 1` 当前直接显示 `scoopc_ast` 与 `scoopc_hir`；如果 P9-T06 的 `cargo tree -p scoopc_lir` 完成条件按 full transitive tree 字面执行，会与 P9-T05 已完成的 MIR stage 依赖形状以及 PLAN §1.2 允许依赖前一阶段 crate 的规则冲突。该校验基线必须先修正为可由 cargo/dependency-gate 强制的 direct stage dependency 规则，或先发布一个 HIR/AST-free 的 MIR/LIR 输入 crate/surface。
- 目标：
  - 发布 LIR-owned source/class-ctor payload 合同，使 LIR builder 和 IR 类型不直接命名 `crate::hir` / `scoopc_hir` / `crate::ast` / `scoopc_ast`。
  - 把 class ctor init 所需的 delegation kind、ctor call、param default/body/init-block payload 等收窄到 MIR/LIR-owned 输入面，避免 P9-T06 抽 crate 时用 HIR/AST façade 绕过依赖约束。
  - 修复 P9-T06 的依赖校验基线：明确 `scoopc_lir` 的 direct dependencies 不含 HIR/AST/umbrella façade，并由 `dependency_gate` 覆盖；若需要新增更窄的 MIR input crate/surface，必须在本任务中落地并同步 P9-T06 依赖描述。
- 必须修改的主要位置：
  - `crates/scoopc/src/effect_lowered/ir.rs`
  - `crates/scoopc/src/effect_lowered/builder.rs`
  - `crates/scoopc_mir/src/**`（发布 HIR/AST-free class ctor/source payload 或更窄 MIR input surface）
  - `tools/scoop_tools/src/dependency_gate.rs`（加入 direct dependency / source-boundary 防回归规则）
  - `TODO-7.md` 中 P9-T06 的依赖和验证描述（如校验基线需要结构性修正）
- 必须实现的内容：
  1. 消除 production LIR 代码中对 `crate::hir` / `scoopc_hir` / `crate::ast` / `scoopc_ast` 的直接引用，不允许改走 `scoopc_mir::hir` 或 `scoopc::hir` 作为 façade workaround。
  2. 为 class ctor init body lowering 发布明确 owner 的 LIR/MIR source payload 数据，覆盖当前 `MonoClassInit` / `ClassCtor` / `ClassInitStep` / `CtorDelegationKind` 路径。
  3. 为 P9-T06 准备可执行的依赖验证：`scoopc_lir` 抽取后 direct dependencies 不含 HIR/AST/`scoopc`，且 full dependency tree 的解释不与 PLAN §1.2 的 stage DAG 规则冲突。
  4. 补强 `dependency_gate`，防止 LIR source tree 重新出现 HIR/AST direct residual 或通过 umbrella façade 回流。
- 验证：
  1. `cargo fmt`
  2. `cargo build --workspace`
  3. `cargo test --all --all-targets`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  5. `cargo run -p scoop_tools -- dependency-gate`
  6. 残余搜索：`rg -n "crate::hir|scoopc_hir|crate::ast|scoopc_ast|scoopc::hir|scoopc::ast|scoopc_mir::hir|scoopc_mir::ast" crates/scoopc/src/effect_lowered`
  7. `git diff --check`
- 完成条件：
  - P9-T06 可以在不使用 façade workaround、不弱化 LIR 输入模型的情况下创建 `scoopc_lir`；
  - production LIR 代码不直接命名 HIR/AST owner；
  - P9-T06 的依赖验证口径已与 PLAN §1.2 和实际 crate DAG 对齐。
- 依赖：P9-T05R
- 完成记录：
  - 2026-05-24：发布 MIR-owned source payload 边界。`scoopc_mir::mir::source_payload` 承接当前 LLVM source-body helper 仍需的 HIR-shaped transitional payload；`effect_lowered::source` 不再直接 re-export HIR/AST owner，而是转发 MIR-owned source payload。
  - class ctor init payload 已收窄：新增 `class_ctor_source` / MIR-owned `MonoClassInit`、`ClassCtor`、`ClassCtorDelegationKind`、`CtorCallInfo` 等 LIR 输入面；`LateLoweredClassCtorInitBody` 与 builder 不再直接命名 `crate::hir` / `crate::ast`，delegation kind 也不再匹配 AST owner enum。
  - P9-T06 依赖验证口径已修正为 direct stage dependency：`scoopc_lir` 抽取后 `cargo tree -p scoopc_lir --depth 1` 与 `dependency-gate` 必须禁止 HIR/AST/`scoopc` direct dependency；full transitive tree 可继续反映 `scoopc_mir -> scoopc_hir/scoopc_ast` 的合法前阶段依赖，不作为 LIR direct-dep 失败条件。
  - dependency gate 补强：新增可选 `scoopc_lir` direct-dependency 检查、当前 `crates/scoopc/src/effect_lowered` 与未来 `crates/scoopc_lir/src` 的 HIR/AST source-boundary 规则，并跳过尚未创建的可选 stage crate/root。
  - 验证通过：`cargo fmt`；`cargo build --workspace`；`cargo test --all --all-targets`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`；`cargo run -p scoop_tools -- dependency-gate`；`cargo clippy --all-targets -- -D warnings`；LIR residual 搜索；`git diff --check`。

### [TODO] P9-T06-b：发布 LIR-owned ordinary-callee suspend 合同

- 阻塞原因：
  - P9-T06 必须把 `effect/` 与 `effect_facts/builder.rs` 抽入 `scoopc_effect_facts_stage`，同时完成 P9-T03 登记的义务，让 `scoopc_codegen_llvm` 不再经由 `scoopc` façade，而是 direct 依赖 `scoopc_lir` / `scoopc_lir_facts`。
  - 当前 `crates/scoopc_codegen_llvm/src/llvm/**` 的 production code 仍直接使用 `crate::effect::analysis::EffectAnalysisFacts`、`crate::effect::state_machine::CalleeSuspendPlan` 与 ordinary-callee suspend analysis helper；如果 P9-T06 直接移动 `effect/`，LLVM crate 要么继续依赖 effect stage，要么通过 façade/path 隐藏依赖，都会违反 P9-T06 的 codegen direct-LIR 输入边界。
  - 该依赖不是 HIR/AST source payload 问题，P9-T06-a 未覆盖；必须先把 LLVM 仍需的 ordinary-callee suspend 信息发布到 LIR-owned contract 或 LIR facts，才能无 workaround 地抽出 `scoopc_lir` 并切换 LLVM crate。
- 目标：
  - 发布 LIR-owned ordinary-callee suspend/effect-analysis contract，使 LLVM production code 不再直接命名 `crate::effect` / future `scoopc_effect_facts_stage`。
  - 保持当前 ordinary-callee suspend 行为不变；不得通过禁用 callee suspend lowering、弱化 fixture、复制 effect stage façade 或 backend 私有重算来绕过。
  - 为 P9-T06 准备可执行依赖边界：抽取后 `scoopc_codegen_llvm` 只需 direct `scoopc_lir` / `scoopc_lir_facts` 即可获得 codegen 所需 suspend contract。
- 必须修改的主要位置：
  - `crates/scoopc/src/effect/**`
  - `crates/scoopc/src/effect_lowered/**`
  - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`
  - `crates/scoopc_codegen_llvm/src/llvm/**`
  - `crates/scoopc_lir_facts/**`（如 contract 需要进入 data-only facts）
  - `tools/scoop_tools/src/dependency_gate.rs`（加入 codegen direct `effect` residual 防回归规则，如需要）
- 必须实现的内容：
  1. 识别 LLVM ordinary-callee suspend lowering 实际消费的最小数据，发布在 LIR-owned API 或 `scoopc_lir_facts` 中，而不是继续暴露 `effect/` stage API。
  2. 改造 `LlvmStageBaseContext` / emit handoff / codegen context，使其消费 LIR-owned suspend contract；production code 中不得再 import 或引用 `crate::effect::analysis`、`crate::effect::state_machine`、future `scoopc_effect_facts_stage::effect`。
  3. 保留当前 callee suspend 行为与 diagnostics；新增或更新 targeted tests 覆盖 ordinary callee suspend path。
  4. 补强 dependency/source-boundary 检查，防止 LLVM codegen 重新依赖 effect stage 或 `scoopc` façade 获得 suspend analysis。
- 验证：
  1. `cargo fmt`
  2. `cargo build --workspace`
  3. `cargo test --all --all-targets`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  5. `cargo run -p scoop_tools -- dependency-gate`
  6. 残余搜索：`rg -n "crate::effect::|scoopc_effect_facts_stage::effect|scoopc::effect" crates/scoopc_codegen_llvm/src/llvm crates/scoopc/src/pipeline/llvm_codegen_stage.rs`
  7. `git diff --check`
- 完成条件：
  - LLVM production code 不再直接依赖 effect stage API；ordinary-callee suspend contract 由 LIR-owned surface 或 LIR facts 提供。
  - P9-T06 可以把 `effect/` 移入 `scoopc_effect_facts_stage`，并把 `scoopc_codegen_llvm` 切到 direct `scoopc_lir` / `scoopc_lir_facts`，不需要 façade/path workaround。
- 依赖：P9-T06-a

### [TODO] P9-T06：抽出 `scoopc_effect_facts_stage` 与 `scoopc_lir` crate

- 参考：本文件"当前 `scoopc/src/` 主模块体量"。
- 目标：
  - `effect/` + `effect_facts/builder.rs` 进 `scoopc_effect_facts_stage`（与已有的 data crate `scoopc_effect_facts` 区分；前者是 builder + stage，后者是 fact 数据）；
  - `effect_lowered/` 进 `scoopc_lir`。
- 必须修改的主要位置：
  - 新建 `crates/scoopc_effect_facts_stage/` 与 `crates/scoopc_lir/`
  - `crates/scoopc/src/lib.rs` façade
  - `crates/scoopc/src/pipeline/{effect_facts_stage,effect_lowering_stage,lir_facts_builder}.rs` 改 import
  - dependency_gate 激活两个 crate
- 必须实现的内容：
  1. `scoopc_effect_facts_stage` 依赖：base + `scoopc_ast` + `scoopc_hir` + `scoopc_hir_facts` + `scoopc_mir` + `scoopc_mir_facts` + `scoopc_effect_facts`。
  2. `scoopc_lir` direct dependencies：base + `scoopc_mir` + `scoopc_mir_facts` + `scoopc_effect_facts` + `scoopc_lir_facts`（按 PLAN §1.2，LIR 不应再 direct 依赖 hir / ast / umbrella `scoopc`；full transitive tree 可包含 `scoopc_mir` 的合法前阶段依赖）。
  3. P9-T03 中登记的"`scoopc_codegen_llvm` 暂走 façade"义务在本任务完成后切换：`scoopc_codegen_llvm` 改为直接 `use scoopc_lir::...`。
- 验证：
  1. `cargo fmt`
  2. `cargo build --workspace`
  3. `cargo test --all --all-targets`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  5. `cargo run -p scoop_tools -- dependency-gate`
  6. `cargo tree -p scoopc_lir --depth 1`
  7. `git diff --check`
- 完成条件：
  - `cargo tree -p scoopc_lir --depth 1` 不显示 `scoopc_hir` / `scoopc_ast` / `scoopc`；
  - `cargo tree -p scoopc_codegen_llvm` 直接显示 `scoopc_lir`，不再绕 façade。
- 依赖：P9-T06-b

### [TODO] P9-T06R：Review `scoopc_effect_facts_stage` 与 `scoopc_lir` 抽取

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
  - 把 `cone/manifest.rs` / `package.rs` / `graph.rs` 与当前暂驻 `scoopc_hir` 的 `sysroot/` 全部迁到 base crate `scoopc_project_model`（这些只依赖 base）；
  - 把 `cone/archive.rs` / `scoopir/` / `annotations.rs` / `visibility.rs` / `pre_specialize.rs` / `consume.rs` 迁到新 crate `scoopc_cone`（操作层，依赖所有 stage crate）。
- 必须修改的主要位置：
  - `crates/scoopc_project_model/` 扩展
  - `crates/scoopc_hir/src/sysroot/`（P9-T04 后的临时前端归属）
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
  2. `cargo clippy --all-targets -- -D warnings`
  3. `cargo build --workspace`
  4. `cargo test --all --all-targets`
  5. `cargo run -p scoop -- test`（完整 fixture suite，至少 30 分钟 timeout）
  6. `cargo run -p scoop_tools -- dependency-gate`
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
  2. `cargo clippy --all-targets -- -D warnings`
  3. `cargo run -p scoop_tools -- dependency-gate`
  4. `cargo test --all --all-targets`
  5. `cargo run -p scoop -- test`（完整 fixture suite，至少 30 分钟 timeout）
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
