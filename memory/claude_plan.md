# 执行计划与进度记录

## 说明

按要求先落盘计划。这里记录的是可共享的推理摘要、执行步骤、风险判断与进度更新，不包含原始内部思维链。

## 当前目标

完成 `TODO.md` 中第一个未完成任务，并在完成后停止。

## 初始执行步骤

1. 检查最新一次 Git 提交的信息，确认是否提到任何已知遗留问题。
2. 若最新提交提到遗留问题，先定位并修复这些问题，再继续后续步骤。
3. 阅读 `TODO.md`，找出第一个未完成任务。
4. 阅读 `PLAN.md`，确认该任务的上下文、依赖和当前计划状态。
5. 判断该任务是否过大：
   - 若可直接完成，则进入实现。
   - 若过大，则将其拆分为更小的子任务，更新 `PLAN.md` 与 `TODO.md`，并执行拆分后的第一个子任务。
6. 在实现前检查相关代码、测试和规格，避免用临时绕过方案掩盖真实缺陷。
7. 实现该任务。
8. 运行相关测试，并至少补充或更新必要测试；同时检查编译、格式化与告警情况。
9. 更新文档与计划：
   - 在 `TODO.md` 中标记该任务完成，或若受阻则按依赖顺序重排。
   - 在 `PLAN.md` 中记录当前状态、拆分结果、阻塞原因或后续顺序调整。
   - 在本文件中同步记录关键进展和计划变更。
10. 使用清晰的提交信息提交本次变更，然后停止。

## 风险与约束

- 不接受规避式实现、夹带兼容层或仅为夹具通过而写的特判。
- 若发现规格不匹配、缺失语言特性、运行时缺陷或诊断错误，必须先把问题转化为 `TODO.md` 中的显式任务，并调整任务顺序。
- 若工作树存在与当前任务无关的脏改动，不回退它们，只在必要范围内谨慎协作。

## 进度记录

- 已创建本文件并写入初始计划，尚未开始仓库检查。
- 已检查最新提交 `025e229edd3ce9f82d4c600f4eaa750b8000c439`（`[T2003c0c2d2a] Support sibling non-resuming multiple escape direct`）。
- 最新提交信息未直接声明需要先修复的遗留缺陷；当前未发现“提交信息里已点名但未修”的前置问题。
- 已读取 `TODO.md` / `PLAN.md` / `README.md`。
- 已确认 `TODO.md` 中第一个未完成任务是 `T2003c0c2d2b`：`多个 escape-continuation arms + finally`，范围限定为 pure direct、top-level direct、single-site。
- 下一步：读取该任务的详细描述、依赖与前一个已完成任务 `T2003c0c2d2a` 的实现上下文，判断是否需要再拆分。
- 已完成上下文核对：该任务不再拆分，直接在 `crates/scoopc/src/llvm/codegen/effect/multi_escape.rs` 上实现。
- 语义基线已确认：沿用既有 `T1609` 的 escape-continuation `finally` 规则，即 `finally` 在 handle 表达式完成时执行一次；后续 continuation `resume(...)` 进入的 step trampoline 不重复执行 `finally`。
- 计划中的代码改动：
  1. 去掉 pure multiple-escape direct 路径对 `handle.finally` 的统一 early reject，但继续保留“sibling non-resuming + finally”边界诊断。
  2. 在主 handle 路径新增 `finally_bb` / `finally_unwind_bb`，让初始 body 前缀与首个命中的 escape arm 在正常完成或 `Raise.raise` 向外传播时都经过 cleanup。
  3. 不改 step trampoline 的 `finally` 语义，保持与既有单 arm multi-perform escape-continuation 一致。
  4. 把旧的 pure-finally build 负例改成正例 run-pass，并新增 outward-raise run-pass 与一个新的 sibling+finally build 负例。
- 代码实现已完成：
  - `codegen_handle_expr_multiple_escape_top_level_direct` 已支持 pure direct top-level direct single-site 的 `multiple escape arms + finally`。
  - 初始 body 前缀与首个 escape arm 现在会通过 `finally_unwind` 处理向外传播的 `Raise.raise`。
  - `step trampoline` 未引入新的 `finally`，保持 `T1609` 的既有一次性 cleanup 语义。
- 新增/调整回归：
  - run-pass：`effect_multi_escape_multi_arm_with_finally`
  - run-pass：`effect_multi_escape_multi_arm_with_finally_raise`
  - build：`effect_multi_escape_multi_arm_with_nonresuming_finally_is_error`
- 定向验证已通过：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_escape_multi_arm_with_finally.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_escape_multi_arm_with_finally_raise.scoop`
  - `cargo run -p scoop --features llvm -- build tests/fixtures/build/effect_multi_escape_multi_arm_with_nonresuming_finally_is_error.scoop -o /tmp/multi_escape_nonresuming_finally.out`
- 全量验证已通过：
  - `cargo fmt --all`
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 文档状态已更新：
  - `TODO.md` 已将 `T2003c0c2d2b` 标记为完成，并澄清 `finally` 的一次性语义。
  - `PLAN.md` 已记录本轮实现、回归与新的下一步任务 `T2003c0c2d2c`。
- 下一步：检查最终 diff，提交 git commit，然后停止。
