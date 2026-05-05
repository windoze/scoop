# TODO（P7：切换主线并执行 full regression）

> 生成时间：2026-05-02  
> 设计基线：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md)  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 前置条件：`TODO-P6.md` 已完整完成；refactor LLVM codegen 新路径已在并行模式下端到端生成并运行 effect/continuation 程序；`--effect-pipeline legacy|refactor` 选择器仍存在且当前默认值仍可控。  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 本阶段目标：把新的 effect-refactor 路径切成默认主线；在保留显式 `legacy` 参数作为短期回滚/比对入口的前提下，完成一次完整回归与 GC env 验证，证明默认主线已经可以由 refactor 路径承担；同时冻结 P7 -> P8 handoff，确保 P8 只需删除旧主线，而不再补做架构设计。

## 全局约束

- [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) 是本阶段唯一设计基线；若实现过程中需要改变主张，必须先回写该文档，再继续实现。
- [`PLAN.md`](./PLAN.md) 与 [`TODO-P0.md`](./TODO-P0.md)、[`TODO-P1.md`](./TODO-P1.md)、[`TODO-P2.md`](./TODO-P2.md)、[`TODO-P3.md`](./TODO-P3.md)、[`TODO-P4.md`](./TODO-P4.md)、[`TODO-P5.md`](./TODO-P5.md)、[`TODO-P6.md`](./TODO-P6.md) 是本阶段执行前提；P7 不得重新开启 P0-P6 已收敛的 selector / typed HIR / direct-style MIR / effect facts / late-lowered representation / LLVM effect backend 讨论。
- 本阶段只处理两件事：
  1. 把 refactor 路径切成默认主线；
  2. 在新默认值下完成 full regression 与 GC env 验证。
- 明确禁止：
  - 在 P7 中删除 legacy 路径、legacy CLI 参数、legacy backend、或旧 dump/fixture；这些属于 P8。
  - 在 P7 中重新设计 `StepSchema`、`ContinuationSchema`、`resolved_outward_cases`、`impl_plan`、late lowering、LLVM ABI、GC root 模型、runtime error 语义、或 dropped continuation 语义；这些在 P4-P6 已经闭合。
- P7 结束时，默认 effect/continuation 主线必须是 refactor。
  - `scoop`、`scoopc`、fixtures harness、测试 helper、以及任何默认构造 `Session` / pipeline config 的入口，在**未显式指定 selector** 时都必须走 refactor；
  - `--effect-pipeline legacy` 必须继续可用，作为短期回滚/比对入口；
  - `--effect-pipeline refactor` 若当前已存在，可继续保留用于显式测试与文档示例；
  - 但“省略 selector”时的行为必须稳定等于 refactor。
- `tools/scoop_tools` 当前不直接构造 `scoopc::Session`，因此不属于 selector 默认值翻转的实现范围；P7 只需保证它的文档、脚本或调用示例不再假定 legacy 默认值。
- refactor 成为默认后，legacy 路径只能通过**显式 selector** 进入。
  - 明确禁止：
    - 在省略 selector 时静默回落到 legacy；
    - 当 refactor 路径失败时自动 fallback 到 legacy；
    - 在 driver / fixture / test helper 中偷偷把默认 `Session` 强制设回 legacy；
    - 用“默认还是 legacy，但 CI/测试都显式加 refactor 参数”冒充完成。
- legacy 路径在 P7 仅允许承担两类职责：
  1. 显式 compare/rollback 入口；
  2. P8 删除前的短期兜底。
  - 明确禁止：继续把新功能、主要 bugfix、或 refactor 正确性修复落在 legacy 路径上，然后依赖默认 fallback 掩盖问题。
