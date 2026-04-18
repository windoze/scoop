# 本次执行计划（高层摘要）

## 目标
- 先检查最新一次 Git 提交是否提到任何遗留问题；如果有，优先修复这些问题。
- 读取 `TODO.md`，定位第一个未完成任务。
- 如果该任务过大或存在前置依赖缺口，则先把任务拆分并更新 `PLAN.md` / `TODO.md`。
- 只完成一个任务（或一个新拆出的首个子任务），完成后测试、更新文档、提交 Git，并停止。

## 约束与执行原则
- 不绕过规范，不接受临时性 workaround。
- 若发现规范不匹配、缺失特性、已有缺陷或被最新提交提到的未解决问题，必须先修复或将其显式前置到 `TODO.md`。
- 任何关键进展、计划调整、阻塞原因，都要同步更新本文件。
- 所有输出与记录使用中文。

## 预计步骤
1. 查看最新提交信息与改动摘要，确认是否提到已知问题或 TODO。
2. 读取 `TODO.md` 与 `PLAN.md`，确定当前第一优先级任务。
3. 结合代码现状评估任务范围；如过大，则拆解为更小子任务并更新计划文件。
4. 阅读相关源码、测试、规范或夹具，定位实现入口与风险点。
5. 实现任务所需修改，必要时补充/调整测试。
6. 运行与任务相关的验证：
   - 优先运行最小相关测试集；
   - 再运行必要的更大范围测试；
   - 若改动涉及通用路径，补跑 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`（视耗时和改动范围裁剪，但要保证充分验证）。
7. 更新 `TODO.md`、`PLAN.md`、本文件，记录完成情况或依赖调整。
8. 检查工作区改动，使用清晰的提交信息创建 Git commit。
9. 停止，不继续处理下一个任务。

## 计划文件更新规则
- 完成“检查最新提交”后，补记检查结论。
- 确认首个目标任务后，补记任务编号/标题与是否需要拆分。
- 开始改代码前，补记将要修改的模块与测试策略。
- 测试完成后，补记测试命令与结果。
- 提交前，补记最终完成状态与提交说明。

## 当前进展
- 已检查最新提交 `75776dcb4a21a88ab390cc458602894e7eb8373d`（`[T4004a1] 接通顶层 pattern 注解静态路径`），提交说明本身未额外声明需要先修复的遗留问题。
- 已确认 `TODO.md` 当前首个可执行未完成条目为 `T4004a2`：为顶层 `val` pattern binder 补齐 initializer 驱动推断与跨文件类型可见性。
- 目前不需要再拆分任务；计划直接实现该子任务并完成验证。

## 即将进行的代码改动
- 放宽 `typecheck/headers.rs` 中“顶层 pattern binding 必须显式整体类型注解”的门禁，只保留对普通顶层命名 `val` 缺注解的约束。
- 在 `typecheck/type_env.rs` 保留编译单元文件 AST 视图，供跨文件顶层值类型收集时复用。
- 在 `typecheck/expr/collect.rs` 实现“跨文件顶层值类型表”收集：
  - 先收集显式类型注解路径；
  - 再对无整体注解的顶层 pattern binding 做 initializer 驱动推断；
  - 让其它文件对这些 binder 的静态引用可见。
- 在 `typecheck/expr/entry.rs` 调整顶层 initializer 检查逻辑，使顶层 pattern binding 在无整体类型注解时复用 initializer 推断结果。

## 测试计划
- 先跑定向 `cargo test -p scoopc top_level_`。
- 再跑 `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`。
- 补跑 `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck_multi`。
- 最后跑 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。

## 已完成的实现
- 已放宽 `typecheck/headers.rs`：顶层 `val` pattern binding 不再在 header phase 强制整体类型注解，改由 initializer typecheck 负责推断。
- 已扩展 `typecheck/type_env.rs`：记录编译单元文件 AST，供跨文件顶层值类型收集复用。
- 已重写 `typecheck/expr/collect.rs` 中的顶层值类型收集逻辑：
  - 先跨文件收集显式类型注解的顶层值；
  - 再对无整体注解的顶层 pattern binding 做迭代推断；
  - 让推断出的 binder 类型进入跨文件可见的 top-level value type 表。
- 已调整 `typecheck/expr/entry.rs` 顶层 initializer 检查：
  - 有整体类型注解时仍按 expected-type 校验；
  - 无整体注解的顶层 pattern binding 直接以 initializer 推断类型驱动 binder 类型分发。
- 已更新回归：
  - 删除旧的 `tests/fixtures/typecheck/top_level_val_pattern_missing_type_is_error.scoop`；
  - 新增同文件推断回归 `tests/fixtures/typecheck/top_level_val_pattern_inferred_same_file_ok.scoop`；
  - 新增多文件回归目录 `tests/fixtures/typecheck_multi/top_level_val_pattern_inferred_cross_file/`。

## 已完成验证
- `cargo test -p scoopc top_level_`：通过。
- `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`：通过，`fixtures: ok (329)`。
- `cargo run -p scoop -- test --fixtures /tmp/t4004a2-typecheck-multi`：通过，`fixtures: ok (4)`。
- `cargo test --all`：通过。
- `cargo clippy --all-targets -- -D warnings`：通过。

## 当前状态
- `T4004a2` 已完成，可以在更新 `TODO.md` / `PLAN.md` 后提交。
- 下一未完成任务为 `T4004b`，本次不会继续执行。
