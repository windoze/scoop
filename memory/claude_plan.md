# 本轮执行计划

## 说明

按要求先建立本文件，用于记录本轮的执行计划、关键决策、进度更新与测试结果。
出于信息安全与协作可读性考虑，这里记录的是可审阅的执行方案、观察结论和状态变化，
不写入冗长的私有推理原文。

## 初始步骤

1. 检查最新一次 Git 提交，确认是否提到需要优先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 评估该任务是否足够小且边界清晰：
   - 若可直接完成，则进入实现。
   - 若过大，则先细化 `PLAN.md` 与 `TODO.md`，将首个子任务作为本轮执行目标。
4. 在实现过程中，如果通过测试、审查或探查发现任何既有缺陷、规格不匹配或未完成边界：
   - 立即优先修复；
   - 若当前无法直接修复且它是前置依赖，则先更新 `TODO.md` / `PLAN.md` 记录前置任务，再停止。
5. 完成目标后执行相关测试与质量检查。
6. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况。
7. 提交 Git commit，然后停止，不继续处理下一项任务。

## 进度记录

- 状态：已完成最新提交、`TODO.md`、`PLAN.md` 的初步检查。
- 结论：
  - 最新提交说明未声明需要优先修复的既有问题。
  - 本轮首个未完成任务已确认为 `T5000b4cR Review：确认 effect/state-machine emitter 上下文边界成立`。
- 当前执行计划：
  1. 审查 `crates/scoopc/src/llvm/codegen/mod.rs` 中新的 effect lowering 上下文定义与 helper。
  2. 审查 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`、`effect/mod.rs`、`call/resume.rs`、`closure/mod.rs` 等调用点，确认 effect 专属状态是否已集中、是否仍存在大段直接操作外层主上下文的路径。
  3. 若审查发现既有缺陷或边界泄漏，立即修复并补充验证。
  4. 运行相关测试与质量检查。
  5. 更新 `TODO.md`、`PLAN.md` 与本文件，记录 review 结论并提交。
- 当前下一步：读取并核对 effect lowering 上下文与 state-machine emitter 的实现细节。

## 审查中发现的问题

- `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 仍残留 4 处手工保存/恢复
  `function_cx.return_context` 与 `function_cx.current_fun_return_ty` 的模式；
- 这些位置中间夹着会通过 `?` 提前返回的调用，一旦出错就不会执行恢复逻辑；
- 这既是 effect emitter 边界未完全收口的问题，也会让 `MainCodegen` 在错误路径上留下不一致的函数级状态。

## 修复计划

1. 在 `MainCodegen` 中补一个统一 helper，负责临时安装“禁用普通 return context + `current_fun_return_ty = Never`”语义，并保证无论成功还是失败都恢复。
2. 把 `state_machine_emitter.rs` 中 4 处手工保存/恢复改为该 helper。
3. 重新审查剩余 effect emitter 调用点，确认 effect 专属状态与 generic function/body 状态边界已经清晰。

## 当前结果

- 已完成代码修复：
  - 在 `crates/scoopc/src/llvm/codegen/mod.rs` 中新增 `with_local_never_return_semantics(...)`；
  - 已将 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 中 4 处手工保存/恢复
    `function_cx.return_context` / `current_fun_return_ty` 的路径改为统一 helper。
- 已完成复核结论：
  - effect emitter 专属状态已集中在 `EffectLoweringCodegenCx` 及其子上下文中；
  - `effect/mod.rs`、`call/resume.rs`、`closure/mod.rs` 与 `state_machine_emitter.rs`
    对 effect 专属状态的访问均经 getter / `with_*` helper / `take+restore` 入口进行；
  - 剩余 `FunctionBodyCodegenCx` 与 `current_source_id` 仍属于 backend 的 generic lowering /
    function-body 上下文，不属于 effect emitter 专属状态。
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc llvm::`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - 以上全部通过。
- 文档状态：
  - `TODO.md` 已将 `T5000b4cR` 标记为完成；
  - `PLAN.md` 已补记 review 结论，并把下一条待执行任务推进到 `T5000b4R`。
- 当前下一步：检查工作区差异并提交本轮更改，然后停止。
