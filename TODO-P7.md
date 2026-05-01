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
  - `scoop`、`scoopc`、`scoop_tools`、fixtures harness、测试 helper、以及任何默认构造 `Session` / pipeline config 的入口，在**未显式指定 selector** 时都必须走 refactor；
  - `--effect-pipeline legacy` 必须继续可用，作为短期回滚/比对入口；
  - `--effect-pipeline refactor` 若当前已存在，可继续保留用于显式测试与文档示例；
  - 但“省略 selector”时的行为必须稳定等于 refactor。
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

## P7-T01：翻转顶层 selector 默认值为 refactor，同时保留显式 `legacy` 参数作为短期 compare/rollback 入口

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P7
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §8
  - 前置实现参考：[`TODO-P0.md`](./TODO-P0.md) P0-T01 / P0-T02
- 目标：
  - 把“默认 mode = legacy”翻转为“默认 mode = refactor”；
  - 让 omission-based default 在 `scoop` / `scoopc` / `scoop_tools` / tests / fixtures 中统一生效；
  - 同时保留显式 `--effect-pipeline legacy` 作为过渡期入口，供 P7 compare 与 P8 前的短期回滚使用。

- 必须实现的内容：
  1. 翻转 selector 的默认值。
     - 优先检查并修改以下位置或其等价实现：
       - `crates/scoopc/src/session/` 中 `EffectPipelineMode` / `SessionOptions` / `Session::new()` 默认值
       - `crates/scoop/src/cli.rs`
       - `crates/scoop/src/commands/**/*.rs`
       - `crates/scoopc/src/bin/scoopc.rs`
       - `tools/scoop_tools/**` 或构造 `Session` 的对应位置
     - 要求：省略 selector 时统一进入 refactor。
  2. 保留并验证显式 `legacy` 参数。
     - `--effect-pipeline legacy` 必须继续可解析、可传入 session、可贯穿 dispatcher，并实际进入旧主线；
     - `--effect-pipeline refactor` 若当前已存在，也应继续保留，以便测试与文档显式表述；
     - 明确禁止：通过“删掉 legacy 参数，只剩默认 refactor”来完成 P7。
  3. 更新默认构造入口。
     - 若当前 `Session::new()`、test helper、fixture helper、或 tool helper 默认构造 legacy session，则必须切为 refactor；
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
  - 禁止把默认值只改在某一个入口上，而让其它入口（如 `scoop_tools`、fixtures、test helper）仍默认 legacy。
  - 禁止把“默认 refactor”实现成“先尝试 refactor，失败再自动跑 legacy”。
  - 禁止为了通过 P7 而移除显式 `legacy` compare 入口；P8 才负责删除。
  - 禁止重新把 pipeline mode 渗入低层业务实现函数；selector 仍必须停留在 CLI / session / dispatcher 层。

- 验证：
  1. 新增/更新定向测试，推荐命名：
     - `default_effect_pipeline_is_refactor_*`
     - `explicit_legacy_pipeline_still_available_*`
  2. 运行：
     - `cargo test -p scoop cli`
     - `cargo test -p scoopc session`
     - 若 `scoop_tools` 或 fixtures helper 有单独测试入口，补充对应定向测试
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
  - （执行时填写）

## P7-T01R：Review selector 默认值翻转，确认 omission=refactor 且 explicit legacy 仍是唯一短期回滚入口

- 参考：
  - [`PLAN.md`](./PLAN.md) §0，§2/P7
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §8
  - [`TODO-P0.md`](./TODO-P0.md) P0-T01 / P0-T02
- 重点：
  - omission-based default 是否已统一切到 refactor；
  - `legacy` 是否只剩显式入口，而不是还存在隐藏默认值或 fallback；
  - `scoop`、`scoopc`、`scoop_tools`、fixtures helper、test helper 是否已经一致。
- 必须检查的文件/位置：
  - `crates/scoop/src/cli.rs`
  - `crates/scoop/src/commands/**/*.rs`
  - `crates/scoopc/src/bin/scoopc.rs`
  - `crates/scoopc/src/session/**`
  - `tools/scoop_tools/**` 或其等价位置
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
  - （执行时填写）

## P7-T02：更新默认主线切换后的 driver/fixture/test/docs 假设，并锁定“无显式 selector 时不得悄悄回 legacy”

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
  - （执行时填写）

## P7-T02R：Review 默认主线假设与 hidden-fallback 守护，确认 omission/default 真正代表 refactor 主线

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
  - （执行时填写）

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
- 依赖：P7-T02R
- 完成记录：
  - （执行时填写）

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
