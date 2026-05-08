## 当前执行计划

说明：按要求先记录可执行计划与关键判断依据；这里记录的是面向实现的计划与进展，不写逐字内部推理。

1. 先读取 `TODO.md`，按标题是否带有 `[DONE]` 判定首个未完成任务。
2. 检查最近一次提交消息，确认是否存在与该任务直接相关且明确标记为未完成的问题；若有，则将其视为当前任务的一部分或按要求补充为前置任务。
3. 阅读当前任务在 `TODO.md` 中的详细要求、依赖、验证标准，并只围绕该任务收集必要上下文，避免开放式问题排查。
4. 如任务可直接完成：实现改动、补齐/更新测试、运行必要验证，直到任务满足要求。
5. 如遇到阻塞当前任务且必须先修复的真实缺口：在 `TODO.md` 中插入最小必要前置任务，保持当前任务未完成，并更新依赖说明；仅在阶段计划确有变化时修改 `PLAN.md`。
6. 完成后更新 `memory/claude_plan.md` 记录实际执行结果，更新 `TODO.md` 完成标记与完成记录，按仓库约定提交 git commit，然后停止，不继续下一个任务。

## 进展记录

- 已创建本计划文件。
- 已读取 `TODO.md` 与最近一次提交；当前首个未完成任务为 `CG-T07S0a24`（frontend authoritative contract：use-site eff row receiver mismatch）。
- 最近一次提交是 `[CG-T07S0] Reject named args on FunPtr.invoke`，与当前任务不构成直接相关的未完成前置问题，因此继续执行 `CG-T07S0a24`。
- 已复现并确认当前实现上的真实缺口在 fixture/scan 主线：`infer` fixture 之前仍走旧 `typecheck_fixture(...)` 入口，导致没有消费 refactor authoritative typed-HIR 诊断；与此同时，单文件子集扫描对 `tests/fixtures/infer/effects/*.scoop` 的 phase 识别也需要向上回溯到 `infer` 目录。
- 已验证未提交实现：
  - `infer_fixture(...)` 在 refactor 模式下改为加载 typed HIR stage output，从 authoritative frontend 主线获取诊断。
  - `phase_name(...)` 现在会为嵌套单文件子集向上查找真实 phase 目录，保证 `tools/run_fixture_scan.sh --no-build tests/fixtures/infer/effects/...` 仍按 `infer` phase 执行。
  - 新增 `crates/scoop/src/fixtures/mod.rs` 回归测试，覆盖上述两条路径。
- 已通过验证：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/infer/effects/use_site_eff_row_receiver_mismatch_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/infer/effects/use_site_eff_row_default_and_explicit_ok.scoop`
  - `cargo test -p scoop infer_fixtures_use_refactor_typed_hir_diagnostics -- --nocapture`
  - `cargo test -p scoop phase_name_walks_up_to_phase_dir_for_nested_single_file_subset -- --nocapture`
  - `tools/run_fixture_scan.sh --no-build tests/fixtures/infer/effects/use_site_eff_row_receiver_mismatch_is_error.scoop`
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
- 已更新 `TODO.md`：`CG-T07S0a24` 已标记为 `[DONE]`，并补充完成记录与验证命令。
- 已更新 `FAILED_FIXTURES.md`：Round 3 失败数从 7 降到 6，移除了 `tests/fixtures/infer/effects/use_site_eff_row_receiver_mismatch_is_error.scoop`。
- 下一步：提交当前所有未提交文件，提交后停止，不继续下一个任务。
