# 执行计划与进度记录

## 说明

按要求，本文件会在执行过程中持续更新，用于记录：

- 当前目标
- 执行步骤
- 关键决策依据
- 已完成事项
- 遇到的问题与调整

说明：这里记录的是可审阅的计划、依据与进展摘要，不包含逐字的内部思维草稿。

## 初始目标

本轮只完成 `TODO.md` 中**第一个未完成任务**，完成后停止。

在开始具体任务前，先执行以下检查：

1. 查看最新一次 Git 提交，确认提交信息中是否提到需要先修复的既有问题。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 如果该任务过大，则将其拆分为可执行子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 实现当前目标任务。
5. 运行相关测试与质量检查，至少包括与改动相关的测试；若可行，补充 `cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`。
6. 更新 `TODO.md` 与 `PLAN.md`。
7. 提交 Git commit，然后停止。

## 当前假设

- 仓库可能已有未提交改动，因此执行过程中不会回滚非本人修改。
- 需要先阅读仓库现状后才能确定具体任务边界与测试范围。

## 进度

- [x] 创建本文件并写入初始计划。
- [x] 检查最新提交。
- [x] 读取并分析 `TODO.md`。
- [x] 判断首个未完成任务是否需要拆分。
- [x] 实现任务。
- [x] 运行测试与质量检查。
- [x] 更新 `TODO.md` / `PLAN.md`。
- [ ] 提交本轮变更。

## 当前结论

- 最新提交为 `[T0150e] 补齐 when 字面量模式完整性`，提交标题本身未声明一个需要优先修复的遗留缺陷。
- `TODO.md` 中第一个未完成任务为 `T0150f`：字面量完整性：`comptime / const` 语境审计与补齐。
- 当前不需要把 `T0150f` 再拆分到更细子任务：
  - `crates/scoopc/src/comptime/eval.rs` 已支持 `Int/Float/Bool/Char/String/Unit` 字面量；
  - `TupleLit` / `ArrayLit` 已可求值，但当前统一以 `ConstValue::Tuple` 承载；
  - `comptime if` / `comptime for` / `const fun` 的最小解释器路径已存在；
  - 缺口主要在于：回归覆盖不足，以及需要把 `Array/Tuple/Unit` 的当前行为锁定为“稳定可工作”或“稳定报错”。

## 下一步

1. 审计已有 `tests/fixtures/comptime/**` 与 `crates/scoopc/src/comptime/tests.rs` 的覆盖空白。
2. 若发现实现缺口，则先修实现；若实现已可工作，则补齐 fixture / 单测，明确：
   - Int（含 `0x` / `0b`）、Bool、Char、Float 在 `const val` / `const fun` / `comptime if/for` 中的行为；
   - Unit 在 `const fun` 返回值与 `const val` 绑定中的行为；
   - Tuple / Array 在 `const val` 与 `comptime for` 中的行为边界（当前是否等价按 tuple 展示）。
3. 跑针对性测试后，再跑格式化、全量测试与 clippy。

## 已做改动（待验证）

- 在 `crates/scoopc/src/comptime/value.rs` 明确注释：`ConstValue::Tuple` 当前也用于承载 array literal / 常量序列，作为 v0 阶段的稳定外显行为。
- 新增 `tests/fixtures/comptime/literal_const_comptime_matrix.scoop` 与对应 `.comptime`：
  - 覆盖 Int（含 `0x` / `0b`）、Bool、Char、Float 在 `const fun + comptime block/if` 路径中的求值；
  - 覆盖 `Unit` 直接绑定与经 `const fun` 返回；
  - 覆盖 `Tuple` / `Array` 顶层 `const val` 以及在 `comptime for` 中的迭代结果。
- 在 `crates/scoopc/src/comptime/tests.rs` 新增 Rust 单测：
  - 锁定上面同一组语义，避免只依赖 golden fixture。

## 验证结果

- 已通过：
  - `cargo fmt --all`
  - `cargo test -p scoopc literal_matrix_across_const_fun_const_val_and_comptime_paths -- --nocapture`
  - `cargo test --all`
  - `cargo run -p scoop -- test`（`fixtures: ok (871)`）
  - `cargo clippy --workspace --all-targets --message-format short -- -D warnings`

## 当前状态

- `T0150f` 已在 `TODO.md` 标记为完成。
- `PLAN.md` 已同步更新为 DONE，并记录本轮新增的 fixture / 单测 / 行为边界。
- 下一步仅剩：
  1. 复查工作区差异。
  2. 提交 Git commit。
