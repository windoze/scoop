## 执行思路摘要

说明：我不会写入不可公开的逐字内部推理，但会持续维护一份足够详细的行动计划、判断依据摘要、执行进度和变更记录，便于随时审阅。

当前目标：严格按仓库根目录 `TODO.md` 的优先顺序，只完成“第一个未完成任务”并停止。在开始任务前，先检查最新提交是否提到任何既有问题；如果提到，先修复这些问题。执行过程中遇到任何既存缺陷、规格不匹配、实现边界缺失、测试回归或依赖缺口，都必须立即纳入当前范围，优先修复，或在 `TODO.md` 中插入前置任务后停止。

## 分步计划

1. 检查最新一次 Git 提交信息与当前工作树状态，确认是否已有被明确提及但尚未解决的问题。
2. 阅读 `TODO.md`，定位第一个未完成任务；同时阅读 `PLAN.md` 获取当前规划上下文。
3. 判断该任务是否可在一次迭代中完整落地：
   - 若可直接完成，继续实现。
   - 若过大或依赖缺失，则把任务拆解为更小子任务，更新 `PLAN.md` 与 `TODO.md` 的顺序和依赖，并在本次只处理新的第一个子任务。
4. 在实现前补充必要上下文：
   - 阅读相关代码、测试、规范或最近改动。
   - 运行最小必要命令复现现状。
   - 如果在探查中发现任何既有问题，先修复问题或把其登记为当前任务的前置任务。
5. 实现当前目标任务，保证改动符合现有架构和规范，不引入绕过式方案。
6. 运行相关验证：
   - 先跑与改动直接相关的测试。
   - 再按需要运行更广验证，至少覆盖任务影响面。
   - 按要求检查格式、lint 和警告，必要时运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings` 及相关测试。
7. 更新文档与任务状态：
   - 在 `TODO.md` 中标记该任务完成，或在受阻时调整依赖顺序并保留为未完成。
   - 在 `PLAN.md` 中记录当前状态、拆分结果、阻塞原因或后续顺序。
   - 持续同步本文件，记录关键判断、计划调整与完成情况。
8. 检查 `git diff`，确认只包含预期修改，不回退他人改动。
9. 提交本次变更，提交信息应清晰描述任务编号与内容。
10. 停止，不继续执行后续任务。

## 进度记录

- 已创建本计划文件，下一步将检查最新提交、工作树、`TODO.md` 与 `PLAN.md`。
- 已检查最新提交 `a489b034 [T5000b2] Extract shared compilation-unit codegen context`，提交正文无额外 pre-existing issue 说明。
- 已确认当前首个未完成任务为 `T5000b2R Review：确认 MainCodegen 构造边界已开始从“巨型输入包”收口`。
- 已完成 review 取证：
  - `crates/scoopc/src/llvm/emit.rs` 中 `CompilationUnitCodegenCx::new(CompilationUnitCodegenInputs { ... })` 仅保留 1 个编译单元构造入口；
  - `fresh_main_codegen()` 统一承接顶层声明、reachable top-level function body 发射与入口 `main` exit-code lowering；
  - `fresh_child_codegen()` 已覆盖 effect-call wrapper、top-level immutable init、closure body lowering、object init lowering 4 条 child/nested 路径；
  - 实现代码中已无残留 `MainCodegenInputs { ... }` 手写构造。
- 已完成验证：
  - `cargo test -p scoopc llvm::`
  - `cargo clippy --all-targets -- -D warnings`
  - 结果均通过。
- 当前结论：
  - 未发现需要先于 `T5000b3` 插入的新前置缺陷任务；
  - 共享编译单元输入与函数级局部状态已经开始分离，但更深层的 cache / effect emitter 上下文分层仍属于后续 `T5000b3` / `T5000b4` 范围。
- 下一步：更新 `TODO.md` / `PLAN.md`，将 `T5000b2R` 标记完成，并把待执行任务切换为 `T5000b3`，随后检查差异并提交。