- 本阶段允许并要求执行 full regression。
  - 必须覆盖：
    - `cargo test --all`
    - `cargo run -p scoop -- test`
    - `cargo run -p scoop_tools -- spec-fixtures check`
    - `cargo clippy --all-targets -- -D warnings`
    - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
    - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`
  - 本阶段不允许像 P0-P6 那样以“不做 full regression”为理由跳过广覆盖验证。
- full regression 与 GC env 验证必须默认在**省略 `--effect-pipeline`** 的情况下运行，以证明新默认值真实生效。
  - 允许额外执行显式 `legacy` smoke / compare；
  - 但这只能是附加验证，不能替代默认路径的回归通过。
- 若 full regression 暴露问题：
  - 默认修复策略必须是修正 refactor 默认路径、共享模块、测试假设、或短期保留的 compare/rollback 入口；
  - 明确禁止：通过恢复 legacy 默认值、把失败 fixture 改成显式 `legacy`、缩小回归范围、标记跳过、或依赖 hidden fallback 来“通过” P7。
- 若某些测试/文档/工具当前显式依赖 `--effect-pipeline refactor` 才能表达“默认新路径”的含义，P7 必须把它们改成默认省略 selector 的写法；
  - 仅保留那些**明确在做 compare、rollback、或 legacy 存活性 smoke** 的显式 `legacy` / `refactor` 用法。
- 本阶段可以修改：
  - CLI/help 文案
  - `Session` 默认值
  - commands/fixtures/test helpers 的默认构造路径
  - README / 开发文档 / fixture 注释中关于默认主线的说明
  - CI 脚本或仓库内回归脚本（若仓库中已有）
  但这些修改只能服务“默认切换 + 回归收口”，不能引入新的架构分叉。
- 若 P0-P6 落地的实际模块名与 TODO 推荐路径不同，P7 实施时允许使用等价位置；
  - 但必须在完成记录中写清楚实际映射，避免 P8 删除时漏清理。

## [DONE] P7-T01：翻转顶层 selector 默认值为 refactor，同时保留显式 `legacy` 参数作为短期 compare/rollback 入口

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P7
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §8
  - 前置实现参考：[`TODO-P0.md`](./TODO-P0.md) P0-T01 / P0-T02
- 目标：
  - 把“默认 mode = legacy”翻转为“默认 mode = refactor”；
  - 让 omission-based default 在 `scoop` / `scoopc` / tests / fixtures 中统一生效；
  - 同时保留显式 `--effect-pipeline legacy` 作为过渡期入口，供 P7 compare 与 P8 前的短期回滚使用。

- 必须实现的内容：
  1. 翻转 selector 的默认值。
     - 优先检查并修改以下位置或其等价实现：
       - `crates/scoopc/src/session/` 中 `EffectPipelineMode` / `SessionOptions` / `Session::new()` 默认值
       - `crates/scoop/src/cli.rs`
       - `crates/scoop/src/commands/**/*.rs`
       - `crates/scoopc/src/bin/scoopc.rs`
       - 任何实际构造 `Session` / pipeline config 的等价位置
       - 若仓库中存在调用 `scoop` / `scoopc` 的 wrapper script，再同步修改对应位置；当前 `tools/scoop_tools` Rust binary 本身不在 selector 翻转范围内
     - 要求：省略 selector 时统一进入 refactor。
  2. 保留并验证显式 `legacy` 参数。
     - `--effect-pipeline legacy` 必须继续可解析、可传入 session、可贯穿 dispatcher，并实际进入旧主线；
     - `--effect-pipeline refactor` 若当前已存在，也应继续保留，以便测试与文档显式表述；
     - 明确禁止：通过“删掉 legacy 参数，只剩默认 refactor”来完成 P7。
  3. 更新默认构造入口。
      - 若当前 `Session::new()`、test helper、fixture helper、或其它实际构造 `Session` 的 helper 默认构造 legacy session，则必须切为 refactor；
     - 同时保留一个显式 legacy 构造入口或等价配置方式，供 compare/rollback 场景使用；
     - 明确禁止：在 helper 内写“默认 legacy，但命令层再覆盖成 refactor”的双重语义。
  4. 更新 CLI/help/parse 测试。
     - 至少要覆盖：
       - 缺省时为 refactor
       - 显式 `legacy`
       - 显式 `refactor`
       - 非法取值报错
     - 若 `scoopc` CLI 与 `scoop` CLI 共用 parse helper，则两端都必须被测试覆盖。
  5. 更新驱动层与 dispatcher 的注释或等价文档说明。
     - 至少要明确：
       - 默认值已翻转为 refactor；
       - legacy 参数仅为短期 compare/rollback 入口；
       - P8 将删除 legacy 默认/显式入口。

- 必须遵从的约束：
  - 禁止把默认值只改在某一个入口上，而让其它实际构造 `Session` 的入口（如 fixtures、test helper）仍默认 legacy。
  - 禁止把“默认 refactor”实现成“先尝试 refactor，失败再自动跑 legacy”。
  - 禁止为了通过 P7 而移除显式 `legacy` compare 入口；P8 才负责删除。
  - 禁止重新把 pipeline mode 渗入低层业务实现函数；selector 仍必须停留在 CLI / session / dispatcher 层。

- 验证：
  1. 新增/更新定向测试，推荐命名：
     - `default_effect_pipeline_is_refactor_*`
     - `explicit_legacy_pipeline_still_available_*`
  2. 运行：
      - `cargo test -p scoop --no-default-features cli`
      - `cargo test -p scoopc --no-default-features session`
      - 若 fixtures helper 或其它实际构造 `Session` 的 helper 有单独测试入口，补充对应定向测试
  3. 最小 smoke：
     - `cargo run -p scoop -- dump-ast tests/fixtures/parse/hello.scoop`
     - `cargo run -p scoop -- --effect-pipeline legacy dump-ast tests/fixtures/parse/hello.scoop`
     - `cargo run -p scoop -- build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p7_default.ll`
     - `cargo run -p scoop -- --effect-pipeline legacy build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p7_legacy.ll`
     - `cargo run -p scoop -- test --fixtures tests/fixtures/build/emit_llvm_basic.scoop`
  4. 要求：
     - 省略 selector 的命令使用 refactor 默认路径；
     - 显式 `legacy` 路径继续可用；
     - 没有任何入口在 omission 情况下仍默认为 legacy。

- 完成条件：
  - 默认主线已翻转为 refactor；
  - 显式 `legacy` 参数仍可用；
  - 后续任务可以在“默认就是 refactor”的前提下修回归，而不是继续围绕显式 refactor 参数工作。
- 依赖：`TODO-P6.md` 最后一项 review 完成
- 完成记录：
  - 2026-05-05：完成默认 selector 翻转。实际映射：`crates/scoopc/src/session/mod.rs` 将 `EffectPipelineMode::Refactor` 设为 `Default`，因此 `Session::new()`、`Session::with_sysroot(...)` 与 `SessionOptions::default()` 均在省略 selector 时进入 refactor；`crates/scoop/src/cli.rs` 与 `crates/scoopc/src/driver_cli.rs` 的 CLI omission 默认值同步改为 refactor；`crates/scoop/src/fixtures/mod.rs` / `crates/scoop/src/fixtures/run_pass.rs` 的 fixture 子进程 selector 传播改为只在显式 legacy 时追加 `--effect-pipeline legacy`，默认/显式 refactor 语义通过 omission 进入 refactor。显式 `--effect-pipeline legacy` 与 `--effect-pipeline refactor` 均继续可解析并传入 session/dispatcher；未新增 refactor 失败后回 legacy 的 fallback。
  - 同步更新 CLI/help/driver/dispatcher 注释，说明默认已是 refactor、legacy 仅为短期 compare/rollback 入口，P8 将删除 legacy selector 分支。
  - 验证通过：`cargo fmt --all`；`cargo test -p scoop --no-default-features cli`；`cargo test -p scoopc --no-default-features session`；`cargo test -p scoopc --no-default-features driver_cli`；`cargo test -p scoop --no-default-features default_refactor_pipeline`；`cargo test -p scoop --no-default-features explicit_legacy_pipeline`；`cargo run -p scoop -- dump-ast tests/fixtures/parse/hello.scoop`；`cargo run -p scoop -- --effect-pipeline legacy dump-ast tests/fixtures/parse/hello.scoop`；`cargo run -p scoop -- build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p7_default.ll`；`cargo run -p scoop -- --effect-pipeline legacy build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p7_legacy.ll`；`cargo run -p scoop -- test --fixtures tests/fixtures/build/emit_llvm_basic.scoop`；`cargo clippy --all-targets -- -D warnings`。

## [DONE] P7-T01R：Review selector 默认值翻转，确认 omission=refactor 且 explicit legacy 仍是唯一短期回滚入口

- 参考：
  - [`PLAN.md`](./PLAN.md) §0，§2/P7
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §8
  - [`TODO-P0.md`](./TODO-P0.md) P0-T01 / P0-T02
- 重点：
  - omission-based default 是否已统一切到 refactor；
  - `legacy` 是否只剩显式入口，而不是还存在隐藏默认值或 fallback；
  - `scoop`、`scoopc`、fixtures helper、test helper、以及其它实际构造 `Session` 的入口是否已经一致。
- 必须检查的文件/位置：
  - `crates/scoop/src/cli.rs`
  - `crates/scoop/src/commands/**/*.rs`
  - `crates/scoopc/src/bin/scoopc.rs`
  - `crates/scoopc/src/session/**`
  - 任何实际构造 `Session` / pipeline config 的等价位置
  - 相关 fixture/test helper 位置

- 验证：
  - 重新运行 P7-T01 的全部测试与命令；
  - 额外搜索：
    - `rg "Legacy|Refactor|effect[-_]pipeline|Session::new\(|default.*legacy|default.*refactor" crates/scoop crates/scoopc tools/scoop_tools`
  - 要求：
    - 允许命中：显式 compare 测试、注释、帮助文本；
    - 不允许命中：遗漏的默认 legacy 构造点、自动 fallback、或 helper 中的隐藏旧默认值。

- 完成条件：
  - review 能明确说明：默认主线切换已经真实完成，而不是只在少数命令上换了壳；
  - 可进入 P7-T02。
- 依赖：P7-T01
- 完成记录：
  - 2026-05-05：完成 selector 默认值翻转 review。确认实际默认来源集中在 `crates/scoopc/src/session/mod.rs` 的 `EffectPipelineMode::Refactor` default，`Session::new()`、`Session::with_sysroot(...)` 与 `SessionOptions::default()` 在 omission 情况下均进入 refactor；`crates/scoop/src/cli.rs` 与 `crates/scoopc/src/driver_cli.rs` 的 CLI omission 默认值也均为 refactor。`scoop` commands 通过 `SessionOptions::new(effect_pipeline)` 统一传递 selector，`scoop test` / run-pass / run_pass_cone 子进程只在显式 legacy session 下追加 `--effect-pipeline legacy`，默认 refactor 通过 omission 传播。
  - 搜索确认没有遗漏的默认 legacy 构造点、自动 fallback 或 retry legacy：`default.*legacy|legacy.*default|fallback.*legacy|retry.*legacy` 在 `crates/**` 与 `tools/scoop_tools` 无命中。显式 legacy 命中均为短期 compare/rollback 入口、dispatcher legacy 分支或对应测试；未发现 helper 在 omission 情况下强制 legacy。
  - 验证通过：`rg "Legacy|Refactor|effect[-_]pipeline|Session::new\(|default.*legacy|default.*refactor" crates/scoop crates/scoopc tools/scoop_tools`；`cargo test -p scoop --no-default-features cli`；`cargo test -p scoopc --no-default-features session`；`cargo test -p scoopc --no-default-features driver_cli`；`cargo test -p scoop --no-default-features default_refactor_pipeline`；`cargo test -p scoop --no-default-features explicit_legacy_pipeline`；`cargo run -p scoop -- dump-ast tests/fixtures/parse/hello.scoop`；`cargo run -p scoop -- --effect-pipeline legacy dump-ast tests/fixtures/parse/hello.scoop`；`cargo run -p scoop -- build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p7_default.ll`；`cargo run -p scoop -- --effect-pipeline legacy build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p7_legacy.ll`；`cargo run -p scoop -- test --fixtures tests/fixtures/build/emit_llvm_basic.scoop`；`cargo fmt --all`；`cargo clippy --all-targets -- -D warnings`。

## [DONE] P7-T02：更新默认主线切换后的 driver/fixture/test/docs 假设，并锁定“无显式 selector 时不得悄悄回 legacy”

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P7
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §8
  - 前置实现参考：[`TODO-P0.md`](./TODO-P0.md) P0-T04、[`TODO-P6.md`](./TODO-P6.md) P6-T05
- 目标：
  - 清理并更新仓库内所有“默认还是 legacy”或“正常路径必须显式 `--effect-pipeline refactor`”的假设；
  - 让 build/run/test/build-fixtures/run-pass/runtime_gc/spec-fixtures 的**默认**路径都代表 refactor 主线；
  - 同时把显式 `legacy` 用法收敛到 compare/rollback/smoke 场景，防止 hidden fallback 继续存活。

- 必须实现的内容：
  1. 盘点并更新默认主线相关的 driver/fixture/test 假设。
     - 至少检查并必要时修改：
       - `crates/scoop/src/commands/**/*.rs`
       - `crates/scoop/src/fixtures/mod.rs`
       - `crates/scoop/src/fixtures/expectations.rs`
       - `crates/scoopc/src/llvm/tests.rs`
       - 其它会默认构造 session / pipeline mode 的测试 helper
       - README / 开发文档 / fixture 注释中对“默认主线”的说明（若仓库里存在）
  2. 把“默认路径验证 refactor”与“显式 legacy compare”分开。
     - 正常 smoke / regression / 示例命令应优先省略 selector；
     - 只有以下场景允许保留显式 `legacy`：
       - compare 测试
       - rollback/smoke 测试
       - P8 删除前的存活性守护测试
     - 若当前某些测试只是为了表达“这是新路径”而显式写 `--effect-pipeline refactor`，P7 必须改为默认路径写法，除非该测试明确在比较 explicit refactor 与 default 是否一致。
  3. 增加默认路径与显式 refactor 等价的自动化断言。
     - 至少选择代表性入口覆盖：
       - `dump-*`
       - `build --emit-llvm`
       - `run`
       - `test --fixtures ...`
     - 要求：
       - omission 路径与显式 `--effect-pipeline refactor` 的退出状态一致；
       - 对稳定文本输出（如 dump / emitted `.ll`）应尽量比较一致；
       - 若某输出含不稳定字段，必须先正规化再比较。
  4. 增加 hidden-fallback 守护。
     - 至少要能自动证明：
       - omission 不会静默切到 legacy；
       - refactor 失败不会自动 retry legacy；
       - fixture harness / helper 不会因为某类 phase 仍未显式接 selector 就偷偷落回旧路径。
  5. 若仓库内存在任何“默认 legacy，CI/脚本显式加 refactor”的脚本或说明，必须改成“默认 refactor，legacy 仅显式 compare”。

- 必须遵从的约束：
  - 禁止用批量文本替换粗暴删除所有显式 `legacy` / `refactor`，必须保留 compare/rollback 需要的显式 `legacy` 用法。
  - 禁止把 full regression 中的失败测试改成显式 `legacy` 以通过 P7。
  - 禁止新增隐藏环境变量或测试私货来切回 legacy 默认值。
  - 禁止在 docs/test 文案里继续把 legacy 描述成默认主线。

- 验证：
  1. 新增/更新定向测试，推荐命名：
     - `default_pipeline_matches_explicit_refactor_*`
     - `no_hidden_legacy_fallback_*`
  2. 运行默认 vs 显式 refactor 对比 smoke：
     - `cargo run -p scoop -- dump-mir tests/fixtures/mir/handle_perform.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor dump-mir tests/fixtures/mir/handle_perform.scoop`
     - `cargo run -p scoop -- build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p7_default_build.ll`
     - `cargo run -p scoop -- --effect-pipeline refactor build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p7_explicit_refactor_build.ll`
     - `cargo run -p scoop -- test --fixtures tests/fixtures/build/emit_llvm_basic.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/emit_llvm_basic.scoop`
  3. 运行显式 legacy compare smoke：
     - `cargo run -p scoop -- --effect-pipeline legacy dump-mir tests/fixtures/mir/handle_perform.scoop`
     - `cargo run -p scoop -- --effect-pipeline legacy test --fixtures tests/fixtures/build/emit_llvm_basic.scoop`
  4. 额外搜索：
     - `rg -e "--effect-pipeline refactor|--effect-pipeline legacy|default.*legacy|fallback.*legacy" . --glob '!target/**'`
     - 执行时必须在完成记录中总结：哪些显式 `refactor` 保留了，哪些改成默认，哪些显式 `legacy` 仍是合理 compare/rollback 用法。

- 完成条件：
  - 仓库中的默认路径假设已经统一切到 refactor；
  - 显式 `legacy` 用法已收敛到 compare/rollback 场景；
  - 没有隐藏 legacy fallback 或默认值残留。
- 依赖：P7-T01R
- 完成记录：
  - 2026-05-05：完成默认主线假设清理与守护。`crates/scoop/src/cli.rs` 中仅用于表达“新路径”的 `dump-effect-facts` / `dump-effect-lowered` parse 测试已改为省略 selector，显式 `refactor` parse 测试仅保留为 selector 存活性覆盖；`dump-effect-facts` / `dump-effect-lowered` legacy unsupported 诊断改为提示“省略 selector 使用默认 refactor，或显式使用 `--effect-pipeline refactor`”；`mir_refactor` fixture 诊断改为说明默认或显式 refactor 均可，显式 legacy 不支持。
  - 新增 `crates/scoop/tests/p7_default_pipeline.rs` 黑盒守护：默认路径与显式 `--effect-pipeline refactor` 在 `dump-mir`、`build --emit-llvm`、`run`、`test --fixtures` 四个代表入口上退出状态/stdout/stderr（以及 LLVM IR 产物）一致；默认 `scoop test` 可在不传 selector 时运行 refactor-only build fixture `effect_refactor_no_legacy_handler_stack_calls.scoop`，证明 fixture harness 不会因省略 selector 落回 legacy。
  - 更新 `crates/scoop/src/commands/build.rs` 的 build 守护：reachable self-contained handle 样本现在已能由 refactor lowering 正常产出 IR，测试改为正向断言 refactor/default build 输出包含 refactor-owned symbol 且不含 `scoop_effect_handler_stack` / `scoop_effect_outcome`，避免继续把已闭合路径当作“应失败”边界。
  - 更新 `EFFECT_REFACTOR.md` P6 -> P7 handoff 说明：P6 中的显式 refactor 命令是默认翻转前的历史 handoff 示例；P7 之后当前 smoke/regression 默认应省略 selector，显式 `refactor` 只用于 default-vs-explicit 等价检查，显式 `legacy` 只用于 P8 前短期 compare/rollback smoke。
  - 显式 `refactor` 保留项：CLI/scoopc/session 的存活性 parse 覆盖、`P7` default-vs-explicit 等价测试、legacy unsupported 诊断中的显式替代说明、P6 历史 handoff 记录。显式 `legacy` 保留项：CLI/session 存活性覆盖、dispatcher legacy 分支、P7/P8 前 compare/rollback smoke、legacy unsupported 诊断和历史完成记录；未发现正常默认路径仍依赖显式 `legacy`。
  - 搜索结论：执行 `rg -e "--effect-pipeline refactor|--effect-pipeline legacy|default.*legacy|fallback.*legacy" . --glob '!target/**'`。命中主要来自历史 TODO/完成记录、P6 handoff 文档、当前 P7 任务说明、显式 compare/rollback 测试、以及本次新增的守护测试/诊断；实现代码中未发现省略 selector 时默认 legacy、refactor 失败后 retry legacy、或 fixture/helper hidden fallback 的路径。
  - 验证通过：`cargo fmt --all`；`cargo test -p scoop --no-default-features cli`；`cargo test -p scoop --no-default-features dump_effect`；`cargo test -p scoop --test p7_default_pipeline`；`cargo test -p scoop legacy_frames`；`cargo test -p scoop no_hidden_legacy_fallback`；`cargo run -p scoop -- dump-mir tests/fixtures/mir/handle_perform.scoop`；`cargo run -p scoop -- --effect-pipeline refactor dump-mir tests/fixtures/mir/handle_perform.scoop`；`cargo run -p scoop -- build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p7_default_build.ll`；`cargo run -p scoop -- --effect-pipeline refactor build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p7_explicit_refactor_build.ll`；`cargo run -p scoop -- test --fixtures tests/fixtures/build/emit_llvm_basic.scoop`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/emit_llvm_basic.scoop`；`cargo run -p scoop -- --effect-pipeline legacy dump-mir tests/fixtures/mir/handle_perform.scoop`；`cargo run -p scoop -- --effect-pipeline legacy test --fixtures tests/fixtures/build/emit_llvm_basic.scoop`；上述 `rg` 搜索；`cargo clippy --all-targets -- -D warnings`。

