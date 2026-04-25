# 执行记录

说明：按要求先记录可共享的执行计划、约束、检查点与后续进展。这里不会写入不可共享的内部推理细节，但会持续更新关键判断、实施步骤、阻塞项和完成状态，便于随时审阅当前进度。

## 当前目标

- 本轮只处理 `TODO.md` 中第一个未完成任务，完成后立即停止。
- 在开始该任务前，先检查最新提交是否提到任何既有问题；如果提到，先修复这些问题。
- 在执行过程中，只要发现任何既有 bug、回归、规格不匹配、不完整实现边界或依赖缺失，都必须先修复，或把它们作为前置任务插入 `TODO.md` 后停止，不能绕过。

## 初始执行计划

1. 检查最新提交信息与工作树状态，确认是否有被最新提交明确提到、但尚未修复的既有问题。
2. 阅读 `TODO.md` 与 `PLAN.md`，定位第一个未完成任务，并确认是否需要拆分成更小子任务。
3. 如果任务过大或被前置问题阻塞：
   - 在 `PLAN.md` 中细化计划；
   - 在 `TODO.md` 中新增或重排前置子任务；
   - 提交这些计划调整并停止。
4. 如果任务可直接执行：
   - 先阅读相关代码、测试和规范文件；
   - 实现任务所需改动；
   - 运行相关测试，并修复过程中暴露的既有问题。
5. 任务完成后：
   - 更新 `TODO.md`，标记该任务完成；
   - 更新 `PLAN.md`，反映当前状态与后续计划；
   - 记录本文件中的关键进展；
   - 提交 git commit；
   - 停止，不进入下一个任务。

## 强制检查项

- 不使用规避方案、局部特判、缩窄测试形状或偏离规格的实现。
- 代码改动后至少运行与变更直接相关的测试；若范围较大，还要补充更广的回归验证。
- 保持编译与 lint 无警告；若当前任务影响范围允许，将运行 `cargo clippy --all-targets -- -D warnings`。
- 若发现 `README.md`、注释、模块组织存在与当前任务直接相关的问题，需要一并修复。

## 进展日志

- 已创建本计划文件，准备开始检查最新提交、任务列表与现有状态。
- 已检查最新提交、`TODO.md` 与 `PLAN.md`：
  - 最新提交 `[T5000b3b] Split intrinsics lowering module` 未在提交说明中额外声明待修既有问题；
  - 当前首个未完成条目为 `T5000b3bR Review：确认 intrinsics/ 拆分没有把 builtin/sysroot 继续堆回根模块`。
- review 过程中发现一个必须先修复的既有边界问题：
  - `crates/scoopc/src/llvm/codegen/mod.rs` 中仍保留 `codegen_string_trim_indent`、`codegen_string_method`、`codegen_to_string_method`、`expr_is_builtin_char`，以及一整组 `Char/Int/Float` builtin lowering helper；
  - 这些不是单纯共享工具，而是上一轮 `intrinsics/builtin.rs` 拆分未完成后继续留在根模块的 builtin lowering 主体实现；
  - 因此当前不能直接把 `T5000b3bR` 标记完成，必须先把这组 helper 真正迁入 `crates/scoopc/src/llvm/codegen/intrinsics/builtin.rs`，再复跑 review 验证。
- 当前执行中的修复计划：
  1. 将上述 builtin helper 从 `codegen/mod.rs` 迁入 `intrinsics/builtin.rs`，并按实际调用面调整可见性；
  2. 清理根模块中的残留定义，确认 `call/dispatch.rs`、字符串插值等调用路径仍通过清晰接口访问 builtin lowering；
  3. 运行定向测试与 lint，确认没有行为回归；
  4. 更新 `TODO.md` / `PLAN.md` / 本文件，记录修复后的 review 结论并提交。
- 修复与验证已完成：
  - 已将 `codegen_string_trim_indent`、`codegen_string_method`、`codegen_to_string_method`、`expr_is_builtin_char`，以及 `Char` / `Int` / `Float` builtin helper 从 `crates/scoopc/src/llvm/codegen/mod.rs` 迁入 `crates/scoopc/src/llvm/codegen/intrinsics/builtin.rs`；
  - `crates/scoopc/src/llvm/codegen/mod.rs` 行数已从 11240 降到 9946，`intrinsics/builtin.rs` 行数增至 2166；对应 builtin helper 定义现只出现在 `intrinsics/builtin.rs`；
  - 已完成验证：
    - `cargo fmt --all`
    - `cargo test -p scoopc llvm::`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
- 当前收尾动作：
  1. 将 `T5000b3bR` 在 `TODO.md` 中标记完成；
  2. 在 `PLAN.md` 与本文件记录 review 结论、残留共享 helper 的后续归属与下一条任务；
  3. 检查最终 diff 并提交本轮变更后停止。
