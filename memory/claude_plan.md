# 执行计划与进度记录

## 说明
按要求，本文件在任何代码/命令执行前创建，并持续记录本轮的执行计划、关键决策、完成情况与必要调整。

## 初始执行计划
1. 检查最新一次提交，确认提交信息中是否提到需要先处理的遗留问题；若有，则先定位并修复这些问题。
2. 阅读 `TODO.md`，识别第一个未完成任务。
3. 评估该任务是否足够小且可在本轮完整实现。
   - 若可直接完成：进入实现。
   - 若过大或存在明确前置依赖：拆分任务，更新 `PLAN.md` 与 `TODO.md`，然后执行拆分后的第一个子任务。
4. 在开始修改代码前，补充本文件中的实施方案与影响范围。
5. 实现目标任务，确保实现符合规范，不引入临时规避方案。
6. 运行相关验证：
   - 至少运行与改动直接相关的测试。
   - 若任务影响公共路径，补充运行更广泛的测试。
   - 在适用时运行 `cargo fmt` 与 `cargo clippy --all-targets -- -D warnings`。
7. 更新文档与计划：
   - 在 `TODO.md` 中将本轮完成的任务标记为完成。
   - 在 `PLAN.md` 中更新当前状态、后续安排，以及必要的任务依赖调整。
   - 持续更新本文件，记录关键步骤和计划变化。
8. 检查工作区改动，确认只包含本轮必要修改，并以清晰的提交信息创建提交。
9. 停止，不继续处理下一个任务。

## 当前状态
- 已检查最新提交：`c7d8a39377b9d59d5d84525171aeca2f0a1c4a79`，提交信息仅为 `[T3103a] Normalize @Safe/@Unsafe do-vs-closure binding`，提交正文没有额外列出待先修问题。
- 已读取 `TODO.md` 与 `PLAN.md`。
- `T3104` 的文档同步、验证与状态回写已完成，当前处于提交前复核阶段。

## 本轮目标
- 完成 `T3104`，不推进后续任务。

## 当前实施方案
1. 审阅 `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`README.md`、`TODO.md`、`PLAN.md` 以及仓库内相关说明，定位仍使用旧规则的文字、示例与 doctest。
2. 将文档统一到以下规则：
   - 普通局部 block 必须写作 `do { ... }`。
   - bare `{ ... }` 在普通表达式位置属于 closure，并保持 trailing lambda / multiple trailing lambdas 优先级。
   - 局部 annotated block 只接受 `@Safe do { ... }` / `@Unsafe do { ... }`。
   - `@Safe { ... }` 只保留 annotated closure 语义；裸 `@Unsafe { ... }` 应视为无效旧写法。
3. 若 `SCOOP_FULL_SPEC.md` 中代码块变化影响 spec fixtures，则运行 `cargo run -p scoop_tools -- spec-fixtures sync` 与 `cargo run -p scoop_tools -- spec-fixtures check` 并纳入改动。
4. 运行文档任务相关验证，优先覆盖 `spec-fixtures`、`cargo test --all`、`cargo run -p scoop -- test`，并在需要时补 `cargo clippy --all-targets -- -D warnings`。
5. 更新 `TODO.md`、`PLAN.md` 与本文件，标记 `T3104` 完成后提交。

## 风险与检查点
- 规范代码块更新后可能引起生成 fixture 漂移；如出现，必须同步生成并校验，不能只改正文。
- 若扫描过程中发现实现与规范仍不一致的未跟踪问题，需要按规则先回写 `TODO.md`/`PLAN.md`，再停止当前轮。

## 当前进度
- 已确认最新提交未在正文中声明额外遗留问题，无需先插入 pre-fix。
- 已定位本轮任务为 `T3104`，且判断为可在一轮内完成的文档/规范同步任务。
- 已完成首轮文档修改，涉及文件：
  - `SCOOP_FULL_SPEC.md`
  - `SCOOP_RUNTIME.md`
  - `README.md`
  - `TODO.md`
  - `PLAN.md`
- 规范更新内容包括：
  - 明确普通局部 block 只能写 `do { ... }`。
  - 明确 bare `{ ... }` 始终属于 closure / trailing lambda，并补充 multiple trailing lambdas 与 `do` block 的边界。
  - 明确 `@Unsafe do { ... }` 是唯一局部 unsafe block 形式，`@Unsafe { ... }` 必须报错。
  - 保留并强调 `@Safe { ... }` 的 annotated closure 语义。
- 验证阶段已完成，接下来只剩检查 diff、创建提交并停止。

## 验证结果
- `cargo run -p scoop_tools -- spec-fixtures sync`：通过，`spec fixtures: ok (1)`。
- `cargo run -p scoop_tools -- spec-fixtures check`：通过，`spec fixtures: ok (1)`。
- `cargo test --all`：通过。
- `cargo run -p scoop -- test`：通过，`fixtures: ok (1005)`。
- `cargo clippy --all-targets -- -D warnings`：通过。

## 收尾检查
- `TODO.md` 已将 `T3104` 标记为 `[DONE]` 并补充进展/验证记录。
- `PLAN.md` 已记录 `T3104` 完成情况，并将当前执行顺序推进到 `T3201`。
- 当前工作区仅包含本轮相关文档与计划文件改动，未出现额外代码/生成文件漂移。