## [DONE] P7-T02R：Review 默认主线假设与 hidden-fallback 守护，确认 omission/default 真正代表 refactor 主线

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P7
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §8
- 重点：
  - 省略 selector 的 smoke / fixtures / tests 是否已经真正覆盖 refactor；
  - 显式 `refactor` 用法是否只在必要 compare 场景下保留；
  - 显式 `legacy` 是否只剩 compare/rollback/smoke 用途；
  - 是否还有任何隐藏 legacy fallback。
- 必须检查的文件/位置：
  - `crates/scoop/src/commands/**/*.rs`
  - `crates/scoop/src/fixtures/mod.rs`
  - `crates/scoop/src/fixtures/expectations.rs`
  - `crates/scoopc/src/llvm/tests.rs`
  - README / 相关开发文档 / fixture 注释（若已修改）

- 验证：
  - 重新运行 P7-T02 的全部测试与命令；
  - 额外搜索：
    - `rg -e "--effect-pipeline refactor|--effect-pipeline legacy|fallback.*legacy|retry.*legacy|default.*legacy" . --glob '!target/**'`
  - 要求：
    - 允许命中：显式 compare 测试、文档中说明 legacy 为临时入口的文字；
    - 不允许命中：正常默认路径仍被写成 legacy，或 refactor 失败后自动 fallback 到 legacy 的实现。

- 完成条件：
  - review 能明确说明：full regression 将真正覆盖“新默认主线”，而不是表面切换；
  - 可进入 P7-T03。
- 依赖：P7-T02
- 完成记录：
  - 2026-05-05：完成默认主线假设与 hidden-fallback 守护 review。确认 `crates/scoop/src/commands/mod.rs` 统一从 CLI selector 构造 `SessionOptions`；`SessionOptions::default()` / `Session::new()` 的默认仍为 refactor；`crates/scoop/src/fixtures/mod.rs` / `run_pass.rs` 的 fixture 子进程只在显式 legacy session 下追加 `--effect-pipeline legacy`，默认与显式 refactor 均通过 omission 进入 refactor。`crates/scoop/tests/p7_default_pipeline.rs` 已覆盖 `dump-mir`、`build --emit-llvm`、`run`、`test --fixtures` 四个代表入口的 default-vs-explicit refactor 等价，并用 refactor-only build fixture 守护 fixture harness 不会隐藏回 legacy。
  - 显式 `refactor` review 结论：保留在 CLI/session 存活性测试、P7 default-vs-explicit 等价测试、legacy unsupported 诊断提示和 P6 历史 handoff 记录中；未发现正常默认 smoke/regression 需要靠显式 `--effect-pipeline refactor` 表达新主线。显式 `legacy` review 结论：保留为 CLI/session 存活性、dispatcher legacy 分支、P7/P8 前 compare/rollback smoke 和 legacy unsupported 诊断语境；未发现 omission/default 路径仍默认 legacy 或 refactor 失败后 retry/fallback legacy。
  - 搜索结论：指定全仓搜索 `rg -e "--effect-pipeline refactor|--effect-pipeline legacy|fallback.*legacy|retry.*legacy|default.*legacy" . --glob '!target/**'` 命中主要来自历史 TODO/完成记录、P6 handoff 文档、当前 P7 任务说明、显式 compare/rollback 测试、诊断和 hidden-fallback 守护文本；限定实现范围 `crates tools tests` 后仅剩 `dump-effect-facts` / `dump-effect-lowered` legacy unsupported 诊断、`mir_refactor` fixture 诊断、以及 `build.rs` hidden-fallback 断言文本，未发现 hidden fallback 实现路径。
  - 验证通过：`cargo fmt --all`；`cargo test -p scoop --no-default-features cli`；`cargo test -p scoop --no-default-features dump_effect`；`cargo test -p scoop --test p7_default_pipeline`；`cargo test -p scoop legacy_frames`；`cargo test -p scoop no_hidden_legacy_fallback`；`cargo run -p scoop -- dump-mir tests/fixtures/mir/handle_perform.scoop`；`cargo run -p scoop -- --effect-pipeline refactor dump-mir tests/fixtures/mir/handle_perform.scoop`；`cargo run -p scoop -- build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p7_default_build.ll`；`cargo run -p scoop -- --effect-pipeline refactor build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p7_explicit_refactor_build.ll`；`cargo run -p scoop -- test --fixtures tests/fixtures/build/emit_llvm_basic.scoop`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/emit_llvm_basic.scoop`；`cargo run -p scoop -- --effect-pipeline legacy dump-mir tests/fixtures/mir/handle_perform.scoop`；`cargo run -p scoop -- --effect-pipeline legacy test --fixtures tests/fixtures/build/emit_llvm_basic.scoop`；上述 `rg` 搜索；`rg -e "--effect-pipeline refactor|--effect-pipeline legacy|fallback.*legacy|retry.*legacy|default.*legacy" crates tools tests --glob '!target/**'`；`cargo clippy --all-targets -- -D warnings`。

## [DONE] P7-T02T：发布并消费 generic class instance layout handoff，解除 `Task<T>` constructor 在 refactor LLVM 默认路径上的阻塞

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P7
  - [`TODO-P5.md`](./TODO-P5.md) P5-T04 / P5-T05 / P5-T08
  - [`TODO-P6-part3.md`](./TODO-P6-part3.md) P6-T05a / P6-T06
- 背景：
  - 执行 P7-T02S 时，`tests/fixtures/build/task_atomic_claim_no_mutex_llvm.scoop` 已越过原始 `HandleDispatch` completion payload source 缺口，以及后续暴露的 generic resume surface ABI、resume-boundary wrapper projection、plain local-effect closure、enum ctor、task transport、atomic 与 panic lowering 缺口；
  - 当前默认 refactor build 停在 `scoop.core.Task<T>` constructor：`refactor pure assignment ... ClassCtor { class_fqn: "scoop.core.Task" ... } ... class field type`；
  - 根因是 refactor class ctor 仍按未实例化 generic class declaration field layout 取 LLVM payload，而不是消费 canonical materialized `Task<Int>` / `Task<(Int, Any)>` instance layout handoff。

- 必须实现的内容：
  1. 为 generic class instance 发布 canonical field/layout handoff。
     - 至少覆盖 `Task<T>` 这类 class field 含 `T` / `__TaskState<T>` 的实例；
     - layout key 必须稳定绑定到 materialized `InstanceKey` / concrete type args，而不是只按 raw class FQN 查询 declaration layout。
  2. 让 refactor LLVM class ctor lowering 消费该 concrete instance layout。
     - `Task<Int>` 与 `Task<(Int, Any)>` constructor 必须按 concrete field ABI 存储 `__claim` 与 `__state`；
     - 不得把 `T` 临时擦成 `Any`、不得回 legacy class ctor lowering、不得按 fixture 名字特判。
  3. 保持 GC/type descriptor 与 field trace bitmap 语义正确。
     - concrete field 中含 GC ref / enum / tuple carrier 时，descriptor 与 root tracing 不能退化；
     - 不得通过禁用 root/trace 检查绕过 moving-GC 语义。
  4. 增加或更新定向测试，覆盖 generic class ctor 使用 concrete field layout 的路径。

- 验证：
  - `cargo test -p scoopc --lib effect_lowered llvm::codegen::effect_refactor llvm::tests`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build/task_atomic_claim_no_mutex_llvm.scoop`
  - 修复后继续恢复 P7-T02S 的完整定向 fixture 验证。

- 完成条件：
  - `Task<Int>` / `Task<(Int, Any)>` constructor 在默认 refactor LLVM path 下不再触发未实例化 generic field layout；
  - `task_atomic_claim_no_mutex_llvm.scoop` 可继续推进到 P7-T02S 原验证矩阵；
  - 没有引入 legacy fallback、fixture 特判或 generic erasure workaround。
- 依赖：P7-T02R
- 完成记录：
  - 2026-05-05：发布并消费 concrete generic class instance layout handoff。`RefactorAbiQuery` 现在携带 `TypeId -> RefactorClassInstanceLayout` 查询面，layout key 由 concrete source type args 生成的 canonical class key（如 `scoop.core.Task<Int>` / `scoop.core.Task<(Int, Any)>`）绑定；materialization 会跳过仍含 type param 的未实例化类型，并对 concrete class field 中残留的 type param fail fast。
  - refactor class ctor lowering 现在通过 assignment target local 的 concrete source type 查询 class instance layout，再用 concrete class key 分配 payload、type descriptor 与字段存储；production/pass MIR class ctor 与 class member read/write 也改为优先消费 target/receiver local 的 concrete type，避免 `MemberAccessMetadata.receiver_ty` 中的 generic declaration type 把 `Task.__state` 等字段带回 raw `Task<T>` layout。
  - 新增 `llvm::tests::refactor_class_ctor_uses_concrete_generic_instance_layout`，覆盖 `Box<String>` constructor 发布 concrete payload type、使用 concrete type descriptor、且不回 raw `Box<T>` descriptor。
  - 验证通过：`cargo test -p scoopc --lib refactor_class_ctor_uses_concrete_generic_instance_layout`；`cargo test -p scoopc --lib effect_lowered`；`cargo test -p scoopc --lib llvm::codegen::effect_refactor`；`cargo test -p scoopc --lib llvm::tests`；`cargo clippy --all-targets -- -D warnings`。
  - 说明：任务要求中的 `cargo test -p scoopc --lib effect_lowered llvm::codegen::effect_refactor llvm::tests` 按当前 Cargo 语法会报 `unexpected argument`，因此已拆为上述三个等价 filter 分别运行。`cargo run -p scoop -- test --fixtures tests/fixtures/build/task_atomic_claim_no_mutex_llvm.scoop` 已不再触发 `Task<T>` 未实例化 generic class field layout / member read drift；当前推进到后续 `P7-T02S` 范围内的 `refactor pure assignment local21 rvalue Use(Const(String)) ... source-backed literal span` blocker。

## [DONE] P7-T02S：修复默认 build fixture 中暴露的 refactor LLVM/lowering 缺口，解除 P7-T03 full regression 阻塞

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P7
  - [`TODO-P5.md`](./TODO-P5.md) P5-T04 / P5-T05 / P5-T07b
  - [`TODO-P6-part2.md`](./TODO-P6-part2.md) P6-T02qg / P6-T02j
- 背景：
  - P7-T03 运行默认 `cargo run -p scoop -- test` 时，build fixture 阶段暴露出多个默认 refactor 阻塞，不能通过显式 `legacy`、跳过、缩小覆盖或改弱 fixture 形状绕过；
  - `tests/fixtures/build/extern_enter_native_no_statepoint_writeback.scoop` 需要 refactor pure/body lowering 支持当前 fixture 形状中的 interpolated string、`@Extern` native enter/leave、`GC.handleNew` / `GC.handleDrop` 普通 runtime ABI，并保持 no-statepoint writeback 断言；
  - `tests/fixtures/build/int_literal_default_int_overflow_fail.scoop`、`int_literal_neg_int8_overflow_fail.scoop`、`int_literal_uint8_overflow_fail.scoop` 需要 refactor 默认路径在 `.toString()` / narrow integer target 场景下保留 `scoop::llvm::invalid_literal` 诊断，而不是先在 effect facts / frontend wrapper 处失败；
  - `tests/fixtures/build/task_atomic_claim_no_mutex_llvm.scoop` 需要 `scoop.core.__task_drive_waiting::<(Int, Any)>` 的 handle site2 正确发布 `HandleDispatch` contract，当前错误为 non-`Unit` handle arm completion payload source state st8 缺少 completion payload source。

