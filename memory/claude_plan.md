# 执行计划

## 当前目标

本轮只完成 `TODO.md` 中第一个未完成任务，并在完成后停止。

## 已知约束

- 在开始执行仓库命令前，先记录计划，并在关键进展后持续更新本文件。
- 先检查最新一次提交是否提到已有问题；如果有，先修复这些问题，再处理 `TODO.md`。
- 需要读取 `TODO.md`，找到第一个未完成任务。
- 如果该任务过大，需要先拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`。
- 完成实现后，需要运行相关测试，并尽量满足无告警要求，包括 `cargo clippy --all-targets -- -D warnings`。
- 完成后需要更新 `TODO.md`、`PLAN.md`，并提交 git commit，然后停止。

## 执行步骤

1. 检查最新一次 git commit，确认是否提到需要先处理的已有问题。
2. 阅读 `TODO.md`、`PLAN.md`，识别第一个未完成任务及其上下文。
3. 如任务过大，先制定更小的子任务，更新 `PLAN.md` 与 `TODO.md`，并以第一个子任务作为本轮目标。
4. 阅读相关代码、测试与文档，确认实现位置与影响范围。
5. 实现本轮任务，并在必要处补充注释或文档。
6. 运行格式化、测试、lint/clippy，修复发现的问题。
7. 更新 `TODO.md`、`PLAN.md` 与本文件，记录结果。
8. 使用清晰的提交信息创建 git commit。
9. 停止，不继续处理下一个任务。

## 记录方式

- 我不会在这里写出逐字的内部推理草稿，但会持续记录足够复核的决策、步骤、发现的问题和计划变更。
- 每完成一个关键步骤，或计划发生变化时，都会更新本文件。

## 当前进展

### 2026-04-11 初步检查结论

1. 已检查最新一次提交 `92f3a175cac06198be1ffa556d56d03ed3fe6e40`，提交信息为 `[T1818] 实现 Int key 哈希 Set/Map`。
2. 该提交信息本身未提到需要优先修复的既有遗留问题；当前未发现“先处理最新提交中明确点名问题”的前置阻塞。
3. 已定位 `TODO.md` 中第一个未完成任务为 `T1819 [TODO] Ranges 增强：.. syntax sugar / until / for (x in range) integration`。

### 对 T1819 的范围判断

已阅读 `TODO.md`、`PLAN.md` 以及相关代码，结论如下：

- `for (x in range)` 的主体能力其实已经在旧任务中完成：
  - `typecheck/expr/stmt.rs` 已对 `IntProgression` 写入 `ForLoopIterableKind::IntProgression`。
  - `hir/lower/stmt.rs` 已实现 `lower_for_int_progression`，会把 `for (x in prog)` 展开为 while 循环。
- `..` 已经是 parser/AST 里的现有半成品：
  - `ast::BinaryOp` 已有 `RangeInclusive`。
  - `parser/expr.rs` 已把 `Symbol::DotDot` 解析为 `BinaryOp::RangeInclusive`。
  - 但 `typecheck/expr/ops.rs` 目前仅把它放行为 `Any`。
  - `llvm/codegen/mod.rs` 目前对它直接报 `UnsupportedMainBody { kind: "range operator" }`。
- `until` 目前在 stdlib 中尚未实现；它看起来只需要纯 Scoop 扩展函数即可，无需新增 runtime 能力。

### 修订后的本轮实现计划

本轮直接完整实现 `T1819`，不拆分 `TODO.md`：

1. 让 `a..b` 在类型检查和 lowering/codegen 路径上落到现有 `Int.rangeTo(endInclusive, step)` 语义。
2. 在 `stdlib/prelude.scoop` 中补 `Int.until(endExclusive)`。
3. 新增一个综合 run-pass fixture，同时覆盖：
   - `..`
   - `until`
   - `for (x in range)` 集成
4. 跑格式化、测试、clippy。
5. 更新 `TODO.md` / `PLAN.md` / 本文件并提交。

### 已完成的实现

1. **typecheck**：
   - `crates/scoopc/src/typecheck/expr/ops.rs`
   - `BinaryOp::RangeInclusive` 不再返回 `Any`。
   - 现在仅接受 `Int` / 可吸收到 `Int` 的整数字面量，并返回 `scoop.core.IntProgression`。
   - 这一步让 `for (x in 1..5)` 能直接复用已有的 `IntProgression` 专用 `for-in` 路径。

2. **HIR lowering**：
   - `crates/scoopc/src/hir/lower/expr.rs`
   - `lhs..rhs` 现在会在 lowering 阶段改写为：
     - 先把 `lhs`、`rhs` 绑定到合成局部变量；
     - 再调用现有 `scoop.core.rangeTo(start, end, step)`；
     - `step` 通过新的 stdlib helper 派生默认值 `1`。
   - 这样避免了两个风险：
     - 不需要在 LLVM 后端继续保留 `range operator` special-case；
     - 不会重复求值左右端点表达式。

3. **stdlib**：
   - `stdlib/prelude.scoop`
   - 新增内部 helper：`__scoop_range_default_step(sample: Int)`，通过 `sizeOf(sample)` 派生 `1`，避免直接写字面量。
   - 新增：`Int.until(endExclusive)`，语义为 exclusive end。

4. **fixtures / 文档**：
   - 新增 `tests/fixtures/run-pass/stdlib_ranges_enhanced_basic.scoop` + `.stdout`
   - 覆盖：
     - `..`
     - `until`
     - 直接 `for (x in 2..4)`
     - range 两端表达式只求值一次
   - 更新 `tests/fixtures/run-pass/kotlin_ranges_progressions_basic.scoop` 注释
   - 更新 `STDLIB_COMPLETENESS.md` 中 range 能力状态

### 验证结果

- `cargo fmt --all`：通过
- 单独编译并运行新 fixture：
  - `cargo run -p scoop -- build tests/fixtures/run-pass/stdlib_ranges_enhanced_basic.scoop -o /tmp/stdlib_ranges_enhanced_basic`
  - `/tmp/stdlib_ranges_enhanced_basic`
  - 输出与 `stdlib_ranges_enhanced_basic.stdout` 一致，且 `lhs` / `rhs` 各只打印一次
- `cargo test --all`：通过
- `cargo run -p scoop -- test`：通过（`fixtures: ok (902)`）
- `cargo clippy --workspace --all-targets --message-format short -- -D warnings`：通过

### 收尾状态

- 已将 `TODO.md` 中 `T1819` 标记为完成，并补充完成说明。
- 已更新 `PLAN.md`，把 `T1819` 记为 DONE，下一步为 `T1820`。
- 下一步只剩 git diff 复核与 commit。
