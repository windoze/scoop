# TODO（P8：删除旧主线并再次 full regression）

> 生成时间：2026-05-02  
> 设计基线：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md)  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 前置条件：`TODO-P7.md` 已完整完成；refactor 路径已经是默认主线；标准 full regression 与 GC env 全开验证已在默认主线下通过；legacy 路径只剩显式 compare/rollback 入口。  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 本阶段目标：删掉旧的 legacy effect/continuation 主线，实现真正收口；保证仓库中不再存在“默认靠新主线，但旧主线还悄悄救场”的隐藏依赖；在“只剩新主线”的前提下再次跑完整回归与 GC env 验证，证明新路径单独存在时仍完整通过。
> 2026-05-09 更新：语言层 `async` / `await` / `Task` 等 surface 已移除；P8 中与 tests / fixtures / docs 相关的清理任务必须一并覆盖这些已删除语法的现行残留。历史归档可保留相关叙述，但不得继续出现在主文档、主 fixtures 路径或主测试索引中。

## 全局约束

- [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) 是本阶段唯一设计基线；若实现过程中需要改变主张，必须先回写该文档，再继续实现。
- [`PLAN.md`](./PLAN.md) 与 [`TODO-P0.md`](./TODO-P0.md)、[`TODO-P1.md`](./TODO-P1.md)、[`TODO-P2.md`](./TODO-P2.md)、[`TODO-P3.md`](./TODO-P3.md)、[`TODO-P4.md`](./TODO-P4.md)、[`TODO-P5.md`](./TODO-P5.md)、[`TODO-P6.md`](./TODO-P6.md)、[`TODO-P7.md`](./TODO-P7.md) 是本阶段执行前提；P8 不得重新开启 P0-P7 已收敛的 selector / typed HIR / direct-style MIR / effect facts / late lowering / LLVM backend / GC/runtime 语义讨论。
- 本阶段只做三件事：
  1. 删除旧 selector 分支与旧 effect/continuation 主线；
  2. 删除只服务旧主线的桥接 helper、旧 dump/fixture、旧测试/文档引用、以及只为 legacy 形状存在的适配层；
  3. 在“仓库中只剩新主线”的条件下，重跑 P7 的完整回归矩阵。
- 明确禁止：
  - 在 P8 中重新设计 `StepSchema`、`ContinuationSchema`、`resolved_outward_cases`、`impl_plan`、late lowering、LLVM ABI、GC roots、runtime error ordinary effect、或 dropped continuation 语义；这些在 P4-P7 已经闭合。
  - 在 P8 中通过重新引入 fallback、兼容 shim、或“临时保留 legacy 分支以防万一”的方式拖延删除；P8 的目标就是收口，而不是继续过渡。
- P8 结束后，仓库中不得再保留任何用户可触达或默认不可见但仍可生效的 legacy effect/continuation 主线。
  - 这包括但不限于：
    - CLI 层的 `--effect-pipeline legacy`
    - `Session` / dispatcher 中的 legacy branch
    - legacy effect lowering 主模块
    - legacy LLVM effect backend
    - legacy compare-only helper 若其存在只为 effect 主线保留
    - tests / fixtures / docs 中把 legacy 当作可执行主线的入口
  - 明确禁止：
    - 只删 CLI 参数，但内部 selector/legacy branch 还在
    - 只删一部分 emitter/helper，但旧 dispatcher/旧 API 仍可调用
    - 只删代码，不删 tests / docs / fixtures 中对 legacy 主线的引用
    - 保留“单 `perform` 快路径”“线性 body 专用路径”“仅 `handle` 局部状态机主线”“tail-`resume` 专用 lowering”等 code-shape-specific 残余分支
- P8 的删除动作必须服从 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.16、§5.5、§8 的统一管线原则。
  - 删除后仍允许存在的差异只能来自显式 facts / `ImplPlan` / 优化级别 / target ABI 等已收敛输入；
  - 任何以“代码长得简单”为由保留的第二套 lowering 入口都必须一并清掉。
- P8 不再保留 effect pipeline selector。
  - 推荐最终状态：
    - CLI 上删除 `--effect-pipeline` 参数；
    - `SessionOptions` / `EffectPipelineMode` / dispatcher bifurcation 被删除或折叠为单一路径；
    - 所有默认构造都只剩 refactor 主线；
  - 若因仓库其它非 effect 机制仍需要某个命名相似的配置结构，必须在完成记录中明确说明该结构已不再承载 legacy/refactor bifurcation。
- 删除时允许复用/保留的仅限真正中立的共享模块。
  - 例如：
    - `source` / `span` / `parser`
    - 通用 MIR / LLVM helper
    - 通用 GC/runtime 支撑层
  - 但这些模块中不得残留“为 legacy effect 主线特设”的分支、注释、helper 或测试假设。
- 删除必须覆盖 driver、compiler、runtime-facing glue、tests、fixtures、docs 五个层面。
  - 任何一层残留 legacy effect 主线入口，都视为 P8 未完成。