- 必须实现的内容：
  1. 修复 refactor pure/body lowering 对上述 extern/native/GC handle/interpolated-string fixture 形状的支持。
     - `@Extern` 调用必须继续通过 `scoop_enter_native` / `scoop_leave_native` 暴露 roots；
     - `GC.handleNew` / `GC.handleDrop` 必须走普通 managed runtime ABI，不得回 legacy backend；
     - 不得删除 fixture 中用于保持 root live 的表达式形状。
  2. 修复 refactor 默认路径中的 invalid integer literal 诊断传播。
     - `.toString()` surface、负号、`Int8` / `UInt8` 等 narrow target 必须仍产生 `scoop::llvm::invalid_literal`；
     - 不得用更宽整数类型或删除 `.toString()` 来规避。
  3. 修复 late-lowered `HandleDispatch` 对 non-`Unit` handle arm completion payload source 的发现/发布。
     - 必须覆盖 sysroot task drive 形状中 arm completion payload 经中间 local / tuple / enum carrier 传播的路径；
     - 不得只特判 `__task_drive_waiting` 名字或 fixture 路径。
  4. 保持 P5/P6 已固定的 completion payload / pending payload / handle dispatch contract。
     - 禁止重新发明 body/arm/finally 返回协议；
     - 禁止回 raw MIR/HIR 在 P6 backend 现场猜 completion 值。
  5. 增加或更新定向测试，覆盖上述缺口。
  6. 确认相关 build fixtures 在省略 selector 的默认 refactor 路径下通过，并继续检查原有 IR 子串。

- 验证：
  - `cargo test -p scoopc --lib effect_lowered llvm::codegen::effect_refactor llvm::tests`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build/extern_enter_native_no_statepoint_writeback.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build/int_literal_default_int_overflow_fail.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build/int_literal_neg_int8_overflow_fail.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build/int_literal_uint8_overflow_fail.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build/task_atomic_claim_no_mutex_llvm.scoop`
  - 修复后恢复执行 P7-T03 的完整标准矩阵。

- 完成条件：
  - 上述 build fixture 在默认 refactor 主线下保持原 fixture 形状通过；
  - 未引入 hidden legacy fallback 或 fixture 降级。
- 依赖：P7-T02T
- 完成记录：
  - 2026-05-05：本轮已部分修复 P7-T02S 的直接缺口：f-string MIR `Todo` 改为显式 `InterpolatedString` rvalue 并接入 refactor value lowering；`extern_enter_native_no_statepoint_writeback.scoop` 已越过 f-string / `GC.handleNew` / `GC.handleDrop` lowering，当前仅剩 root-load IR 子串需随 refactor 命名更新；default / neg Int8 / UInt8 integer overflow fixtures 已恢复 `scoop::llvm::invalid_literal`；`task_atomic_claim_no_mutex_llvm.scoop` 已越过原始 non-`Unit` handle arm completion payload source 缺口，并补齐后续暴露的 generic resume surface ABI、resume-boundary wrapper complete projection、plain local-effect closure、enum ctor、task transport、atomic 与 panic lowering。但该 fixture 继续暴露 `Task<T>` generic class constructor/layout handoff 缺口，已新增 prerequisite `P7-T02T`；本任务保持未完成。
  - 2026-05-05：P7-T02T 已解除 `Task<T>` generic class constructor/layout handoff 缺口；`task_atomic_claim_no_mutex_llvm.scoop` 当前继续推进到 `scoop.core.__task_drive_waiting::<(Int, Any)>` source-slice 中的 `Use(Const(String))` / `source-backed literal span` lowering blocker，仍归入本任务后续修复范围；本任务保持未完成。
  - 2026-05-05：完成剩余默认 build fixture 缺口修复。refactor MIR local slot creation 现在会在 compiler-temporary member access local 上消费 concrete receiver field contract，避免 `Task<Int>` / `Task<(Int, Any)>` 的 `__state` read 因 stale local type drift 失败，同时忽略 non-value member callee refs；materialized generic callable source id 会在 slot creation 后切回实际 materialized callable source，用于 source-backed string/char/int literal 读取且不影响 slot type resolution；refactor effect-neutral lowering 在 `Never` assignment（如 `panic(...)`）后显式发出 `unreachable`，避免 never-return catch arm继续参与 handle completion payload lowering。
  - 新增定向覆盖：`llvm::tests::task_step_o0_build_fixture_uses_concrete_task_state_member_layout` 覆盖单文件 O0 refactor lowering；`commands::build::tests::build_refactor_task_atomic_fixture_lowers_o0_without_legacy_mutex` 覆盖真实 build frontend + refactor LLVM stage + ABI visibility handoff 下的 O0 task atomic fixture。
  - 验证通过：`cargo fmt --all`；`cargo test -p scoopc --lib effect_lowered`；`cargo test -p scoopc --lib llvm::codegen::effect_refactor`；`cargo test -p scoopc --lib llvm::tests`；`cargo test -p scoop build_refactor_task_atomic_fixture_lowers_o0_without_legacy_mutex -- --nocapture`；`cargo run -p scoop -- test --fixtures tests/fixtures/build/extern_enter_native_no_statepoint_writeback.scoop`；`cargo run -p scoop -- test --fixtures tests/fixtures/build/int_literal_default_int_overflow_fail.scoop`；`cargo run -p scoop -- test --fixtures tests/fixtures/build/int_literal_neg_int8_overflow_fail.scoop`；`cargo run -p scoop -- test --fixtures tests/fixtures/build/int_literal_uint8_overflow_fail.scoop`；`cargo run -p scoop -- test --fixtures tests/fixtures/build/task_atomic_claim_no_mutex_llvm.scoop`；`cargo clippy --all-targets -- -D warnings`。

## [DONE] P7-T02U：修复默认 run-pass 暴露的 refactor async/task resume payload ABI 阻塞

- 参考：
  - [`TODO-P5.md`](./TODO-P5.md) P5-T05 / P5-T07b
  - [`TODO-P6-part2.md`](./TODO-P6-part2.md) P6-T02qg / P6-T02qd
  - [`TODO-P6-part3.md`](./TODO-P6-part3.md) P6-T03h / P6-T04
- 背景：
  - 执行 P7-T03 的默认 `cargo run -p scoop -- test` 时，前置回归已修复到 run-pass 阶段；
  - `tests/fixtures/run-pass/async_await_minimal_int_basic.scoop` 当前可完成 LLVM frontend prepare 并开始运行，但只输出 `before` 后以 exit status 1 退出；
  - 之前的直接 blocker 包括 generic `Async.await<T>` resume payload frame slot 在 P6 ABI materialization 中遇到裸 `T`，已确认这类 resume payload 不能按普通 source-value ABI 处理；
  - 将 `ResumePayload` frame slot 改为 surface resume ABI 后，程序仍未完成 async await happy path，说明 task/continuation resume payload 注入、task drive 或 dropped/runtime-error 交界仍有 refactor 默认路径语义缺口。

- 必须实现的内容：
  1. 修复 `Async.await<T>` / `Task<T>` 默认 refactor run-pass 的 resume payload ABI 与 payload 注入路径。
     - generic effect operation 的 shared surface resume payload 可以使用 erased ABI；
     - 但 concrete task body / continuation resume / boundary result local 必须按实际 `T` 还原，不得把 `T` 当作 `Unit`、`Any` 或 fixture 特判；
     - `async { 41 }`、`await t`、`__task_join(task)` 必须真实恢复并返回 `Int`。
  2. 保持 P5/P6 已发布的 continuation composition、surface resume wrapper projection、resume payload binding 与 dropped continuation 语义。
     - 不得回 legacy effect backend；
     - 不得跳过 runtime-error ordinary effect case；
     - 不得通过改弱 fixture 或显式 `legacy` selector 绕过。
  3. 若需要调整 sysroot task helper 以显式闭合不可达路径，必须保持语义等价且不隐藏真实 resume/drive 错误。
  4. 增加或更新定向测试/fixture，覆盖最小 async/await happy path在默认 refactor run-pass 下完成。

- 验证：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/async_await_minimal_int_basic.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/async_await_minimal_int_basic.scoop`
  - 修复后恢复执行 P7-T03 的完整标准矩阵。

- 完成条件：
  - `async_await_minimal_int_basic.scoop` 在省略 selector 的默认 refactor 路径下输出完整 expected stdout 并 exit 0；
  - 修复没有引入 hidden legacy fallback、fixture 降级或 task/continuation 特判。
- 依赖：P7-T02S
- 完成记录：
  - 2026-05-05：修复默认 refactor run-pass 的 async/task resume payload ABI 阻塞。refactor LLVM resume lowering 现在在 task transport `(Int, Any)` resume site 遇到 self-route fallback 时，会按 continuation object type descriptor 动态选择可接收 task transport 的 owner resume adapter；adapter 使用真实 continuation owner 的 frame/resume binding 恢复 concrete resumed local/home，再由当前 resume boundary 的 dispatch plan 消费 owner `Step`，避免把 `__task_drive_waiting` 的 wrapper resume payload 误写成 resume call answer。
  - resume payload 注入现在支持将 task transport 解码到 concrete consumer local，例如 `Async.await<Int>` 的 resumed local `Int`，同时保留普通 identity payload store；没有回 legacy backend、没有修改 fixture 形状、没有把 `T` 擦成 `Unit` / `Any`。
  - 新增 `default_refactor_runs_async_await_task_resume_payload_cli`，覆盖默认 refactor `run tests/fixtures/run-pass/async_await_minimal_int_basic.scoop` 的完整 stdout：`before` / `after` / `41` / `done` / `42`。
  - 验证通过：`cargo fmt --all`；`cargo run -p scoop -- run tests/fixtures/run-pass/async_await_minimal_int_basic.scoop`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/async_await_minimal_int_basic.scoop`；`cargo test -p scoop --test p7_default_pipeline default_refactor_runs_async_await_task_resume_payload_cli -- --nocapture`；`cargo test -p scoop --test p7_default_pipeline`；`cargo test -p scoopc --lib refactor_llvm_continuation_protocol`；`cargo test -p scoopc --lib llvm::codegen::effect_refactor`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_escape_continuation_resume_later_exit.scoop`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_resume_double_resume_exit.scoop`；`cargo check --all`；`cargo clippy --all-targets -- -D warnings`。

## [DONE] P7-T02V：修复默认 run-pass 暴露的 refactor callable-value receiver / pattern binder / FunPtr 阻塞

- 参考：
  - [`TODO-P6-part2.md`](./TODO-P6-part2.md) P6-T02g / P6-T02o
  - [`TODO-P6-part3.md`](./TODO-P6-part3.md) P6-T03e / P6-T03i
  - [`TODO-P7.md`](./TODO-P7.md) P7-T03
- 背景：
  - 继续执行 P7-T03 的默认 `cargo run -p scoop -- test` 时，run-pass 阶段已推进到 callable-value 相关 fixture；
  - `tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop` 当前仍会在默认 refactor 生成的可执行程序中挂起，超过单 fixture 1 分钟限制；
  - 该 fixture 同时覆盖顶层 callable value、顶层 pattern binder、局部 destructuring binder、`when` pattern binder、receiver function value named args 与 top-level `FunPtr` direct call，不能通过显式 legacy、跳过、缩小 fixture 或改弱调用形状绕过。

- 必须实现的内容：
  1. 闭合 receiver function value lowering。
     - receiver lambda 的 `this` 必须在 HIR/MIR/closure codegen 中作为真实 callable 参数发布和消费；
     - 不得把 `this` 留成 `Todo("unbound local ref")`，也不得只在局部 lambda 形状特判。
  2. 闭合 top-level callable value direct-call handoff。
     - 顶层 `val f: (...) -> ... = { ... }` 和顶层 pattern binder 产出的 function value 必须能作为 direct-call source 正确进入 refactor effect-facts 与 value lowering；
     - callable carrier fallback 只能用于已发布的 plain callable/legacy HIR closure alias，不能隐藏 effectful callable 的 step contract。
  3. 闭合 `FunPtr` top-level direct call。
     - 顶层 `FunPtr<F>` direct call 的 surface effect row、named arg mapping、receiver 参数顺序与 native indirect call ABI 必须与局部 `FunPtr` 调用一致；
     - effectful `FunPtr` 仍必须通过显式 effect contract，不得被当作 pure 普通函数调用。
  4. 闭合 callable-value receiver/pattern binder 的 GC root 与 runtime 行为。
     - 从 enum/tuple/pattern binder 取出的 closure/function value 在后续 GC-sensitive receiver/string arg 求值期间必须保持 rooted；
     - 不得依赖 fixture 中“没有 GC”或禁用 root 检查。
  5. 保持 `String.length` / `String.concat` / direct `toString` 等 builtin member/function-value 形状走 refactor-owned lowering，不回 legacy statement lowering。

- 验证：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/receiver_function_value_call_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/top_level_callable_value_call_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_direct_named_call_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_receiver_call_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop`
  - 修复后恢复执行 P7-T03 的完整标准矩阵。

