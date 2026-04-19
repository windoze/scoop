# 本轮执行记录（T4004b）

## 任务结论
- 本轮执行的首个未完成任务是 `T4004b`：打通顶层 `val` pattern binder 的 HIR / LLVM once-init lowering。
- 最新提交 `aac2f9b632aaf0b1430ff0f78e45a8a23c2914c7` 未声明额外需要先修复的遗留问题；执行开始时工作树干净。
- `T4004b` 已完成，`TODO.md` 与 `PLAN.md` 已同步更新；下一项待执行任务为 `T4004R`，本轮不会继续。

## 实现摘要
1. 定位当前断点：
   - 最小 probe `val (a, b) = (1, 2)` 之前在 `cargo run -p scoop -- build ...` 时会报 `scoop::llvm::unsupported_main_body: top-level value ref`。
   - 原因是顶层 pattern `val` 在 HIR 中仍保留为匿名顶层声明，`a/b` 的引用能解析，但后端没有对应的 `top_level_immutable_values` 元数据。
2. 调整 HIR lowering：
   - `lower_file` / `lower_item_into` 现在允许一条顶层 AST item 展开为多个 HIR item。
   - 顶层 pattern `val` 现会被 lowering 成一组顶层 immutable value：
     - 隐藏 subject：承载 initializer 的 once-init；
     - 隐藏 check：仅 variant 路径生成，负责统一复用局部 destructuring 的运行期匹配失败语义；
     - 可见 binder：每个 binder 都成为普通顶层 immutable value，继续复用既有投影 / `when` 提取 helper。
3. 对接现有后端主线：
   - 所有顶层 pattern binder、隐藏 subject、隐藏 check 都进入 `top_level_immutable_values` side table。
   - 因此它们直接复用普通顶层 `val` 已有的 once-init、guard、递归初始化失败检测与跨文件读取路径，没有引入新的 ad-hoc codegen 分支。
4. 补充回归：
   - Rust lowering 单测：验证顶层 variant pattern 会生成隐藏 subject/check，且 binder 初始化先触发 check、再复用 subject 做提取。
   - 单文件 run-pass：覆盖 tuple / struct / enum 顶层 binder 的运行期读取和顶层 initializer 链。
   - cone 多文件 run-pass：覆盖跨文件 binder 读取与多文件 build/run。

## 验证结果
- `cargo test -p scoopc lower_typed_single_source_file_expands_top_level_pattern_into_hidden_subject_and_check -- --nocapture`：通过。
- `cargo test -p scoopc top_level_`：通过。
- `cargo run -p scoop -- test --fixtures <临时 root，仅含 run-pass/top_level_val_pattern_runtime_basic>`：通过，`fixtures: ok (1)`。
- `cargo run -p scoop -- test --fixtures <临时 root，仅含 run_pass_cone/top_level_val_pattern_multi_file_basic>`：通过，`fixtures: ok (1)`。
- 最小 build probe：
  - `cargo run -p scoop -- build <临时>/main.scoop -o <临时>/a.out`
  - 执行产物返回 `3`，说明先前的 `top-level value ref` 构建失败路径已消失。
- `cargo test --all`：通过。
- `cargo clippy --all-targets -- -D warnings`：通过。

## 文档状态
- `TODO.md`：已将 `T4004b` 标记为 `[DONE]`，并记录实现与验证结果。
- `PLAN.md`：已追加本轮完成记录，并把下一项推进到 `T4004R`。

## 剩余动作
- [已完成] 检查最终 diff 与 whitespace。
- [待执行] 提交一次 Git commit，提交后停止。
