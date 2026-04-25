# 本轮执行计划

## 约束说明

- 按要求先记录计划，再执行命令或代码检查。
- 思考与记录使用中文。
- 本轮目标是：先处理最新提交里提到的既有问题；若无，则完成 `TODO.md` 中第一个未完成任务；完成后测试、更新文档、提交 git，然后停止。

## 初始执行步骤

1. 查看最新一次 git 提交信息，确认是否明确提到待修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对当前计划、依赖关系和任务上下文。
4. 结合代码与测试状态判断该任务是否可直接完成：
   - 如果可以直接完成，进入实现。
   - 如果任务过大，先把任务拆成更小的子任务，并更新 `TODO.md` 与 `PLAN.md`，然后只执行拆分后的第一个子任务。
   - 如果发现阻塞该任务的既有缺陷、规格不匹配、缺失特性或回避性实现，则优先修复该问题；若本轮无法直接修复，则把它作为前置任务插入 `TODO.md`，更新 `PLAN.md`，提交后停止。

## 实施阶段计划

1. 阅读相关模块与现有测试，确认正确修改点。
2. 实现任务，避免引入规避性逻辑或与规格不一致的临时方案。
3. 补充或调整测试，覆盖修复路径和回归场景。
4. 运行必要验证：
   - 相关测试
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 如任务相关，再运行对应 fixture / 工具检查
5. 若验证中暴露任何既有问题，立即转为本轮优先事项并修复，或按要求改写 `TODO.md` / `PLAN.md` 后停止。

## 收尾计划

1. 更新 `TODO.md`，标记本轮完成的任务。
2. 更新 `PLAN.md`，反映当前状态、依赖变化和后续顺序。
3. 同步更新本文件，记录关键进展、计划变化和已完成步骤。
4. 检查 git diff，确认只包含本轮需要提交的变更。
5. 使用清晰的提交信息提交。
6. 停止，不继续处理下一个任务。

## 进度记录

- 已完成：初始化计划文件。
- 已完成：检查最新提交与任务列表，确认最新提交标题未声明新的待修复既有问题；`TODO.md` 首个未完成任务为 `T5000b3aR Review：确认 call/ 拆分形成稳定 lowering 边界`。
- 已完成：核对 `crates/scoopc/src/llvm/codegen/call/{abi,dispatch,resume}.rs` 与 `crates/scoopc/src/llvm/codegen/mod.rs` 的函数定义分布。
  - `codegen_call_impl`、`codegen_top_level_fun_call_impl`、vtable/itable dispatch、funptr/function-value call、call arg ABI、ordinary callee resume、top-level effect-call wrapper 等主体实现均已位于 `call/` 子模块；
  - `codegen/mod.rs` 中对应入口现为薄委托，根模块仍保留少量共享数据结构与命名 helper；
  - `call/dispatch.rs` 对 sysroot builtin、class ctor、interface/vtable helper 的依赖是单向委托，未发现新的双向主体耦合；
  - closure/effect 主题只经 `declare_*callee_resume_entry`、`codegen_callee_resume_entry_function`、`call_callee_resume_entry_from_state` 等窄入口消费 call-resume 能力。
- 结论：当前没有发现必须先修复或先插入 `TODO.md` 的新前置缺陷，可以直接完成 `T5000b3aR` 文档与验证收尾。
- 已完成：执行验证命令，`cargo test -p scoopc llvm::`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。
- 已完成：更新 `TODO.md` 与 `PLAN.md`，将 `T5000b3aR` 标记为完成，并将下一条待执行任务切换为 `T5000b3b`。
- 进行中：检查工作区变更并准备提交本轮结果。
