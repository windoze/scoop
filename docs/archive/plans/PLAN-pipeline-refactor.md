# Pipeline Refactor 执行计划

> 生成时间：2026-05-20
> 设计基线：[`PIPELINE_REFACTOR.md`](./PIPELINE_REFACTOR.md)
> 审计基线：[`PIPELINE-CLEANUP.md`](./PIPELINE-CLEANUP.md)
> 归档前置基线：
> - [`docs/archive/designs/SYSROOT_RESHAPE_R2.md`](./docs/archive/designs/SYSROOT_RESHAPE_R2.md)
> - [`docs/archive/plans/PLAN-sysroot-reshape-r2.md`](./docs/archive/plans/PLAN-sysroot-reshape-r2.md)
> - [`docs/archive/plans/TODO-sysroot-reshape-r2.md`](./docs/archive/plans/TODO-sysroot-reshape-r2.md)
> 当前状态：P0-P10 已完成；P10 已冻结 per-cone artifact、portable TypeStore wire format、fingerprint cache、`scoopld`/`link-cone`、consumer 子进程派发、single-file virtual cone 零回退、`scoop` facade 依赖收紧与最终 review。后续跨 cone generic body wire format 如需推进，必须作为 P11 单独设计。

## 0. 目标

本轮的目标不是继续在现有 `scoopc` 单 crate 上局部修补，而是把编译器收口成清晰的阶段图、facts 图和 crate DAG。

目标主线是：

```text
AST -> HIR -> MIR -> effect facts -> LIR -> codegen
```

其中：

1. `effect_lowered` 被正式视为 LIR。
2. `AST -> HIR` 被正式视为 cone-level semantic frontend barrier。
3. `codegen` 的唯一 authoritative IR 输入是 `LIR`。
4. 所有静态可判定的 Scoop 源码错误，目标上都必须在 `AST -> HIR` 之前收口。

## 1. 硬约束

### 1.1 阶段边界

1. 每个阶段只消费前一阶段的 authoritative 输出，以及更早阶段发布的 facts。
2. 后续阶段不得回头重跑上游阶段。
3. 后续阶段不得把上游 `StageOutput` 整包嵌套进自己的输出中作为长期接口。

### 1.2 crate 依赖

1. stage crate 只允许依赖：
   - 基础 crate
   - 自己的 fact crate
   - 前一个阶段 crate
   - 更早阶段的 fact crate
2. fact crate 只允许依赖基础 crate。
3. fact crate 不得依赖任何 stage crate，也不得依赖任何其它 fact crate。

### 1.3 facts 规则

1. 每个阶段发布的 facts 必须有独立含义。
2. facts 不能简单搬运、重新导出或嵌套别的 facts/stage outputs。
3. 下游不得把两个 fact table 当作可替代输入；若出现替代关系，要么是 facts 不完整，要么是下游设计错误。

### 1.4 编译单元与编译顺序

1. 编译单元 = 一个 cone 的全部 Scoop 源文件。
2. 整个 build 输入 = 多个 cone-level compilation unit 构成的 source-cone DAG。
3. cone 之间按 DAG 拓扑顺序编译。
4. cone 内不定义语义性的文件编译顺序，只定义阶段屏障。
5. 文件顺序只用于稳定性，不用于语义。

### 1.5 前端语义约束

1. `global object / top-level val / top-level var` 不能是 generic。
2. `@CallingConvention` 函数不能是 generic。
3. `top-level var` 必须显式标注 `@Global` 或 `@ThreadLocal`。

### 1.6 comptime 范围裁剪

1. 现有 comptime surface 与实现整体移除。
2. 这里应包括现有 `const` 相关语义点。
3. 不保留“旧 comptime surface 的专门 reject 逻辑”；移除后相关代码应自然以普通 parse/resolve/typecheck 失败暴露。

### 1.7 优化框架

1. HIR 不承载 optimization pass，只承载语义规范化。
2. MIR 承载 backend-neutral 的普通调用图/控制流/实例级优化。
3. LIR 承载 effect/control 相关的窄优化 family。
4. codegen 只承载 backend-specific 的 target/physical IR 优化。

## 2. 当前问题摘要

详细问题见 `PIPELINE-CLEANUP.md`。这里只提炼执行计划直接要处理的几类根问题：