- 完成条件：
  - 上述 callable-value / receiver / pattern binder / FunPtr fixtures 在省略 selector 的默认 refactor 路径下通过；
  - 单个 fixture 不再挂起超过 1 分钟；
  - 未引入 hidden legacy fallback、fixture 降级或 callable-value 特判。
- 依赖：P7-T02U
- 完成记录：
  - 2026-05-05：执行 P7-T03 时暴露该阻塞项，已完成部分前置修复：effect-lowered 单测 phase 引用同步到正式 `effect_lowered`；HIR golden 单测改为消费默认 refactor typed HIR；async string / async fun 显式 return / bool toString / plain `String.concat` / receiver lambda `this` 参数等缺口已有局部修复与定向验证。当前仍阻塞在 `callable_value_pattern_binder_receiver_named_args_basic.scoop` 默认 refactor 可执行程序挂起，需要本任务继续闭合 callable-value receiver/pattern binder/FunPtr 的完整 runtime contract；本任务保持未完成。
  - 2026-05-05：完成 callable-value receiver / pattern binder / `FunPtr` 默认 refactor 阻塞修复。MIR lowering 现在把 `FunPtr<F>` 识别为 callable value，不再把局部 `fp(...)` 降为 `Todo("call callee lowering pending")`；effect facts 可从 `Value(Nominal(FunPtr<F>))` 读取 surface effect row；refactor LLVM MIR lowering 新增 native `FunPtr` indirect-call 路径，并让 `scoop.unsafe.invoke::<...>$overload$...` 先归一到 template FQN 后按同一 ABI lowering，覆盖 receiver 参数、named arg 重排、sret 与 effectful signature 边界。
  - 2026-05-05：修复 top-level callable value direct-call 的 closure 来源。refactor lowering 现在对 `topNamed(...)` / `topPatternF(...)` 直接从 authoritative top-level immutable value 读取 closure object，而不是扫描并复用可能被跳过的 callee temp；同时只物化后续确实作为值使用的 top-level function-value ref，避免把 ordinary direct-call callee 临时重新带入 source slice。
  - 2026-05-05：验证通过：`cargo fmt --all`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/receiver_function_value_call_basic.scoop`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/top_level_callable_value_call_basic.scoop`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_direct_named_call_basic.scoop`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_receiver_call_basic.scoop`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop`；补充 baseline `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_extern_call_basic.scoop`；`cargo test -p scoopc --lib effect_lowered`；`cargo test -p scoopc --lib llvm::codegen::effect_refactor`；`cargo test -p scoopc --lib llvm::tests`；`cargo clippy --all-targets -- -D warnings`。额外 broad guard `cargo test -p scoop --test p7_default_pipeline` 当前仍在 `default_refactor_runs_async_await_task_resume_payload_cli` 暴露 `async_await_minimal_int_basic.scoop` 的 `HandleDispatch` completion payload source 失败，属于后续 `P7-T03` full regression 范围，不作为本任务的 callable-value / `FunPtr` 完成条件。

## [DONE] P7-T02W：闭合 refactor class ctor / object init hidden ordinary effect handoff，解除 P7-T03 run-pass 阻塞

- 参考：
  - [`TODO-P4.md`](./TODO-P4.md) P4-T03 / P4-T04
  - [`TODO-P5.md`](./TODO-P5.md) P5-T03 / P5-T05
  - [`TODO-P6-part3.md`](./TODO-P6-part3.md) P6-T03f / P6-T03g / P6-T04
  - 当前阻塞 fixture：`tests/fixtures/run-pass/class_init_hidden_raise_helper_try_catch_basic.scoop`
- 背景：
  - 继续执行 P7-T03 的默认 `cargo run -p scoop -- test` 时，run-pass 已推进到 class ctor / object init hidden ordinary effect 场景；
  - `helper()` 中的 `Box()` class ctor 触发 `BoomObject` init，object init 内执行 `Raise.raise(RuntimeError.NullAssertionFailed)`；
  - 当前 refactor facts 仍把 `helper` 与 `main` 标为 `NoOutward` / `Plain`，`main` 中对 `helper()` 的 call site 也没有 boundary，因此 hidden runtime error 只激活旧 ordinary effect 状态，却没有通过 refactor `HandleDispatch` 被外层 `try/catch` 消费；
  - 直接现象是程序只输出 `main_before_call` / `helper_before_ctor` / `boom.init`，没有进入 `caught` 分支。

- 必须实现的内容：
  1. 为 class ctor / object init / class init step 中的 hidden ordinary effects 发布 refactor facts handoff。
     - `Rvalue::ClassCtor`、class header `super(...)`、secondary ctor `this(...)` / `super(...)` delegation、property initializer、`init` block、object init 中的 ordinary `Raise<RuntimeError>` 必须能贡献到 caller body facts；
     - `helper()` 这类表面 declared `Pure` 但 class/object init 内 hidden raise 的函数，必须在 facts/solver 中暴露真实 outward case，而不是继续被归为 `NoOutward` plain callable；
     - 不得靠 runtime TLS side effect、process exit、或 HIR-only hidden channel 绕过 P4 facts。
  2. 为 class ctor boundary 发布 late-lowered / LLVM lowering contract。
     - 若 class ctor / object init 可能向外传播 ordinary runtime error，caller 的 class ctor statement 必须有显式 boundary 或等价 published lowering contract；
     - 外层 `try/catch` 的 `HandleDispatch` 必须能消费该 outward case，并恢复到 catch arm；
     - 禁止在 P6 body emitter 现场扫描 HIR class init 或按 fixture 名字猜测。
  3. 收口当前临时暴露的 HIR helper 依赖。
     - 本轮 P7-T03 已为 refactor class ctor named/default/delegation 接通了参数映射和初始化执行，并让 class ctor HIR 初始化表达式中不在 refactor pass-view 内的纯 helper 可按需生成普通 HIR body；
     - 本任务必须将这类 helper reachability / callable body 发布纳入 canonical refactor handoff，或以等价方式证明它不是 hidden legacy fallback；
     - 不得把该路径作为长期 workaround 保留到 P7-T03 完成记录之外。
  4. 保持已有 class ctor run-pass 语义。
     - `class_ctor_named_default_and_delegation_basic.scoop`、`class_secondary_ctor_delegation_this_and_super_basic.scoop`、`class_init_super_ctor_args_eval_order_basic.scoop` 必须继续通过；
     - named/default args 的源码求值顺序与默认参数求值顺序不得退化。
  5. 增加或更新定向测试，覆盖 hidden class/object init ordinary effect 经 helper 被外层 try/catch 捕获。

- 验证：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/class_init_hidden_raise_helper_try_catch_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/class_ctor_named_default_and_delegation_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/class_secondary_ctor_delegation_this_and_super_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/class_init_super_ctor_args_eval_order_basic.scoop`
  - 修复后恢复执行 P7-T03 的完整标准矩阵。

- 完成条件：
  - hidden `Raise<RuntimeError>` 从 class ctor / object init 经普通 helper 传播到外层 `try/catch`，并在默认 refactor run-pass 下匹配 golden；
  - refactor facts / late-lowered / LLVM handoff 不再把该路径误判为 `NoOutward` plain callable；
  - 未引入 hidden legacy fallback、fixture 降级或 runtime-only workaround。
- 依赖：P7-T02V
- 完成记录：
  - 2026-05-05：完成 class ctor / object init hidden ordinary effect handoff 修复。MIR `Rvalue::ClassCtor` 现在发布稳定 `SiteId` 与 hidden init effect row，site id 在函数体其它 call/handle/resume site 分配完成后补齐，避免扰动既有 site 编号；hidden effect row 由 HIR class/object init side table 在 MIR handoff 阶段汇总，覆盖 class property initializer / init block / ctor default 与 delegation 参数 / super ctor args 中触达的 object init `Raise<RuntimeError>`。
  - P4 facts 新增 `ClassCtorSiteEffectFacts`，`helper()` 中 `Box()` class ctor site 现在贡献 `Raise<RuntimeError>` outward case，solver 将 `helper` 从误判的 `NoOutward/Plain` 修正为 effect-step single-case callable；`main` 中 `helper()` call site 随后由 P4 solver 发布为 effect-step call，并被外层 `HandleDispatch` 视为 self-contained handled body case。
  - P5/P6 handoff 新增 class-ctor boundary source / lowering。late lowering 会为 class ctor statement anchor 发布 `ClassCtor` boundary、source classification 与 step emission；refactor LLVM body emitter 消费该 boundary，执行 ctor/init lowering并捕获 object init ordinary outcome，将 `RuntimeError` payload 作为 canonical Step case 交给外层 handle routing，不再依赖 helper 被误判为 plain 后的 TLS 传播早退路径。
  - 新增定向单测 `effect_facts::builder::tests::class_ctor_hidden_object_init_raise_publishes_class_ctor_site_case`，覆盖 class ctor 触发 object init raise 时 P4 发布 `ClassCtor` site facts 且 helper outward case 为 single-case。
  - 验证通过：`cargo fmt --all`；`cargo check -p scoopc`；`cargo test -p scoopc --lib class_ctor_hidden_object_init_raise_publishes_class_ctor_site_case`；`cargo test -p scoopc --lib effect_lowered`；`cargo test -p scoopc --lib llvm::codegen::effect_refactor`；`cargo test -p scoopc --lib llvm::tests`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/class_init_hidden_raise_helper_try_catch_basic.scoop`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/class_ctor_named_default_and_delegation_basic.scoop`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/class_secondary_ctor_delegation_this_and_super_basic.scoop`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/class_init_super_ctor_args_eval_order_basic.scoop`；`cargo clippy --all-targets -- -D warnings`。

## [DONE] P7-T02X：闭合 cross-call escaped continuation member provenance 与 resume-boundary continuation composition，解除 P7-T03 continuation run-pass 阻塞

- 参考：
  - [`TODO-P6-part2.md`](./TODO-P6-part2.md) P6-T02qa / P6-T02q / P6-T02qd
  - [`TODO-P6-part3.md`](./TODO-P6-part3.md) P6-T03h
  - 当前阻塞 fixture：`tests/fixtures/run-pass/continuation_escape_binder_resume_effect_row_runtime_basic.scoop`
- 背景：
  - 继续执行 `P7-T03` 的默认 `cargo run -p scoop -- test` 时，run-pass 推进到 `continuation_escape_binder_resume_effect_row_runtime_basic.scoop`；
  - 已确认 `cell.saved = Some(acceptBoom(k))` 需要跨过 identity helper、`Option.Some` payload path、以及 `start(cell)` 跨函数参数/成员写入，把 readback 的 `cell.saved.Some(k)` 接回 `start` 中 `Ask.current` escape-continuation binder 的 authoritative route；
  - 当前实现已能发布部分 cross-call member provenance，并让 `k.resume(5)` 越过 frontend prepare，但运行仍停在 resume-boundary composition 语义：`Boom.next` arm 中的 `k.resume(7)` 没有正确组合 underlying `start` continuation 与 caller `main` resume boundary，最终没有更新 `cell.resumedTotal` 到期望的 `12`。

