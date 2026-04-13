## 执行计划（公开版）

说明：我不会写出不可公开的完整内部推理，但会在此持续记录可公开的执行计划、关键判断、进度与变更原因。

### 初始步骤

1. 检查最新一次 Git 提交，确认提交说明中是否提到任何已知遗留问题。
2. 如果最新提交提到遗留问题，先定位并修复这些问题，再继续后续任务。
3. 阅读 `TODO.md`，找出第一个未完成任务。
4. 阅读 `PLAN.md`，核对现有计划与任务依赖。
5. 判断该任务是否可以在本轮完整完成：
   - 若可以，直接实现。
   - 若过大或存在前置缺口，则把任务细分，并同步更新 `PLAN.md` 与 `TODO.md`，随后只执行新的第一个子任务。
6. 对本轮目标进行实现、测试、文档更新、提交。
7. 完成本轮后立即停止，不继续处理下一个任务。

### 执行约束

- 优先修复最新提交中明确提到的遗留问题。
- 不接受规避式实现；若遇到规范缺口、实现边界或阻塞问题，必须先在 `TODO.md`/`PLAN.md` 中显式建模并调整顺序。
- 本轮只完成一个任务或一个新拆出的首个子任务。
- 代码修改后需要运行相关验证，目标包含无警告构建与必要测试。

### 进度记录

- 2026-04-14：已创建本计划文件，准备开始检查最新提交与任务列表。
- 2026-04-14：已检查最新提交 `c9b00143e3a064fa366278bfbdd783254bb19e85`，提交主题为 `[T2003r3b1] Route no-suspend handles through unified emitter`，提交说明未额外提及待补遗留问题，因此继续按 `TODO.md` 主线推进。
- 2026-04-14：已读取 `TODO.md` 与 `PLAN.md`，当前第一个未完成任务是 `T2003r3b2`：由 unified emitter 接管 `SingleNonResuming`。
- 2026-04-14：已确认本任务无需再拆分。当前代码里 unified no-continuation 入口只覆盖 `NoSuspendSites`，而 `SingleNonResuming` 仍在 `codegen_handle_expr` 中走旧的单 arm specialized 主路径。

### 当前实施方案

1. 扩展 unified no-continuation 入口分类，使 `SingleNonResuming` 进入统一入口。
2. 为该入口补充最小 plan 校验，确认其只包含 non-resuming arm、且与 simplification 分类一致。
3. 把现有 single non-resuming 旧主路径收口成局部 helper，由 unified 入口调用；保留现有 `Raise.raise` 与 custom single-payload non-resuming 行为。
4. 更新 LLVM 定向单测，验证 single non-resuming representative sample 已被 unified 入口选中，不再返回 `None`。
5. 视需要补充或复用 run-pass fixture，随后执行格式化、定向测试、LLVM run-pass 验收、`clippy -D warnings`。
6. 通过后更新 `TODO.md` / `PLAN.md`、提交 git commit，并停止。

### 本轮结果

- 2026-04-14：已完成代码实现。
  - `UnifiedNoContinuationEntrypoint` 已扩展到 `SingleNonResuming`。
  - `codegen_handle_expr` 中原 single non-resuming 主路径已收口为局部 helper，由 unified no-continuation 入口调用。
  - 新增定向单测与 run-pass fixture，覆盖 single non-resuming + `finally` + nested handle representative sample。
- 2026-04-14：已完成验证。
  - `cargo fmt --all`
  - `cargo test -p scoopc llvm::codegen::effect::tests:: -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_single_nonresuming_finally_nested_handle.scoop`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo test --all`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 2026-04-14：额外确认了一个文档/命令语义细节：
  - `scoop test --fixtures <PATH>` 中的 `--fixtures` 表示 fixture 根目录，不是 phase 过滤器。
  - 因此把 `tests/fixtures/run-pass` 作为 `--fixtures` 参数时，会改变根目录语义，不能稳定表达“只跑 run-pass 子集”。
  - 本轮已经在 `PLAN.md` 中记录该事实，并在完成说明中写入实际执行的验收命令。