1. P2 已让 `HirStageOutput = { hir, hir_facts }` 成立，并移除 HIR-carried MIR snapshot、HIR typed contract bridge 与 `ProgramFacts` 重叠 owner。
2. P3 已让 `MirStageOutput = { mir, mir_facts }` 语义成立，并把 root inventories、snapshot binding、pass artifacts 与 MIR pass metadata 收口到 MIR-owned handoff。
3. `EffectFactsStageOutput` 和 `EffectLoweredStageOutput` 都还在嵌套上游输出，而不是只发布本阶段产物。
4. MIR 普通优化已由显式 pass pipeline 拥有；backend 层仍有 P7 范围内的去虚化 residual。
5. codegen 仍直接依赖 HIR compatibility scaffold、raw MIR/pass view、effect facts 和 LIR 的混合输入。

## 3. 阶段总览

| 阶段 | 名称 | 目标 |
| --- | --- | --- |
| P0 | Remove current comptime | 先移除现有 comptime/const surface 与实现，清空边界条件 |
| P1 | Base crates + cone unit model | 固定基础 crate、cone-level compilation unit 和 source-cone DAG 定义 |
| P2 | HIR barrier + hir_facts | 把所有静态源码语义收口到 `AST -> HIR`，建立独立 `hir_facts` |
| P3 | MIR boundary + MIR pass pipeline | 收口 `MirStageOutput`，建立 `mir_facts` 与正式 MIR analysis/opt pipeline |
| P4 | effect facts purity | 让 effect facts 变成真正只读分析输出，不修改 MIR 输出本体 |
| P5 | LIR output + LIR opt family | 把 `effect_lowered` 收实为正式 LIR，并发布独立 `lir_facts` |
| P6 | Global init model | 落实 object once、top-level eager init、per-cone init routine 和 storage policy |
| P7 | LLVM backend cleanup | 让 LLVM backend 只依赖 `LIR + LIR facts + base context` |
| P8 | Final verification | 清理残余、冻结边界、为未来 C backend 预留干净接口 |
| P9 | Stage crate split | 把 AST/HIR/MIR/effect_stage/LIR/codegen 拆为独立 stage crate；cone 按数据/操作两层拆为 `scoop_project_model` 扩展 + 新 `scoopc_cone`；用 `cargo build` 强制 PLAN §1.2 的依赖方向 |
| P10 | Per-cone build artifact | 每个 cone 的编译产物落到 `build/<profile>/cones/<cone>/`；下游 cone 通过反序列化 fact 注入而不再扫上游源码；解决 cross-process `TypeId` stable wire format；per-cone fingerprint chain 替换整项目 fingerprint |

阶段间原则上顺序推进；只有在不破坏上述依赖关系时，才允许在相邻阶段间拆小步 PR。

## 4. 各阶段计划

### P0：Remove current comptime

目标：在正式讨论任何 stage/fact crate 之前，先把现有 comptime 整体移出主线。

必须完成：

1. 删除 package-level `comptime if` 裁剪路径。
2. 删除 runtime comptime plan。
3. 删除 `const` 相关语言 surface 与其 lowering/codegen 支持。
4. 删除只为 comptime 保留的跨阶段特判、占位节点和绕路接口。

完成标准：

1. 正式 pipeline 中不再保留现有 comptime/const surface。
2. 代码库中不再存在“为了兼容旧 comptime”而保留的专门逻辑。
3. `PIPELINE_REFACTOR.md` 中的主线阶段图不再依赖任何 comptime 特例。

### P1：Base crates + cone compilation unit model

目标：先把基础层和编译单元模型固定下来，给后续 stage/fact crate 留出落点。

必须完成：

1. 设计并落地基础 crate 壳层：
   - `span`
   - `source`
   - `types`
   - `ids`
   - `project_model`
2. 明确 `ProjectInput` / `ProjectContext` / `SourceConeGraph` 在未来 architecture 中的角色。
3. 明确 “cone = compilation unit” 的正式含义，并让 facade 层按这个概念组织后续 API。

完成标准：

1. 后续 stage/fact crate 都有明确的基础依赖层。
2. 文档和代码中不再把“单文件”当正式 compilation unit 定义。

### P2：HIR barrier + hir_facts