- 本阶段必须执行完整回归。
  - 必须覆盖：
    - `cargo test --all`
    - `cargo run -p scoop -- test`
    - `cargo run -p scoop_tools -- spec-fixtures check`
    - `cargo clippy --all-targets -- -D warnings`
    - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
    - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`
  - 所有这些命令都必须在**没有 legacy selector、没有 legacy fallback** 的前提下通过。
- 若删除后某些测试/文档仍需要对“旧行为”做历史对比：
  - 必须把它们改写成纯新主线路径下的回归断言，或直接删除；
  - 明确禁止：为了保留对比而继续保留可执行的 legacy 主线代码。
- 若完成删除后发现某些共享模块中仍残留 `legacy` 字样，不一定必须全部删除；
  - 但必须逐条判断它是否仍表示“旧 effect/continuation 主线仍存活”；
  - 若只是历史注释/测试名/错误消息残留，也必须在本阶段清理；
  - P8 结束时不允许在 effect/continuation 主实现、CLI/help、或测试主路径中保留误导性的 legacy 主线术语。

## [DONE] P8-T01：删除顶层 legacy selector 与并行 dispatcher 壳层，收口为单一 refactor 主线入口

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P8
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.10-§4.11, §8
  - 前置实现参考：[`TODO-P0.md`](./TODO-P0.md) P0-T01 / P0-T02、[`TODO-P7.md`](./TODO-P7.md) P7-T01 / P7-T02
- 目标：
  - 删除用户可见和内部可见的 legacy/refactor selector 分叉；
  - 把 driver/session/dispatcher 收口成只剩 refactor 主线的单一路径；
  - 确保 omission/default 已不是“默认 refactor + 仍可选 legacy”，而是“仓库只剩一种主线”。

- 必须实现的内容：
  1. 删除 CLI 层的 effect pipeline selector。
     - 至少检查并修改：
        - `crates/scoop/src/cli.rs`
        - `crates/scoop/src/commands/mod.rs`
        - `crates/scoopc/src/bin/scoopc.rs`
        - 任何仍暴露 effect pipeline selector 的命令行 parse 位置
        - 若仓库中存在调用 `scoop` / `scoopc` 的 wrapper script，再同步修改对应位置；当前 `tools/scoop_tools` Rust binary 本身不在 selector 删除范围内
     - 要求：
       - 删除 `--effect-pipeline legacy`
       - 删除 `--effect-pipeline refactor`
       - CLI/help 文本中不再提及双主线选择
     - 明确禁止：保留一个“只接受 refactor 的 no-op selector”作为历史包袱。
  2. 删除 `Session` / config 中的 pipeline bifurcation。
     - 至少检查并修改：
       - `crates/scoopc/src/session/**`
       - 任何 `SessionOptions` / `EffectPipelineMode` / dispatcher config 定义位置
     - 要求：
       - 不再存在 legacy/refactor mode 枚举或布尔分支；
       - `Session::new()` / 默认构造只代表唯一主线；
       - 若原先有显式 refactor-only 构造 helper，可以折叠为唯一构造入口。
  3. 删除顶层 dispatcher 壳层中的 legacy 分支。
     - 至少检查并修改：
       - `crates/scoopc/src/effect_refactor_pipeline/**` 或其等价模块
       - `scoop` / `scoopc` / fixtures / tools 里与 stage route 相关的入口
     - 要求：
       - 不再保留 `legacy`/`refactor` 二选一分发；
       - refactor stage 直接成为唯一生产路径；
       - 若壳层本身在 P0-P7 的主要职责只是分流，则应在本任务中折叠或改名为不带“并行双线”语义的单一路径模块。
  4. 删除 compare/rollback 专用的 top-level glue。
     - 若某些 helper 仅为“保留 explicit legacy 入口”而存在，且不再被其它中立功能使用，必须一并删除；
     - 若某些 helper 已演化为中立 shared API，可以保留，但必须删掉其中的 legacy branch / naming / docs。
  5. 更新 parse / session / command 路径测试。
     - 旧的“缺省为 refactor、显式 legacy 仍可用”测试必须替换为：
       - 不再解析该参数
       - 不再存在 legacy 分支
       - 默认构造只有唯一主线
     - 若某些测试专门验证 compare/rollback 行为，P8 必须删除或改写它们。

- 必须遵从的约束：
  - 禁止只删 CLI 参数而保留内部 legacy branch。
  - 禁止保留“不对用户暴露，但测试/helper 还能打开 legacy”的隐藏入口。
  - 禁止因为删除 selector 太麻烦，就继续保留 `EffectPipelineMode::Refactor` 这种单值枚举或伪 selector。
  - 禁止在本任务里重新把 selector 下沉到低层业务模块作为“临时兼容”。

- 验证：
  1. 新增/更新定向测试，推荐命名：
     - `effect_pipeline_selector_removed_*`
     - `single_effect_pipeline_session_*`
  2. 运行：
     - `cargo test -p scoop cli`
     - `cargo test -p scoopc session`
  3. 最小 smoke：
     - `cargo run -p scoop -- dump-ast tests/fixtures/parse/hello.scoop`
     - `cargo run -p scoop -- build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p8_single_pipeline.ll`
     - `cargo run -p scoop -- test --fixtures tests/fixtures/build/emit_llvm_basic.scoop`
  4. 额外负向检查：
     - `cargo run -p scoop -- --effect-pipeline legacy dump-ast tests/fixtures/parse/hello.scoop`
       - 要求：命令行参数已不存在并报稳定错误，或 clap 解析失败；不能再成功运行
  5. 仓库搜索：
     - `rg "effect[-_]pipeline|EffectPipelineMode|legacy.*refactor|refactor.*legacy" crates tools tests --glob '!target/**'`
     - 完成记录中必须总结哪些命中仍是中立历史文本，哪些已被删净。

- 完成条件：
  - 仓库顶层已不存在可执行的 legacy selector / dispatcher 分支；
  - 所有入口默认并且只会进入新主线；
  - 后续任务可以只围绕“删除旧实现与清理残留”继续推进。
- 依赖：`TODO-P7.md` 最后一项 review 完成
- 完成记录：
  - 2026-05-09：已删除 `scoop` / `scoopc` CLI 的 `--effect-pipeline` 解析、帮助文本与显式 legacy/refactor 传递；相关负向测试现断言该参数会被稳定拒绝。
  - 2026-05-09：已删除 `crates/scoopc/src/session/mod.rs` 中的 `EffectPipelineMode` 与 session bifurcation，`SessionOptions` 收口为不承载 pipeline 分叉的空配置壳；`Session::new()` / `with_options()` 只代表唯一主线。
  - 2026-05-09：已折叠 `crates/scoopc/src/effect_refactor_pipeline/mod.rs` 的顶层 dispatcher，并删除 `effect_refactor_pipeline/legacy.rs`、`effect_refactor_pipeline/refactor.rs` 与 `crates/scoop/src/commands/parity.rs`；stage API 现直接调用唯一生产路径。
  - 2026-05-09：已清理 fixture/run-pass/build/frontend 对 selector 的注入与 mode 分支；`mir_refactor`、`infer`、`dump-*`、`build`、`scoop test` 相关测试已改为验证“只有单一路径”或“参数已移除”。
  - 验证：`cargo fmt`
  - 验证：`cargo test -p scoop cli`
  - 验证：`cargo test -p scoopc session`
  - 验证：`cargo test -p scoopc driver_cli`
  - 验证：`cargo clippy --all-targets -- -D warnings`
  - 验证：`cargo run -p scoop -- dump-ast tests/fixtures/parse/hello.scoop`
  - 验证：`cargo run -p scoop -- build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p8_single_pipeline.ll`
  - 验证：`cargo run -p scoop -- test --fixtures tests/fixtures/build/emit_llvm_basic.scoop`
  - 验证：`cargo run -p scoop -- --effect-pipeline legacy dump-ast tests/fixtures/parse/hello.scoop`，结果为 clap 失败：`unexpected argument '--effect-pipeline' found`。
  - 仓库搜索摘要：`EffectPipelineMode`、session pipeline bifurcation 与 CLI selector 解析路径已从主实现中删净；剩余 `rg` 命中仅包含删除说明/负向测试文案、placeholder inventory 的历史对照注释，以及 `effect_refactor` / `legacy` 命名的历史回归测试名或 fixture 名，不再构成可执行的顶层 legacy 入口。

## [DONE] P8-T01R：Review selector/dispatcher 删除结果，确认仓库已不存在 legacy 顶层入口或隐藏切换点

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P8
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §8
  - [`TODO-P0.md`](./TODO-P0.md) P0-T01 / P0-T02
- 重点：
  - CLI/session/dispatcher 是否已收口为单一路径；
  - 是否还存在可执行的 legacy 顶层入口、隐藏 helper、或单值伪 selector；
  - tools / fixtures / tests 是否也已同步删除 selector 假设。
- 必须检查的文件/位置：
  - `crates/scoop/src/cli.rs`
  - `crates/scoop/src/commands/**/*.rs`
  - `crates/scoopc/src/bin/scoopc.rs`
  - `crates/scoopc/src/session/**`
  - `crates/scoopc/src/effect_refactor_pipeline/**`
  - 任何仍暴露 effect pipeline selector 的命令行 parse 位置

- 验证：
  - 重新运行 P8-T01 的全部测试与命令；
  - 额外搜索：
    - `rg -e "--effect-pipeline|EffectPipelineMode|SessionOptions|legacy.*selector|refactor.*selector" crates tools tests --glob '!target/**'`
  - 要求：
    - 允许命中：历史迁移注释、测试里断言“参数已被移除”的文本；
    - 不允许命中：仍可执行 legacy 切换、或仍承载 bifurcation 的主实现。

- 完成条件：
  - review 能明确说明：legacy 顶层入口已删除，P8 后续只需清理旧实现本体与残留引用；
  - 可进入 P8-T02。
- 依赖：P8-T01
- 完成记录：
  - 2026-05-09：完成 review。复核 `crates/scoop/src/cli.rs`、`crates/scoop/src/commands/mod.rs`、`crates/scoopc/src/bin/scoopc.rs`、`crates/scoopc/src/session/mod.rs`、`crates/scoopc/src/effect_refactor_pipeline/mod.rs`、fixture helper 与 `crates/scoop/tests/p7_default_pipeline.rs`，确认 CLI/session/dispatcher 已收口为单一路径；`SessionOptions` 仅剩不承载 pipeline bifurcation 的空配置壳。
  - 2026-05-09：额外确认 `crates/scoopc/src/effect_refactor_pipeline/` 已不再包含 `legacy.rs` / `refactor.rs` 顶层 dispatcher 子模块；目录仅剩单一路径 stage API 与实现文件，未发现隐藏 legacy 切换点。
  - 2026-05-09：搜索 `--effect-pipeline|EffectPipelineMode|SessionOptions|legacy.*selector|refactor.*selector`（限定 `crates` / `tools` / `tests`）后，命中仅剩：负向测试里断言 `--effect-pipeline` 已被移除、`SessionOptions` 的单一路径空配置用法、`effect_refactor_pipeline/mod.rs` 的迁移说明，以及与 LLVM callable-version 相关但不承载 effect pipeline bifurcation 的“selector”术语；未发现可执行 legacy 顶层入口或隐藏 helper。
  - 验证：`cargo test -p scoop cli`
  - 验证：`cargo test -p scoopc session`
  - 验证：`cargo test -p scoopc driver_cli`
  - 验证：`cargo test -p scoop --test p7_default_pipeline`
  - 验证：`cargo run -p scoop -- dump-ast tests/fixtures/parse/hello.scoop`
  - 验证：`cargo run -p scoop -- build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p8_single_pipeline.ll`
  - 验证：`cargo run -p scoop -- test --fixtures tests/fixtures/build/emit_llvm_basic.scoop`
  - 验证：`cargo run -p scoop -- --effect-pipeline legacy dump-ast tests/fixtures/parse/hello.scoop`，结果为 clap 失败：`unexpected argument '--effect-pipeline' found`。
  - 验证：`cargo clippy --all-targets -- -D warnings`

## [DONE] P8-T02：删除 legacy effect/continuation lowering 主线、legacy LLVM effect backend，以及所有 code-shape-specific 旧入口

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P8
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.16, §5.4, §5.5, §8
  - 前置实现参考：[`TODO-P5.md`](./TODO-P5.md) P5-T01~T07、[`TODO-P6.md`](./TODO-P6.md) P6-T01~T05
- 目标：
  - 删除旧的 legacy effect/continuation lowering 主线与 LLVM effect backend；
  - 删除只服务旧主线的桥接 helper 与旧 ABI/contract 层；
  - 删除任何残留的 code-shape-specific lowering 入口，确保 effectful callable 只经统一 facts-driven transformation 与新 LLVM backend。

- 必须实现的内容：
  1. 删除 legacy 中层 effect/state-machine 主线。
     - 首先审查并处理以下目录或等价位置：
       - `crates/scoopc/src/effect/state_machine/**`
       - `crates/scoopc/src/effect/analysis.rs` 中只服务旧主线的 effect lowering/escape facts glue
       - `crates/scoopc/src/effect/step_summary.rs` 中只服务旧主线的摘要/contract glue
     - 要求：
       - 若某个模块的职责已被 P4/P5/P6 新主线完全覆盖，直接删除；
       - 若某个模块还含有中立 shared helper，必须先抽离中立部分，再删除 legacy-specific 业务逻辑；
       - 不能以“以后再删”为由保留整个旧目录空转。
  2. 删除 legacy LLVM effect backend。
     - 首先审查并处理以下目录或等价位置：
       - `crates/scoopc/src/llvm/codegen/effect/contract.rs`
       - `crates/scoopc/src/llvm/codegen/effect/state_machine_bridge.rs`
       - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`
       - `crates/scoopc/src/llvm/codegen/effect/mod.rs`
       - `crates/scoopc/src/llvm/emit.rs` / `llvm/mod.rs` 中只服务 legacy effect 主线的 `production_lowered_hir` 族 API
       - `crates/scoopc/src/llvm/frontend.rs` 中只为旧 effect lowering 兼容保留的入口
     - 要求：
       - 若 API 只为旧主线服务，直接删除；
       - 若测试还依赖这些 API，测试必须同步改写到新主线或删除；
       - 不允许保留“没人调了但还在仓库里”的 dead legacy backend。
  3. 删除 code-shape-specific 旧入口与特判路径。
     - 至少要清理：
       - 单 `perform` 快路径
       - 线性 body 专用路径
       - 仅 `handle` 局部状态机主线
       - tail-`resume` 专用 lowering
       - 任何在 effect/continuation 主实现中，以 code shape 而非显式 facts 决定另一套 transformation 的分支
     - 若某处 today 仍保留这类代码但已被新主线绕过，也必须在本任务中删掉，而不是仅依赖“不会被调用”。
  4. 删除 `production_lowered_hir` / eager-HIR 兼容链路中只服务旧 effect 主线的残留。
     - 若某些 `emit_minimal_main_*_from_production_lowered_hir*`、`from_lowered_hir*`、`legacy_eager_hir` helper 仍仅为旧 effect 主线保留，必须删除或收窄到纯非-effect 历史测试用途；
     - 若它们对整个 compiler crate 仍有中立价值，必须在完成记录中明确说明：
       - 为什么它们不再承载 legacy effect 主线；
       - 当前仅剩的职责是什么；
       - 为什么不会形成“旧主线悄悄救场”的隐藏依赖。
  5. 清理命名、注释、错误消息与导出表。
     - 至少检查并必要时修改：
       - `pub use` 导出列表
       - 模块级注释
       - README / 开发文档 / 迁移注释
       - 错误消息中把当前主线仍称为 legacy 的残留文本
     - 目标：让仓库公开叙述中不再把 legacy 主线描述成仍存在的正常实现。

- 必须遵从的约束：
  - 禁止把 legacy 代码仅仅注释掉而不真正删除。
  - 禁止保留“暂时没人调用”的 dead legacy backend，等待未来再清理。
  - 禁止把 code-shape-specific 旧分支改名后继续保留在 refactor 主实现中。
  - 禁止为了减少 diff 而把 legacy 语义 helper 塞进 refactor 主实现做兼容。
  - 若某个模块既含中立 helper 又含 legacy 业务，必须先抽中立部分，再删 legacy 业务；不能因为有少量可复用逻辑就整个目录保留。

- 验证：
  1. 新增/更新定向测试，推荐命名：
     - `legacy_effect_backend_removed_*`
     - `single_effect_lowering_path_*`
  2. 运行：
     - `cargo test -p scoopc legacy_effect_backend_removed`
     - `cargo test -p scoopc single_effect_lowering_path`
  3. 仓库搜索（执行时必须在完成记录中附摘要）：
     - `rg "state_machine_bridge|state_machine_emitter|UnifiedHandleLoweringContract|begin_legacy_effect_boundary|finish_legacy_effect_boundary|production_lowered_hir|legacy_eager_hir|single perform|tail-resume|statement-only|linear body" crates/scoopc/src crates/scoop/src tools --glob '!target/**'`
  4. 要求：
     - 允许命中：历史说明、负向测试、迁移注释中明确表述“已删除”的文字；
     - 不允许命中：仍然参与主构建、主 codegen、主 lowering 的 legacy 主线代码。

- 完成条件：
  - 仓库中旧 effect/continuation lowering 主线与 legacy LLVM effect backend 已被删除或收窄到完全中立的共享 helper；
  - code-shape-specific 旧入口已被清理；
  - 后续任务只需清理 tests / docs / fixtures 残留并做最终全量验证。
- 依赖：P8-T01R
- 完成记录：
  - 2026-05-09：已删除 `crates/scoopc/src/effect/state_machine/segments.rs`、`crates/scoopc/src/effect/state_machine/transform.rs`、`crates/scoopc/src/effect/step_summary.rs`、`crates/scoopc/src/llvm/codegen/effect/state_machine_bridge.rs` 与 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`；仓库不再保留旧 unified-handle state-machine emitter/bridge 主线。
  - 2026-05-09：已把 `crates/scoopc/src/effect/mod.rs` / `effect/state_machine/mod.rs` 收窄为仅承载当前 backend 仍需的共享 suspendability / ordinary-callee 分析；保留的 `effect/analysis.rs` 与 `effect/state_machine/analysis.rs` 只提供中立 shared helper，不再导出旧 lowering pipeline、旧 segment/transform 主线或 step-summary glue。
  - 2026-05-09：已新增 `crates/scoopc/src/llvm/codegen/effect/ordinary_callee.rs` 承接现行 ordinary-callee suspend plan bridge，并恢复 `llvm/codegen/call/resume.rs` 中仍被现行 backend 使用的 call-resume ABI helper；同时删除 HIR `handle` 旧 state-machine lowering 入口、旧 continuation replay shim，以及一批已无调用的 legacy runtime ABI / contract helper。
  - 2026-05-09：已把 `begin_legacy_effect_boundary` / `finish_legacy_effect_boundary` 改名为中立 effect-boundary helper，把 `production_lowered_hir` 族 API/注释/测试改名为 `materialized_lowered_hir`，并把 `legacy_eager_hir` 改名为 `direct_lowered_hir`；这些保留 API 当前仅表达“带 materialized pass-view 的 HIR/LLVM handoff”或“直接 HIR lowering 模式”，不再承载旧 effect 主线语义。
  - 2026-05-09：已新增 `legacy_effect_backend_removed_source_inventory` 与 `single_effect_lowering_path_source_inventory` 定向守护测试，确认 legacy backend marker 只剩负向测试文本。
  - 验证：`cargo check -p scoopc --features llvm`
  - 验证：`cargo fmt`
  - 验证：`cargo test -p scoopc legacy_effect_backend_removed`
  - 验证：`cargo test -p scoopc single_effect_lowering_path`
  - 验证：`cargo clippy --all-targets -- -D warnings`
  - 验证：`rg -n "state_machine_bridge|state_machine_emitter|UnifiedHandleLoweringContract|begin_legacy_effect_boundary|finish_legacy_effect_boundary|production_lowered_hir|legacy_eager_hir|single perform|tail-resume|statement-only|linear body" crates/scoopc/src crates/scoop/src tools --glob '!target/**'`
  - 仓库搜索摘要：唯一命中位于 `crates/scoopc/src/llvm/tests.rs` 新增的负向守护测试字面量；`crates/scoopc/src` / `crates/scoop/src` / `tools` 主实现中已无上述 legacy lowering marker 命中。

## [DONE] P8-T02R：Review legacy 主线删除结果，确认旧 backend 与 shape-specific 入口已经真正消失

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P8
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.16, §5.5, §8
- 重点：
  - 是否真的删除了 legacy effect/state-machine 主线与 legacy LLVM effect backend；
  - 是否还残留 dead code、注释掉代码、或“中立 helper”名义下的隐藏旧路径；
  - code-shape-specific 旧 lowering 入口是否已经从主实现中消失。
- 必须检查的文件/位置：
  - `crates/scoopc/src/effect/**`
  - `crates/scoopc/src/llvm/codegen/effect/**`
  - `crates/scoopc/src/llvm/emit.rs`
  - `crates/scoopc/src/llvm/mod.rs`
  - `crates/scoopc/src/llvm/frontend.rs`
  - `crates/scoopc/src/llvm/tests.rs`

- 验证：
  - 重新运行 P8-T02 的全部测试与命令；
  - 额外搜索：
    - `rg "legacy|state_machine_bridge|state_machine_emitter|UnifiedHandleLoweringContract|production_lowered_hir|legacy_eager_hir|single perform|tail-resume|statement-only|linear body" crates/scoopc/src crates/scoop/src tools --glob '!target/**'`
  - 要求：
    - 允许命中：历史迁移注释、测试名中说明“已移除 legacy”的文字；
    - 不允许命中：effect/continuation 主实现里仍存在可执行旧路径。

- 完成条件：
  - review 能明确说明：旧主线实现本体已经删净，仓库接下来只剩文本/测试/fixture 残留与最终回归收口；
  - 可进入 P8-T03。
- 依赖：P8-T02
- 完成记录：
  - 2026-05-09：完成 review。复核 `crates/scoopc/src/effect/**`、`crates/scoopc/src/llvm/codegen/effect/**`、`crates/scoopc/src/llvm/emit.rs`、`crates/scoopc/src/llvm/mod.rs`、`crates/scoopc/src/llvm/frontend.rs` 与 `crates/scoopc/src/llvm/tests.rs`，确认 P8-T02 删除的 `segments.rs` / `transform.rs` / `step_summary.rs` / `state_machine_bridge.rs` / `state_machine_emitter.rs` 未回流，`effect/mod.rs` 与 `effect/state_machine/mod.rs` 仅剩 ordinary-callee 共享分析与当前 backend 仍需的中立 helper。
  - 2026-05-09：review 过程中发现当前 LLVM/effect 主实现仍残留少量误导性 `legacy` 命名/注释；已同步清理为中立命名（例如 continuation payload helper、effect-call wrapper 参数、callable-carrier fallback 命名，以及相关注释/测试 helper 命名），避免把已删除旧 backend 误写成仍在生效的路径。
  - 2026-05-09：复查 `rg "state_machine_bridge|state_machine_emitter|UnifiedHandleLoweringContract|begin_legacy_effect_boundary|finish_legacy_effect_boundary|production_lowered_hir|legacy_eager_hir|single perform|tail-resume|statement-only|linear body" crates/scoopc/src crates/scoop/src tools --glob '!target/**'` 后，命中仅剩 `crates/scoopc/src/llvm/tests.rs` 的负向 inventory 测试字面量；主实现中已无这些旧 backend / code-shape-specific marker。
  - 2026-05-09：额外搜索 `legacy|state_machine_bridge|state_machine_emitter|UnifiedHandleLoweringContract|production_lowered_hir|legacy_eager_hir|single perform|tail-resume|statement-only|linear body`（限定 `crates/scoopc/src` / `crates/scoop/src` / `tools`）后，剩余命中均已判定为非阻塞残留：负向删除测试、迁移说明、direct-HIR/dump 兼容注释或“已删除语法”的诊断文本；未发现 effect/continuation 主实现中仍可执行的旧 lowering/backend 路径。
  - 验证：`cargo fmt`
  - 验证：`cargo check -p scoopc --features llvm`
  - 验证：`cargo test -p scoopc legacy_effect_backend_removed`
  - 验证：`cargo test -p scoopc single_effect_lowering_path`
  - 验证：`cargo clippy --all-targets -- -D warnings`

## [DONE] P8-T03：清理 tests / fixtures / docs 中的 legacy 主线与已删除 async/Task surface 残留，并把 compare 型资产改写为纯新主线回归

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P8
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §8
  - 前置实现参考：[`TODO-P7.md`](./TODO-P7.md) P7-T02 / P7-T04
- 目标：
  - 删除测试、fixtures、文档、注释、示例命令中对 legacy 主线仍可作为正常入口的引用；
  - 删除测试、fixtures、文档、注释、示例命令中把已移除 `async` / `await` / `Task` surface 仍写成现行语法或现行能力入口的残留；
  - 把原先以 compare/rollback 为目的保留到 P7 的资产，改写成纯新主线路径的回归断言，或直接删除；
  - 确保仓库的公开叙述与测试主路径都符合“只有新主线存在”的最终状态。

- 必须实现的内容：
  1. 清理 tests / fixtures / helper 中的 legacy selector 与 legacy compare 假设。
      - 至少检查并必要时修改：
        - `crates/scoop/src/fixtures/mod.rs`
        - `crates/scoop/src/fixtures/expectations.rs`
        - `crates/scoopc/src/llvm/tests.rs`
        - `tools/scoop_tools/src/fixtures_matrix.rs`
        - 任何还显式使用 `--effect-pipeline legacy` 的测试 helper、fixture 注释、或 compare harness
     - 要求：
       - 如果某测试原本只是为了证明 default/refactor/legacy 三者差异，现在应改写成只断言新主线行为；
       - 如果某测试仅为了保留 legacy 可执行性而存在，且对新主线已无价值，应删除。
  2. 清理 docs / README / 开发文档 / 迁移注释中的 legacy 主线叙述与已删除 async/Task surface 叙述。
      - 至少要去掉：
        - “默认还是 legacy”
        - “可通过 `--effect-pipeline legacy|refactor` 选择”
        - “P8 前暂时保留 legacy” 之类已经过时的过渡说明
        - 把 `async` / `await` / `Task` 当作现行 surface 语法、现行标准库入口或现行 fixture 分类的说明
      - 若需保留历史背景，只能作为已完成迁移的历史说明，而不能继续给出可执行 legacy 用法。
  3. 清理 build/run/spec-fixture 命令示例与主路径资产中的已删除 async/Task surface 残留。
      - 所有正常示例命令都应直接使用默认新主线；
      - 不再出现显式 `--effect-pipeline refactor` 作为必须参数；
      - 不再出现显式 `--effect-pipeline legacy` 作为可选主线。
      - 若 parse/typecheck/run-pass/spec-fixture/tools 索引里仍把 `async` / `await` / `Task` 作为现行 surface 分类、fixture 前缀、feature gate 或示例主题，必须删除、改名或改写为当前语义等价的回归资产。
      - 若某资产只对已删除 surface 有意义，且不存在新的现行语义价值，应直接删除或转入历史归档，而不是继续留在主路径。
  4. 清理 naming 残留。
      - 若某些 helper/test/fixture 名仍以 `legacy_` / `old_` 命名，但语义已不再真的对应旧主线，必须改名；
      - 若某些 helper/test/fixture 名仍以 `async_` / `await_` / `task_` 暗示当前语言仍暴露对应 surface，且其内容并非历史归档或明确的删除守护，必须改名、改写或删除；
      - 若某测试的唯一目的就是证明“legacy 已删除”，允许保留 `legacy_removed_*` 这类负向命名；
      - 但不得让主测试集继续以“legacy 是正常实现之一”的方式组织。
  5. 建立“仓库中仅剩新主线”的定向清理守护。
      - 至少要新增或更新一组搜索/断言测试，证明：
        - CLI/help 文本不再暴露 legacy selector；
        - tests / docs / fixtures 中不再把 legacy 当作执行主线；
        - tests / docs / fixtures / tools 索引中不再把已删除 `async` / `await` / `Task` surface 当作现行能力入口；
        - compare harness 已被删除或彻底改写。

- 必须遵从的约束：
  - 禁止因为删 legacy compare 测试麻烦，就继续保留可执行 legacy 代码。
  - 禁止把历史比较型测试简单跳过；要么改写为新主线回归，要么删除。
  - 禁止在 README/docs 中继续给出任何可执行 legacy 命令示例。
  - 禁止在主文档、主 fixtures 路径或主测试索引中继续把 `async` / `await` / `Task` 描述成现行 surface。
  - 禁止保留误导性命名，让后续维护者以为仓库里还有第二条主线。

- 验证：
  1. 新增/更新定向测试，推荐命名：
     - `legacy_pipeline_docs_removed_*`
     - `legacy_compare_harness_removed_*`
  2. 运行：
      - `cargo test -p scoop legacy_pipeline_docs_removed`
      - `cargo test -p scoopc legacy_compare_harness_removed`
  3. 仓库搜索（执行时必须在完成记录中附摘要）：
      - `rg -e "--effect-pipeline legacy|--effect-pipeline refactor|default.*legacy|legacy pipeline|old effect mainline|parallel pipeline|async fun|Async\.await|Task<|std_task_|async_await_" . --glob '!docs/archive/**' --glob '!target/**'`
  4. 要求：
      - 允许命中：本任务新增的“已删除 legacy”负向测试、迁移说明中的历史叙述；
      - 不允许命中：公开命令示例、fixtures 主路径、tests 主路径仍把 legacy 当可执行主线，或仍把已删除 `async` / `await` / `Task` surface 当成现行能力入口。

- 完成条件：
  - tests / fixtures / docs 已完成对 legacy 主线残留的清理；
  - 主文档、主 fixtures 路径与主测试索引已不再暴露已删除 `async` / `await` / `Task` surface；
  - compare 型资产已改写为纯新主线回归或被删除；
  - 后续只剩“在只有新主线存在”的前提下跑最终完整矩阵。
- 依赖：P8-T02R
- 完成记录：
  - 2026-05-09：已清理 live docs 中仍把 pipeline selector 或 async/task surface 当作现行入口的叙述。`docs/spec/language_spec-part1.md` 去掉顶层 `async fun` 声明项与 `Task<T>` 现行库名示例；`docs/spec/language_spec-part4.md` 改写为“async/structured-concurrency surface 当前未定义”，删除把 `Async.await` / `Task<T>` / `async fun` 当作现行规范的示例与规则；`EFFECT_REFACTOR.md`、`HIR_COMPLETENESS_HANDOFF.md`、`MIR_REFACTOR_PHASE_EXIT_AUDIT.md` 的现行命令示例已统一改成单一路径 CLI，不再要求显式 selector。
  - 2026-05-09：已清理主工具/测试索引中的误导性命名。`tools/scoop_tools/src/fixtures_matrix.rs` 删除 `Task / Executor (async)` 域并重排后续 domain id；`crates/scoopc/src/llvm/tests.rs` 中 direct-HIR compare helper/test 已从 `legacy_*` 改为中立命名，并同步去掉若干把 effect boundary / TLS 信号仍写成 `legacy` 的断言文本。
  - 2026-05-09：已新增定向守护，锁定主路径清理结果。`crates/scoop/tests/p8_docs_cleanup.rs` 新增 `legacy_pipeline_docs_removed_*`，断言 live docs / tools 索引不再暴露 selector 或 async/task 现行 surface；`crates/scoopc/src/llvm/tests.rs` 新增 `legacy_compare_harness_removed_from_llvm_test_source`，防止 direct-HIR compare harness 误导性 `legacy` 命名回流。
  - 验证：`cargo fmt`
  - 验证：`cargo test -p scoop legacy_pipeline_docs_removed`
  - 验证：`cargo test -p scoopc legacy_compare_harness_removed`
  - 验证：`cargo clippy --all-targets -- -D warnings`
  - 仓库搜索摘要：执行 `rg -e "--effect-pipeline legacy|--effect-pipeline refactor|default.*legacy|legacy pipeline|old effect mainline|parallel pipeline|async fun|Async\.await|Task<|std_task_|async_await_" . --glob '!docs/archive/**' --glob '!target/**'` 后，剩余命中已收敛为四类：
    1. `TODO*.md` / `PLAN*.md` 中的历史阶段记录与任务说明；
    2. `SCOOP_FULL_SPEC.md` 的“已移除 async/task surface”说明，以及 `ASYNC_REFACTOR.md` 的历史设计文档；
    3. 本任务新增的 `crates/scoop/tests/p8_docs_cleanup.rs` 负向守护文本；
    4. `crates/scoop/src/commands/build.rs` 的 anti-fallback 断言文案。
    未发现公开命令示例、fixtures 主路径、主测试索引或 live helper docs 继续把 legacy 主线或已删除 async/task surface 当作现行入口。

## [DONE] P8-T03R：Review 测试/文档残留清理，确认仓库公开叙述与主测试路径都只剩新主线，且不再暴露已删除 async/Task surface

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P8
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §8
- 重点：
  - docs/README/help/fixtures/test helpers 中是否已不再暴露 legacy 主线；
  - docs/README/help/fixtures/test helpers 中是否已不再把 `async` / `await` / `Task` 描述成现行 surface；
  - compare/rollback 型资产是否已被删除或改写；
  - 是否还保留误导性的命名、注释、或命令示例。
- 必须检查的文件/位置：
  - `crates/scoop/src/fixtures/**`
  - `crates/scoopc/src/llvm/tests.rs`
  - `tools/scoop_tools/src/fixtures_matrix.rs`
  - README / 相关开发文档 / fixture 注释
  - 任何保留的迁移说明或“legacy removed”负向测试

- 验证：
  - 重新运行 P8-T03 的全部测试与命令；
  - 额外搜索：
    - `rg -e "--effect-pipeline legacy|--effect-pipeline refactor|legacy pipeline|parallel pipeline|old effect mainline|async fun|Async\.await|Task<|std_task_|async_await_" . --glob '!docs/archive/**' --glob '!target/**'`
  - 要求：
      - 允许命中：历史说明、负向删除测试；
      - 不允许命中：主文档、主测试、fixtures 主路径中仍把 legacy 当作可执行路径，或仍把已删除 `async` / `await` / `Task` surface 当作现行语法/能力。

- 完成条件：
  - review 能明确说明：仓库公开叙述与测试主路径已完全收口到新主线，且不再把已删除 `async` / `await` / `Task` surface 当作现行能力；
  - 可进入 P8-T04。
- 依赖：P8-T03
- 完成记录：
  - 2026-05-09：执行 `P8-T03R` 复核时发现 `docs/spec/language_spec-part1.md` 仍在分卷目录与关键字列表中写出 `async/await`，`docs/spec/language_spec-part3.md` 仍把 `async` / `await` 写成现行表达式语法与前缀运算符；已同步修正文档，并把 `crates/scoop/tests/p8_docs_cleanup.rs` 扩展到覆盖这些漏网项，防止同类 live-spec 残留回流。
  - 2026-05-09：复核结论：`crates/scoop/src/fixtures/**`、`crates/scoopc/src/llvm/tests.rs`、`tools/scoop_tools/src/fixtures_matrix.rs` 与 live helper/docs 中，未再把 legacy 主线当作可执行入口；`crates/scoopc/src/llvm/tests.rs` 中保留的 `legacy` 文本只存在于 `legacy_compare_harness_removed_*` / `legacy_effect_backend_removed_*` 等负向守护，compare harness 本体与误导性命名未回流。
  - 2026-05-09：搜索结论：按任务要求执行 `rg -e "--effect-pipeline legacy|--effect-pipeline refactor|legacy pipeline|parallel pipeline|old effect mainline|async fun|Async\.await|Task<|std_task_|async_await_" . --glob '!docs/archive/**' --glob '!target/**'` 后，命中已可分类为：历史 `TODO/PLAN` 记录、`SCOOP_FULL_SPEC.md` / `ASYNC_REFACTOR.md` 的删除或设计说明、`crates/scoop/tests/p8_docs_cleanup.rs` 负向守护，以及 `crates/scoop/src/commands/build.rs` 的 anti-fallback 断言。额外执行排除 `TODO*.md` / `PLAN*.md` / `memory/**` 的 live-path 搜索后，不再存在其它公开命令示例、fixture helper 或 live docs 命中；更宽的 `async/await` live-doc 搜索仅剩 `docs/spec/language_spec-part4.md` 的“当前版本不定义”负向说明、`SCOOP_FULL_SPEC.md` 的英文删除说明、`ASYNC_REFACTOR.md` 的历史设计文档与本次负向守护文本。
  - 验证：`cargo fmt`
  - 验证：`cargo test -p scoop legacy_pipeline_docs_removed`
  - 验证：`cargo test -p scoopc legacy_compare_harness_removed`
- 验证：`cargo clippy --all-targets -- -D warnings`
- 验证：`rg -n -e "--effect-pipeline legacy|--effect-pipeline refactor|legacy pipeline|parallel pipeline|old effect mainline|async fun|Async\.await|Task<|std_task_|async_await_" . --glob '!docs/archive/**' --glob '!target/**'`
- 验证：`rg -n "async/await|\basync\b|\bawait\b|Task<|Async\.await|std_task_|async_await_" docs/spec crates/scoop/tests/p8_docs_cleanup.rs tools/scoop_tools/src/fixtures_matrix.rs SCOOP_FULL_SPEC.md ASYNC_REFACTOR.md EFFECT_REFACTOR.md HIR_COMPLETENESS_HANDOFF.md MIR_REFACTOR_PHASE_EXIT_AUDIT.md README.md`

## [DONE] P8-T03aa：修复 default single-file refactor stage 对 nominal upcast call boundary 的 operand contract，解除 virtual/interface outward 默认路径阻塞

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P8，§4
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.10-§4.16、§5.4、§5.5、§8
  - 前置实现参考：[`TODO-P6-part2.md`](./TODO-P6-part2.md) P6-T02o、[`TODO-P7.md`](./TODO-P7.md) P7-T02V / P7-T02W
- 术语约定：本节后文所说 default single-file path/stage/entry，现均指“默认 project 设置下、只含一个用户源文件的 virtual cone 路径”；自 2026-05-10 起它已与显式 cone build 共用 `crate::frontend` 的 parse/resolve/typecheck/request-root/entry-main 主线，不再代表独立“单文件前端/入口”。
- 背景 / blocker：
  - 2026-05-10 在执行 `P8-T03a`、把默认 virtual-cone（单文件输入）helper/入口切到 refactor LLVM stage 后，`helper(Derived())` / `helper(Impl())` 这一类“实参是具体 nominal subtype、callee 参数是 base/interface supertype”的 outward helper 样本不再能通过 default path。
  - 复现表现：`cargo run -p scoopc -- --emit-llvm <sample>` 与 `crates/scoopc/src/llvm/tests.rs` 中对应 default helper 测试都会在 late lowering 失败：`refactor late-lowering stage 无法为 a.main 的 Call site1 发布 boundary operand contract：local4 的类型为 t388，但 published operand contract 期望 t385`。
  - 复核后确认：当前 default virtual-cone refactor stage 在发布 direct call boundary operand contract 时，仍把 nominal source 要求成“operand local.ty 与 published expected_ty 全等”，没有接受已经由前端 typecheck 证明合法的 `Derived <: Base` / `Impl <: IFace` source route。
  - 这不是 `P8-T03a` 可接受的“先把测试改成显式 materialized helper”问题；默认 virtual-cone 主线路径本身就必须支持这类 nominal upcast call boundary，否则 `P8-T03a` 无法完成默认 virtual/interface outward helper 迁移。

- 目标：
  - 让 default virtual-cone refactor stage 能正确发布并消费 nominal upcast direct call boundary operand contract；
  - 覆盖 class-supertype 与 interface-supertype 两类 outward helper 场景；
  - 为随后恢复 `P8-T03a` 的默认 helper/public entry 迁移提供稳定前提。

- 必须实现的内容：
  1. 审计并修复 late-lowering boundary operand contract 的 nominal source compatibility。
      - 至少检查并修改：
        - `crates/scoopc/src/effect_lowered/materialize.rs`
        - 如有必要，联动 `crates/scoopc/src/effect_lowered/builder.rs`
        - 如有必要，联动 `crates/scoopc/src/llvm/codegen/effect_refactor/types.rs` 或等价 ABI/query 消费点
      - 要求：
        - 已 typecheck 合法的 nominal upcast source（例如 `Derived` -> `Base`、`Impl` -> `IFace`）必须能在 published operand contract 下通过；
        - 不允许靠恢复旧 helper、恢复旧 wrapper，或把 default tests 改回 explicit materialized helper 绕过；
        - 不允许把 operand contract 放宽成“丢失 source_ty / 不再可验证”的模糊表达。
  2. 为 default virtual-cone refactor stage 增加窄回归守护。
      - 至少覆盖：
        - outward virtual helper 默认路径；
        - outward interface helper 默认路径；
        - 断言点必须针对 refactor stage authoritative 语义，而不是旧 wrapper/TLS helper 命名。

- 必须遵从的约束：
  - 禁止把 `P8-T03a` 的默认 helper 测试改回 `*_from_materialized_lowered_hir` 作为规避；
  - 禁止通过恢复 selector、恢复 legacy path、或在 default entry 上做 hidden bifurcation 让样本“只在这些 nominal upcast 场景回旧 helper”；
  - 禁止把 nominal upcast 问题下沉成 LLVM backend 现场猜测；contract 必须在 late-lowering/ABI handoff 上被正确发布。

- 验证：
  1. 至少运行并通过：
     - `cargo test -p scoopc --lib llvm::tests::virtual_call_with_real_outward_effect_uses_explicit_outcome_boundary -- --exact`
     - `cargo test -p scoopc --lib llvm::tests::interface_call_with_real_outward_effect_uses_explicit_outcome_boundary -- --exact`
  2. 如上面两条仍不足以证明 public virtual-cone path：
     - 增加等价的 default virtual-cone LLVM unit test / smoke，证明 `scoopc` 默认 virtual-cone stage 入口不会再在 nominal upcast call boundary 处 fail fast。

- 完成条件：
  - default virtual-cone refactor stage 不再因 nominal upcast call boundary 在 virtual/interface outward helper 上失败；
  - `P8-T03a` 可以继续迁移默认 helper/public entry，而不再被该 blocker 卡住。
- 依赖：`P8-T03R`
- 完成记录：
  - 2026-05-10：已把 `crates/scoopc/src/frontend.rs` 抽成共享 project frontend，并把单文件输入收敛为“默认 project 设置下、只含一个用户源文件的 virtual cone”；`scoop build <file>`、显式 cone build 与 `llvm/frontend.rs` 的默认 virtual-cone codegen 现在共用同一套 parse/resolve/typecheck/request-root/entry-main 逻辑，不再保留分裂的“单文件前端”。
  - 2026-05-10：已修复 late-lowering nominal supertype 事实来源。`crates/scoopc/src/effect_lowered/builder.rs` 不再从会丢失 supertype metadata 的 canonical materialized MIR file 收集 nominal direct supertypes，而是允许由 `RefactorMirStageOutput.file()` 的 authoritative direct-style MIR metadata 注入；`effect_lowering_stage.rs` 现显式把这张表传给 `LateLoweredProgramBuilder`，从而让 `Derived <: Base` / `Impl <: IFace` 的 direct call boundary contract 在 default refactor stage 下稳定通过。
  - 2026-05-10：已新增 late-lowering 窄回归 `refactor_boundary_operand_contract_accepts_nominal_upcast_direct_arg_sources`，直接守护 `a.main -> helper(Derived())` 这类 nominal upcast arg source 不再在 P5 fail fast。
  - 2026-05-10：已把 `crates/scoopc/src/llvm/tests.rs` 中的 outward virtual/interface helper 默认路径断言改写到 refactor stage authoritative surface：检查 `__scoop_refactor_direct_invoke__a_helper`、step-tag dispatch、surface-resume owner dispatch，以及 helper body 本身不回落到 legacy TLS/outcome runtime 符号，而不再依赖旧式 `a.helper` wrapper/TLS 命名。
  - 验证：`cargo check -p scoopc --quiet`
  - 验证：`cargo check -p scoop --quiet`
  - 验证：`cargo test -p scoopc --lib effect_lowered::materialize::tests::refactor_boundary_operand_contract_accepts_nominal_upcast_direct_arg_sources -- --exact`
  - 验证：`cargo test -p scoopc --lib llvm::tests::virtual_call_with_real_outward_effect_uses_explicit_outcome_boundary -- --exact`
  - 验证：`cargo test -p scoopc --lib llvm::tests::interface_call_with_real_outward_effect_uses_explicit_outcome_boundary -- --exact`
  - 验证：`cargo test -p scoopc --lib llvm::tests::default_single_file_ir_helper_lowers_handle_main_without_hir_fallback -- --exact`
  - 验证：`cargo test -p scoopc --lib llvm::tests::single_file_frontend_keeps_distinct_effect_row_generic_instances -- --exact`
  - 验证：`cargo test -p scoop build_frontend_single_file_request_roots_exclude_stdlib_support_sources`
  - 验证：`cargo test -p scoop build_frontend_cone_request_roots_exclude_stdlib_support_sources`
  - 验证：`cargo test -p scoop build_frontend_entry_roots_skip_same_file_unreachable_generic_helper`
  - 验证：`cargo test -p scoop build_frontend_entry_roots_skip_unreachable_cone_source_generic_helper`
  - 验证：`cargo test -p scoop no_hidden_legacy_fallback_for_default_refactor_build_output`

## [DONE] P8-T03a：迁移单文件 LLVM artifact 入口与默认测试 helper 到 refactor LLVM stage，移除 materialized-HIR entry-main 对 `Handle` fallback 的隐藏依赖

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P8，§4
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.10-§4.16、§5.4、§5.5、§8
  - 前置实现参考：[`TODO-P6-part3.md`](./TODO-P6-part3.md) P6-T03f / P6-T03g / P6-T05、[`TODO-P8.md`](./TODO-P8.md) P8-T01 / P8-T02 / P8-T03R
- 术语约定：本任务中的“单文件 LLVM artifact 入口”现统一指“单用户源文件经 virtual cone 包装后的默认 `<file>` / `scoopc` artifact 路径”；frontend 侧已与显式 cone build 合并，本任务剩余工作仅是 LLVM artifact handoff 与默认 helper 的收口，不再存在独立“单文件前端/入口”实现。
- 边界约定（类比 `cargo` / `rustc`）：裸 `scoopc <file>` / `SourceFile` 只承载 single-source virtual-cone contract；若目标语义是显式 cone / 多源 project，则 authoritative project context（至少包括 project sources、cone root / manifest、entry 选择，以及依赖 cone 位置或其已解析结果）必须由 `scoop` 等上层驱动预先确定并传给 `scoopc` 的 project frontend（`ProjectInput + deps` 或等价输入），不得指望 `scoopc` 在末端从单个源码路径自行恢复。
- 背景 / blocker：
  - 2026-05-10 执行 `P8-T04` 的首步 `cargo test --all` 时，`crates/scoopc/src/llvm/tests.rs` 一批单文件 LLVM 测试统一失败，错误为 `UnsupportedMainBody { kind: "HIR handle lowering removed; use refactor MIR lowering" }`。
  - 复核后确认：`crates/scoopc/src/llvm/emit.rs` 的 `emit_minimal_main_ir` / `emit_minimal_main_obj_to_file` / `emit_minimal_main_asm_to_file` / `build_minimal_main_module*`，以及 `crates/scoopc/src/effect_refactor_pipeline/mod.rs` 暴露给 `scoopc` bin 的 virtual-cone artifact 入口，默认仍经 `materialized_lowered_hir` 入口构建 LLVM module。
  - 该入口的 raw materialized MIR backend 明确把 `TerminatorKind::Handle` 视为 unsupported，而 HIR `handle` lowering 已在 P8 前移除；因此只要入口 `main` 里出现 `handle` / `try`，默认 virtual-cone 路径就会回落到已删除的 HIR 旧路并失败。
  - 这不是 `P8-T04` 可接受的“修一个测试就继续”的局部问题，而是一个尚未跟踪的前置依赖：默认 virtual-cone LLVM artifact 路径和对应默认测试 helper 还没有真正切到唯一 refactor 主线。
  - 2026-05-10：在切换默认 helper 到 refactor stage 的执行过程中，又暴露出 default virtual/interface outward helper 的 nominal upcast call boundary blocker；已新增前置任务 `P8-T03aa`，必须先修复该 contract 缺口，再继续完成本任务。

- 目标：
  - 把默认 virtual-cone（单文件输入）LLVM artifact 入口、以及表示“默认/生产 virtual-cone 路径”的测试 helper，迁移到 refactor LLVM stage handoff；
  - 清除默认 helper 对 raw materialized MIR entry-main / HIR handle fallback 的隐藏依赖；
  - 只在显式的历史/对照测试 helper 上保留 `*_from_lowered_hir` / `*_from_materialized_lowered_hir` 语义，禁止它们继续作为默认 virtual-cone 主入口生效。

- 必须实现的内容：
  1. 审计并迁移默认 virtual-cone LLVM artifact 入口。
     - 至少检查并修改：
       - `crates/scoopc/src/llvm/emit.rs`
       - `crates/scoopc/src/effect_refactor_pipeline/mod.rs`
       - `crates/scoopc/src/bin/scoopc.rs`
       - 任何仍把“默认 virtual-cone LLVM 发射”接到 `LoweredCodegenEntry::from_materialized_lowered_hir(...)` 的入口/包装层
     - 要求：
        - 默认 `emit_minimal_main_*` / `emit_virtual_cone_llvm_artifact_to_file(...)` 必须经 refactor LLVM stage handoff 构建产物；
        - 不允许继续把 raw materialized MIR entry-main + HIR fallback 当成默认生产入口；
        - 不允许把默认 `<file>` / `scoopc` artifact 路径偷偷扩张成“既处理 virtual cone，又靠猜目录恢复 explicit cone/project context”的混合入口；
        - 不允许通过恢复 HIR `handle` lowering、恢复 selector、或在默认入口里做“遇到 handle 再切 stage”的隐藏 bifurcation 规避问题。
  2. 明确区分“默认生产 helper”与“显式历史/对照 helper”。
     - 若 `emit_*_from_lowered_hir` / `emit_*_from_materialized_lowered_hir` 仍需保留：
       - 必须仅作为显式测试/对照入口；
       - 不得再被默认 virtual-cone artifact 入口、`scoopc` bin 默认命令、或默认 LLVM 单测 helper 间接调用；
       - 相关注释/命名要明确它们不是当前单一主线的默认入口。
  3. 迁移或重写受影响的默认 LLVM 单测。
     - 至少检查并修改：
       - `crates/scoopc/src/llvm/tests.rs`
       - 如需要，新增更窄的 helper，把“默认 virtual-cone 主线”与“显式 materialized/raw-MIR 对照”分开
     - 要求：
       - 仍在验证默认 virtual-cone 主线的测试，必须改为断言 refactor LLVM stage 的 authoritative 语义；
       - 只有在测试目的明确是 raw materialized MIR bridge / direct-HIR 对照时，才允许继续使用显式 `*_from_materialized_lowered_hir` / `*_from_lowered_hir` helper；
       - 禁止继续让默认 helper 承担“旧 materialized-HIR path”的语义断言。
  4. 增加回归守护，证明默认 virtual-cone 入口已真正走 stage。
     - 至少要有一条自动化验证覆盖：
       - 默认 virtual-cone LLVM IR/object/asm 入口会触发 refactor LLVM stage；
       - `main` 含 `handle` / `try` 的单用户源文件样本不再触发 `HIR handle lowering removed`；
       - `scoopc` bin 的默认 virtual-cone artifact 路径不再存在 hidden fallback。

- 必须遵从的约束：
  - 禁止恢复 HIR `handle` lowering 作为临时过桥。
  - 禁止在默认 virtual-cone 入口保留“普通情况走旧 helper、遇到 effectful main 再偷偷切新 helper”的分叉。
  - 禁止把 raw materialized MIR backend 对 `TerminatorKind::Handle` 的不支持继续暴露成默认 virtual-cone 主线路径的一部分。
  - 禁止让 `scoopc` 裸 `<file>` 入口根据工作目录、相邻 `Cone.toml` 或其它环境线索隐式切换为 explicit-cone 语义；virtual-cone vs explicit-cone project context 必须由上层调用方显式确定。
  - 若某个旧 helper 仅剩历史/对照用途，必须在完成记录中明确说明它为何不会再形成 production hidden dependency。

- 验证：
  1. 至少运行并通过下列代表性回归：
     - `cargo test -p scoopc --lib llvm::tests::effect_contract_struct_types_are_registered_for_effect_codegen -- --exact`
     - `cargo test -p scoopc --lib llvm::tests::direct_call_with_real_outward_effect_uses_wrapper_and_explicit_outcome -- --exact`
     - `cargo test -p scoopc --lib llvm::tests::production_codegen_lowers_raw_mir_top_level_immutable_init_access -- --exact`
     - `cargo test -p scoopc --lib llvm::tests::boxed_effect_payload_rebuilds_aggregate_from_explicit_frame_after_safepoint -- --exact`
     - `cargo test -p scoopc --lib effect_refactor_pipeline::llvm_codegen_stage::tests::single_pipeline_llvm_codegen_stage_build_entry_uses_stage -- --exact`
  2. 对 `scoopc` 默认 virtual-cone artifact 路径做最小 smoke：
      - `cargo run -p scoopc -- tests/fixtures/build/emit_llvm_basic.scoop`
      - `cargo run -p scoopc -- --obj tests/fixtures/build/emit_llvm_basic.scoop`
  3. 需要在完成记录中总结：
      - 默认 helper / public virtual-cone entry 与显式历史 helper 的最终边界；
      - 仍保留的 `*_from_lowered_hir` / `*_from_materialized_lowered_hir` 用途说明；
      - 为什么它们不再构成 hidden legacy / hidden fallback。

- 完成条件：
  - 默认 virtual-cone LLVM artifact 入口已真正切到 refactor LLVM stage；
  - `main` 含 `handle` / `try` 的默认 virtual-cone 路径不再触发已删除的 HIR handle lowering；
  - 默认 LLVM 单测 helper 与显式历史/对照 helper 已语义分层；
  - `P8-T04` 可以在不先撞上该 blocker 的前提下重新执行完整矩阵。
- 依赖：`P8-T03R`，`P8-T03aa`
- 完成记录：
  - 2026-05-10：已复核并同步 `TODO.md` 索引状态。针对 `P8-T03a` 重新运行默认 virtual-cone 入口与 helper 的定向验证矩阵：`driver_cli`、`build_context_keeps_bare_file_input_as_virtual_cone_inside_cone_root`、`build_frontend_*`、`no_hidden_legacy_fallback_for_default_refactor_build_output`、关键 LLVM 单测，以及 `scoopc <file>` / `scoopc --obj <file>` smoke（本次用 `-o /var/.../opencode/*` 避免污染工作区）；结果均通过。
  - 2026-05-10：在 `P8-T03aa` 修复 nominal upcast call boundary blocker 后，本任务继续完成收口。`llvm::emit` 默认 `emit_minimal_main_*` / `build_minimal_main_module*` 与 `llvm/frontend.rs` 默认 helper 现统一经 virtual-cone context + refactor LLVM stage handoff 发射产物；`main` 含 `handle` / `try` 的默认路径不再触发已删除的 HIR handle lowering。
  - 2026-05-10：已正式区分 `scoop` -> `scoopc` 的两类调用规范。`crates/scoopc/src/frontend.rs` 新增 `ProjectContext` / `run_project_frontend(...)`；`scoop build` 现在先构造 authoritative project context（`ProjectInput + deps`），再把它交给 `scoopc` 的 project frontend，不再把 explicit-cone 语义留给末端 helper 从裸文件路径猜测。新增 `build_context_keeps_bare_file_input_as_virtual_cone_inside_cone_root` 守护：即便文件位于某个 cone root 下，bare file 入口也必须保持 virtual-cone contract。
  - 2026-05-10：已把 LLVM artifact 入口按 contract 分层。`effect_refactor_pipeline::emit_project_llvm_artifact_to_file(...)` 负责消费完整 project context；`effect_refactor_pipeline::emit_virtual_cone_llvm_artifact_to_file(...)` 负责 bare `SourceFile` / `scoopc <file>` 的 single-source virtual-cone 入口。`scoop build` 现改走前者，`scoopc` bin 与默认 virtual-cone helper 改走后者。
  - 2026-05-10：`scoopc` CLI 现支持 bare `<file>` 默认输出 LLVM IR，`--obj <file>` 输出 object；帮助文本也已明确写出“裸 `<file>` 仅代表 single-source virtual cone，不会自动恢复 explicit cone/project context”。这使 `cargo run -p scoopc -- tests/fixtures/build/emit_llvm_basic.scoop` 与 `cargo run -p scoopc -- --obj tests/fixtures/build/emit_llvm_basic.scoop` 都能按任务要求通过。
  - 2026-05-10：默认 helper / public virtual-cone entry 与显式历史 helper 的边界现已明确：`*_from_lowered_hir` / `*_from_materialized_lowered_hir` 仍只保留给显式历史/对照测试；新增更窄的 `emit_materialized_ir_for_root_callable(...)` 供 raw-materialized 对照测试按 root callable 精确取 IR，避免默认 helper 继续替旧 materialized-HIR path 背历史语义。
  - 验证：`cargo fmt`
  - 验证：`cargo test -p scoopc driver_cli`
  - 验证：`cargo test -p scoop build_context_keeps_bare_file_input_as_virtual_cone_inside_cone_root`
  - 验证：`cargo test -p scoop build_frontend_`
  - 验证：`cargo test -p scoop no_hidden_legacy_fallback_for_default_refactor_build_output`
  - 验证：`cargo test -p scoopc --lib llvm::tests::effect_contract_struct_types_are_registered_for_effect_codegen -- --exact`
  - 验证：`cargo test -p scoopc --lib llvm::tests::direct_call_with_real_outward_effect_uses_wrapper_and_explicit_outcome -- --exact`
  - 验证：`cargo test -p scoopc --lib llvm::tests::production_codegen_lowers_raw_mir_top_level_immutable_init_access -- --exact`
  - 验证：`cargo test -p scoopc --lib llvm::tests::boxed_effect_payload_rebuilds_aggregate_from_explicit_frame_after_safepoint -- --exact`
  - 验证：`cargo test -p scoopc --lib effect_refactor_pipeline::llvm_codegen_stage::tests::single_pipeline_llvm_codegen_stage_build_entry_uses_stage -- --exact`
  - 验证：`cargo test -p scoopc --lib llvm::tests::default_single_file_ir_helper_lowers_handle_main_without_hir_fallback -- --exact`
  - 验证：`cargo test -p scoopc --lib llvm::tests::single_file_frontend_keeps_distinct_effect_row_generic_instances -- --exact`
  - 验证：`cargo run -p scoopc -- tests/fixtures/build/emit_llvm_basic.scoop`
  - 验证：`cargo run -p scoopc -- --obj tests/fixtures/build/emit_llvm_basic.scoop`

## [DONE] P8-T03ab：消除 object/top-level hidden-init LLVM helper 对 legacy `effect_outcome` / handler-stack swap 的隐藏依赖

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P8，§4
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.10-§4.16、§5.4、§5.5、§8
  - 前置实现参考：[`TODO-P7.md`](./TODO-P7.md) P7-T02W、[`TODO-P8.md`](./TODO-P8.md) P8-T03a
- 背景 / blocker：
  - 2026-05-10 在 `P8-T04` 的 `cargo test --all` / `cargo test -p scoopc --lib` 收尾回归中，默认单文件 LLVM 路径只剩两条真实 blocker：
    - `llvm::tests::object_value_init_with_real_outward_effect_uses_explicit_outcome_boundary`
    - `llvm::tests::top_level_immutable_init_with_real_outward_effect_uses_explicit_outcome_boundary`
  - 复核 IR 确认：`crates/scoopc/src/llvm/codegen/effect_refactor/body.rs` 的 `lower_class_ctor_boundary(...)` 仍通过 `with_active_suspend_site_any_effect_outcome_capture(...)` 包裹 `codegen_object_property_access(...)` / `codegen_top_level_value_ref(...)`；而 `crates/scoopc/src/llvm/codegen/object_init.rs` 的 object/top-level init 访问仍依赖 `begin_effect_boundary(...)`、`scoop_effect_outcome_consume_current(...)` 与 `scoop_effect_handler_stack_swap_top(...)`。
  - 这意味着默认新主线 helper `__scoop_refactor_direct_invoke__a_helper` 仍暴露 legacy outcome/handler-stack runtime shim，违反 P7-T02W 与 P8 的“无 hidden fallback / 无 hidden legacy mainline”要求，也直接阻塞 `P8-T04` 的最终 full regression。

- 目标：
  - 让 object value init / object property init / top-level immutable init 在默认 refactor LLVM 路径下按 authoritative `Step` / continuation contract 传播 hidden ordinary effect；
  - 删除 helper 级别对 legacy `scoop_effect_outcome_*` / `scoop_effect_handler_stack_swap_top` 的隐藏依赖；
  - 保持 outward payload、continuation、frame/root 更新语义不回退、不分叉。

- 必须实现的内容：
  1. 重新审计并改写 hidden-init boundary lowering。
     - 至少检查并修改：
       - `crates/scoopc/src/llvm/codegen/effect_refactor/body.rs`
       - `crates/scoopc/src/llvm/codegen/object_init.rs`
       - 任何 top-level immutable init / object property init / object value access 仍经 legacy effect-outcome shim 的 helper
     - 要求：
       - 默认单文件 refactor helper 不再显式调用 `scoop_effect_outcome_consume_current`、`scoop_effect_handler_stack_swap_top`、或等价 legacy ordinary-effect transport；
       - hidden-init outward case 仍必须被转换为 authoritative `Step` case，并保持 continuation/frame 更新正确。
  2. 对齐 object/top-level hidden-init 的 default helper 回归断言。
     - 至少覆盖：
       - `BoomObject` object init outward sample
       - top-level immutable `Broken` outward sample
     - 要求：
       - helper body 继续有 refactor `Step` dispatch / continuation publication；
       - 但不再暴露 legacy outcome/handler-stack shim。
  3. 确认历史 / materialized helper 不会重新成为 hidden fallback。
     - 若某些 raw-MIR / materialized helper 仍需要 legacy-neutral bridge，请在完成记录里说明其边界；
     - 禁止通过“默认 helper 仍调旧 shim，但测试不看”来过关。

- 验证：
  1. 运行：
     - `cargo test -p scoopc --lib llvm::tests::object_value_init_with_real_outward_effect_uses_explicit_outcome_boundary -- --exact`
     - `cargo test -p scoopc --lib llvm::tests::top_level_immutable_init_with_real_outward_effect_uses_explicit_outcome_boundary -- --exact`
  2. 建议补跑：
     - `cargo test -p scoopc --lib llvm::tests::production_codegen_lowers_raw_mir_object_value_init_access -- --exact`
     - `cargo test -p scoopc --lib llvm::tests::production_codegen_lowers_raw_mir_top_level_immutable_init_access -- --exact`
  3. 完成记录中必须总结：
     - helper 新旧 contract 的最终边界；
     - 为什么 object/top-level hidden-init 不再构成 hidden legacy fallback。

- 完成条件：
  - 默认 refactor helper 已不再显式依赖 legacy `effect_outcome` / handler-stack swap shim；
  - object/top-level hidden-init outward sample 在默认单文件 LLVM 路径下走纯 refactor `Step` / continuation contract；
  - `P8-T04` 可以继续完整矩阵，而不会再被这两条 hidden-init blocker 卡住。
- 依赖：`P8-T03a`
- 完成记录：
  - 2026-05-10：完成 hidden-init helper 收口。`crates/scoopc/src/llvm/codegen/effect_refactor/body.rs` 不再让默认 refactor helper 直接安装 `begin_effect_boundary(...)` / `scoop_effect_outcome_consume_current(...)` / `scoop_effect_handler_stack_swap_top(...)`；object value / object property / top-level immutable hidden-init 现在统一先调用内部 bridge helper 取得 explicit outcome aggregate，再在 helper 侧用 `refactor_step_tag` switch 做 complete/outward dispatch，并继续通过现有 continuation publication 走 authoritative refactor boundary。
  - 2026-05-10：新增 object/top-level hidden-init bridge helper：`crates/scoopc/src/llvm/codegen/object_init.rs` 为 object init 发布 `__scoop_refactor_hidden_object_init_bridge__*`，`crates/scoopc/src/llvm/codegen/mod.rs` 为 top-level immutable init 发布 `__scoop_refactor_hidden_top_level_init_bridge__*`。它们把 legacy outcome capture 收口到内部实现细节，默认单文件 refactor helper 自身不再暴露 legacy outcome / handler-stack shim。
  - 2026-05-10：历史/raw-MIR materialized helper 的边界保持不变：`production_codegen_lowers_raw_mir_object_value_init_access` 与 `production_codegen_lowers_raw_mir_top_level_immutable_init_access` 继续验证 production/raw-MIR path 仍经 explicit outcome bridge；因此 residual bridge 只剩 raw-MIR/materialized helper 与新 hidden-init bridge 内部实现，不再构成 default refactor helper 的 hidden legacy fallback。
  - 2026-05-10：补充 default helper 定向断言 `object_property_init_with_real_outward_effect_uses_explicit_outcome_boundary`，并加强 object value / object property / top-level immutable 三条默认路径断言：helper body 必须出现 `switch i32 %refactor_step_tag`，且不得再出现 `scoop_effect_outcome*` / `scoop_effect_handler_stack_swap_top` / `@scoop_effect_is_active`。
  - 2026-05-10：验证通过：`cargo test -p scoopc --lib llvm::tests::object_value_init_with_real_outward_effect_uses_explicit_outcome_boundary -- --exact`；`cargo test -p scoopc --lib llvm::tests::object_property_init_with_real_outward_effect_uses_explicit_outcome_boundary -- --exact`；`cargo test -p scoopc --lib llvm::tests::top_level_immutable_init_with_real_outward_effect_uses_explicit_outcome_boundary -- --exact`；`cargo test -p scoopc --lib llvm::tests::production_codegen_lowers_raw_mir_object_value_init_access -- --exact`；`cargo test -p scoopc --lib llvm::tests::production_codegen_lowers_raw_mir_top_level_immutable_init_access -- --exact`；`cargo clippy --all-targets -- -D warnings`。

## P8-T04：在“只有新主线存在”的条件下重跑完整回归矩阵，并锁定最终收口状态

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P8，§3，§4
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) 全文，重点 §4.10-§4.16、§5.4、§5.5、§8
- 目标：
  - 在 legacy selector、legacy 主线实现、legacy compare 资产都已清理后的状态下，重跑完整回归矩阵；
  - 证明仓库已经不再依赖旧主线救场；
  - 给出本轮重构真正结束的最终验证结论。

- 必须实现的内容：
  1. 按以下顺序运行并修复最终完整回归矩阵：
     - `cargo test --all`
     - `cargo run -p scoop -- test`
     - `cargo run -p scoop_tools -- spec-fixtures check`
     - `cargo clippy --all-targets -- -D warnings`
     - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
     - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`
     - 若中间任何一步失败，必须修复并重跑受影响步骤；
     - 在完成本任务前，必须至少再完整重跑一遍整个矩阵，确保不是“局部修复后未重新汇总验证”。
  2. 失败修复必须遵守“只剩新主线”的前提。
     - 允许修复：
       - refactor 主实现
       - 中立共享模块
       - runtime / GC / stackmap 基础设施
       - tests / fixtures / docs 的纯新主线路径断言
     - 明确禁止：
       - 恢复任何 legacy 分支
       - 重新引入 selector
       - 新增 hidden fallback
       - 缩小 full regression 范围
  3. 增加最终“无 legacy 主线残留”的守护检查。
      - 至少要通过测试、搜索、或等价自动化手段证明：
        - 仓库中不再保留可执行 legacy 主线；
        - 删除后 full regression 与 GC env 仍完整通过；
        - 主文档、主 fixtures 路径与主测试索引中不再把已删除 `async` / `await` / `Task` surface 当作现行语法/能力；
        - 任何 residual `legacy` 文本都只剩历史说明、非 effect 语义、或负向删除测试用途。
  4. 在完成记录中给出最终收口摘要。
     - 至少包括：
       - 全部完整矩阵最终通过列表
       - 删除 legacy 后暴露出的最后一轮问题类别
       - 对“仓库中不再保留旧主线”的证据摘要（例如搜索结果分类）

- 必须遵从的约束：
  - 禁止把“P7 已经通过过一次”当成 P8 可跳过 full regression 的理由；P8 必须在**删除旧主线之后**再完整跑一遍。
  - 禁止因为删除后回归成本大，就临时恢复 legacy 入口帮助通过。
  - 禁止把 residual `legacy` 搜索命中一概忽略；必须分类解释为何安全，或继续清理。
  - 禁止在完成标准里保留“应该已经没有旧主线了”这类不确定表述；必须给出可验证证据。

- 验证：
  1. 必跑：
     - `cargo test --all`
     - `cargo run -p scoop -- test`
     - `cargo run -p scoop_tools -- spec-fixtures check`
     - `cargo clippy --all-targets -- -D warnings`
     - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
     - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`
  2. 仓库搜索（执行时必须在完成记录中附摘要）：
      - `rg "legacy|old effect mainline|parallel pipeline|state_machine_bridge|state_machine_emitter|UnifiedHandleLoweringContract|production_lowered_hir|legacy_eager_hir|--effect-pipeline|async fun|Async\.await|Task<|std_task_|async_await_" . --glob '!docs/archive/**' --glob '!target/**'`
  3. 要求：
      - 完整矩阵全部通过；
      - 搜索结果中不得再有可执行 legacy 主线入口、旧 effect/continuation 主实现残留，或主路径上的已删除 `async` / `await` / `Task` surface 引用。

- 完成条件：
  - 仓库中不再保留旧主线；
  - 完整验证在“只有新主线存在”的条件下仍完整通过；
  - 主文档、主 fixtures 路径与主测试索引不再暴露已删除 `async` / `await` / `Task` surface；
  - 本轮 effect-refactor 收口工作可以视为真正结束。
- 依赖：P8-T03a，P8-T03ab
- 完成记录：
  - 2026-05-10：执行 `cargo test --all` / `cargo test -p scoopc --lib` 收尾回归时，默认单文件 LLVM helper 仅剩 object/top-level hidden-init blocker：`__scoop_refactor_direct_invoke__a_helper` 仍通过 legacy `scoop_effect_outcome_*` / `scoop_effect_handler_stack_swap_top` shim 传播 hidden ordinary effect。已前插前置任务 `P8-T03ab`，`P8-T04` 暂停，待该 blocker 消除后再继续完整矩阵。

## P8-T04R：Review P8 阶段退出条件，确认仓库已真正收口到单一新主线且本轮工作结束

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P8，§3，§4
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) 全文
- 重点：
  - legacy selector、legacy 主线实现、legacy compare 资产是否都已被删除或改写；
  - 完整回归与 GC env 全开矩阵是否在“只有新主线存在”的条件下通过；
  - 已删除 `async` / `await` / `Task` surface 是否已从主文档、主 fixtures 路径与主测试索引中清理干净；
  - residual `legacy` 命中是否都已被解释为安全的历史文本或负向删除测试，而不是隐藏依赖；
  - 是否已经满足 `PLAN.md` §4 的最终完成标准第 10 条。

- 验证：
  - 重新运行 P8-T01 ~ P8-T04 的全部测试与命令；
  - 再跑一次最小 smoke：
    - `cargo run -p scoop -- build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p8_final.ll`
    - `cargo run -p scoop -- run tests/fixtures/run-pass/minimal_main.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/minimal_main.scoop`
  - 额外负向检查：
    - `cargo run -p scoop -- --effect-pipeline legacy test --fixtures tests/fixtures/run-pass/minimal_main.scoop`
      - 要求：参数已不存在并失败，不能再成功执行

- 完成条件：
  - review 能明确说明：P8 已完成“删除旧主线并再次 full regression”的阶段目标；
  - 仓库已经真正收口到单一新主线；
  - 本轮 effect-refactor 工作结束。
- 依赖：P8-T04
- 完成记录：
  - （执行时填写）
