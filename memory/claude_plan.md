# 当前执行计划（初始）

## 说明

按要求先记录计划与决策摘要，再开始任何仓库检查或命令执行。这里记录的是可审计的执行计划、关键判断依据与后续进度更新，不包含逐字原始内部推理。

## 目标

本轮只完成 `TODO.md` 中“第一个未完成任务”，但在此之前必须先检查最新提交是否提到已有问题；如果提到，先修复该问题。执行过程中若发现任何既有 bug、回归、规约不匹配、实现边界缺口或测试暴露出的已有问题，都要立即纳入本轮范围，优先修复或在 `TODO.md` 中插入前置任务后停止。

## 步骤计划

1. 查看最新一次 git 提交信息，确认是否显式提到某个已有问题、已知缺陷、回归或待补修复点。
2. 读取 `TODO.md` 和 `PLAN.md`，识别第一个未完成任务，并理解当前任务排序与依赖。
3. 如果该任务过大：
   - 在 `PLAN.md` 中细化任务；
   - 在 `TODO.md` 中拆分成更小的子任务并重排顺序；
   - 选择拆分后的第一个子任务作为本轮目标。
4. 在正式实现前检查相关代码、测试和规格上下文，识别任何阻塞性的既有问题。
5. 实现目标任务或其前置修复。
6. 运行充分验证：
   - 至少运行与改动直接相关的测试；
   - 若改动影响面较大，再运行更广的测试；
   - 按要求运行无警告检查，例如 `cargo clippy --all-targets -- -D warnings`（若时间和影响面允许则纳入本轮验证）。
7. 更新文档与计划：
   - 勾选 `TODO.md` 中已完成任务；
   - 更新 `PLAN.md`；
   - 按进度更新本文件。
8. 提交 git commit，提交信息描述本轮完成事项。
9. 停止，不继续下一个任务。

## 初始风险与关注点

- 最新提交若只“提到”问题但未修复，需要先处理中断原任务流程。
- 若首个未完成任务依赖当前尚未实现或存在缺陷的语言特性，不允许绕过，必须先把缺口作为前置任务写回 `TODO.md`/`PLAN.md`。
- 若工作树已有未提交改动，需要谨慎避免覆盖用户改动。

## 进度记录

- 已完成：初始计划写入。
- 已完成：检查最新提交 `205c5211`，提交信息仅为 `Update plan`，未显式提到需优先修复的既有问题。
- 已完成：读取 `TODO.md` / `PLAN.md`，确认当前顺序上的首个可执行未完成子任务为 `T4016T1b`：禁止带 effect 的函数类型使用 `as/as?`，收口函数类型转换语义。
- 已完成：阅读 `typecheck/expr/infer.rs`、`typecheck/expr/error.rs`、`llvm/codegen/mod.rs`、`SCOOP_FULL_SPEC.md` 中与 `as/as?`、function type、effect row 相关的主线。
- 已确认的既有问题：
  - 当前 `is_cast_allowed` 仍把几乎所有 `ref -> ref` 显式 cast 一律放行。
  - `Any as? (() -> Int / Pure!)` 这类 cast 能走过前端，但会在 LLVM 代码生成阶段失败为 `unsupported_main_body: type check target type`。
  - 因此函数类型显式 cast 语义本身尚未收口，不能只处理 non-`Pure` effect row 而忽略 pure function target。
- 当前决定：
  - 在本轮内直接完成 `T4016T1b`，并把语义收口为：
    - 显式 `as/as?` 不再定义在函数类型 runtime cast 上；
    - 继续保留现有的 `closed Pure! function -> Any` 擦除门禁；
    - 对 source/target 为 non-`Pure` 函数类型时，给出明确诊断，说明 effect row 不具备 runtime-checkable semantics；
    - 对其余函数类型显式 cast（例如 `Any -> (() -> Int / Pure!)`）同样在前端拒绝，避免把未定义/未实现的 function runtime cast 漏到 LLVM。
- 已完成实现：
  1. 在 `typecheck/expr/infer.rs` 的 cast 路径加入 `check_function_type_cast_boundary`，让 direct function `as/as?` 在前端即被拒绝；`closed Pure! function -> Any` 继续允许。
  2. 在 `typecheck/expr/error.rs` 中新增两个稳定诊断：
     - `scoop::typecheck::function_type_cast_not_supported`
     - `scoop::typecheck::effectful_function_type_cast_not_supported`
  3. 更新 `SCOOP_FULL_SPEC.md`，明确函数类型的显式 runtime cast 不成立，普通函数子类型 / coercion 才是合法路径。
  4. 新增 4 个定向 fixtures，覆盖：
     - `Any -> pure function` 的 `as?` 禁止；
     - non-`Pure` function `as` 禁止；
     - non-`Pure` function `as?` 禁止；
     - `Pure! -> Any` 的显式 cast 仍可通过。
- 已完成验证：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
  - `cargo run -p scoop -- test`
  - `cargo test --all`
  - `cargo run -p scoop_tools -- spec-fixtures check`
  - `cargo clippy --all-targets -- -D warnings`
- 已完成文档收口：
  - `TODO.md` 已将 `T4016T1b` 标记为 `[DONE]`，并补充完成说明、回归列表与下一顺位 `T4016T1c`。
  - `PLAN.md` 已同步为“`T4016T1b` 完成、下一步 `T4016T1c -> T4016T1R -> T4016T2 -> T4016T3`”。
- 当前待完成：
  - 提交本轮改动并停止。