目标：把 `AST -> HIR` 变成真正的 semantic frontend barrier，并建立独立 `hir_facts`。

必须完成：

1. 收口所有静态可判定的源码语义错误到 HIR 屏障。
2. 从 `LoweredHir` 拆出正式 `hir_facts`。
3. 把当前分散在：
   - `LoweredHir` side tables
   - `TypedHirEffectContracts`
   - `ProgramFacts`
   中的职责重新分配。
4. 明确 HIR 阶段的 declaration legality 约束：
   - `@CallingConvention` non-generic
   - global roots non-generic
   - `top-level var` storage policy gate
5. 保证 HIR 内部若需要 CFG 式分析，也只使用 HIR-owned semantic CFG，不依赖 MIR crate。

完成标准：

1. `HirStageOutput = { hir, hir_facts }` 语义成立。
2. `LoweredHir` 不再扮演多阶段 bundle。
3. 通过 HIR 屏障后，后续阶段不再报新的普通源码语义错误。

### P3：MIR boundary + MIR pass pipeline

目标：把 MIR 从“materialization 附带物”变成正式阶段，并建立清晰的 MIR analysis/optimization 流水线。

必须完成：

1. `MirStageOutput` 只发布 MIR-owned 产物。
2. 在 P2 已移除 HIR typed contract 泄漏的基础上，收口 optional materialized snapshot、root inventories 与 pass artifacts。
3. 建立独立 `mir_facts` / pass artifacts 查询面。
4. 把现有优化逻辑重排成显式 MIR pipeline：
   - escape analysis（analysis/facts）
   - devirtualization（optimization）
   - summary-driven inlining（optimization）
   - closure simplification（optimization）
   - 必要的 cleanup / summary refresh
5. 删除 HIR 层 dispatch 去虚化。

完成标准：

1. `MirStageOutput = { mir, mir_facts }` 语义成立。
2. MIR 去虚化/内联只有一个 authoritative owner。
3. MIR pass 不再只是 materializer 尾部的隐式开关。

### P4：effect facts purity

目标：把 effect facts 变成真正的只读分析输出，而不是会修改 MIR 输出本体的半分析半改写层。

必须完成：

1. effect facts stage 对 MIR 输入只读。
2. 若它需要额外 type context 或 analysis-owned context，这些内容作为自己的输出发布，而不是写回 MIR。
3. `EffectFactsStageOutput` 不再嵌套整份 `MirStageOutput` 作为长期接口。
4. `effect_facts` crate 只承载 effect/control 合同本身。

完成标准：

1. `EffectFactsStageOutput = { effect_facts }` 成立，或等价的窄输出形式成立。
2. effect facts stage 不再修改 `MirStageOutput` 本体。

### P5：LIR output + LIR opt family

目标：把 `effect_lowered` 收实为正式 LIR，并补齐 codegen 需要的 backend-neutral 合同。

必须完成：

1. `EffectLoweredStageOutput` 退化成正式 `LirStageOutput`。
2. 建立独立 `lir_facts` / query layer。
3. LIR 输出补齐：
   - plain callable surface
   - effect-step callable contract
   - dynamic invoke contract
   - dispatch owner/slot selection结果
   - continuation/resume publication
4. 建立正式的 `LIR optimization family`：
   - local state-machine elimination
   - 简单 higher-order wrapper 的定向 inline/devirt
   - wrapper state folding
   - dead state / dead slot cleanup

完成标准：

1. codegen 不再为了 plain callable / dynamic invoke / dispatch 去回看 raw MIR/HIR。
2. LIR optimization 被明确为一组窄 pass，而不是散落在 lowering/codegen 里的特判。

### P6：Global init model

目标：把全局初始化模型正式落到 HIR/MIR/LIR contract 与 codegen entry 之上。

必须完成：

1. 明确并实现全局初始化根分类：
   - `object`
   - top-level `val`
   - annotated top-level `var`
2. object once 语义与 top-level eager init 语义彻底分离。
3. per-cone init routine 与 final entry init order contract 在 LIR/codegen 之间闭合。
4. `@Global` / `@ThreadLocal` storage policy 全链路闭合。
5. global roots non-generic 约束在 HIR 屏障内稳定拒绝。

完成标准：

