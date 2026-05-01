# Claude Plan

## Constraints

- 在开始任何代码或命令执行前，先写入本文件。
- 不记录内部私有推理细节；改为记录可审计的执行计划、依据、关键决策与进度更新。
- 本次调用只处理第一个未完成的详细任务；完成后停止。

## Initial Execution Plan

1. 读取 `TODO.md`，把它当作任务索引，而不是任务详情来源。
2. 按 `TODO.md` 引用顺序检查对应的 `TODO-Px.md` 文件，定位第一个“未明确记录为完成”的详细任务。
3. 查看最近提交信息，确认是否存在与该任务直接相关且明确标注未完成的问题；如果有，则将其作为当前任务一部分，或按要求加入详细 TODO 作为前置任务。
4. 阅读当前任务的详细要求、约束、依赖、验证方式，并检查相关代码与测试位置。
5. 直接实现当前任务；如果遇到阻塞当前任务的真实缺陷或缺失能力，不做规避，而是在对应 `TODO-Px.md` 中插入最小必要前置任务，并同步 `TODO.md`。
6. 运行与当前任务直接相关的验证；如果需要，补充或修复测试，直到相关测试、`cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings` 达到任务要求，或明确识别出必须先解决的前置阻塞。
7. 更新文档记录：
   - 在对应 `TODO-Px.md` 中记录任务完成情况；
   - 若任务索引、标题、顺序、文件引用变化，则同步更新 `TODO.md`；
   - 仅当阶段计划发生真实变化时才更新 `PLAN.md`。
8. 使用清晰的 git 提交信息提交本次变更，然后停止，不进入下一个任务。

## Progress Log

- 2026-05-02 当前调用：已重新核对 `TODO.md`、`TODO-P0.md` 与最近提交，确认 `P0-T01` 已完成且已提交；当前第一个未完成详细任务为 review 任务 `P0-T01R`。
- `git log -1 --oneline` 显示最新提交为 `[P0-T01] Add shared effect pipeline session selector`，未显式记录与 `P0-T01R` 直接相关且尚未完成的问题；因此按 `P0-T01R` 原定义继续执行 review。
- 当前执行计划：
  1. 阅读 `P0-T01R` 指定的关键文件与调用路径，确认 `scoop` / `scoopc` 是否共用同一 session pipeline bit，且默认值仍为 `Legacy`。
  2. 搜索仓库中与 pipeline mode 相关的使用点，确认是否残留环境变量、全局静态或其它旁路切换机制。
  3. 重新运行 `P0-T01` 要求的定向测试，并额外运行 `P0-T01R` 的 `dump-ast` smoke（legacy/refactor 各一次）。
  4. 若 review 发现问题，直接在当前任务内修复并重新验证；若未发现问题，则仅更新 `TODO-P0.md` 的完成记录。
  5. 检查是否需要同步 `TODO.md` 或 `PLAN.md`；若无结构变化则不改。
  6. 提交本次 review 结果，完成后停止，不进入 `P0-T02`。
- Review 检查结果：
  - `crates/scoop/src/cli.rs` 与 `crates/scoopc/src/driver_cli.rs` 均以 `scoopc::session::EffectPipelineMode` 作为唯一 mode 语义来源，默认值均为 `Legacy`。
  - `crates/scoop/src/commands/mod.rs` 把 CLI 解析结果统一收口为 `SessionOptions::new(effect_pipeline)`；`dump-ast`、`dump-hir`、`dump-mir`、`dump-ir`、`build`、`run`、`test` 等路径均消费该配置。
  - `crates/scoopc/src/bin/scoopc.rs` 通过 `Session::with_options(cli.session_options)` 构造 session；未发现第二套 selector 或额外旁路。
  - 搜索 `EffectPipelineMode|SessionOptions|effect_pipeline` 后，生产代码命中集中在 CLI、session、driver glue 与 fixture/test glue；未见 parser 或低层业务模块命中。额外搜索 `std::env|thread_local!|OnceLock|static` 后，也未发现与 pipeline selector 相关的环境变量/全局静态偷渡。
- 验证已完成并通过：
  - `cargo test -p scoop --no-default-features cli`
  - `cargo test -p scoopc --no-default-features session`
  - `cargo test -p scoopc --no-default-features driver_cli`
  - `cargo run -p scoop --no-default-features -- --effect-pipeline legacy dump-ast tests/fixtures/parse/hello.scoop`
  - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-ast tests/fixtures/parse/hello.scoop`
  - `cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`
- 两次 `dump-ast` smoke 输出一致，review 未发现需要修复的问题；已回写 `TODO-P0.md` 的 `P0-T01R` 完成记录。下一步：检查工作区状态并提交本任务，然后停止。

- 已创建初始计划文件，下一步开始读取 `TODO.md` 和详细任务文件定位当前应执行任务。
- 已读取 `TODO.md` 与 `TODO-P0.md`，确认第一个未完成详细任务为 `P0-T01`：建立新旧主线共享的 CLI / Session pipeline selector。
- 下一步：检查最近一次提交信息是否包含与 `P0-T01` 直接相关且尚未完成的问题；若无，则开始阅读 `scoop` / `scoopc` 的 CLI 与 `session` 代码并实施改动。
- 最近提交仅为 `Update plan`，未发现与 `P0-T01` 直接相关的未完成 issue 标记。
- 已完成首轮实现：
  - 在 `crates/scoopc/src/session/mod.rs` 中新增 `EffectPipelineMode`、`SessionOptions`、显式 `Session::with_options(...)` 构造入口；
  - 在 `crates/scoop/src/cli.rs` 增加全局 `--effect-pipeline <legacy|refactor>`；
  - 在 `crates/scoopc` 中抽出可测试的 `driver_cli` 参数解析模块，并让 `scoopc` 二进制复用该解析；
  - 将 `dump-ast` / `dump-hir` / `dump-mir` / `dump-ir` / `build` / `run` / `test` 以及其它当前会创建 `Session` 的命令切到统一的 `SessionOptions`。
- 下一步：补齐新增函数签名引发的调用点更新，运行 `cargo fmt` 与定向测试/检查，修复编译或测试失败后再回写 TODO 完成记录。
- 定向验证已完成并通过：
  - `cargo test -p scoop --no-default-features cli`
  - `cargo test -p scoopc --no-default-features session`
  - `cargo test -p scoopc --no-default-features driver_cli`
  - `cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`
- 为满足无 warning/clippy gate，顺手修复了验证过程中暴露的现有机械性 lint/unused 问题；未改变任务语义或 TODO 顺序。
- 已在 `TODO-P0.md` 中回写 `P0-T01` 完成记录。下一步：检查最终 diff，提交当前任务，并停止在本任务边界。
