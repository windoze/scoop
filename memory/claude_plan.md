# 当前任务执行思路

用户要求我在动手执行任何命令前，先把完整思路和执行计划写入这个文件，并在后续关键进展时持续更新。

## 目标

本轮只完成 `TODO.md` 里的第一个未完成任务，然后停止。开始正式任务前，还需要先检查最近一次提交里是否提到了任何既有问题；如果有，这些问题全部都在当前范围内，必须先修复，再继续处理 `TODO.md`。

## 约束与工作原则

1. 先检查最近一次提交信息及其上下文，确认是否提到已知问题、回归、遗漏修复或后续待补工作。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 判断该任务是否可以在本轮完整落地。
   - 如果可以：直接实现、测试、更新文档、提交。
   - 如果过大：把任务拆成更小的子任务，更新 `PLAN.md` 与 `TODO.md`，然后只执行拆分后的第一个子任务。
4. 任何计划变化、关键发现、实现完成、测试完成后，都要回写这个文件，便于外部查看进度。
5. 不跳到下一个任务。
6. 尽量保证编译、测试、clippy 无警告；如果时间上只适合跑与改动相关的子集，也要先跑子集定位，再尽可能补齐要求中的全量检查。
7. 不回退用户已有修改；如果工作树里有现存改动，需要先辨认并在不破坏现状的前提下工作。

## 预期执行步骤

1. 查看最近一次提交的提交信息与改动范围，确认是否有“已知问题待修复”。
2. 查看当前工作树状态，识别是否存在未提交改动。
3. 阅读 `TODO.md` 与 `PLAN.md`，理解任务优先级与上下文。
4. 如果最近提交提到待修问题，先定位并修复这些问题，补测试，更新 `PLAN.md` / `TODO.md`（如果需要）。
5. 定位第一个未完成任务，阅读相关代码与测试。
6. 若任务过大，先拆分：
   - 更新 `PLAN.md`
   - 在 `TODO.md` 中把原任务替换/补充为可执行子任务
   - 选择第一个子任务作为本轮目标
7. 实现本轮目标。
8. 运行格式化、相关测试、`cargo clippy --all-targets -- -D warnings`，必要时修复问题直到通过。
9. 更新 `TODO.md` 勾选本轮任务，更新 `PLAN.md` 当前状态，并同步更新本文件记录完成情况。
10. 检查 `README.md` 是否因本轮变更需要同步。
11. 提交 git commit，提交信息应清晰对应本轮任务。
12. 停止，不继续处理下一个任务。

## 当前未知项

- `T0150g` 涉及的三个子语境里，当前哪些已经可用、哪些还缺 fixture 或实现补丁。
- 直接方法调用语境是否会命中现有已知限制（例如字面量后缀解析、method dispatch 特判、数组 rvalue 链式调用等）。
- 多文件 + 插值字符串组合是否有隐藏的跨文件 lowering/codegen 问题。

## 风险点

- 如果最近提交提到的问题范围较大，可能会改变当前优先级，需要先修复这些问题。
- 如果首个未完成任务跨越编译器、运行时和测试夹具，可能需要拆分后再执行。
- `cargo clippy --all-targets -- -D warnings` 可能暴露与本轮改动无关但已存在的警告，需要评估是否纳入当前修复范围。

## 已确认信息（第一次勘察后）

- 最近一次提交 `0d89126b201918bd5c0daf9fc8ca84adc0d01c1b` 只更新了 `memory/claude_plan.md`，提交说明为“`[T0150f] 同步执行记录完成态`”，未提到新的既有代码问题，因此当前无需先插入额外修复分支。
- 当前工作树仅有我刚写入的 `memory/claude_plan.md` 修改，未发现其他未提交改动。
- `TODO.md` 中第一个未完成任务是 `T0150g`：**字面量完整性：多文件 + 插值字符串 + 直接方法调用语境**。
- 结合 `PLAN.md` 当前描述，`T0150g` 已经是从伞型任务中拆出的可执行粒度，本轮先按现有粒度直接做审计与补齐，不再进一步拆分；除非后续读码/试跑发现范围远超预期。

## 本轮最新执行计划

1. 阅读现有 literals / 插值 / 直接方法调用相关实现与 fixtures，确认当前覆盖空洞。
2. 跑最小定向验证，找出真实缺口是“仅缺回归夹具”还是“实现也需要补丁”。
3. 针对 `T0150g` 完成代码与 fixture 补齐。
4. 运行格式化、测试、`clippy`。
5. 更新 `TODO.md`、`PLAN.md` 与本文件，然后提交本轮变更并停止。

## 本轮关键发现

1. **直接方法调用并不是主要缺口**：`42.toString()`、`'A'.toInt()`、`[1,2,3].size()` 在最小手工样例里都可工作。
2. **f-string 的真实缺口有两个**：
   - `Bool` / `Char` 插值在 LLVM codegen 中缺分支，`f"{true}"` 会报 `UnsupportedMainBody(string interpolation expr type)`。
   - 多文件场景下，f-string 的静态文本片段错误地通过 `entry_source().slice(span)` 从入口文件切片，导致 helper 文件中的插值字符串文本损坏；我在新加的 cone fixture 首次跑通时直接复现了这个问题。

## 已完成实现

- `crates/scoopc/src/llvm/codegen/mod.rs`
  - `codegen_interpolated_string` 新增 `Bool` 分支：runtime `scoop_bool_to_string` → `ScoopString.len/data`。
  - `codegen_interpolated_string` 新增 `Char` 分支：复用 `codegen_char_method_to_string`。
  - `codegen_interpolated_string` 的文本片段读取从 `entry_source().slice(...)` 改为 `current_source_slice(...)`，修复多文件 f-string 文本错乱。
- 新增 fixture：`tests/fixtures/run_pass_cone/literal_multi_file_interpolation_direct_basic/**`
  - 覆盖 helper 文件里的 Char / Float / Array 字面量；
  - 覆盖 `Bool` / `Char` / `Float` / array-literal method result 的 f-string 插值；
  - 覆盖字面量直接方法调用。

## 已完成验证

- 定向验证：`cargo run -p scoop -- run tests/fixtures/run_pass_cone/literal_multi_file_interpolation_direct_basic`
- 全量验证：
  1. `cargo fmt --all`
  2. `cargo test --all`
  3. `cargo run -p scoop -- test` → `fixtures: ok (872)`
  4. `cargo clippy --workspace --all-targets --message-format short -- -D warnings`

## 当前状态

- `T0150g` 已完成并已在 `TODO.md` / `PLAN.md` 记录。
- 下一步只剩检查 diff、提交 git commit，然后停止本轮工作。
