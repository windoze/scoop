# 本轮执行计划

## 说明

按要求先记录本轮的执行思路与步骤。这里保留可公开的分析摘要，不写内部隐式推理细节；但会完整记录执行顺序、判断依据、发现的问题、以及后续调整。

## 初始目标

本轮目标是：

1. 检查最新提交，确认是否提到需要优先修复的既有问题。
2. 阅读 `TODO.md`，找到第一个未完成任务。
3. 如该任务过大，则先拆分任务，并同步更新 `PLAN.md` / `TODO.md`。
4. 仅执行当前应做的第一个任务。
5. 运行相关测试与质量检查，修复过程中发现的既有问题。
6. 更新 `TODO.md`、`PLAN.md`、本文件，并提交 git commit。
7. 完成后停止，不继续做下一个任务。

## 详细步骤

1. 查看最近一次提交的信息，判断是否显式提到 pre-existing issue。
2. 打开并阅读 `TODO.md`、`PLAN.md`，确认任务顺序与依赖。
3. 如果任务依赖尚未满足，先把阻塞项作为前置任务加入 `TODO.md`，并更新 `PLAN.md`，随后停止。
4. 如果任务可直接实现，则定位相关代码、测试和规范文件，完成修改。
5. 运行最小充分测试，再运行更全面的检查：
   - 相关单测 / 集成测试 / fixture 测试
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 如变更影响格式，则运行 `cargo fmt`
6. 若测试暴露既有缺陷，则该缺陷立即转为当前范围内问题，优先修复或在 `TODO.md` 中前置。
7. 修改完成后：
   - 在 `TODO.md` 标记任务完成，或在被阻塞时重排任务
   - 在 `PLAN.md` 记录当前状态
   - 在本文件补充执行结果
   - 提交 git

## 当前状态

- 已完成：建立本轮计划文件。
- 已完成：检查最近一次提交、阅读 `TODO.md` / `PLAN.md`，确认当前第一个未完成任务是 `T5000dR Review：确认 generic early MIR / ANF template 的语义边界正确`。
- 已完成：初步复核 `crates/scoopc/src/mir/mod.rs`、`crates/scoopc/src/mir/lower.rs`、`crates/scoopc/src/monomorph/lower.rs`，确认当前 MIR 数据结构和 lowering 输入保持 backend-agnostic，没有直接混入 LLVM builder / statepoint / mangled symbol 等后端细节。
- 新发现问题：
  1. `crates/scoopc/src/mir/mod.rs`、`crates/scoopc/src/mir/lower.rs`、`crates/scoop/src/commands/dump_mir.rs` 的顶部说明仍偏向早期“最小 dump/if-when CFG” 口径，没有准确描述当前 generic early MIR / ANF template 的职责边界。
  2. 现有测试覆盖了 monomorphized MIR 会保留 call/perform metadata，但缺少一条直接证明 `dump-mir` 仍输出 generic template、不会提前 materialize `::<T>` 实例的回归测试。
- 已完成：补齐 `crates/scoopc/src/mir/mod.rs`、`crates/scoopc/src/mir/lower.rs`、`crates/scoop/src/commands/dump_mir.rs` 的边界说明，并新增回归单测 `mir::tests::dump_mir_keeps_generic_functions_as_templates_before_monomorphization`。
- review 过程中新增发现并已修复：
  - `crates/scoopc/src/hir/lower/mod.rs` 中 `lower_typed_for_dump(...)` 共享了会 materialize standalone generic fun 实例的 compilation-unit lowering 主线，导致 `dump-mir` 可能把 `pkg.fun::<Int>` 这类 monomorphic item 混进 generic template 输出；
  - 已抽出 `lower_for_compilation_unit_multi_files_internal(...)`，并让 dump 路径显式关闭 generic fun instance materialization，保持 `dump-mir` 与 `dump-ir` 的语义边界分离。
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc mir::tests::dump_mir_keeps_generic_functions_as_templates_before_monomorphization -- --nocapture`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir`
  - `cargo test -p scoopc monomorph::lower -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已完成：更新 `TODO.md` / `PLAN.md`，将 `T5000dR` 标记完成，并记录本轮 review 结论与修复项。
- 待执行：检查变更集并提交 git commit，然后停止。
