# TODO（P8：删除旧主线并再次 full regression）

> 生成时间：2026-05-02  
> 设计基线：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md)  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 前置条件：`TODO-P7.md` 已完整完成；refactor 路径已经是默认主线；标准 full regression 与 GC env 全开验证已在默认主线下通过；legacy 路径只剩显式 compare/rollback 入口。  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 本阶段目标：删掉旧的 legacy effect/continuation 主线，实现真正收口；保证仓库中不再存在“默认靠新主线，但旧主线还悄悄救场”的隐藏依赖；在“只剩新主线”的前提下再次跑完整回归与 GC env 验证，证明新路径单独存在时仍完整通过。

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
    - tests/fixtures/docs 中把 legacy 当作可执行主线的入口
  - 明确禁止：
    - 只删 CLI 参数，但内部 selector/legacy branch 还在
    - 只删一部分 emitter/helper，但旧 dispatcher/旧 API 仍可调用
    - 只删代码，不删 tests/docs/fixture 中对 legacy 主线的引用
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

## P8-T01：删除顶层 legacy selector 与并行 dispatcher 壳层，收口为单一 refactor 主线入口

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
       - `tools/scoop_tools/**` 或任何带命令行 parse 的等价位置
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
  - （执行时填写）

## P8-T01R：Review selector/dispatcher 删除结果，确认仓库已不存在 legacy 顶层入口或隐藏切换点

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
  - `tools/scoop_tools/**`

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
  - （执行时填写）

## P8-T02：删除 legacy effect/continuation lowering 主线、legacy LLVM effect backend，以及所有 code-shape-specific 旧入口

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
  - 后续任务只需清理 tests/docs/fixtures 残留并做最终全量验证。
- 依赖：P8-T01R
- 完成记录：
  - （执行时填写）

## P8-T02R：Review legacy 主线删除结果，确认旧 backend 与 shape-specific 入口已经真正消失

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
  - （执行时填写）

## P8-T03：清理 tests/fixtures/docs 中的 legacy 主线残留，并把 compare 型资产改写为纯新主线回归

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P8
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §8
  - 前置实现参考：[`TODO-P7.md`](./TODO-P7.md) P7-T02 / P7-T04
- 目标：
  - 删除测试、fixtures、文档、注释、示例命令中对 legacy 主线仍可作为正常入口的引用；
  - 把原先以 compare/rollback 为目的保留到 P7 的资产，改写成纯新主线路径的回归断言，或直接删除；
  - 确保仓库的公开叙述与测试主路径都符合“只有新主线存在”的最终状态。

- 必须实现的内容：
  1. 清理 tests / fixtures / helper 中的 legacy selector 与 legacy compare 假设。
     - 至少检查并必要时修改：
       - `crates/scoop/src/fixtures/mod.rs`
       - `crates/scoop/src/fixtures/expectations.rs`
       - `crates/scoopc/src/llvm/tests.rs`
       - 任何还显式使用 `--effect-pipeline legacy` 的测试 helper、fixture 注释、或 compare harness
     - 要求：
       - 如果某测试原本只是为了证明 default/refactor/legacy 三者差异，现在应改写成只断言新主线行为；
       - 如果某测试仅为了保留 legacy 可执行性而存在，且对新主线已无价值，应删除。
  2. 清理 docs / README / 开发文档 / 迁移注释中的 legacy 主线叙述。
     - 至少要去掉：
       - “默认还是 legacy”
       - “可通过 `--effect-pipeline legacy|refactor` 选择”
       - “P8 前暂时保留 legacy” 之类已经过时的过渡说明
     - 若需保留历史背景，只能作为已完成迁移的历史说明，而不能继续给出可执行 legacy 用法。
  3. 清理 build/run/spec-fixture 命令示例中的 legacy 入口。
     - 所有正常示例命令都应直接使用默认新主线；
     - 不再出现显式 `--effect-pipeline refactor` 作为必须参数；
     - 不再出现显式 `--effect-pipeline legacy` 作为可选主线。
  4. 清理 naming 残留。
     - 若某些 helper/test/fixture 名仍以 `legacy_` / `old_` 命名，但语义已不再真的对应旧主线，必须改名；
     - 若某测试的唯一目的就是证明“legacy 已删除”，允许保留 `legacy_removed_*` 这类负向命名；
     - 但不得让主测试集继续以“legacy 是正常实现之一”的方式组织。
  5. 建立“仓库中仅剩新主线”的定向清理守护。
     - 至少要新增或更新一组搜索/断言测试，证明：
       - CLI/help 文本不再暴露 legacy selector；
       - tests/docs/fixtures 中不再把 legacy 当作执行主线；
       - compare harness 已被删除或彻底改写。