1. top-level values 不再 lazy first-access。
2. 所有 linked cones 的 top-level init 在 `main` 前完成。
3. object once 只服务 object，不再和 top-level init 共享旧语义路径。

### P7：LLVM backend cleanup

目标：让 LLVM backend 只依赖 `LIR + LIR facts + base context`。

必须完成：

1. 删除 codegen 层 dispatch 去虚化 fast path。
2. 删除 reachability 层的临时去虚化推断。
3. 删除 HIR scaffold 在 LLVM 入口里的长期地位。
4. LLVM backend 不再直接依赖：
   - HIR body
   - raw MIR body
   - 上游整包 stage outputs
5. 把保留的 LLVM 专属逻辑收口到：
   - backend-specific data initializer helper
   - LLVM target pass pipeline

完成标准：

1. `codegen_llvm` 的输入边界清晰。
2. 任何需要 HIR/MIR/effect facts 的现象，都被视为 LIR 不完整，而不是 backend 正常设计。

### P8：Final verification

目标：冻结新边界，并为未来 C backend 留出干净接口。

必须完成：

1. 全仓搜索确认：
   - 无旧 comptime surface
   - 无 HIR 层去虚化
   - 无 codegen 层去虚化
   - 无 stage output 嵌套上游整包
2. 文档与实现同步。
3. 若暂不实现 C backend，也至少固定它未来应依赖的输入边界。

完成标准：

1. `LLVM backend` 与未来 `C backend` 共用同一套 `LIR + LIR facts` 输入边界。
2. `PIPELINE_REFACTOR.md` 中的结构性约束都能在代码里找到对应实现落点。

### P9：Stage crate split

目标：把 §1.2 要求的 stage / fact crate DAG 用 cargo crate 边界硬化，让任何 PR 一旦违反 DAG 都在 `cargo build` 阶段失败，而不是依赖人工 review 或 grep-based gate。

必须完成：

1. 把以下模块抽成独立 crate：
   - `scoopc_ast`（含 lexer / parser / syntax / ast）
   - `scoopc_hir`（含 resolve / typecheck / infer / intrinsics / vtable / itable / expr_facts）
   - `scoopc_mir`（含 monomorph / opt pass pipeline / RTTI 算法 / mangler）
   - `scoopc_effect_stage`（含 effect builder + effect facts builder；与已有 data crate `scoopc_effect_facts` 区分）
   - `scoopc_lir`（effect_lowered）
   - `scoopc_codegen_llvm`（含 stackmap）
2. 把 cone 模块按数据 vs 操作两层拆开：
   - `Cone.toml` / `SourceConeGraph` / sysroot loader 等纯数据进 base crate `scoop_project_model`；
   - archive / scoopir / annotations / visibility / pre-specialize / consume 进新 crate `scoopc_cone`，依赖所有 stage crate。
3. 删除阻塞拆分的后向边：`InstanceKey` / `TemplateKey` / `ExternAbi` 等被反向引用的小型 stable 数据迁到 base crate；删除 P3 残留 `devirtualize.rs`。
4. 扩展 `tools/scoop_tools` 的 `dependency-gate` 以覆盖所有新 crate；同时禁止任何 stage crate 反向依赖 `scoopc_cone`。
5. `scoopc` umbrella crate 仅保留 façade re-exports 与 driver 编排（`pipeline/` / `session/` / `frontend.rs`）。

完成标准：

1. `cargo tree` 显示每个 stage crate 的依赖严格符合 §1.2 的 DAG 方向。
2. `dependency-gate` 把所有 base + fact + stage + cone crate 的边界全部纳入硬检查。
3. `scoopc/src/` 下不再有任何 stage 实际定义（全部 façade re-export）。
4. 完整 fixture suite 在新 crate 边界下保持通过。

### P10：Per-cone build artifact

目标：利用 P9 的 crate 边界让每个 cone 的编译产物正式落地到磁盘，下游 cone 通过反序列化上游 fact 注入下游 `Index` / `TypeEnv`，而不再重扫上游源码。

必须完成：

