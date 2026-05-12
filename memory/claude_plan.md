## 当前执行计划

说明：按安全与协作要求，这里记录可执行计划、关键判断依据、进度与变更，不记录内部私有推理细节。

1. 读取 `TODO.md`，严格按标题是否带有 `[DONE]` 判断完成状态，定位第一个未完成任务。
2. 检查最近一次提交信息，判断是否存在与该任务直接相关且明确未完成的事项；如有，将其视为当前任务范围或在 `TODO.md` 中补充为前置依赖。
3. 阅读当前任务在 `TODO.md` 中的完整要求、依赖、验证标准与完成记录；必要时只读取与该任务直接相关的代码与测试文件，不做开放式问题扫荡。
4. 实现该任务；若遇到阻塞当前任务的真实缺口、回归或规范不匹配，不绕过，改为先修复阻塞问题或在 `TODO.md` 中新增最小前置任务并调整顺序。
5. 运行该任务要求的验证命令，以及必要的回归测试；若任务完成涉及代码质量要求，则补跑 `cargo fmt`、相关测试，必要时运行 `cargo clippy --all-targets -- -D warnings`。
6. 更新文档与任务状态：将当前任务标题加上 `[DONE]`，补全完成记录；仅当阶段级计划发生变化时才更新 `PLAN.md`。
7. 检查工作区改动，按要求提交本次任务相关改动；若是恢复上次未提交的同一任务，则一并纳入提交。
8. 提交后停止，不继续下一个任务。

## 进度记录

- 已写入初始执行计划，下一步读取 `TODO.md` 并定位首个未完成任务。
- 已确认首个未完成任务是 `P0-T01：建立 stable-id 外部 surface 审计脚手架`。
- 已检查最近一次提交：`Update plan`，未发现与 `P0-T01` 直接相关且需要并入当前任务的未完成事项。
- 已读取 `PLAN.md`、`STABLE_ID.md` 与 `crates/scoopc/src/llvm/tests.rs` 的相关段落，当前实现重点收敛为：
  1. 在 `llvm/tests.rs` 增加 object/external symbol 审计 helper，并能按角色区分 runtime/native import、用户 ABI symbol、compiler-private helper。
  2. 增加 stable-id 审计测试骨架，覆盖 top-level function、materialized generic callable、closure body/resume/env、effect helper shell/continuation outcome helper、object init bridge/object init function/top-level init bridge。
  3. 把 `STABLE_ID.md` §11 的 grep 审计点固化为测试常量与审计 helper，扫描 `crates/scoop/src`、`crates/scoopc/src`、`tests/fixtures`。
  4. 在测试注释或 helper 注释中明确：允许变化的是 symbol/linkage/dump/fixture/RTTI/JSON identity 文本；不允许变化的是语义、运行结果、typecheck、effect/continuation/GC 行为。
- 下一步：编辑 `crates/scoopc/src/llvm/tests.rs` 实现上述 helper 与测试，然后运行定向测试、grep 审计和必要的格式化/静态检查。
- 已完成代码实现：
  1. 在 `crates/scoopc/src/llvm/tests.rs` 增加了 object symbol 审计 helper、external symbol 分类器、grep 审计常量与 repo 扫描 helper。
  2. 增加了 `stable_id_audit_*` / `external_symbol_*` 测试，覆盖当前任务要求的 symbol/helper 家族。
  3. 为满足本次任务的 lint gate，补充了几处既有 `clippy::too_many_arguments` 精确豁免并修复了一处 `needless_borrow`。
- 已完成验证：
  1. `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id_audit -- --nocapture`
  2. `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc external_symbol -- --nocapture`
  3. `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
  4. `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
- grep 审计摘要已拿到：`module.add_function(..., None)` 101 命中，`stable_template_symbol_suffix` 7 命中，`source_path.*decl_span` 5 命中，`scoop.lambda$[0-9]+` 2 命中，`__schema[0-9]+` 2 命中，`__k[0-9]+` 4 命中，`t[0-9]+__` 当前 0 命中。
- 下一步：检查 diff 与工作区状态，准备提交 `P0-T01` 完成记录并创建 git commit，然后停止。