- 必须遵从的约束：
  - 禁止因为删 legacy compare 测试麻烦，就继续保留可执行 legacy 代码。
  - 禁止把历史比较型测试简单跳过；要么改写为新主线回归，要么删除。
  - 禁止在 README/docs 中继续给出任何可执行 legacy 命令示例。
  - 禁止保留误导性命名，让后续维护者以为仓库里还有第二条主线。

- 验证：
  1. 新增/更新定向测试，推荐命名：
     - `legacy_pipeline_docs_removed_*`
     - `legacy_compare_harness_removed_*`
  2. 运行：
     - `cargo test -p scoop legacy_pipeline_docs_removed`
     - `cargo test -p scoopc legacy_compare_harness_removed`
  3. 仓库搜索（执行时必须在完成记录中附摘要）：
     - `rg -e "--effect-pipeline legacy|--effect-pipeline refactor|default.*legacy|legacy pipeline|old effect mainline|parallel pipeline" . --glob '!target/**'`
  4. 要求：
     - 允许命中：本任务新增的“已删除 legacy”负向测试、迁移说明中的历史叙述；
     - 不允许命中：公开命令示例、fixtures 主路径、tests 主路径仍把 legacy 当可执行主线。

- 完成条件：
  - tests/fixtures/docs 已完成对 legacy 主线残留的清理；
  - compare 型资产已改写为纯新主线回归或被删除；
  - 后续只剩“在只有新主线存在”的前提下跑最终完整矩阵。
- 依赖：P8-T02R
- 完成记录：
  - （执行时填写）

## P8-T03R：Review 测试/文档残留清理，确认仓库公开叙述与主测试路径都只剩新主线

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P8
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §8
- 重点：
  - docs/README/help/fixtures/test helpers 中是否已不再暴露 legacy 主线；
  - compare/rollback 型资产是否已被删除或改写；
  - 是否还保留误导性的命名、注释、或命令示例。
- 必须检查的文件/位置：
  - `crates/scoop/src/fixtures/**`
  - `crates/scoopc/src/llvm/tests.rs`
  - README / 相关开发文档 / fixture 注释
  - 任何保留的迁移说明或“legacy removed”负向测试

- 验证：
  - 重新运行 P8-T03 的全部测试与命令；
  - 额外搜索：
    - `rg -e "--effect-pipeline legacy|--effect-pipeline refactor|legacy pipeline|parallel pipeline|old effect mainline" . --glob '!target/**'`
  - 要求：
    - 允许命中：历史说明、负向删除测试；
    - 不允许命中：主文档、主测试、fixtures 主路径中仍把 legacy 当作可执行路径。

- 完成条件：
  - review 能明确说明：仓库公开叙述与测试主路径已完全收口到新主线；
  - 可进入 P8-T04。
- 依赖：P8-T03
- 完成记录：
  - （执行时填写）

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
       - tests/fixtures/docs 的纯新主线路径断言
     - 明确禁止：
       - 恢复任何 legacy 分支
       - 重新引入 selector
       - 新增 hidden fallback
       - 缩小 full regression 范围
  3. 增加最终“无 legacy 主线残留”的守护检查。
     - 至少要通过测试、搜索、或等价自动化手段证明：
       - 仓库中不再保留可执行 legacy 主线；
       - 删除后 full regression 与 GC env 仍完整通过；
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
     - `rg "legacy|old effect mainline|parallel pipeline|state_machine_bridge|state_machine_emitter|UnifiedHandleLoweringContract|production_lowered_hir|legacy_eager_hir|--effect-pipeline" . --glob '!target/**'`
  3. 要求：
     - 完整矩阵全部通过；
     - 搜索结果中不得再有可执行 legacy 主线入口或旧 effect/continuation 主实现残留。

- 完成条件：
  - 仓库中不再保留旧主线；
  - 完整验证在“只有新主线存在”的条件下仍完整通过；
  - 本轮 effect-refactor 收口工作可以视为真正结束。
- 依赖：P8-T03R
- 完成记录：
  - （执行时填写）

## P8-T04R：Review P8 阶段退出条件，确认仓库已真正收口到单一新主线且本轮工作结束

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P8，§3，§4
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) 全文
- 重点：
  - legacy selector、legacy 主线实现、legacy compare 资产是否都已被删除或改写；
  - 完整回归与 GC env 全开矩阵是否在“只有新主线存在”的条件下通过；
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