1. 跨进程 `TypeId` / type identity stable wire format 已由 P10-T01 固定为 portable `TypeStore` serialization：persisted facts/LIR 仍可携带 `TypeId`，但必须与 `type_store.bin` 同步落盘，下游进程先重建同构本地 type universe 再消费 facts/LIR。
2. 给 4 套 fact crate（`hir_facts` / `mir_facts` / `effect_facts` / `lir_facts`）以及 LIR program 加 `Serialize` / `Deserialize`（默认 bincode 二进制）+ schema version。
3. 在 `scoopc_cone` 内定义 `ConeArtifact` 结构与磁盘布局：

   ```text
   build/<profile>/cones/<cone-name>@<version>/
     manifest.json
     hir_facts.bin / mir_facts.bin / effect_facts.bin / lir_facts.bin
     lir_program.bin
     objs/scoop.o + objs/native_*.o
     inputs.fingerprint / outputs.fingerprint
   ```

4. 把 `crates/scoopc/src/frontend.rs::run_frontend` 从扁平 build_closure_sources 循环改为按 `SourceConeGraph::compilation_units()` 拓扑顺序运行；下游 cone 通过 `scoopc_cone` 的 fact 注入接口获取上游内容，不再 parse 上游源。
5. 把 `crates/scoop/src/commands/build/incremental.rs` 的整项目 SHA-256 fingerprint 替换为 per-cone inputs/outputs fingerprint chain；旧 whole-build SHA 只允许作为历史 `build.json` 审计字段存在，不再作为 cache 决策路径；上游 outputs.fingerprint 不变 → 下游加载 artifact 跳过上游所有 stage。
6. per-cone 多进程并发编译 CLI + driver 抽象（P10-T05 落地 surface，P10-T06 落地真正子进程并发）：
   - `scoop build` / `scoop run` 加 `-j / --jobs N` 参数与 `SCOOP_BUILD_JOBS` 环境变量回退，默认 `DEFAULT_BUILD_JOBS = 4`；
   - 在 `scoop` crate 内发布 `ConcurrencyStrategy` 与 `SubprocessConeCompiler` trait，把"如何决定并发数"和"如何在子进程里跑一个 cone"与调度 driver 解耦，给后续按物理 CPU / 远端 worker 池等策略和分布式编译留接入点；
   - `scoopc` binary 始终保持单纯编译执行体（即便后续暴露 `build-single-cone` 子命令也只接受最小输入：cone 标识 / 上游 artifact 路径 / 输出 artifact 目录），不持有 driver / 调度 / DAG 遍历职责；
   - 设计基线：cone DAG 中互不依赖的 cone 应允许并发跑；scoop 拥有 driver、scoopc 仅做 single-cone 编译执行体。
7. `scoopld` 抽出 + `scoopc link-cone` 子命令（P10-T06-b）：
   - `crates/scoopld` 拥有最终 link、runtime C object 编译和独立 link fingerprint cache；依赖只能指向 base/project-model/cone metadata 层，不得依赖 stage/codegen/`scoopc`/`scoop`；
   - `scoopc link-cone` 只解析单次 link 请求并 forward 到 `scoopld::link`，不 walk DAG、不调度、不 fork 其它 cone；
   - `scoop` driver 负责收集 transitive dependency artifact 的 `objs/*`，通过 `--dep-obj` 传给 `link-cone`，link cache 存在 `build/<profile>/link/<consumer>@<ver>/inputs.fingerprint`；
   - `build-single-cone` 不允许在缺非默认 sysroot 上游 artifact 时从源码兜底 lower，上游 artifact 缺失必须是硬错误。
8. scoop facade 化与 project model 边界整顿（P10-T06-c / P10-T06-d）：
   - `scoop build` / `scoop run` 的 executable 路径把 consumer cone 也纳入 `build-single-cone` 子进程派发，随后只读取 consumer/dependency artifact manifest 的 `objs/*` 并派发 `link-cone`；
   - 单文件输入由 `scoop` 在 `build/<profile>/virtual/<name>@0.0.0/` materialize 为标准 cone 后走同一 `build-single-cone` / `link-cone` 子进程路径，`scoopc` 不再提供裸文件 Legacy CLI；
   - cone-local native C/C++ object 在 `build-single-cone` 内写入 cone artifact；
   - 纯 project metadata crate 统一命名为 `scoop_project_model`，artifact manifest/schema/fingerprint helper 下沉到该 crate，`scoopc_cone` 只保留 stage payload 磁盘层；`scoop` crate 的内部 workspace 依赖白名单只允许 `scoop_project_model`，编译与 link 只能通过子进程 CLI 交互。

