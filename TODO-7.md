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

### [DONE] P9-T06-b：发布 LIR-owned ordinary-callee suspend 合同

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
  - P9-T06 可以移动剩余 effect-facts builder/stage glue，并把 `scoopc_codegen_llvm` 切到 direct `scoopc_lir` / `scoopc_lir_facts`，不需要 façade/path workaround。
- 依赖：P9-T06-a
- 完成记录：
  - 2026-05-24：ordinary-callee suspend planning 从 `effect` owner 迁入 LIR-owned `effect_lowered::ordinary_callee` surface；旧 `scoopc::effect` module 已删除，LLVM production 改为通过 `crate::effect_lowered::ordinary_callee::{EffectAnalysisFacts, CalleeSuspendPlan, ...}` 消费 suspend contract。
  - `EffectAnalysisFacts` 改为 LIR-owned 窄数据结构；HIR facts 到该结构的转换留在 `pipeline::llvm_codegen_stage` handoff 内，避免未来 `scoopc_lir` 为 ordinary-callee planning 直接依赖 HIR facts/stage。source payload 继续通过 P9-T06-a 已发布的 LIR/MIR-owned namespace 使用。
  - 防回归：`dependency_gate` 新增 LLVM stage handoff 与 LLVM production source-tree 规则，禁止 `crate::effect::`、future `scoopc_effect_facts_stage::effect` 与 `scoopc::effect` 回流；LIR HIR/AST source-boundary 继续覆盖迁入的 ordinary-callee planner。
  - 验证通过：`cargo fmt`；`cargo build --workspace`；`cargo test --all --all-targets`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`；`cargo run -p scoop_tools -- dependency-gate`；`cargo clippy --all-targets -- -D warnings`；残余搜索 `rg -n "crate::effect::|scoopc_effect_facts_stage::effect|scoopc::effect" crates/scoopc_codegen_llvm/src/llvm crates/scoopc/src/pipeline/llvm_codegen_stage.rs` 无命中；`git diff --check`。

### [DONE] P9-T06-c：发布 codegen-owned LLVM stage handoff 合同

- 阻塞原因：
  - 执行 P9-T06 的 `scoopc_codegen_llvm -> scoopc_lir` direct dependency 切换时，当前 `crates/scoopc_codegen_llvm/src/llvm/{emit,frontend}.rs` 仍在 production API 中引用 `crate::frontend` 与 `crate::pipeline::{LlvmCodegenStageInput,LlvmCodegenStageOutput,LlvmStageBaseContext,LlvmArtifactKind}`。
  - 这些 handoff 类型和 single-file frontend orchestration 仍由 `scoopc` umbrella pipeline 拥有；如果直接移除 `scoopc_codegen_llvm` 对 `scoopc` 的依赖，LLVM crate 无法独立编译，若保留 `scoopc` façade 则 P9-T06 的完成条件不成立。
  - 该问题不是 LIR HIR/AST source payload 或 ordinary-callee suspend 合同问题，P9-T06-a/b 未覆盖；必须先把 LLVM backend 需要的 stage handoff/context 发布到 codegen-owned 或 codegen-neutral 窄合同，再完成 crate direct dependency 切换。
- 目标：
  - 让 `scoopc_codegen_llvm` 的 production source 能在不依赖 `scoopc` façade 的情况下编译；
  - 将 `LlvmStageBaseContext`、`LlvmCodegenStageOutput`、`LlvmCallableSourceContract`、`LlvmDispatchCallKey` 与 stage-output emit 输入收口到 backend-owned API，或发布等价的 codegen-neutral handoff crate/API；
  - 保持 `scoopc` pipeline 仍负责 frontend orchestration，但只构造 codegen-owned handoff，不再作为 LLVM crate 的 normal dependency。
- 必须修改的主要位置：
  - `crates/scoopc_codegen_llvm/src/llvm/{emit,frontend,codegen/**}`
  - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`
  - `crates/scoopc/src/lib.rs` 的 LLVM façade
  - `crates/scoopc_codegen_llvm/Cargo.toml`
  - `tools/scoop_tools/src/dependency_gate.rs`
- 必须实现的内容：
  1. 明确 `scoopc_codegen_llvm` production 所需 handoff 类型的 owner；不得继续通过 `scoopc` normal dependency、path façade 或 duplicated stub 来绕过 Cargo 边界。
  2. 改造 `emit`/stage-output API，使 codegen crate 消费 `LIR + LIR facts + backend base context` 的 direct handoff；single-file frontend helper 如仍保留，必须由 `scoopc` wrapper 编排或通过不引入 normal `scoopc` 依赖的接口注入。
  3. 更新 `scoopc` pipeline 构造新的 codegen-owned handoff，并保持现有 build/run/fixture 行为不变。
  4. 补强 `dependency_gate`：P9-T06 后 `scoopc_codegen_llvm` 不再允许 `scoopc` temporary façade dependency。
- 验证：
  1. `cargo fmt`
  2. `cargo check -p scoopc_codegen_llvm`
  3. `cargo build --workspace`
  4. `cargo test --all --all-targets`
  5. `cargo run -p scoop_tools -- dependency-gate`
  6. `cargo tree -p scoopc_codegen_llvm --depth 1`
  7. `git diff --check`
- 完成条件：
  - `cargo tree -p scoopc_codegen_llvm --depth 1` 不显示 `scoopc`；
  - `scoopc_codegen_llvm` production source 不再需要 `crate::frontend` / `crate::pipeline` umbrella owner 才能编译；
  - P9-T06 可以继续完成 codegen direct `scoopc_lir` / `scoopc_lir_facts` 切换。
- 依赖：P9-T06-b
- 完成记录：
  - 2026-05-24：LLVM stage handoff 类型改由 `scoopc_codegen_llvm::llvm` 拥有，包含 `LlvmStageBaseContext`、`LlvmCodegenStageOutput`、`LlvmCallableSourceContract`、`LlvmDispatchCallKey` 与 `LlvmArtifactKind`；`scoopc` pipeline 只负责从 frontend/HIR/MIR/effect facts 构造该 handoff。
  - `scoopc_codegen_llvm` 已移除 normal `scoopc` façade dependency，改为 direct 依赖 `scoopc_lir` / `scoopc_lir_facts` 与必要 base/backend crates；`scoopc::llvm` 保留单文件 frontend wrapper，现有 CLI/build helper API 继续可用但 orchestration 不进入 codegen crate。
  - 为保持 P9-T06 后续 LIR/codegen 边界可执行，补齐 LIR-owned source payload re-export、codegen-owned source-boundary dependency gate，并修复抽 crate 后暴露的 stage-test 编译与 in-process effect-facts handoff转换问题；P10 的 stable wire format 仍由后续 P10-T01 处理。
  - 验证通过：`cargo fmt`；`cargo check -p scoopc_codegen_llvm`；`cargo build --workspace`；`cargo test --all --all-targets`（60 分钟 timeout 完整通过）；`cargo run -p scoop_tools -- dependency-gate`；`cargo tree -p scoopc_codegen_llvm --depth 1`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。

### [DONE] P9-T06：抽出 `scoopc_effect_facts_stage` 与 `scoopc_lir` crate

- 参考：本文件"当前 `scoopc/src/` 主模块体量"。
- 目标：
  - `effect_facts/builder.rs` 与剩余 effect-facts stage glue 进 `scoopc_effect_facts_stage`（与已有的 data crate `scoopc_effect_facts` 区分；前者是 builder + stage，后者是 fact 数据；旧 `effect/` ordinary-callee planner 已由 P9-T06-b 迁入 LIR owner）；
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
- 依赖：P9-T06-c
- 完成记录：
  - 2026-05-24：`scoopc_effect_facts_stage` 与 `scoopc_lir` 已作为独立 workspace crate 接入；`scoopc` 仅保留 façade re-export，原 `effect_facts/builder.rs` 与 effect-facts stage glue 由 `scoopc_effect_facts_stage` 拥有，`effect_lowered/` 由 `scoopc_lir` 拥有。
  - `scoopc_lir` direct dependencies 为 base + `scoopc_mir` / `scoopc_mir_facts` / `scoopc_effect_facts` / `scoopc_lir_facts`，不 direct 依赖 `scoopc_hir` / `scoopc_ast` / umbrella `scoopc`；`dependency_gate` 已覆盖 LIR direct dependency 与 source-boundary residual。
  - P9-T03 登记的 LLVM 临时 façade 义务已闭合：`scoopc_codegen_llvm` direct 依赖 `scoopc_lir` / `scoopc_lir_facts`，`cargo tree -p scoopc_codegen_llvm --depth 1` 不显示 umbrella `scoopc`。
  - 验证通过：`cargo fmt`；`cargo build --workspace`；`cargo test --all --all-targets`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`；`cargo run -p scoop_tools -- dependency-gate`；`cargo tree -p scoopc_lir --depth 1`；`cargo tree -p scoopc_codegen_llvm --depth 1`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。

### [DONE] P9-T06R：Review `scoopc_effect_facts_stage` 与 `scoopc_lir` 抽取

- 参考：P9-T06。
- 重点：
  - LIR crate 是否摆脱 HIR；
  - `scoopc_codegen_llvm` 的依赖方向是否切换为 `scoopc_lir` 直依赖；
  - effect stage builder 是否在新 crate 内仍能正常构造 facts。
- 验证：重新运行 P9-T06 的所有验证。
- 依赖：P9-T06
- 完成记录：
  - 2026-05-24：复核 P9-T06 抽取结果。`cargo tree -p scoopc_lir --depth 1` 显示 `scoopc_lir` direct dependencies 仅为 base + `scoopc_mir` / `scoopc_mir_facts` / `scoopc_effect_facts` / `scoopc_lir_facts`，未 direct 依赖 `scoopc_hir` / `scoopc_ast` / umbrella `scoopc`。
  - LLVM 依赖方向已闭合：`cargo tree -p scoopc_codegen_llvm --depth 1` 显示 backend direct 依赖 `scoopc_lir` / `scoopc_lir_facts` 与必要 base/backend crates，不再显示 umbrella `scoopc` 或 effect stage builder crate。
  - Review 修正：发现 `dependency_gate` 尚未实际把 `scoopc_effect_facts_stage` 纳入受检 stage crate；已新增 `effect-facts-stage` crate kind、允许依赖白名单与拒绝后续 stage/umbrella 依赖的单测，确保 effect facts builder crate 不会反向依赖 LIR/codegen/umbrella。同步修正 `scoopc` façade 中关于 effect facts / LIR fact product 的过时注释。
  - effect facts builder 功能验证：`cargo test --all --all-targets` 覆盖并通过 `scoopc_effect_facts_stage` 的 16 个 builder/solver 单测，run-pass fixtures 全绿，确认新 crate 内仍能正常构造 facts。
  - 验证通过：`cargo fmt`；`cargo build --workspace`；`cargo test --all --all-targets`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`；`cargo run -p scoop_tools -- dependency-gate`；`cargo tree -p scoopc_lir --depth 1`；`cargo tree -p scoopc_codegen_llvm --depth 1`；`cargo tree -p scoopc_effect_facts_stage --depth 1`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。

### [DONE] P9-T07：cone 两层拆分（`scoopc_project_model` 扩展 + 新 `scoopc_cone`）

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
- 完成记录：
  - 2026-05-24：完成 cone 两层拆分。`cone/manifest.rs` / `package.rs` / `graph.rs` 与 sysroot path/FS 层已迁入 base crate `scoopc_project_model`；AST-holding `Sysroot { files }` 留在 `scoopc_hir::sysroot`，并通过 `scoopc_project_model::sysroot` 的 path-layer API 加载源文件。
  - 新增 `scoopc_cone` 操作层 crate，承载 `.cone` archive read/write、ScoopIR export、annotation classes、visibility tables、pre-specialize index 与 downstream consume/inject API；`scoopc_cone` 作为 cone 操作层位于所有 stage crate 之上（`scoopc_codegen_llvm` 以 `default-features = false` 进入直接依赖，避免为 cone API 启用 LLVM backend feature）；`scoopc::cone::*` 改为单文件 façade，继续 re-export project-model 数据层与 cone 操作层。
  - dependency gate 已把 `scoopc_cone` 纳入 forbidden workspace crate 集合，使 base/fact/stage crate 不能反向依赖 cone 操作层。
  - 验证通过：`cargo fmt`；`cargo build --workspace`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（421/421）；`cargo run -p scoop_tools -- dependency-gate`；`cargo tree -p scoopc_project_model`；`cargo tree -p scoopc_cone`；`git diff --check`。

### [DONE] P9-T07R：Review cone 两层拆分

- 参考：P9-T07。
- 重点：
  - cone 数据层是否真的只在 base；
  - cone 操作层是否独立成 crate 且无 stage crate 反向依赖；
  - sysroot loader 归属决策是否落地。
- 验证：重新运行 P9-T07 的所有验证。
- 依赖：P9-T07
- 完成记录：
  - 2026-05-24：复核 P9-T07 结果。`scoopc_project_model` direct workspace dependencies 仅为 base crates（`scoopc_ids` / `scoopc_source`，其传递边仍在 base DAG 内），未引用 AST/HIR/MIR/LIR/codegen crate；sysroot path-layer 由 `scoopc_project_model::sysroot` 拥有，`scoopc_hir::sysroot` 仅保留 AST-holding 加载层。
  - `scoopc_cone` direct dependencies 覆盖 `scoopc_ast`、`scoopc_hir`、`scoopc_mir`、`scoopc_effect_facts_stage`、`scoopc_lir`、`scoopc_codegen_llvm` 以及当前 operation 所需 fact/project/base crates；全仓 `scoopc_cone` 搜索显示除 umbrella façade 与 dependency gate 外无 stage/base/fact crate 反向引用。
  - Review 修正：发现 `scoopc_cone` 缺少 P9-T07 完成条件要求的 direct `scoopc_codegen_llvm` stage edge；已以 `default-features = false` 加入，保证 cone 操作层位于所有 stage crate 之上但不强制启用 LLVM backend。另发现 P9-T07 标题已标 `[DONE]`，但本文件缺少完成记录；本 review 已补齐 P9-T07 完成记录，保持 `TODO.md` / `TODO-7.md` 状态同步。
  - 验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo build --workspace`；`cargo test --all --all-targets`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（421/421）；`cargo run -p scoop_tools -- dependency-gate`；`cargo tree -p scoopc_project_model`；`cargo tree -p scoopc_cone`；`git diff --check`。

### [DONE] P9-T08：`scoopc` umbrella crate 收尾 + dependency_gate 全面强化

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
- 完成记录：
  - 2026-05-24：移除 `scoopc` umbrella 对 backend-private `inkwell` / `llvm-sys` 的直接 feature/dependency ownership，并清理 stale umbrella manifest deps、删除 `crates/scoopc/build.rs`；LLVM 21.1 toolchain 检查迁入 `scoopc_codegen_llvm/build.rs`，由 backend crate 自己在 `llvm` feature 下负责。
  - `scoopc::llvm` 收敛为 facade：backend API 直接 re-export `scoopc_codegen_llvm::llvm::*`，单文件 driver helper 移入 `pipeline/` 编排层；`scoopc::stackmap` 改为 re-export `scoopc_codegen_llvm::stackmap`，不再通过 `#[path]` 编译 backend 源文件。
  - `dependency_gate` 增加 `scoopc_cone` 专门 crate kind 与 cone/stage 方向测试；gate 现在检查 base、fact、AST/HIR/MIR/effect-facts/LIR/codegen stage 与 cone，并新增 umbrella backend-private dependency/facade 残留检查。
  - README crate overview 更新为最终 P9 crate 结构，明确 `scoopc` 只承担 umbrella facade 与 driver 编排，`scoopc_codegen_llvm` 拥有 LLVM backend 私有依赖，`scoopc_cone` 位于 stage/fact/base crate 之上且 stage 不反向依赖 cone。
  - 验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo build --workspace`；`cargo test --all --all-targets`；`cargo run -p scoop -- test`（1507/1507）；`cargo run -p scoop_tools -- dependency-gate`；`git diff --check`。

### [DONE] P9-T08R：Review umbrella 收尾

- 参考：P9-T08。
- 重点：
  - umbrella `scoopc` 是否仅剩 façade + driver；
  - dependency_gate 是否覆盖全部 stage / cone crate 反向边；
  - 文档（README / crate overview）是否同步。
- 验证：重新运行 P9-T08 的所有验证。
- 依赖：P9-T08
- 完成记录：
  - 2026-05-24：复核 P9-T08 umbrella 收尾结果。`scoopc` 当前保留 facade re-export、`frontend.rs`、`pipeline/`、CLI/session/driver helper 与测试审计模块；stage/backend/cone 实现均由独立 crate 拥有，`scoopc::llvm` 与 `scoopc::cone` 仅作兼容 re-export。
  - `dependency_gate` 覆盖 base、fact、AST/HIR/MIR/effect-facts/LIR/codegen stage 与 `scoopc_cone`，并包含 stage 反向依赖 cone、codegen 反向依赖 frontend stage、LIR 反向依赖 HIR/AST、umbrella backend-private dependency/facade residual 等规则和单测。
  - README crate overview 已同步最终 P9 crate 结构，明确 `scoopc` 是 umbrella facade/driver 编排，`scoopc_codegen_llvm` 拥有 LLVM 私有依赖，`scoopc_cone` 位于 stage/fact/base 之上且 stage 不反向依赖 cone。
  - 验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo build --workspace`；`cargo test --all --all-targets`；`cargo run -p scoop -- test`；`cargo run -p scoop_tools -- dependency-gate`；`git diff --check`。

### [DONE] P9-T09：P9 全包清场、文档同步与依赖审计

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
- 完成记录：
  - 2026-05-24：完成 P9 全包清场审计。`dependency-gate` 通过并列出 16 个 pipeline crate 分类（base/fact/stage/codegen/cone）与 549 个 source-boundary 文件检查；`cargo tree --depth 1` 复核 AST/HIR/MIR/effect-facts/LIR/codegen/cone direct dependencies 与 P9 任务记录一致。
  - `crates/scoopc/src/` 复核结果：保留项为 facade re-export、`frontend.rs`、`pipeline/` 编排 wrapper、session/CLI/driver helper 与测试审计模块；stage/backend/cone implementation owner 均在独立 crate 内，未发现需要迁移的旧 stage 实际定义文件。
  - README 与 `PIPELINE-CLEANUP.md` 已同步 P9 后边界冻结：后续任何 stage 行为、handoff 或 fact 改动必须落在 owning crate，不能在 umbrella、其它 stage、cone 或 backend crate 中新增跨 crate fallback/shim。
  - 验证通过：`cargo run -p scoop_tools -- dependency-gate`；`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo build --workspace`；`cargo test --all --all-targets`；`git diff --check`。

### [DONE] P9-T09R：Review P9 全包完成度

- 参考：P9-T09。
- 重点：是否所有 stage crate 都按 DAG 方向依赖；是否还残留 `scoopc/src/` 下的旧实现；P10 是否可以从干净的 crate 边界开始。
- 验证：重新运行 P9-T09 的所有验证。
- 依赖：P9-T09
- 完成记录：
  - 2026-05-24：复核 P9-T09 全包清场结果。`cargo tree --depth 1` 复核 base、fact、AST/HIR/MIR/effect-facts/LIR/codegen/cone direct dependencies，stage 方向与 PLAN §1.2 一致；`scoopc_codegen_llvm` 只直接依赖 LIR/LIR facts 与 base，`scoopc_cone` 位于 stage/fact/base 之上，未发现 stage 反向依赖 cone。
  - `crates/scoopc/src/` 复核结果保持为 umbrella facade 与 driver/orchestration：`lib.rs` re-export base/fact/stage crate，`llvm.rs` 与 `cone.rs` 仅作兼容 facade，实际 stage/backend/cone owner 均在独立 crate；未发现旧 stage 实现目录残留。
  - P10 可以从 P9 后冻结的干净 crate 边界开始；后续 per-cone artifact、TypeStore wire format 与 frontend orchestration 改动必须落在 owning crate，不能通过 umbrella 或跨 crate shim 绕开。
  - 验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo run -p scoop_tools -- dependency-gate`（16 个 pipeline crate，549 个 source-boundary 文件）；`cargo build --workspace`；`cargo test --all --all-targets`；`cargo run -p scoop -- test`（1507/1507）；`git diff --check`。

---

## P10：Per-cone build artifact

利用 P9 已经独立出来的 stage / fact crate 边界，让每个 cone 的编译产物落地到磁盘，下游 cone 通过反序列化 fact 注入而不是吃源码。

### [DONE] P10-T01：解决 `TypeId` cross-process stable wire format

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
- 完成记录：
  - 2026-05-24：选择方案 B（portable `TypeStore` serialization）并落地。理由：当前 facts/LIR 已大量以本地 `TypeId` 表达类型关系，直接替换为文本 key 会扩大 schema 改动并丢失进程内查询效率；按 `TypeStore` 的拓扑序 `TypeKind` 列表序列化，可让下游进程先重建同构本地 type universe，再安全消费 facts/LIR 内的 `TypeId`。
  - 新增 `WireSchemaVersion` / `WIRE_SCHEMA_VERSION`，并把 schema version 挂到 `HirFacts`、`MirFacts`、`EffectFacts`、`LirFacts` 与 `LateLoweredProgram`；新增 portable `TypeStore` serde 实现，反序列化时重建 hash-cons index 并校验所有子 `TypeId` 引用范围。
  - 为 base identity/source/span/project/type 类型、四套 fact crate、LIR facts、LIR program 与必要 MIR/HIR source payload 增加 `Serialize` / `Deserialize`；MIR/HIR placeholder reason 改为 wire-friendly owned string或显式静态字符串 serde helper，并同步 placeholder inventory guard。
  - 新增 bincode round-trip 测试：`scoopc_types` 覆盖含 builtin、nominal、function、effect row 的 `TypeStore` 往返；四套 fact crate覆盖 schema/content 往返；`scoopc_lir` 覆盖 `LateLoweredProgram` 往返。新增字段后必须继续满足同一 wire-format 约束：持久化结构需可 serde 往返，并通过 schema version 管理兼容性。
  - 验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test -p scoopc_types -p scoopc_hir_facts -p scoopc_mir_facts -p scoopc_effect_facts -p scoopc_lir_facts -p scoopc_lir`；`cargo test --all --all-targets`；`cargo run -p scoop -- test`（1507/1507，fixtures: ok 1536）；`cargo run -p scoop_tools -- dependency-gate`；`git diff --check`。

### [DONE] P10-T01R：Review TypeStore wire format

- 参考：P10-T01。
- 重点：
  - 选择的方案是否真的解决跨进程问题；
  - schema version 是否覆盖所有 fact crate；
  - round-trip 测试是否覆盖 generic / effect / continuation 等复杂类型。
- 验证：重新运行 P10-T01 的所有验证。
- 依赖：P10-T01
- 完成记录：
  - 2026-05-24：复审 P10-T01 的方案 B（portable `TypeStore` serialization）。`TypeStore` wire 形态按 `TypeKind` 拓扑列表持久化，反序列化时重建 hash-cons index 并校验所有子 `TypeId` 范围，满足跨进程先重建本地同构 type universe 再消费 facts/LIR 的要求。
  - schema version 覆盖确认：`WireSchemaVersion` / `WIRE_SCHEMA_VERSION` 已挂到 `HirFacts`、`MirFacts`、`EffectFacts`、`LirFacts` 与 `LateLoweredProgram`；四套 fact crate 和 LIR program 都具备 serde/bincode 往返入口。
  - 补强 review 发现的覆盖缺口：原 round-trip 多为空产品，新增复杂类型/控制合同测试，覆盖 generic type parameter、nominal args/use-site effect row、star projection/union、effect step schema、continuation schema、callable/body effect facts、LIR effect-step callable、dynamic invoke、resume packing、continuation object 与 surface resume dispatch。
  - 验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test -p scoopc_types -p scoopc_hir_facts -p scoopc_mir_facts -p scoopc_effect_facts -p scoopc_lir_facts -p scoopc_lir`；`cargo test --all --all-targets`；`cargo run -p scoop -- test`；`git diff --check`。

### [DONE] P10-T02：定义 per-cone build artifact 磁盘布局与 `scoopc_cone` 读写 API

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
- 完成记录：
  - 2026-05-24：在 `scoopc_cone::artifact` 发布 per-cone build artifact API：`ConeArtifact`、`ConeArtifactManifest`、`ConeArtifactSchemaVersions`、stage product/fingerprint 分组，以及 `ConeArtifact::write(&self, dir: &Path)` / `ConeArtifact::read(dir: &Path)`。
  - 磁盘布局按任务要求落地并文档化：`manifest.json` 记录 cone name/version、compiler version、HIR/MIR/effect/LIR facts 与 LIR program schema version；五个 stage payload 使用 bincode 写入 `hir_facts.bin` / `mir_facts.bin` / `effect_facts.bin` / `lir_facts.bin` / `lir_program.bin`；objects 写入 `objs/`；fingerprint bytes 写入 `inputs.fingerprint` / `outputs.fingerprint`。
  - 与现有 ScoopIR consume 路径并存：本任务只新增 artifact IO surface 和 public re-export，没有切换 `inject_cone_dependency_public_api`。
  - 新增 `scoopc_cone` 单测覆盖写入 → 读取 → fact/LIR/object/fingerprint 等价、布局文件存在性，以及 object file name 不得逃逸 `objs/`。
  - 验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test -p scoopc_cone`；`cargo test --all --all-targets`；`git diff --check`。

### [DONE] P10-T02R：Review per-cone artifact schema

- 参考：P10-T02。
- 重点：磁盘布局是否覆盖 PLAN §1.2 要求的 stage / fact 完整集合；版本兼容策略是否合理。
- 验证：重新运行 P10-T02 的所有验证。
- 依赖：P10-T02
- 完成记录：
  - 2026-05-24：复审 P10-T02 per-cone artifact schema。确认 `ConeArtifact` 持久化集合覆盖 HIR/MIR/effect/LIR facts、LIR program、本 cone Scoop/native object 与 inputs/outputs fingerprint，磁盘布局保持直接目录树而非 `.cone` tar 归档，符合 PLAN §1.2 下游通过 fact/LIR artifact 消费上游 cone 的边界。
  - 修复 review 发现的版本兼容缺口：`ConeArtifact::read` 现在先校验 `manifest.json` 中的 compiler version 与 HIR/MIR/effect/LIR facts、LIR program schema versions；不兼容时显式报错并要求整 cone 重编，落实 P10-T02 文档化的 coarse-grained 兼容策略。
  - 新增 `scoopc_cone` 回归测试覆盖 incompatible compiler version 与 schema version 拒绝路径，保留原有写入 -> 读取 -> stage product/object/fingerprint/layout 等价验证。
  - 验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test -p scoopc_cone`；`cargo test --all --all-targets`；`git diff --check`。

### [DONE] P10-T03-a：补齐 `ConeArtifact` frontend import payload

- 阻塞原因：
  - 执行 P10-T03 前置审计时发现，P10-T02 的 `ConeArtifact` 只持久化 HIR/MIR/effect/LIR facts、LIR program、objects 与 fingerprints；
  - P10-T03 要求 `import_upstream_artifacts(&[&ConeArtifact], &mut Index, &mut TypeEnv) -> Result<()>`，但当前 artifact 不包含现有 `.cone` 注入路径所需的 public API、annotation classes、symbol visibility、pre-specialize metadata，也没有可从 HIR facts 还原 `TypeEnv` 所需的完整 public declaration/typealias/visibility surface；
  - 直接让 P10-T03 继续从上游源码或临时 ScoopIR 生成器注入会绕开 per-cone artifact 边界，不能证明“下游 cone 不再读上游源”。
- 目标：
  - 在 `scoopc_cone` 的 per-cone artifact schema 中补齐 frontend import payload，使 artifact 本身足以给下游 frontend 注入上游 cone 的 public API 与可见性边界；
  - 复用现有 ScoopIR/annotation/visibility/pre-specialize schema 或抽出等价的 artifact-owned payload，不新增依赖上游源码的导入路径；
  - 为 P10-T03 提供可直接调用的 artifact-to-frontend import 基础。
- 必须修改的主要位置：
  - `crates/scoopc_cone/src/artifact.rs`
  - `crates/scoopc_cone/src/consume.rs`
  - `crates/scoopc_cone/src/scoopir/`
  - `crates/scoopc_cone/src/annotations.rs`
  - `crates/scoopc_cone/src/visibility.rs`
  - `crates/scoopc_cone/src/pre_specialize.rs`
- 必须实现的内容：
  1. 扩展 `ConeArtifact`/manifest/schema version，使 artifact 可持久化 frontend import payload：public API、cone-preserved annotation classes、symbol visibility、pre-specialize metadata；若选择新 payload 名称，必须文档化并保持 `ConeArtifact::read` 的兼容校验。
  2. 提供从 artifact payload 构造现有注入输入的 API，避免 P10-T03 再从上游 source 重新导出 public API。
  3. 新增 `import_upstream_artifacts` 所需的低层 helper 或等价 building block，覆盖 `Index` / `TypeEnv` / annotations / visibility / pre-specialize 的注入数据来源。
  4. 单测覆盖 artifact 写入 -> 读取 -> 注入 payload 等价，以及缺失/不兼容 frontend payload 的显式错误。
- 验证：
  1. `cargo fmt`
  2. `cargo clippy --all-targets -- -D warnings`
  3. `cargo test -p scoopc_cone`
  4. `cargo test --all --all-targets`
  5. `git diff --check`
- 完成条件：
  - `ConeArtifact` 本身携带下游 frontend 注入所需的全部 API/可见性 payload；
  - P10-T03 可以在不读取上游 source、不临时重新导出 ScoopIR 的情况下实现 `import_upstream_artifacts(&[&ConeArtifact], &mut Index, &mut TypeEnv)`；
  - 完成记录说明选择复用 ScoopIR payload 或新增 payload 的依据。
- 依赖：P10-T02R
- 完成记录：
  - 2026-05-24：补齐 per-cone artifact frontend import payload。选择复用现有 `.cone` ScoopIR / annotation classes / symbol visibility / pre-specialize schema，封装为 artifact-owned `ConeArtifactFrontendImport` 并持久化为 `frontend_import.json`；选择 JSON 而不是 bincode 的依据是这些 schema 已按 JSON/tagged enum surface 设计并用于 `.cone` 兼容边界，避免新增平行 wire model 或从上游源码重新导出。
  - `manifest.json` 的 `ConeArtifactSchemaVersions` 新增 `frontend_import` schema version；`ConeArtifact::read` 对缺失 frontend schema 或缺失 `frontend_import.json` 显式报错并要求整 cone 重编，保持 P10-T02 的 coarse-grained 兼容策略。
  - 新增 artifact-to-frontend 注入基础：`inject_cone_artifact_frontend_import` 复用既有 `.cone` 注入逻辑，`import_upstream_artifacts(&[&ConeArtifact], &mut Index, &mut TypeEnv)` 可直接从 artifact payload 注入 public API、annotation metadata 与 non-public visibility placeholder，不读取上游 source、不临时重新导出 ScoopIR。
  - 单测覆盖 artifact 写入 -> 读取 -> frontend payload 等价、缺失 payload/schema 的显式错误，以及 artifact payload 注入 `Index` / `TypeEnv` 的 public type/fun 与 hidden visibility 数据来源。
  - 验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test -p scoopc_cone`；`cargo test --all --all-targets`；`cargo run -p scoop -- test`；`git diff --check`。

### [DONE] P10-T03：`run_frontend` 改造为按 cone DAG 拓扑顺序运行

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
- 依赖：P10-T03-a
- 完成记录：
  - 2026-05-25：`run_frontend` 已改为按 `SourceConeGraph::compilation_units()` 拓扑顺序逐 cone 运行 parse/resolve/typecheck；每个 local dependency cone 完成 frontend 后发布 in-memory `ConeArtifact` frontend import payload，下游 cone 只通过 artifact 注入 public API / annotation / visibility / pre-specialize metadata 到自己的 `Index` / `TypeEnv`，不再把依赖 cone source AST 加入下游 indexed/typecheck 输入。
  - `scoopc_cone` 新增从已 typechecked cone state 构造 frontend import 的 API：ScoopIR export、cone-preserved annotation classes 与 non-public visibility 都可复用当前 frontend state；旧 `.cone` JSON 注入路径与 `inject_cone_dependency_public_api` 保持并存，新 frontend orchestration 默认走 artifact payload 注入。
  - 默认 sysroot cone 继续由 `Session::sysroot()` 直接提供，显式额外 sysroot dependency（例如 `scoop.runtime.test`）与 local source dependency 一样发布/注入 artifact；consumer cone 不导出 artifact，避免 frontend-import export 消耗最终 codegen 所需的 AST typecheck side tables。
  - 新增 regression 单测 `downstream_frontend_uses_dependency_artifact_imports`，断言下游只能看到依赖 artifact 注入的无 body synthetic declaration（`<cone:...>`），并保留指定跨 cone fixture 覆盖。
  - 验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo build --workspace`；`cargo test --all --all-targets`；`cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`；`git diff --check`。

### [DONE] P10-T03R：Review per-cone frontend orchestration

- 参考：P10-T03。
- 重点：
  - 下游 cone 是否真的不再读上游源；
  - fact 注入是否覆盖 Index / TypeEnv / annotations / visibility 等所有现有 ScoopIR 注入点；
  - 测试是否能稳定证明跳过上游 source。
- 验证：重新运行 P10-T03 的所有验证。
- 依赖：P10-T03
- 完成记录：
  - 2026-05-25：复审 P10-T03 的 per-cone frontend orchestration。确认 downstream cone 的 active frontend 只索引 sysroot 与本 cone sources，并通过 artifact payload 注入上游 public API / `TypeEnv` / cone-preserved annotations / non-public visibility placeholders；旧 `.cone` `inject_cone_dependency_public_api` 路径仍保留为兼容入口，默认 build frontend 使用 artifact 注入。
  - 修复 review 发现的 per-cone manifest 边界问题：每个 compilation unit 的 `Index` 现在使用该 unit 自己 manifest 中的 `export_entry_points`，而不是 consumer manifest，避免依赖 cone 的导出入口检查被漏掉或错误继承 consumer exports。
  - `scoopc::cone` facade 补充 re-export `import_upstream_artifacts`，保证 P10-T03 要求的新 artifact import helper 可从既有 facade 直接访问。
  - 新增 regression 单测 `dependency_frontend_uses_its_own_export_entry_points`，证明 dependency cone frontend 会按自身 manifest 执行导出入口规则；保留 `downstream_frontend_uses_dependency_artifact_imports` 对 synthetic decl file / no-body import 的证明。
  - 验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo build --workspace`；`cargo test --all --all-targets`；`cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`；`git diff --check`。

### [DONE] P10-T04-a：补齐 per-cone artifact cache handoff 边界

- 阻塞原因：
  - P10-T04 要求 cache hit 时从磁盘加载上游 cone artifact 并跳过上游所有 stage；当前 `run_frontend` 只在进程内发布 `ConeArtifact`，且发布内容主要用于 frontend import，尚未接入 artifact cache 目录或 cache hit 加载路径。
  - 当前 `FrontendOutput` / codegen lowering 仍以 flattened `build_closure_sources + asts` 作为后续 lowering 输入；如果上游 cone 直接 cache hit 而不 parse/typecheck source，后续 lowering 仍没有正式的 artifact handoff 可以消费。
  - 仅替换 `crates/scoop/src/commands/build/incremental.rs` 的 fingerprint 计算会得到表面上的 per-cone hash，但无法满足“cache hit：从磁盘加载 artifact；cache miss：跑 stage 后写盘”和“下游用上游 artifact、跳过上游 stage”的任务要求，属于规避实现边界。
- 目标：
  - 在 P10-T04 的 fingerprint chain 之前，补齐 build/frontend 与 `ConeArtifact` 之间的 cache handoff；
  - cache hit 必须能够加载依赖 cone 的 artifact 并注入下游 frontend，而不是读取依赖 cone source；
  - cache miss 必须写出后续 cache hit 所需的完整下游可见 artifact payload。
- 必须修改的主要位置：
  - `crates/scoopc/src/frontend.rs`
  - `crates/scoop/src/commands/build.rs`
  - `crates/scoop/src/commands/build/incremental.rs`
  - `crates/scoopc_cone/src/artifact.rs`
- 必须实现的内容：
  1. 为 build/frontend 增加显式 per-cone artifact cache 输入（artifact 目录、预期 inputs fingerprint、直接依赖 outputs fingerprint 等），不能通过全局变量或临时 façade 绕过。
  2. dependency cone cache hit 时，从磁盘读取 `ConeArtifact`，校验 schema / compiler / inputs fingerprint，并通过 artifact frontend import 注入下游 `Index` / `TypeEnv`；该路径不得 parse、index 或 typecheck 该 dependency cone 的 source。
  3. dependency cone cache miss 时，按现有 per-cone frontend orchestration 跑该 cone，并写出下一次 cache hit 所需的完整 artifact payload；payload 缺失时必须显式报错或重建，不能静默 fallback 到读源。
  4. 修正后续 lowering/codegen 对 cached dependency cone source AST 的依赖：要么消费 artifact 中的正式 stage/codegen payload，要么把 cached dependency 从后续 lowering source 集合中移除并保持语义正确；不得使用空 AST、synthetic source 或重新读取上游 source 作为替代。
  5. 增加 regression 测试证明上游 dependency cache hit 后，即使 dependency source 被改成会 parse/typecheck 失败的内容，下游仍只消费已验证 artifact；cache miss 仍会重新读取并报告该失败。
- 验证：
  1. `cargo fmt`
  2. `cargo clippy --all-targets -- -D warnings`
  3. `cargo test --all --all-targets`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone`
  5. `git diff --check`
- 完成条件：
  - P10-T04 可以在不靠 whole-project fingerprint、不读 cached dependency source、不使用 placeholder AST 的前提下接入 per-cone inputs/outputs fingerprint chain；
  - 测试稳定证明 dependency cache hit 跳过 source parse/typecheck，cache miss 仍按源文件重建。
- 依赖：P10-T03R
- 完成记录：
  - 2026-05-25：为 frontend/build 增加显式 `FrontendArtifactCache` / `FrontendArtifactCacheEntry` handoff，cache entry 携带 artifact 目录、预期 inputs fingerprint、直接依赖 outputs fingerprint 占位与 miss 写盘策略；`scoop build` 的增量路径现在会为 local dependency cone 准备 `build/<profile>/cones/<cone>@<version>/` artifact cache handoff。
  - dependency cone cache hit 时，`run_frontend_with_artifact_cache` 从磁盘读取 `ConeArtifact`，校验 compiler/schema 与 inputs fingerprint 后直接发布 artifact；下游 cone 通过既有 artifact frontend import 注入 `Index` / `TypeEnv`，不会 parse/index/typecheck cached dependency source。inputs fingerprint 不匹配或 artifact 缺失时按 cache miss 重建；malformed/incompatible artifact 仍显式报错。
  - dependency cone cache miss 时沿现有 per-cone frontend orchestration 解析、resolve、typecheck 并构造完整 frontend import payload，随后写出 artifact 供后续命中使用；cached dependency source 会从 active `FrontendOutput` source/AST 集合中移除，避免 placeholder AST 或重新读取上游 source。
  - 新增 regression `dependency_frontend_cache_hit_uses_artifact_without_reading_source`：先写出 dependency artifact，再把 dependency source 改成 parse/typecheck 失败内容，证明 matching cache hit 仍只消费 artifact；同时证明 fingerprint mismatch 会 cache miss 并重新读取 broken source 报错。
  - 验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone`。

### [DONE] P10-T04：per-cone fingerprint cache + 增量 build

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
- 依赖：P10-T04-a
- 完成记录（2026-05-25）：
  - 实现：
    - `crates/scoop/src/commands/build/incremental.rs`：将原本基于整项目 SHA-256 的 fingerprint 替换为 per-cone inputs/outputs fingerprint chain。`BuildFingerprint` 现包含 `consumer_cone_id` 和 `per_cone: HashMap<ConeId, ConeBuildFingerprint>`，每个 cone 有 `inputs_fingerprint`、`cached_outputs_fingerprint` 和 `direct_dependency_outputs_fingerprints`。`compute_cone_build_fingerprint` 按编译图的拓扑序为每个 cone 计算 inputs（包含 toolchain hash + 每个直接依赖的 outputs.fingerprint），随后从磁盘读取 artifact 的 outputs.fingerprint（如果存在且 inputs 匹配）填充 `cached_outputs_fingerprint`，否则用 `inputs_fingerprint` 作为占位向下传递；最终通过 consumer cone 的 inputs.fingerprint 派生总 fingerprint。`build.json` schema 升至 v4，包含 per-cone 的 inputs/outputs 摘要字段以便排查。
    - `crates/scoopc_cone/src/artifact.rs`：新增 `compute_outputs_fingerprint(dir)`（不含 `outputs.fingerprint` 文件本身的全 artifact 文件 sha256）和 `ConeArtifact::write_with_computed_outputs_fingerprint(dir)`，由 frontend 在写盘时统一调用，确保磁盘上落的 outputs.fingerprint 是确定性函数。
    - `crates/scoopc/src/frontend.rs`：cache miss 路径走 `write_with_computed_outputs_fingerprint`；cache hit 路径在加载 artifact 时通过 `read_with_inputs_fingerprint` 二次校验 inputs 一致性，避免不一致回退。
    - `crates/scoopc_cone/src/artifact.rs`：补充 `read_with_inputs_fingerprint` 与 `InputsFingerprintMismatch` 错误类型，使 cache hit 严格按"inputs 完全相等"放行，避免噪声。
    - `crates/scoop/src/commands/build.rs`：build 完成、产物落盘后，**重新计算一次 fingerprint** 再写入 `build.json`。这是 user-cone short-circuit 不变量的关键：第一次 build 时 cache-miss 的依赖 cone 用 placeholder=inputs 作为占位，build 完成后磁盘上是真实 outputs，下次运行重新计算时直接读真实 outputs；如果 build.json 里存的是 pre-build 的 placeholder fingerprint，下次运行就会 cache miss。修复后 post-build 与下次运行的 fingerprint 严格相等。
  - 测试：
    - `crates/scoop/src/commands/build/incremental.rs::tests`：新增四个 per-cone chain 测试：
      - `per_cone_chain_rebuilds_only_user_cone_when_user_source_changes`（场景 a）；
      - `per_cone_chain_invalidates_dependency_and_user_when_dependency_source_changes`（场景 b）；
      - `per_cone_chain_invalidates_all_cones_when_toolchain_inputs_change`（场景 c，通过 OptLevel 变化代理 toolchain 维度）；
      - `per_cone_chain_post_build_fingerprint_matches_next_run`（防回归：验证 post-build fingerprint == 下一次运行 fingerprint，覆盖修复点）。
  - 验证通过：
    - `cargo fmt`、`cargo clippy --all-targets -- -D warnings`：通过（顺手修了一处历史 `sort_by` → `sort_by_key`）。
    - `cargo test --all --all-targets`：全部通过。
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone`：36/36 通过。
    - `git diff --check`：无空白错误。
  - 时序数据（fixture: `source_path_dependency_public_call`，含 1 consumer + 1 dep cone，release scoop on Apple Silicon）：
    | 场景 | real time | 行为 |
    |---|---|---|
    | 冷启动（无 build dir）| 0.92s | 全量 build |
    | Cache-hit（无修改第二次运行）| 0.18s | "skipping build (cache hit)" |
    | `touch` mtime 但内容未变 | 0.18s | 仍命中（fingerprint 基于内容） |
    | 修改 dep 源 | 0.86s | 全量 rebuild（cache 正确失效）|
    | 修改后第二次运行 | 0.18s | 重新命中 ← 修复后的 round-trip 不变量 |
    Cold vs warm：约 5.1× 加速；修改后第二次仍命中证实 post-build fingerprint 与 next-run 一致。

### [DONE] P10-T04-b：让 cached dependency cone artifact 在所有下游 stage 都正确可见

- 阻塞原因（由 P10-T04R review 在 2026-05-25 发现）：
  - P10-T04 选择 P10-T04-a 第 4 项中"把 cached dependency 从后续 lowering source 集合中移除并保持语义正确"路径：cache-hit dep 时只在 frontend 通过 `inject_cone_artifact_frontend_import` 把 dep 公共 API 注入 consumer 的 frontend `Index` / `TypeEnv`，并把 dep 源从 `build_closure_sources` 中剔除。
  - 但下游 stage（`scoopc_effect_facts_stage` 的 `EffectFactsTypeContext::build`、`scoopc_mir` 的 `mir/materialize/inputs.rs` / `rtti/mod.rs` / `rtti/type_desc.rs`、`scoopc_cone` 的 `scoopir/export.rs` / `annotations.rs` 等）会从 `compilation_sources` / sources 出发**独立重建** `Index` / `TypeEnv`，不复用 frontend 注入结果——cached dep cone 的 synthetic decl_file 既不在 source_map 也不在 compilation_sources 里，frontend 侧的注入对这些 stage 完全不可见。
  - 直接后果（reproducer）：cone fixture `tests/fixtures/run_pass_cone/source_path_dependency_public_call`，先冷 build → 再冷启动 cache hit → 编辑 consumer source 触发 consumer-only 重 build；effect_facts builder 的 `surface_callable_contract`（`crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs:245`）在 `index.by_fqn` 里找不到 dep public fun → 抛 `MissingCallableSurfaceContract`。
  - 这违反 P10-T04-a 第 4 项明确写入的"保持语义正确"条款，属于 P10-T04 的实质性遗漏，不是性能或审美问题。
- 目标：
  - 让 cached dependency cone 的 frontend import payload 在 frontend / mir / effect_facts / 其它需要重建 env+index 的 stage 中都可见；
  - cache-hit dep + 编辑 consumer 的端到端 codegen 走完整 pipeline，与 cold build 等价（除耗时外）；
  - 不允许 fixture-only 规避（如：让该 fixture 的 dep 始终 cache miss）；不允许在 stage 内静默忽略找不到的 callable / type；不允许把 cached dep source 重新塞回 `compilation_sources`。
- 必须修改的主要位置：
  - `crates/scoopc_cone/src/consume.rs`（inject_frontend_import_payload：补足 `FileTypeContext` 注入；把可复用的 inject 抽到一个跨 crate 中性 helper）。
  - `crates/scoopc_hir/src/typecheck/type_env.rs`（外部 `FileTypeContext` 注入接口）；以及如果选择把 inject helper 落在 `scoopc_hir`，则相应模块新增 public API。
  - `crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs`（`EffectFactsTypeContext::build` 在 env+index 重建后应用 cached dep 注入）。
  - `crates/scoopc/src/pipeline/mod.rs` / `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`（把 cached dep artifact 的 frontend_import payload 沿 stage 入口透传到 effect_facts）。
  - `crates/scoopc/src/frontend.rs`（在 cache-hit dep 路径上聚合 `Vec<&ConeArtifactFrontendImport>` 或等价载荷供下游使用）。
  - 其它独立重建 env+index 的路径同步审计：`crates/scoopc_mir/src/mir/materialize/inputs.rs:338`、`crates/scoopc_mir/src/rtti/mod.rs:217`、`crates/scoopc_mir/src/rtti/type_desc.rs:372`、`crates/scoopc_cone/src/scoopir/export.rs:204`、`crates/scoopc_cone/src/annotations.rs:113`——若任一路径在 cache-hit dep 场景会被 consumer-edit 触发，必须同步注入。
- 必须实现的内容：
  1. 在一个能同时被 `scoopc_cone` 与 `scoopc_effect_facts_stage` 依赖的 crate（候选 `scoopc_hir`，或新增中性 inject 数据类型）里发布"frontend import injection"中性 API。`scoopc_cone` 仍负责把 `ConeArtifactFrontendImport` 翻译成中性 payload；下游 stage 拿中性 payload 注入。crate DAG：禁止 `scoopc_effect_facts_stage` 反向依赖 `scoopc_cone`。
  2. inject 必须同时覆盖：types / funs / extension funs / annotation classes / symbol visibility / pre_specialize / synthetic source（含 span 切片）/ `FileTypeContext`（`pkg_prefix`、`imports`、`cone`，cone 用 dep 的 `ConeId` + 实际 ConeKind——artifact 已携带 cone_kind 或必要时持久化新字段）。
  3. `EffectFactsTypeContext::build`（与其它独立重建 env+index 的 stage）显式接收 cached dep 列表参数；在 env+index 重建完后立即应用注入；不允许通过 thread-local / global 状态隐式传递。
  4. `crates/scoopc/src/pipeline/llvm_codegen_stage.rs::run_lir_stage_from_lowered_hir` 与 `build_effect_facts_stage_output_with_compilation_sources` 的签名扩展，让 cached dep 列表沿 stage 边界正式 plumb（不允许把 frontend env/index 直接塞过去；那会破坏 stage 边界）。
  5. 新增 regression 测试：(a) cone-level 单测用 fixture 模拟 cache-hit dep + consumer edit，断言 effect_facts stage 走完不报错；(b) end-to-end fixture 测试：基于 `source_path_dependency_public_call` 写一个明确"先冷 build → 编辑 consumer 触发 effect_facts → 必须成功"的 incremental scenario。原有 4 个 per-cone chain 单测保持通过。
  6. 顺手给已审计到但当前 fixture 触发不到的 stage（mir/rtti/scoopir-export 等）补 cache-hit dep 走通的 regression（哪怕只是 unit 级），避免下次撞同源的回归。
- 验证：
  1. `cargo fmt`
  2. `cargo clippy --all-targets -- -D warnings`
  3. `cargo test --all --all-targets`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone`
  5. 手工 reproducer：fixture `source_path_dependency_public_call`，cold → warm → `echo "" >> src/main.scoop` → effect_facts / typecheck / mir / cone scoopir-export 等 frontend 之后的中端 stage 必须成功（与 cold 行为一致）；反向：`touch deps/.../api.scoop` 改 dep 公共 fun 签名，consumer 应再次失败 with parse/typecheck 类的 dep-related error，证明 dep 失效仍能触达。**端到端 LLVM codegen 第三次 build 由 P10-T04-c 跟踪**——cache-hit dep 时 dep callable 没有进入 consumer 的 `LateLoweredProgram`，`callable_layouts` 查询会失败；该层与 P10-T06 per-cone 子进程编译同源，留给 P10-T04-c。
  6. `git diff --check`
- 完成条件：
  - 上述 reproducer 在 frontend/effect_facts/mir/cone export 等中端 stage 路径都返回与 cold build 等价的结果；
  - cache-hit dep 不再被 stage 独立重建 env+index 时漏掉，frontend / effect_facts / mir / cone export 行为对齐；
  - 不引入 frontend-stage-specific 的 patch（如：把 dep 源塞回 `build_closure_sources` 但 stage 内忽略），所有路径必须沿 cached artifact handoff 走。
- 完成记录（2026-05-25）：
  - 新增 `crates/scoopc_hir/src/cone_import.rs` 中性 inject API（`CachedConeImport` payload + `inject_cached_cone_imports(index, env, &[…])` helper + `SyntheticSourceBuilder`）。crate DAG 满足：`scoopc_cone -> scoopc_effect_facts_stage` 仍是单向，inject helper 落在公共底座 `scoopc_hir`。
  - `crates/scoopc_cone/src/consume.rs` 把 `ConeArtifactFrontendImport` 翻译成中性 `CachedConeImport`，并在 manifest 上持久化 `cone_kind`（`crates/scoopc_cone/src/artifact.rs`、`crates/scoopc_project_model/src/manifest.rs`）。
  - `crates/scoopc/src/frontend.rs::run_project_frontend` 在 cache-hit dep 路径上聚合 `Vec<CachedConeImport>` 并随 `FrontendOutput` 透传。
  - `crates/scoopc/src/pipeline/mod.rs` 与 `crates/scoopc/src/pipeline/llvm_codegen_stage.rs` 扩展 stage 边界：`build_effect_facts_stage_output_with_compilation_sources` / LLVM codegen 入口都接受 `&[CachedConeImport]` 并下发；`emit_project_llvm_artifact_to_file` 通过 `cached_cone_imports.to_vec()` 把 payload 沿 stage 边界正式 plumb。
  - `crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs::EffectFactsTypeContext::build` 在 env+index 重建后立即调用 `scoopc_hir::cone_import::inject_cached_cone_imports`，恢复 `surface_callable_contract` / `FileTypeContext` 的可见性。
  - 回归测试：(a) `crates/scoopc_hir/src/cone_import.rs` 加 2 个 unit test 锁定 inject 后的 `decl_cone` / `decl_file` / `visibility` 不变量与"已 typecheck Index 二次注入"幂等；(b) `crates/scoopc/src/frontend.rs::dependency_frontend_cache_hit_uses_artifact_without_reading_source` 增加 P10-T04-b 透传断言（`output.cached_cone_imports()` 携带正确 payload）。
  - 中端验证：`cargo test -p scoopc_hir`（cone_import:: 4/4 通过）、`cargo test -p scoopc --lib dependency_frontend_cache_hit_uses_artifact_without_reading_source`（通过）。
- 依赖：P10-T04

### [TODO] P10-T04-c：让 cached dependency cone 在 LLVM codegen 层 callable_layouts/LateLoweredProgram 路径上可见

- 阻塞原因（由 P10-T04-b 在 2026-05-25 收口时发现）：
  - P10-T04-b 已经把 cached dep 的 frontend/effect_facts/mir/scoopir-export 注入路径打通，但 cache-hit dep + 编辑 consumer 触发 consumer-only rebuild 时，第三次 build 在 LLVM codegen 阶段会失败：`crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/types.rs::callable_layout_by_root_fqn` 报 `LLVM ABI query 缺少 callable <dep.fqn> 的 published callable version`。
  - 根因：cache-hit dep 时 dep 的源已经从 `build_closure_sources` 中剔除，consumer 的 `LateLoweredProgram` 不包含 dep callable；consumer 在 codegen 时仍会触达 dep callable 的 LLVM ABI 查询，但 dep cone artifact 当前持久化的 `lir_program.bin` 是空 `LateLoweredProgram`、`objs/` 目录为空（这是 P10-T03/T04 阶段就接受的折衷：cold build 把 dep AST 临时塞入 consumer pipeline 让 MIR 一次性 materialize 出 dep callable，第二次 cache-hit 失去这条 implicit handoff）。
  - 这条 LIR/codegen 层的 cache-hit 缺陷与 P10-T06 per-cone 子进程编译同源——本质上需要 dep 自己产出非空 `lir_program.bin` 与 `.o`，然后 consumer 在 LLVM 阶段消费 dep artifact，而不是在 consumer pipeline 里重新 materialize dep callable。
- 目标：
  - cache-hit dep + 编辑 consumer 的端到端 codegen 走完整 pipeline，与 cold build 等价（除耗时外）；
  - 与 P10-T06 per-cone 子进程编译边界一致：dep 的 `LateLoweredProgram` / object 由 dep 自己的子进程产出并落盘到 dep artifact，consumer 在 LLVM 阶段消费；
  - 不允许 fixture-only 规避；不允许把 dep 源重新塞回 consumer 的 `build_closure_sources`；不允许在 codegen 内静默忽略找不到的 callable layout。
- 必须修改的主要位置（最终方案与 P10-T06 协调收口）：
  - `crates/scoopc_cone/src/artifact.rs`（`lir_program.bin` 现状是空，需要承载 per-cone authoritative `LateLoweredProgram` 与对应的 `.o`/`.ll` 输出）。
  - `crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/layout/orchestrator.rs`（consumer LLVM 阶段需要从 cached dep artifact 消费 dep callable 的 layout / ABI，而不是只迭代自身 program）。
  - `crates/scoopc/src/frontend.rs` / `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`（cached dep artifact handoff：把 `lir_program.bin` 与 dep `.o`/`.ll` 路径列表一并下发到 LLVM codegen 入口）。
  - 与 P10-T05/T06 per-cone 子进程编译路径协调（dep 在它自己的子进程里产出非空 LIR / object artifact）。
- 必须实现的内容：
  1. dep cone artifact 持久化非空 `LateLoweredProgram`（从 dep 自身 frontend → MIR → LIR pipeline 端到端跑完产出），并把 dep 的 LLVM `.o`（必要时也 `.ll`）落盘到 dep artifact 目录；
  2. consumer LLVM 阶段从 cached dep artifact 加载 dep `LateLoweredProgram` 与 callable layout，扩展 `callable_layouts` 让 dep callable 的 ABI 查询命中；
  3. consumer 链接阶段从 cached dep artifact 拿 dep `.o`，避免 consumer 重 codegen dep callable；
  4. 与 P10-T06 per-cone 子进程编译边界协调：dep callable 的 LIR/codegen 必须由 dep 自己的子进程驱动，不允许在 consumer pipeline 里再 materialize 一遍；
  5. 回归测试：
     - end-to-end fixture：基于 `tests/fixtures/run_pass_cone/source_path_dependency_public_call` 加一个 cold→warm→`echo "" >> src/main.scoop`→第三次 build 必须成功且 binary 与 cold 等价的 incremental scenario；
     - 反向：`touch deps/.../api.scoop` 改 dep 公共 fun 签名，第三次 build 应触发 dep cache miss 与失败诊断（保证 dep 失效仍能触达）；
     - per-cone artifact unit test：dep cone artifact 加载后 `LateLoweredProgram` 非空，dep `.o`/`.ll` 路径存在并校验过 fingerprint。
- 验证：
  1. `cargo fmt`
  2. `cargo clippy --all-targets -- -D warnings`
  3. `cargo test --all --all-targets`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone`
  5. 手工 reproducer：fixture `source_path_dependency_public_call`，cold → warm → `echo "" >> src/main.scoop` → 第三次 build 必须成功（与 cold 行为一致），产物（exit code、stdout、binary 行为）等价于 cold build。
  6. `git diff --check`
- 完成条件：
  - cache-hit dep + 编辑 consumer 触发的第三次 build 在 LLVM codegen 阶段不再抛 `LLVM ABI query 缺少 callable ... 的 published callable version`；
  - dep cone artifact 持久化非空 LateLoweredProgram 与 object artifact；
  - consumer 不再依赖 cold-build 的 implicit "把 dep AST 塞入 consumer pipeline" handoff；
  - 路径与 P10-T06 per-cone 子进程编译边界一致，不引入旁路。
- 依赖：P10-T04-b（前置）；执行顺序已锁定为先 P10-T05 / P10-T05R / P10-T06 / P10-T06R 把子进程基础设施落地（spec 项 4 强制要求 dep callable 的 LIR/codegen 由 dep 自己的子进程驱动），再回头收口 P10-T04-c——若 P10-T06 完成时 dep `LateLoweredProgram` / `.o` artifact handoff 已经天然生效，则 P10-T04-c 在 P10-T06 commit 中合并标 [DONE] 即可。

### [TODO] P10-T04R：Review per-cone fingerprint cache

- 参考：P10-T04。
- 重点：
  - fingerprint chain 是否能正确传播到下游；
  - cache hit 的耗时是否实际下降到目标量级（之前调研数据：fingerprint scan 是 4s 启动开销的主因）；
  - sysroot 改动是否会触发可控范围的失效，而不是全量。
  - **（2026-05-25 review 中已经验证了 fingerprint chain 与时序，在第三项 cache-hit dep + consumer-edit 端到端路径上发现两层阻塞性缺陷：frontend/effect_facts/mir 中端 stage 缺 cached cone import 重放（已修复，记录在 P10-T04-b），LLVM codegen 层缺 dep `LateLoweredProgram` / `.o` artifact handoff（独立任务 P10-T04-c，与 P10-T06 协调收口）。本任务待 P10-T04-b + P10-T04-c 完成后再做最终复核。）**
- 验证：重新运行 P10-T04 的所有验证；额外覆盖 P10-T04-b / P10-T04-c 提到的 reproducer。
- 依赖：P10-T04、P10-T04-b、P10-T04-c

### [DONE] P10-T05：定义 per-cone 多进程并发编译的 CLI 参数与抽象边界

- 参考：本文件"细化原则"；`PLAN.md §4/P10`；`PIPELINE_REFACTOR.md` 关于 cone-level compilation unit 与 build DAG 编译顺序的设计。
- 背景：
  - 当前 `scoop build` / `scoop run` 由 scoop 驱动 → 直接 in-process 调用 `scoopc::pipeline` / `run_frontend_with_artifact_cache`，整条 cone DAG 在单进程内顺序跑（参见 `crates/scoop/src/commands/build.rs:236-258`、`crates/scoopc/src/frontend.rs:628-672`）；
  - P10-T04 已经把 per-cone artifact 落盘，并把 dependency cone 的 cache hit 路径接通，理论上不互相依赖的 cone 已经可以在多个子进程中同时编译，但还缺：CLI 入口、并发策略抽象、子进程 scoopc 抽象、以及位于 scoop crate 中的 cone DAG 调度 driver。
- 目标：
  - 给 `scoop build` / `scoop run` 加最大并发编译进程数 CLI 参数（推荐 `-j / --jobs`），默认值固定为 4 或其他合理的小固定值；
  - 在 `scoop` crate 内发布 `ConcurrencyStrategy` trait（或等价抽象），把"如何决定并发数"与"驱动调度"解耦；本轮只实现固定值策略，但保留 trait 接口供未来按物理 CPU 数 / 内存 / 远端资源等策略接入；
  - 在 `scoop` crate 内发布 `SubprocessConeCompiler` trait（或等价抽象），把"实际跑一个 cone 的子进程"与"调度 driver"解耦；本轮提供单一 `LocalProcessConeCompiler` 占位实现（可允许部分行为留到 P10-T06 落地），保留 trait 接口供后续"分布式编译"等高级实现挂接；
  - 必要时给 `scoopc` binary 加入"build-single-cone"子命令模式（接受 cone 标识、上游 artifact 路径、本 cone 输出 artifact 目录），仅作为 CLI surface，不引入任何调度 / DAG 遍历 / 多进程逻辑——即"scoopc 仍然只是编译器"；
  - 把 per-cone 多进程并发编译模型回写到 `PLAN.md` §4/P10 与 `PIPELINE_REFACTOR.md` 编译顺序模型，作为后续实现与审计的设计基线；
  - 本任务不引入任何并发执行行为：默认 `--jobs N` 行为可暂时退化为 in-process 顺序运行（与 P10-T04 后等价），CLI / trait / 子命令 surface 与文档基线就位即可。
- 必须修改的主要位置：
  - `crates/scoop/src/cli.rs`（`Build` / `Run` command 加 `-j / --jobs` 参数与 parse 测试）
  - `crates/scoop/src/commands/build.rs`（`BuildOptions` 增加 `jobs` 字段并在主流程透传）
  - `crates/scoop/src/commands/build/`（新增 `concurrency.rs` 或同名模块，承载 `ConcurrencyStrategy` / `SubprocessConeCompiler` trait 与 `LocalProcessConeCompiler` 占位实现）
  - `crates/scoopc/src/bin/scoopc.rs` 与 `crates/scoopc/src/driver_cli.rs`（如选择把"single-cone build"作为 scoopc 子命令暴露，仅扩展 driver_cli 解析 + 调用现有 `scoopc::pipeline` / `scoopc::frontend` per-cone API）
  - `PLAN.md` §4/P10（设计基线追加 per-cone 多进程并发编译条目）
  - `PIPELINE_REFACTOR.md`（在"编译顺序模型 / cone 之间：按 DAG 拓扑排序"附近追加多进程并发执行约束）
- 必须实现的内容：
  1. CLI：`--jobs N` / `-j N` 加入 `Build` 与 `Run` 子命令；非数字、0、负值必须给出明确 diagnostic；可选 `SCOOP_BUILD_JOBS` 环境变量做 fallback；默认值在代码里有命名常量（`DEFAULT_BUILD_JOBS = 4` 或类似）便于后续策略调整；
  2. `ConcurrencyStrategy` trait：暴露 `fn max_concurrent_jobs(&self) -> usize`；默认实现 `FixedJobsStrategy { jobs: usize }`；trait 文档明确"未来按物理 CPU 数 / 内存 / 远端 worker 池策略可在此挂接，但本轮不实现"；
  3. `SubprocessConeCompiler` trait：定义跑单个 cone 的最小接口，输入至少包含 cone 标识 / 上游已落盘 artifact 集合 / 本 cone 输入 fingerprint / artifact 输出目录，输出至少包含本 cone artifact 路径与 outputs.fingerprint；提供 `LocalProcessConeCompiler` 占位实现（本任务允许 `compile_cone` 暂返回 `unimplemented` 或委托给 P10-T06 完成实际 fork+exec，但 trait + 占位类型必须就位）；
  4. 如果选择给 scoopc 加 "build-single-cone" 子命令，必须只接受最小输入（cone 标识 / 上游 artifact 路径列表 / 输出 artifact 目录），且**禁止**在 scoopc 内引入任何 driver / 调度 / 多进程 / DAG 遍历逻辑——它的实现限定在"加载上游 artifacts → 跑 P10-T03 已有 per-cone frontend orchestration → 写 artifact"；
  5. 设计基线：在 `PLAN.md` §4/P10"必须完成"列表中追加"per-cone 多进程并发编译 CLI + driver 抽象"条目，并在`PIPELINE_REFACTOR.md` 编译顺序模型一节追加"cone DAG 中互不依赖的 cone 应允许并发跑、scoop 拥有 driver / scoopc 仅做 single-cone 编译执行体"约束；
  6. 单元测试：CLI parse 测试覆盖 `-j N` / `--jobs N` / 默认值 / invalid value；trait 测试覆盖 fixed strategy；`LocalProcessConeCompiler` 占位 surface（包括"未实现行为"的稳定错误形态）；如果加了 scoopc 子命令，覆盖 happy path 解析与缺少 artifact 的错误。
- 验证：
  1. `cargo fmt`
  2. `cargo clippy --all-targets -- -D warnings`
  3. `cargo build --workspace`
  4. `cargo test --all --all-targets`
  5. `cargo run -p scoop -- build --help`（人工检查 `--jobs` 与 `-j` 出现）
  6. `cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone`（行为不变）
  7. `cargo run -p scoop_tools -- dependency-gate`
  8. `git diff --check`
- 完成条件：
  - CLI / trait surface 落地，default 行为与 P10-T04 后等价；
  - driver 调用点已经能注入 `ConcurrencyStrategy` 与 `SubprocessConeCompiler`，本任务不要求实际并发；
  - `scoopc` binary 没有引入任何"驱动/调度"职责（即使加了子命令，也仅作 single-cone 编译执行体）；
  - `PLAN.md` / `PIPELINE_REFACTOR.md` 已经把 per-cone 多进程并发编译纳入设计基线。
- 依赖：P10-T04-b（前置；P10-T04R 与 P10-T04-c 已被认定为本任务的下游 reviewer/收口任务，不作为本任务前置）。
- 完成记录（2026-05-25）：
  - CLI surface：`crates/scoop/src/cli.rs` 给 `Command::Build` 与 `Command::Run` 都加了 `#[arg(short = 'j', long = "jobs", value_name = "N")] jobs: Option<NonZeroUsize>`；clap `NonZeroUsize` 类型保证 0 与非数字在解析阶段直接报错；负值 `-1` 被 clap 识别为短选项（`UnknownArgument`）。新增 11 个 parse 测试覆盖 long/short/默认/zero/non-numeric/negative。
  - 并发抽象模块：`crates/scoop/src/commands/build/concurrency.rs`（新增）发布：
    - `DEFAULT_BUILD_JOBS = 4` 命名常量 + `BUILD_JOBS_ENV_VAR = "SCOOP_BUILD_JOBS"`；
    - `pub fn resolve_build_jobs(cli_jobs: Option<NonZeroUsize>) -> Result<NonZeroUsize, BuildJobsError>` 实现 CLI > env > default 优先级；
    - `pub fn default_build_jobs() -> NonZeroUsize` 与 `pub enum BuildJobsError { InvalidEnvValue, EnvNotUnicode }`；
    - `pub trait ConcurrencyStrategy { fn max_concurrent_jobs(&self) -> NonZeroUsize }` + `FixedJobsStrategy`；
    - `pub trait SubprocessConeCompiler { fn compile_cone(&self, ConeCompileRequest) -> Result<ConeCompileResponse, SubprocessConeCompileError> }` + `LocalProcessConeCompiler` 占位实现（`compile_cone` 总是返回 `NotYetImplemented`）；
    - `ConeCompileRequest { cone_id, upstream_artifact_dirs, inputs_fingerprint, output_artifact_dir }` 与 `ConeCompileResponse { output_artifact_dir, outputs_fingerprint }`（P10-T05 阶段未被消费，加 `#[allow(dead_code)]`）；
    - 11 个单元测试覆盖 `resolve_build_jobs` 各分支、env-var Mutex 串行化、`FixedJobsStrategy`、`LocalProcessConeCompiler` 占位 surface、trait object-safety。
  - driver 注入点：`crates/scoop/src/commands/build.rs` 给 `BuildOptions` 加 `pub jobs: NonZeroUsize` 字段、`Default` 用 `concurrency::default_build_jobs()`；`pub fn run` 在主流程内构造 `Box<dyn ConcurrencyStrategy>` + `Box<dyn SubprocessConeCompiler>`，并通过 `tracing::debug!(max_jobs = ..., compiler = ?subprocess_cone_compiler, ...)` 实际调用 `concurrency_strategy.max_concurrent_jobs()`（确保 trait 方法在非测试代码下也是 reachable）；driver 仍走 in-process 顺序路径，本任务严格不引入并发执行行为。
  - 透传：`crates/scoop/src/commands/mod.rs` destructure `jobs` from `Command::{Build, Run}`，新增私有 `fn resolve_build_jobs(...) -> Result<NonZeroUsize, miette::Report>` 把 `BuildJobsError` 转 miette diagnostic；`crates/scoop/src/commands/run.rs` 给 `pub fn run` 与 `fn run_for_exit_code` 加 `jobs: NonZeroUsize` 参数（两者都加 `#[allow(clippy::too_many_arguments)]`），把 `jobs` 通过 `BuildOptions` 转给 `build::run`。
  - scoopc binary：未新增 `build-single-cone` 子命令。spec 标记为 "如果选择" / "必要时" 可选；当前 scoopc 没有"加载上游 artifacts → 跑 per-cone frontend → 写 artifact"的稳定 per-cone API surface，落一个返回 `unimplemented` 的 stub 子命令会违反"No Workarounds"规则。本任务严格遵守"scoopc 仍然只是编译器"的 P10-T05 设计基线（trait + scoop driver 注入点已经足够给 P10-T06 子进程驱动落地）；P10-T06 真正实现子进程并发时再决定是否新增 scoopc 子命令暴露 single-cone 入口。
  - 设计基线回写：
    - `PLAN.md` §4/P10"必须完成"追加第 6 项"per-cone 多进程并发编译 CLI + driver 抽象（P10-T05 落地 surface，P10-T06 落地真正子进程并发）"，覆盖 --jobs/SCOOP_BUILD_JOBS、`ConcurrencyStrategy` / `SubprocessConeCompiler`、scoopc-as-pure-compiler 约束、cone DAG 并发基线 4 条子条目；
    - `PIPELINE_REFACTOR.md` 在编译顺序模型一节追加"#### 多进程并发执行约束（P10-T05/T06）"小节，5 条规则：scoop 拥有 driver、scoopc 单 cone 执行体、`ConcurrencyStrategy` 可订制、`SubprocessConeCompiler` 可订制、cache hit + subprocess hybrid 处理。
  - 验证：`cargo fmt` ✓；`cargo clippy --all-targets -- -D warnings` ✓；`cargo build --workspace` ✓；`cargo test --all --all-targets` 全绿；`cargo run -q -p scoop -- test --fixtures tests/fixtures/run_pass_cone` 36/36 PASS（行为与 P10-T04 后等价）；`cargo run -p scoop_tools -- dependency-gate` ✓；`git diff --check` 无 whitespace 问题。

### [TODO] P10-T05R：Review CLI 参数与并发抽象

- 参考：P10-T05。
- 重点：
  - `--jobs` 默认值、环境变量与失败 diagnostic 是否合理；
  - `ConcurrencyStrategy` 与 `SubprocessConeCompiler` trait 是否真的做到"接口/抽象保留"，driver 调用点没有把固定策略 / 固定执行体硬编码；
  - 如果 `scoopc` 加了 "build-single-cone" 子命令，是否仍是单纯编译器（即没有 driver / scheduler / 多进程 fork 逻辑）；
  - default 行为是否完全保持 P10-T04 后的现状；
  - `PLAN.md` / `PIPELINE_REFACTOR.md` 的设计基线追加是否覆盖：cone DAG 并发约束、并发策略可订制、子进程 scoopc 抽象（为分布式编译预留）。
- 验证：重新运行 P10-T05 的所有验证。
- 依赖：P10-T05

### [TODO] P10-T06：在 scoop 中实现 per-cone 子进程并发编译 driver

- 参考：本文件"细化原则"；P10-T05；P10-T04-c（per-cone 子进程必须产出非空 `LateLoweredProgram` + `.o` 让 consumer cache-hit 路径可用）；`PLAN.md §4/P10`；`PIPELINE_REFACTOR.md` 关于 build DAG 与 cone 之间编译顺序的设计。
- 目标：
  - 在 `scoop` 编译 driver 中接入真正的 cone DAG 并发调度器：按拓扑顺序遍历 `SourceConeGraph::compilation_units()`，把所有直接依赖都已落盘的 cone 推到 ready queue；并发度受 `ConcurrencyStrategy::max_concurrent_jobs` 限制；
  - 每个被调度的 cone 通过 `SubprocessConeCompiler::compile_cone(...)` 走子进程跑（默认 `LocalProcessConeCompiler`：fork+exec `scoopc` 的 single-cone 子命令）；
  - 子进程跑完按 P10-T02 已有 per-cone artifact 规范落盘，父进程仅校验 outputs.fingerprint 与 artifact 完整性后把它注入下游 cone 的 artifact cache；
  - 不引入"分布式编译"的实际实现，但必须证明 driver 与 `SubprocessConeCompiler` trait 真正解耦——换一个 trait 实现就可以替换执行体（可加一个测试用 fake compiler 证明）。
- 必须修改的主要位置：
  - `crates/scoop/src/commands/build.rs`（`run` 主流程：把"一次 in-process frontend run"改为"按 cone DAG 子进程并发调度"）
  - `crates/scoop/src/commands/build/concurrency.rs`（或同名 driver 模块，承载调度状态机）
  - `crates/scoop/src/commands/build/incremental.rs`（per-cone artifact cache 入口可能需要扩展给 driver 复用）
  - `crates/scoopc/src/bin/scoopc.rs` 与/或 `crates/scoopc/src/driver_cli.rs`（落实 "build-single-cone" 子命令的实际执行体，仅依赖 `scoopc::pipeline` / `scoopc::frontend` 已有 per-cone API）
  - `crates/scoopc_cone/`（如需要发布"从磁盘加载上游 artifacts 列表 + 注入本 cone frontend"的 helper，必须沿用 P10-T03/T04 已有 artifact handoff 边界，不得新开旁路）
- 必须实现的内容：
  1. cone DAG 调度器：维护 `Pending / Ready / InFlight / Done / Failed` 状态机；只有所有直接依赖 cone 都已 `Done`（artifact 已落盘且 outputs.fingerprint 校验通过）的 cone 才能进入 `Ready`；
  2. 并发限制：调度器从 `ConcurrencyStrategy::max_concurrent_jobs` 取上限；超限 cone 在 ready queue 排队；任何 cone 失败立即停止派发新任务，已派发任务允许跑完，最终汇总 diagnostics；
  3. 子进程执行：通过 `SubprocessConeCompiler::compile_cone(...)` 调用；本轮默认 `LocalProcessConeCompiler` 走 `std::process::Command` 执行 `scoopc build-single-cone --cone-id <id> --upstream-artifact <dir>... --out <dir>` 类语义；子进程 stderr/stdout 必须能被父进程聚合输出（保留诊断顺序与 cone 标识）；
  4. 子进程的 artifact handoff：父进程为每个 ready cone 准备好它的上游 artifact 路径列表（直接依赖 cone 的 `build/<profile>/cones/<cone>@<ver>/`），并通过 CLI 参数 / 文件 / env 传给子进程；子进程跑完通过磁盘 artifact 让父进程读取；**禁止**通过 stdout 传 IR/AST/源码或任何 in-process 共享数据；
  5. cache hit：如果某 cone 的 inputs.fingerprint 命中已有 artifact（沿用 P10-T04 / P10-T04-b 路径），driver 直接跳过子进程派发，标记完成；cache hit 与子进程派发的混合场景必须能正确把 cached artifact 注入下游；
  6. `--jobs 1` 行为：driver 仍然走调度器 + 子进程路径（保证两条路径覆盖；如选择 `--jobs 1` 时退化为 in-process，必须在测试中覆盖该 fallback 并显式记录）；
  7. 失败传播：当任何子进程失败，父进程必须能精确定位到失败 cone、保留 partial artifact、退出码非零，并把子进程 diagnostic 完整聚合显示（带 cone 标识前缀）；
  8. 测试：
     - 集成测试覆盖至少三种调度场景：(a) 单 cone（degenerate 串行）；(b) 链式 `dep -> consumer`（必然顺序）；(c) `dep_a / dep_b -> consumer`（前两个必须并发跑）；
     - 端到端 fixture：基于现有 `tests/fixtures/run_pass_cone/source_path_dependency_public_call`（必要时新增多依赖 fixture）跑 `--jobs 4` 与 `--jobs 1`，断言产物等价；
     - 失败传播测试：故意让一个 dep cone 编译失败，断言父进程退出码非零，failed cone diagnostic 在父进程输出中可见，consumer cone 不会启动；
     - trait 解耦证明：单测里用一个 `RecordingFakeConeCompiler` 替换 `LocalProcessConeCompiler` 跑同一段调度逻辑，验证调度器与执行体真正解耦。
- 验证：
  1. `cargo fmt`
  2. `cargo clippy --all-targets -- -D warnings`
  3. `cargo build --workspace`
  4. `cargo test --all --all-targets`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone`
  6. `cargo run -p scoop -- test`（完整 fixture suite，至少 30 分钟 timeout，覆盖 `--jobs` 默认与 `--jobs 1` 两种路径）
  7. 手工时序复核（在完成记录中给出数据）：fixture `source_path_dependency_public_call` 在 `--jobs 1` vs `--jobs 4` 上 fresh build 与 cache hit 耗时对比；
  8. `cargo run -p scoop_tools -- dependency-gate`
  9. `git diff --check`
- 完成条件：
  - 默认 fresh build 走子进程并发路径，cone DAG 中互不依赖的 cone 实测能并发跑；
  - cache hit 路径不退化；
  - `SubprocessConeCompiler` 默认实现 `LocalProcessConeCompiler` 真的 fork+exec scoopc 子进程，driver 通过 trait 调用，不依赖具体类型；
  - scoopc 仍然只是编译器（"build-single-cone" 子命令实现里没有调度 / 多进程 / DAG 遍历逻辑）；
  - 失败传播在父子进程之间稳定可观测。
- 依赖：P10-T05R

### [TODO] P10-T06R：Review per-cone 并发 driver

- 参考：P10-T06。
- 重点：
  - cone DAG 调度是否严格按依赖关系（不会让 consumer 在任一直接依赖 artifact 落盘前开跑）；
  - `SubprocessConeCompiler` trait 是否真正抽象、driver 不直接 hardcode `Command::new("scoopc")`；
  - 失败传播是否完整（子进程 stderr / exit code / diagnostic 都能在父进程聚合显示，并保留 cone 归属）；
  - 默认 jobs 与 `--jobs 1` 行为是否都被测试覆盖；并发 vs 串行产物是否真的等价；
  - `scoopc` 是否仍然没有承担 driver / 调度职责；
  - cache hit 与子进程派发的混合场景是否仍正确注入下游 cone。
- 验证：重新运行 P10-T06 的所有验证。
- 依赖：P10-T06

### [TODO] P10-T07：P10 全包清场、文档同步与依赖审计

- 目标：
  - 搜索确认 frontend 不再读上游源；
  - 搜索确认 driver 不再保留 in-process 跑全 cone DAG 的兜底路径（多进程并发编译路径已成默认）；
  - 更新 README / SCOOP_RUNTIME 等文档体现新的 per-cone build 与多进程并发编译模型；
  - 把"将来可能要做"的 generic body wire format 列为后续单独 milestone 的入口，不再在本包内闷头扩展。
- 必须修改的主要位置：`TODO.md`、`TODO-7.md`、`PIPELINE-CLEANUP.md`、`PLAN.md`、`PIPELINE_REFACTOR.md`、`README.md`、相关 fixture 的 README。
- 必须实现的内容：
  1. 搜索 `inject_cone_dependency_public_api` 是否仅作为兼容路径保留；
  2. 搜索 frontend 内是否仍存在跨 cone 平铺 source loop；
  3. 搜索 scoop driver 内是否仍存在不走 `SubprocessConeCompiler` 的 in-process whole-DAG fallback；
  4. 文档明确"如果未来需要跨 cone generic body 复用，应单独立项 P11"；
  5. 把整个 build closure 的 SHA fingerprint 路径标记为废弃；
  6. 同步 `PLAN.md` §4/P10 与 `PIPELINE_REFACTOR.md` 编译顺序模型，最终冻结 per-cone 多进程并发编译的设计边界（CLI / 并发策略 / 子进程 scoopc / 分布式编译预留）。
- 验证：
  1. `cargo fmt`
  2. `cargo clippy --all-targets -- -D warnings`
  3. `cargo run -p scoop_tools -- dependency-gate`
  4. `cargo test --all --all-targets`
  5. `cargo run -p scoop -- test`（完整 fixture suite，至少 30 分钟 timeout）
  6. `git diff --check`
- 完成条件：
  - per-cone build 在所有 fixture 上稳定；
  - 多进程并发编译路径成为默认且文档冻结；
  - 文档冻结新边界。
- 依赖：P10-T06R

### [TODO] P10-T07R：Review P10 全包完成度

- 参考：P10-T07。
- 重点：
  - 下游 cone 是否真的不再读上游源；
  - per-cone fingerprint cache 的命中行为是否符合预期；
  - 多进程并发编译路径在 fresh build / cache hit / 失败传播三种场景下都稳定；
  - 是否还有遗漏的 path 上游源加载路径需要清理；
  - `PLAN.md` / `PIPELINE_REFACTOR.md` 的设计基线是否与代码一致。
- 验证：重新运行 P10-T07 的所有验证。
- 依赖：P10-T07