- 必须实现的内容：
  1. 完整发布 cross-call stored continuation provenance。
     - 支持 `receiver.member = Some(identity(k))` 这类 enum wrapper + continuation identity helper 的 payload route；
     - 支持 callee 通过参数对象成员保存 continuation，caller 后续从同一对象成员读回并 pattern extract；
     - provenance 必须绑定到 canonical callable / parameter / member identity，不能按 fixture 名字或 local 编号特判。
  2. 为 resume-boundary wrapper outward case 发布 continuation composition contract。
     - 当 `k.resume(...)` 的 underlying route 指向另一个 callable 的 escaped continuation 时，handler arm continuation binder 必须携带“caller resume state + underlying callee continuation”的组合关系；
     - 后续 `binder.resume(...)` 必须先恢复 underlying continuation，再把 owner `Step` 投影回 caller wrapper step，并继续 caller resume state；
     - 不得把 underlying continuation 直接塞进 caller binder，也不得丢弃 underlying continuation 后只恢复 caller state。
  3. 对齐 surface resume wrapper projection 的 owner-step 选择。
     - cross-owner projection 的 owner step 必须来自 underlying route 的 authoritative owner schema；
     - owner/wrapper case 映射必须按 concrete op 对齐，不能假定 case tag 在两个 step schema 中相同。
  4. 保持 runtime-error ordinary effect 与 one-shot continuation 语义。
     - `Boom.next` arm 中的 `k.resume(7)` 不得触发 double-resume；
     - 若 runtime error outward case 发生，仍必须通过现有 `Raise<RuntimeError>` contract 传播/捕获。
  5. 增加或更新定向测试，覆盖 cross-call member-stored escaped continuation 后续 resume 的 happy path。

- 验证：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/continuation_escape_binder_resume_effect_row_runtime_basic.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_escape_binder_resume_effect_row_runtime_basic.scoop`
  - `cargo test -p scoopc --lib effect_lowered`
  - `cargo test -p scoopc --lib llvm::codegen::effect_refactor`
  - 修复后恢复执行 `P7-T03` 的完整标准矩阵。

- 完成条件：
  - `continuation_escape_binder_resume_effect_row_runtime_basic.scoop` 在默认 refactor run-pass 下输出 `40` / `-1` / `12` 并 exit 0；
  - cross-call escaped continuation 不依赖 hidden legacy fallback、fixture 特判、case-tag 偶然相等或 source-shape 猜测；
  - 后续 `P7-T03` 可继续运行 full regression。
- 依赖：P7-T02W
- 完成记录：
  - 2026-05-05：完成 cross-call escaped continuation member provenance 与 resume-boundary continuation composition 收口。`continuation_escape_binder_resume_effect_row_runtime_basic.scoop` 中 `cell.saved = Some(acceptBoom(k))` 的 readback route 已由 late-lowered provenance 接回 `start` 的 Ask handle binder；本轮补齐 resume-boundary outward case 的 continuation composition handoff，使 `k.resume(5)` 触发的 `Boom.next` handler binder 持有“caller resume state + underlying callee continuation”的组合 continuation，而不是直接丢给 underlying continuation 或只保留 caller state。
  - `LateLoweredResumeBoundaryLowering` 现在发布 `continuation_compositions`，dump 会渲染该 contract；LLVM composed resume dispatch 同时消费 call-boundary 与 resume-boundary composition，并按 published `input_step_schema` / case contract 调用 underlying surface resume，再把 owner `Step` 交回 caller boundary dispatch。ABI layout 同步修正 same-owner wrapper projection，保留 resume-boundary site inventory；只有 cross-owner wrapper 才改由 underlying owner route 驱动 owner trampoline。
  - 新增 `refactor_effect_lowered_resume_boundary_continuation_composition_for_cross_call_escape`，覆盖跨函数成员保存 continuation 后再 resume 的 late-lowered contract。验证通过：`cargo fmt --all`；`cargo run -p scoop -- run tests/fixtures/run-pass/continuation_escape_binder_resume_effect_row_runtime_basic.scoop`（输出 `40` / `-1` / `12`）；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/continuation_escape_binder_resume_effect_row_runtime_basic.scoop`；`cargo test -p scoopc --lib refactor_effect_lowered_resume_boundary_continuation_composition_for_cross_call_escape`；`cargo test -p scoopc --lib effect_lowered`；`cargo test -p scoopc --lib llvm::codegen::effect_refactor`；`cargo clippy --all-targets -- -D warnings`。

## [DONE] P7-T02Y：修复 nested escaped-continuation replay 穿过 arm-local handle 后未继续执行 tail 的阻塞

- 参考：
  - [`TODO-P6-part3.md`](./TODO-P6-part3.md) P6-T03h
  - [`TODO-P7.md`](./TODO-P7.md) P7-T03
  - 当前阻塞 fixture：`tests/fixtures/run-pass/effect_escape_continuation_arm_nested_handle_replay_tail_basic.scoop`
- 背景：
  - 继续执行 `P7-T03` 的默认 `cargo run -p scoop -- test` 时，full regression 推进到 nested escaped-continuation replay 场景；
  - 当前程序输出到 `boom_arm` 后以 exit status 1 退出，未继续输出 `after_start`，也未在后续 `k.resume(11)` 后穿过 inner arm-local `try { k.resume(...) } catch ...` 执行 `inner_arm_after_resume` 和 arm tail `resumed + 1`；
  - 该 fixture 要求 non-tail escape arm 的 segmented-body replay 在 inner continuation outward-suspend 后，由 outer continuation resume 正确回到 inner arm 的剩余 source slice，而不是把 inner result 当成整个 arm result 或把 handled completion 提前返回。

- 必须实现的内容：
  1. 修复 refactor late-lowered / LLVM continuation replay 对 nested handle-in-arm 的 continuation composition。
     - inner `Inner.enter` handler arm 中的 `try { k.resume(7) } catch ...` 不是 tail；当 resumed body outward-suspends at `Boom.next()` 时，后续 outer `k.resume(11)` 必须继续执行 `inner_arm_after_resume`、打印 resumed 值，并执行 `resumed + 1`。
     - 不得把 arm-local handle exit 当作整个 outer continuation completion，也不得跳过 arm tail。
  2. 保持已有 cross-call/member-stored continuation composition 语义。
     - 不得回退 P7-T02X 的 cross-call member provenance / resume-boundary composition；
     - 不得依赖 case tag 偶然相等或按 fixture 名字特判。
  3. 保持 runtime-error ordinary effect 与 one-shot continuation 语义。
     - `try/catch` 仍必须捕获 runtime error case；
     - 不得引入 double-resume 或 dropped-continuation 行为回归。
  4. 增加或更新定向测试，覆盖 nested handle inside escape arm 的 non-tail replay。

- 验证：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_arm_nested_handle_replay_tail_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_escape_continuation_arm_nested_handle_replay_tail_basic.scoop`
  - `cargo test -p scoopc --lib effect_lowered`
  - `cargo test -p scoopc --lib llvm::codegen::effect_refactor`
  - 修复后恢复执行 `P7-T03` 的完整标准矩阵。

- 完成条件：
  - `effect_escape_continuation_arm_nested_handle_replay_tail_basic.scoop` 在默认 refactor run-pass 下完整输出 golden：`outer_body` / `inner_body` / `inner_arm_before_resume` / `after_inner` / `7` / `boom_arm` / `after_start` / `18` / `after_boom` / `11` / `inner_arm_after_resume` / `18` / `after_nested` / `19` / `after_resume` / `119` / `done`；
  - nested escaped continuation replay 不依赖 hidden legacy fallback、fixture 降级或 source-shape 特判；
  - 后续 `P7-T03` 可继续运行 full regression。
- 依赖：P7-T02X
- 完成记录：
  - 2026-05-05：修复 nested escaped-continuation replay 穿过 arm-local handle 后未继续执行 tail 的默认 refactor 阻塞。frame lifting 现在把 handle boundary routing 的动态 consume-to-arm 控制流纳入 state liveness；因此当 resumed body 在 nested arm 内 outward-suspend，并由外层 handle arm 消费时，该 arm 后续需要读取的 source local（例如 `cell` 参数）会被提升进 continuation frame，resume owner trampoline 不再用未恢复的空 local 执行 `cell.saved = Some(k)`。
  - 2026-05-05：收紧 surface resume owner trampoline 的 handle consumption 范围。`handle_boundary_action` 在 surface resume entry 中只允许当前 published surface handle site 消费/pending outward case；非当前 surface route 的外层 handle 不再在 trampoline 内直接消费 `Boom.next`，而是把 Step 交回原始 resume boundary，由原函数中的 outer handle 保存组合 continuation 并返回首次 `18`，后续 `k.resume(11)` 再继续执行 inner arm tail。
  - 新增定向覆盖：`effect_lowered::frame::tests::refactor_frame_lifting_captures_locals_used_by_routed_handle_arm`，锁定被 routed handle arm 读取的 source local 必须进入 continuation frame。
  - 验证通过：`cargo fmt --all`；`cargo test -p scoopc --lib refactor_frame_lifting_captures_locals_used_by_routed_handle_arm`；`cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_arm_nested_handle_replay_tail_basic.scoop`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_escape_continuation_arm_nested_handle_replay_tail_basic.scoop`；`cargo test -p scoopc --lib effect_lowered`；`cargo test -p scoopc --lib llvm::codegen::effect_refactor`；`cargo clippy --all-targets -- -D warnings`。

## [DONE] P7-T02Za：闭合 dynamic dispatch ABI schema identity drift，解除 hidden suspend virtual/interface helper 阻塞

- 参考：
  - [`TODO-P7.md`](./TODO-P7.md) P7-T02Z
  - [`TODO-P6-part2.md`](./TODO-P6-part2.md) P6-T02p / P6-T02o
  - [`TODO-P6-part3.md`](./TODO-P6-part3.md) P6-T03e / P6-T03h
- 背景：
  - 执行 `P7-T02Z` 时，hidden init / top-level init ordinary effect 已能通过 `TopLevelRef` hidden-effect boundary 发布并被外层 handle/catch 捕获；
  - 继续验证 hidden suspend helper 系列时，`effect_handle_hidden_suspend_virtual_helper_basic.scoop` 与 `effect_handle_hidden_suspend_interface_helper_basic.scoop` 暴露 dynamic dispatch lowering 的 ABI/schema identity drift；
  - 具体表现包括：单一 interface candidate 被过早折叠为 `KnownInstance` 后缺少 dynamic-invoke carrier contract；body program 与 ABI program 对同一 callable/surface wrapper 的 `StepSchemaId` / owner version key 不一致，导致 completion payload binding、dynamic-invoke return schema、HandleDispatch contract 与 surface-resume owner trampoline 查询漂移。

- 必须实现的内容：
  1. 保持 `Virtual` / `Interface` dispatch 的 canonical dynamic dispatch contract。
     - 即使候选集合只有一个，也不得把 source-level dynamic dispatch 降级成普通 direct call；
     - receiver carrier、vtable/itable slot、ordered args、dynamic invoke return `Step` 必须由 P5/P6 handoff 明确发布并消费。
  2. 修复 body program 与 ABI program 间的 schema identity 映射。
     - P6 body emitter 不得把当前 body program 的 raw `StepSchemaId` 直接拿去查询 ABI program contract；
     - callable body version、plain local-effect control、synthetic call-surface schema、surface-resume wrapper projection 都必须通过 authoritative version key / ABI query 映射到 ABI program schema；
     - 禁止用 site id、root fqn、case tag 偶然相等作为长期替代 identity。
  3. 修复 continuation composition / surface resume owner trampoline 的 owner lookup。
     - `CanonicalFull` / `SingleCase` / wrapper projection 之间的 owner callable、owner step、wrapper step 必须有稳定 handoff；
     - 不能因为 O0/O2 或 body/ABI program schema id 不同而找不到 owner callable 或误查其它 callable 的 frame/completion payload contract。
  4. 保持 completion payload / resume payload binding contract 精确。
     - `boundary result local -> source local -> return` 的显式注入可以作为合法 alias，但必须由 source-slice classification 或等价 handoff 支撑；
     - 不得用宽松跳过 verifier、按 fixture 名字特判、或忽略 frame slot drift 来通过。
  5. 增加或更新定向测试，覆盖 virtual/interface hidden suspend helper 在默认 refactor run-pass 下通过。