不在本阶段完成（留待后续单独立项 P11）：

- P11：跨 cone generic body 共享（HIR/MIR template wire format）。本阶段下游对上游 generic 仍 re-lower，但只重做 generic body，不再过完整上游 frontend；这是有意识的范围裁剪，不能在 P10 清场中继续扩展。
- `.cone` archive 复活；磁盘格式直接是目录树，`scoop package` 仍保持停用。

完成标准：

1. 4 套 fact + LIR program 能往返序列化（带 schema version）。
2. 跨 cone fixture（如 `tests/fixtures/run_pass_cone/source_path_dependency_public_call`）下游 cone 编译时不再 parse 上游源；用 trace 或断言能稳定证明。
3. per-cone fingerprint chain：上游变 → 上游 + 下游重 build；下游变 → 仅下游重 build；toolchain 变 → 全重 build。
4. cache hit 启动时间不再受 sysroot 重 hash 拖累（实测从 ~4s 量级降到 < 100ms 量级）。
5. `scoop build` / `scoop run` 的 executable 路径没有 in-process whole-DAG fallback：driver 只 materialize 输入、调度 `build-single-cone`、读取 artifact metadata 并派发 `link-cone`。

## 5. 优化 pass 专项约束

为了避免后续“顺手在某层塞一个优化”的回退，本计划单独固定下面这组规则：

1. HIR 不承载 optimization pass。
2. MIR 承载普通调用图/实例级优化。
3. LIR 承载 state-machine / higher-order wrapper 相关的窄优化 family。
4. codegen 只承载 backend-specific 优化。
5. 去虚化只能有一个普通语义 owner：MIR。
6. LIR 可以做 post-state-machine 的 targeted inline/devirt，但只能服务于局部 state-machine elimination，不能升级成全程序通用 inliner。

## 6. 验收标准

当下面这些条件都满足时，才能认为本计划阶段性完成：

1. 每个 stage crate 的依赖只指向前一阶段 crate、基础 crate和更早阶段的 fact crate。
2. 没有任何 fact crate 依赖其它 fact crate 或 stage crate。
3. 没有任何 `StageOutput` 嵌套上一阶段的完整输出。
4. 没有任何下游逻辑把两个 fact table 当成可替代输入。
5. codegen backend 只依赖 `LIR + LIR facts + base context`。
6. 全部静态可判定的源码错误都在 `AST -> HIR` 屏障前收口。
7. 正式 pipeline 中不再保留现有 comptime/const surface 或任何专门兼容逻辑。
8. global object/var/val 与 `@CallingConvention` 的 non-generic 约束在前端稳定生效。
9. top-level eager init 与 object once 语义闭合，覆盖所有 linked cones。
10. 每个 stage 都是独立 crate，且依赖方向由 `cargo build` 强制（违反 §1.2 的 PR 立即失败）；`scoopc` umbrella crate 仅剩 façade re-exports 与 driver 编排。
11. cone 按数据 vs 操作两层归位：纯数据在 base crate `scoop_project_model`，跨 stage 操作在 `scoopc_cone`，没有 stage crate 反向依赖 `scoopc_cone`。
12. 每个 cone 的编译产物落到 `build/<profile>/cones/<cone>/` 下，schema version 完备；下游 cone 通过反序列化 fact 注入而不再扫上游源；per-cone fingerprint chain 替代旧的整项目 fingerprint，cache hit 启动时间从 sysroot 重 hash 量级降到读取 outputs.fingerprint 量级。

## 7. 说明

本计划是执行计划，不是 TODO 列表。

它的用途是：

1. 固定阶段顺序和依赖关系。
2. 固定哪些问题必须在哪一层被解决。
3. 为后续拆任务、拆 PR、补 TODO 提供统一基线。

如果实现过程中发现需要改变：

1. 编译单元定义
2. stage/fact crate DAG
3. HIR 错误收口边界
4. MIR/LIR/codegen 的优化归属
5. 全局初始化语义

都必须先回写 `PIPELINE_REFACTOR.md`，再继续修改代码。