- 验证：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_handle_hidden_suspend_virtual_helper_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_handle_hidden_suspend_interface_helper_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_handle_hidden_suspend_member_helper_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_handle_hidden_suspend_helper_object_property_basic.scoop`
  - 修复后恢复 `P7-T02Z` 的剩余 run-pass blocker 验证。

- 完成条件：
  - virtual/interface hidden suspend helper fixtures 在默认 refactor run-pass 下输出 golden 并 exit 0；
  - dynamic dispatch 不依赖 legacy fallback、direct-call 降级、fixture 特判或 raw schema id 偶然一致；
  - `P7-T02Z` 可以继续处理剩余 run-pass 阻塞。
- 依赖：P7-T02Y
- 完成记录：
  - 2026-05-06：完成 dynamic dispatch ABI/schema identity drift 修复。refactor LLVM body emission 现在只用 ABI-visibility late-lowered program 生成 resume packing method 与 surface-resume owner dispatch，避免把 body program 的 raw `ResumeInterfaceId` / `StepSchemaId` 误用于 ABI layout 查询；主 body program 仍只负责自身 callable body lowering，不再混用两套 program id 空间。
  - EntryMain MIR materialization 现在会在可达扫描中保留 source-level `Virtual` / `Interface` dispatch 的 candidate callable bodies，单候选 interface dispatch 仍保持 dynamic dispatch carrier contract，不降级成 direct call；vtable/itable carrier shell 会把 concrete owner callable `Step` 投影到 published canonical dynamic call-surface `Step`，确保 dynamic invoke return schema 与 call boundary contract 一致。
  - 补齐 dynamic call-surface continuation resume adapter：当 projected dynamic `Step` 携带 owner continuation object 时，shared surface resume 会按 continuation object type descriptor 选择 owner surface-resume entry，再把 owner `Step` 投影回 wrapper `Step`，避免 `k8` 等 synthetic dynamic continuation schema 只停留为未定义声明。
  - 新增 `default_refactor_runs_hidden_suspend_dynamic_dispatch_helpers_cli`，覆盖默认 refactor run 下 virtual/interface hidden suspend helper 的 stdout；既有 member/object-property hidden suspend fixtures 保持通过。未引入 legacy fallback、direct-call 降级、fixture 特判或 raw schema id 偶然映射。
  - 验证通过：`cargo fmt --all`；`cargo check -p scoopc`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_handle_hidden_suspend_virtual_helper_basic.scoop`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_handle_hidden_suspend_interface_helper_basic.scoop`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_handle_hidden_suspend_member_helper_basic.scoop`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_handle_hidden_suspend_helper_object_property_basic.scoop`；`cargo test -p scoopc --lib effect_lowered`；`cargo test -p scoopc --lib llvm::codegen::effect_refactor`；`cargo test -p scoopc --lib llvm::tests`；`cargo test -p scoop --test p7_default_pipeline default_refactor_runs_hidden_suspend_dynamic_dispatch_helpers_cli -- --nocapture`；`cargo clippy --all-targets -- -D warnings`。

## P7-T02Z：闭合 P7-T03 剩余默认 run-pass refactor 阻塞，避免 full regression 依赖 legacy 或 fixture 降级

- 参考：
  - [`TODO-P7.md`](./TODO-P7.md) P7-T03
  - [`TODO-P6-part3.md`](./TODO-P6-part3.md) P6-T03g / P6-T03h / P6-T04
- 背景：
  - 恢复执行 P7-T03 时，已完成一批共享 refactor/MIR/LLVM 修复并重新跑通 `cargo test --all`；
  - 逐个 30 秒 timeout 跑 `tests/fixtures/run-pass/*.scoop` 后，仍有一组默认 refactor run-pass 阻塞，不能通过恢复 legacy 默认、跳过 fixture、改弱 golden 或局部特判绕过。

- 必须实现的内容：
  1. 修复 hidden object/top-level init ordinary effect 在默认 refactor 路径下的真实传播。
     - 覆盖 `object_init_raise_try_catch_basic.scoop`、`object_property_init_raise_helper_try_catch_basic.scoop`、`object_value_init_raise_helper_try_catch_basic.scoop`、`top_level_immutable_init_raise_helper_try_catch_basic.scoop` 以及 hidden suspend helper 系列；
     - object/property/top-level init 内的 `Raise<RuntimeError>` 必须由 published facts / late-lowered boundary / LLVM outcome contract 进入外层 `try/catch` 或 `handle`，不得只靠 legacy TLS side effect。
  2. 修复 remaining dynamic member / intrinsic callable-value lowering。
     - 覆盖 String byte/trim/slice/builder 方法、safe member access + extension、operator overload compare/direct matrix、custom iterator/member dispatch、`GC.pin` / `GC.unpin` 等 member-function callee shape；
     - callable member refs 必须通过 canonical direct/dynamic callable contract 或 explicit intrinsic lowering，不得保留 `Todo` / unresolved member backend guessing。
  3. 修复 runtime type-check/cast 与 parameterized interface/class matching 的 refactor frame/layout gap。
     - 覆盖 `type_check_cast_is_as_asq_basic.scoop`、`type_check_cast_generic_class_instantiation_basic.scoop`、`type_check_cast_parameterized_interface_runtime_match_basic.scoop`；
     - 不得通过禁用 `as` failure 的 ordinary `Raise<RuntimeError>` 或绕过 type descriptor/itable parent-chain 检查来通过。
  4. 修复 remaining effect/continuation/GC/task run-pass semantic regressions。
     - 覆盖剩余 effect indirect perform、multi escape/resume/finally、GC continuation/task/manual task fixtures；
     - 保持 P6 已固定的 Step / continuation / one-shot / runtime-error / GC root contracts，不得新增 legacy fallback 或 case-tag 偶然映射。
  5. 保留本轮已完成的通用修复，并补充必要定向测试。
     - 已完成修复包括：builtin nominal scalar ABI、compiler temporary slot inference、static enum unit-variant member access、effect runtime slot intrinsics、masked MIR shifts、mixed float comparison、Float abs/isNaN/isInfinite direct lowering、f-string stale `Any` part handling、tuple-get slot inference、top-level mutable var MIR store、handle return payload source fallback、completion payload coercion。

- 验证：
  - 对每个 run-pass fixture 继续使用单 fixture 30 秒 timeout；
  - 修复后先重跑上一轮剩余失败清单，再恢复执行 P7-T03 标准矩阵；
  - 至少运行：
    - `cargo test --all`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/<fixed-fixture>.scoop`（逐个，30 秒 timeout）
    - 修复完成后继续 `cargo run -p scoop -- test`

- 完成条件：
  - 上述剩余 run-pass blockers 在默认 refactor 主线下通过；
  - 不存在新增 hidden legacy fallback、fixture 降级或 runtime-only workaround；
  - P7-T03 可以重新只聚焦标准 full regression 矩阵最终收口。
- 依赖：P7-T02Za
- 完成记录：
  - 2026-05-06：本轮完成 hidden init / top-level init ordinary effect 的一部分通用修复：`TopLevelRef` 现在可携带 hidden init `EffectRow` 与稳定 MIR site id；P4/P5/P6 会把 object value / top-level immutable value init 中的 `Raise<RuntimeError>` 发布为显式 boundary 并捕获 outcome；仅作为静态成员 namespace receiver 的 `TopLevelRef` 不再提前执行 object init，避免绕过 member hidden-effect boundary。定向通过：`object_init_raise_try_catch_basic.scoop`、`object_property_init_raise_helper_try_catch_basic.scoop`、`object_value_init_raise_helper_try_catch_basic.scoop`、`top_level_immutable_init_raise_helper_try_catch_basic.scoop`、`class_init_hidden_raise_helper_try_catch_basic.scoop`、`effect_handle_hidden_suspend_member_helper_basic.scoop`、`effect_handle_hidden_suspend_helper_object_property_basic.scoop`。
  - 2026-05-06：继续验证 hidden suspend helper 系列时，`effect_handle_hidden_suspend_virtual_helper_basic.scoop` / `effect_handle_hidden_suspend_interface_helper_basic.scoop` 暴露 dynamic dispatch ABI schema identity drift：single-candidate interface dispatch 被折叠后缺少 dynamic-invoke carrier contract，随后 body program 与 ABI program 的 raw `StepSchemaId` / owner version key 漂移导致 completion payload、dynamic-invoke、HandleDispatch 与 surface-resume owner lookup 失败。已新增 prerequisite `P7-T02Za`；本任务保持未完成。

## P7-T03：在 refactor 成为默认主线后运行标准 full regression 矩阵，并修复所有默认路径回归

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P7
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §7.3, §8
- 目标：
  - 在 omission/default=refactor 的前提下，跑通标准 full regression 矩阵；
  - 把所有在新默认路径上暴露出来的编译、测试、spec-fixture、lint 回归修到通过；
  - 证明 refactor 路径已经足以承担仓库默认主线的日常回归集合。

- 必须实现的内容：
  1. 按以下顺序运行并修复标准 full regression 矩阵：
     - `cargo test --all`
     - `cargo run -p scoop -- test`
     - `cargo run -p scoop_tools -- spec-fixtures check`
     - `cargo clippy --all-targets -- -D warnings`
     - 若中间某步失败，必须修复失败原因并重新运行该步；
     - 在完成本任务前，必须再次重跑整个标准矩阵，确保不是“局部绿、整体仍红”。
  2. 修复默认路径回归时，必须优先修正：
     - refactor 主线实现
     - 中立共享模块
     - 默认路径假设错误的测试/fixture/helper
     - 切默认后暴露的 driver/tooling 问题
     - 明确禁止：把失败样本改成显式 `legacy`、缩小 fixture 覆盖、或恢复 legacy 默认值。
  3. 若 full regression 暴露 legacy compare/rollback 入口本身也被误伤，可以修到“显式 legacy 仍可用”；
     - 但修复必须保持 legacy 只作为显式入口，不得重新变成默认或 hidden fallback。
  4. 若 `spec-fixtures check` 暴露的差异来自真正的语义回归，必须修实现；
     - 只有在设计基线已经变更且文档先行更新的前提下，才允许同步更新 generated fixtures；
     - 明确禁止：先改 fixture/golden 掩盖默认路径错误。
  5. 若 `clippy -D warnings` 暴露的 warning 直接来自 P7 的默认切换或回归修复，必须在本任务中清干净；
     - 不允许靠 `allow` 大范围压制、关 lint、或把 warning 转移到 legacy 路径隐藏起来。

- 必须遵从的约束：
  - 禁止把“单个命令偶尔通过”当成 full regression 通过；必须按完整矩阵收口。
  - 禁止通过恢复 legacy 默认值、自动 fallback、或强制 helper 走 legacy 来让矩阵变绿。
  - 禁止修改 `PLAN.md` 来缩小 P7 的回归范围。
  - 禁止将失败归因为“P8 再删旧路径时自然会好”；P7 必须在默认新主线下先把完整矩阵跑通。

- 验证：
  1. 必跑：
     - `cargo test --all`
     - `cargo run -p scoop -- test`
     - `cargo run -p scoop_tools -- spec-fixtures check`
     - `cargo clippy --all-targets -- -D warnings`
  2. 额外 compare smoke，证明 legacy 仍是显式入口而不是默认：
     - `cargo run -p scoop -- --effect-pipeline legacy test --fixtures tests/fixtures/run-pass/minimal_main.scoop`
     - `cargo run -p scoop -- --effect-pipeline legacy build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p7_post_regression_legacy.ll`
  3. 完成记录中必须给出标准矩阵的最终通过摘要：
     - 哪些命令第一次失败
     - 失败的根因类别
     - 修复后最终通过的命令列表

- 完成条件：
  - `cargo test --all`、`scoop test`、`spec-fixtures check`、`clippy -D warnings` 在默认 refactor 主线下全部通过；
  - 回归修复没有通过恢复 legacy 默认值或 hidden fallback 达成；
  - 后续只剩 GC env 全开验证与 P7->P8 handoff 收口。
- 依赖：P7-T02Z
- 完成记录：
  - 2026-05-05：首轮执行已通过 `cargo test --all`，并修复/同步了多个默认 full-regression 前置回归：refactor MIR/LLVM struct literal lowering、unsafe atomic load 与 nested member lvalue、`sizeOf` MIR intrinsic、array builder/Array size/get/set 与 `toInt` compiler intrinsic、过期 effect-facts/effect-lowered/HIR/MIR/mir_refactor generated snapshots、重复的 `effect_lowered_src` 未实现 phase，以及 `sysroot/task.scoop` 中 `__task_join<T>` 的显式不可达收口。当前标准 `scoop test` 继续阻塞在 `async_await_minimal_int_basic.scoop` 的 task/continuation resume payload 运行期语义，已新增 prerequisite `P7-T02U`；本任务保持未完成。
  - 2026-05-05：本轮恢复 P7-T03 后，`cargo test --all` 首轮先失败在 `build_refactor_task_atomic_fixture_lowers_o0_without_legacy_mutex`，已修复 fatal `scoop.core.panic` 在 refactor path 中被误发布为 DynamicFallback effect boundary、以及 `Nothing` handle arm completion source 的不可达路径处理；随后 `cargo test --all` 通过。`cargo run -p scoop -- test` 继续暴露并已修复默认 run-pass 中的 Char/Int/String/Float `hash()` refactor intrinsic、Char `print` / `toString` runtime 路由、MIR f-string stale part type、以及 class ctor named/default/delegation 参数映射和初始化执行问题；定向通过：`char_runtime_textual_basic.scoop`、`stdlib_hash_basic.scoop`、`class_ctor_arg_eval_scope_shadow_free_basic.scoop`、`class_ctor_named_default_and_delegation_basic.scoop`、`class_secondary_ctor_delegation_this_and_super_basic.scoop`、`class_init_super_ctor_args_eval_order_basic.scoop`。
  - 2026-05-05：当前 `cargo run -p scoop -- test` 阻塞在 `class_init_hidden_raise_helper_try_catch_basic.scoop`：class ctor / object init 内 hidden `Raise<RuntimeError>` 没有进入 refactor facts / boundary lowering，`helper` 和 `main` 被误判为 `NoOutward` plain callable，外层 `try/catch` 无法捕获该 ordinary runtime error。已新增 prerequisite `P7-T02W`，本任务保持未完成。
  - 2026-05-05：本轮继续恢复 `P7-T03`，`cargo test --all` 已通过；`cargo run -p scoop -- test` 先后暴露并已部分修复 pure class ctor 误保留 complete-only Step schema、GC debug/runtime intrinsics 在 refactor effect/control body 中被误判为 effectful DynamicFallback、以及 class ctor hidden-effect active 分支未清理失败构造对象临时 root 的问题。随后 full fixture 推进到 `continuation_escape_binder_resume_effect_row_runtime_basic.scoop`；当前剩余 blocker 是 cross-call escaped continuation member provenance 与 resume-boundary continuation composition 未闭合，已新增 prerequisite `P7-T02X`，本任务保持未完成。
  - 2026-05-05：本轮继续恢复 `P7-T03`，已修复多个默认 refactor full-regression 缺口：wrapper outward continuation schema surface inventory、effect-lowered golden 同步、nested handle routing 选择、local aggregate/Option continuation provenance、same-owner wrapper projection合并、refactor sync/thread runtime intrinsics、object property access/support、struct field vs class field member/atomic lvalue、metadata `TypeKind` lowering、隐式 `it` lambda HIR lowering，并同步相关 HIR/effect-lowered golden。定向验证通过：`effect_refactor_direct_handle_resume_emit_llvm.scoop`、`continuation_resume_runtime_error_boundary.scoop`、`dispatch_and_resume_call.scoop`、`continuation_resume_answer_replay_basic.scoop`、`continuation_resume_continuation.scoop`、`continuation_resume_enum.scoop`、`delegated_property_lazy_init_once_basic.scoop`、`std_sync_basic.scoop`、`delegated_property_lazy_thread_safety_publication_multi_init.scoop`、`unsafe_atomic_int_field_lvalue_llvm.scoop`、`delegated_property_map_backed_basic.scoop`、`delegated_property_observable_raise_does_not_poison_mutex.scoop`、`do_block_multiple_trailing_lambda_boundary.scoop`。当前 `cargo run -p scoop -- test` 阻塞在 `effect_escape_continuation_arm_nested_handle_replay_tail_basic.scoop`：nested escaped-continuation replay 在 inner arm-local handle 后未继续执行 arm tail，已新增 prerequisite `P7-T02Y`；本任务保持未完成。
  - 2026-05-06：恢复 P7-T03 后完成一批通用默认-refactor 修复：builtin nominal scalar ABI 映射；compiler-temporary slot 从 concrete rvalue/field/call/tuple-get 推断；static enum unit variant member access；effect runtime slot intrinsics plain lowering；masked MIR shifts；mixed-width float comparison；Float `abs/isNaN/isInfinite` direct lowering；f-string stale `Any` part handling；perform payload source在 raw `Any` 与 emitted concrete payload 间的 contract 对齐；top-level mutable var MIR `StoreTopLevelVar`；handle body completion payload return fallback；completion payload coercion。验证通过：`cargo test --all`；多个定向 run-pass fixture，包括 `fun_call_add_basic.scoop`、`var_assign_basic.scoop`、`enum_value_only_when_basic.scoop`、`extension_property_getter_basic.scoop`、`effect_runtime_slot_abi_basic.scoop`、`int_bitops_shift.scoop`、`string_equality_basic.scoop`、`float_literal_runtime_basic.scoop`、`float_literal_other_contexts_basic.scoop`、`with_update_tuple_nested_single_eval_basic.scoop`、`enum_function_payload_basic.scoop`、`enum_function_payload_boxed_multi_field_basic.scoop`、`enum_variant_non_scalar_payload_basic.scoop`、`entry_main_args_int_exit_basic.scoop`、`effect_handle_return_from_function_basic.scoop`、`effect_handle_return_from_function_finally.scoop`、`effect_handle_return_from_function_nested_handle.scoop`、`top_level_var_threadlocal_global_counter_basic.scoop`、`stdlib_math_basic.scoop`。剩余 run-pass 阻塞已收敛为新 prerequisite `P7-T02Z`；本任务保持未完成。

## P7-T03R：Review 标准 full regression，确认新默认主线已经覆盖常规回归而不是靠 legacy 兜底

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P7，§3，§4
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §8
- 重点：
  - 标准 full regression 矩阵是否确实在 omission/default=refactor 条件下通过；
  - 修复是否都落在 refactor/default 路径或中立模块，而不是靠 legacy hidden fallback；
  - `spec-fixtures check` 与 `clippy` 是否都已纳入最终通过结论。
- 必须检查的产物：
  - P7-T03 的最终命令输出摘要
  - 相关回归修复位置
  - 若有更新的 fixture/golden，检查其理由是否与设计基线一致

- 验证：
  - 重新运行 P7-T03 的全部命令；
  - 额外搜索：
    - `rg "effect[-_]pipeline.*legacy|fallback.*legacy|retry.*legacy" crates tools tests --glob '!target/**'`
  - 要求：
    - 允许命中：compare 测试、文档说明 legacy 临时入口；
    - 不允许命中：为了让标准 full regression 通过而新增的 hidden fallback 逻辑。

- 完成条件：
  - review 能明确说明：默认新主线已经足以承担常规 full regression；
  - 可进入 P7-T04。
- 依赖：P7-T03
- 完成记录：
  - （执行时填写）

## P7-T04：运行 GC env 全开验证，并冻结 P7 -> P8 handoff：legacy 仅剩显式 compare/rollback 入口

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P7，§2/P8，§3，§4
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §7.3, §8
  - 前置实现参考：[`TODO-P6.md`](./TODO-P6.md) P6-T04 / P6-T05
- 目标：
  - 在默认 refactor 主线下跑通 GC env 全开验证；
  - 证明 moving GC / stress / verify-roots 条件下，新的默认主线仍正确；
  - 同时把 P7 -> P8 的边界锁死：P8 只负责删除 legacy 入口与旧主线代码，不再补做语义设计或额外 correctness 修复。

- 必须实现的内容：
  1. 运行并修复 GC env 全开矩阵：
     - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
     - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`
     - 要求：在 omission/default=refactor 下执行。
  2. 修复暴露出的 moving GC / roots / runtime 问题时，仍必须遵守 P6 已固定的语义边界。
     - 禁止把 `Step` ABI、continuation ABI、runtime error ordinary effect、dropped continuation 语义重新打开设计；
     - 修复只能落在：
       - refactor 默认路径实现
       - 共享 runtime / GC / stackmap 基础设施
       - 默认路径假设错误的测试/fixture/helper
     - 明确禁止：恢复 legacy 默认值或依赖显式 `legacy` 参数来回避 GC env 失败。
  3. 验证 explicit legacy 仍只作为短期 compare/rollback 入口。
     - 至少保留一组显式 legacy smoke，证明它仍可进；
     - 但默认 full regression 与 GC env 不得依赖它。
  4. 冻结 P7 -> P8 handoff contract。
     - 必须在代码注释或等价文档实体中明确写出：
       - 默认主线已经是 refactor；
       - legacy selector 仅为短期 compare/rollback 入口；
       - P8 必须删除 legacy selector 分支、旧 effect/continuation lowering 主线、以及只为 legacy 形状存在的临时适配层；
       - P8 不得重新设计 `Step` / continuation / LLVM / GC/runtime 语义，只做删除与再次 full regression。
  5. 如仓库中存在仍被保留到 P8 的 legacy compare/rollback 测试、文档、或入口点，必须在完成记录中给出清单摘要。
     - 不要求此处新建专门清单文件；
     - 但摘要必须足够让 P8 明确知道要删什么、为什么还留着。

- 必须遵从的约束：
  - 禁止把 GC env 失败解释为“P8 删掉旧路径后自然就好”；P7 必须先在默认主线下跑通。
  - 禁止通过减少 `run-pass` / `runtime_gc` 覆盖、标记跳过、或转成显式 `legacy` 来通过本任务。
  - 禁止在 handoff contract 中保留“P8 再看看要不要重新设计某块”的模糊表述；P7 必须明确：P8 只做删除与回归。
  - 禁止在 P7 删除 legacy 参数或 legacy 代码本体；P8 才做删除。

- 验证：
  1. 必跑：
     - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
     - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`
  2. 抽样显式 legacy smoke：
     - `cargo run -p scoop -- --effect-pipeline legacy test --fixtures tests/fixtures/run-pass/minimal_main.scoop`
     - `cargo run -p scoop -- --effect-pipeline legacy test --fixtures tests/fixtures/runtime_gc/gc_handle_roundtrip.scoop`
  3. 在完成记录中必须附：
     - GC env 矩阵最终通过摘要
     - 仍保留到 P8 的 legacy 入口/测试/文档清单摘要

- 完成条件：
  - GC env 全开验证在默认 refactor 主线下全部通过；
  - legacy 只剩显式 compare/rollback 入口；
  - P7 -> P8 handoff 已明确锁定为“删除旧主线并再次 full regression”。
- 依赖：P7-T03R
- 完成记录：
  - （执行时填写）

## P7-T04R：Review P7 阶段退出条件，确认默认主线已切换且 P8 只需删除旧主线并再次 full regression

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P7，§2/P8，§3，§4
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §7.3, §8
- 重点：
  - omission/default 是否已经稳定指向 refactor；
  - standard full regression 与 GC env 全开矩阵是否都在默认主线下通过；
  - legacy 是否只剩显式 compare/rollback 入口，没有 hidden fallback；
  - P8 是否已经可以只做删除与再次 full regression，而无需再补 selector / backend / ABI / runtime 设计。

- 验证：
  - 重新运行 P7-T01 ~ P7-T04 的全部测试与命令；
  - 再跑一次最小默认路径 smoke：
    - `cargo run -p scoop -- build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p7_final_default.ll`
    - `cargo run -p scoop -- run tests/fixtures/run-pass/minimal_main.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/minimal_main.scoop`
  - 额外跑一次显式 legacy smoke：
    - `cargo run -p scoop -- --effect-pipeline legacy test --fixtures tests/fixtures/run-pass/minimal_main.scoop`

- 完成条件：
  - review 能明确说明：P7 已完成“切换主线并执行 full regression”的阶段目标；
  - P8 可以在不重新讨论任何 effect/continuation 设计的前提下，直接进入删除旧主线并再次 full regression。
- 依赖：P7-T04
- 完成记录：
  - （执行时填写）
