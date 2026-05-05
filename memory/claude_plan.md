# 执行计划

## 当前目标

- 当前详细任务：`TODO-P7.md` 中的 `P7-T02T`。
- 目标：发布并消费 generic class instance layout handoff，修复 `Task<T>` constructor 在默认 refactor LLVM path 中使用未实例化 generic declaration field layout 的问题。
- 完成后更新 `TODO-P7.md` 与 `TODO.md`，验证并提交，然后停止。

## 执行步骤

1. 读取 `TODO.md`，确认索引中的阶段文件和任务顺序。
2. 按索引顺序读取对应 `TODO-Px.md` 文件，使用详细文件作为唯一完成状态来源。
3. 选择第一个标题未显式带 `[DONE]` 的详细任务；若最新提交指出与该任务直接相关的未完问题，将其纳入当前任务或作为前置项处理。
4. 阅读该任务的详细要求、约束、依赖和验证要求。
5. 检查相关代码与测试，实施最小且符合规范的修改；不通过缩小范围、替代表达或 fixture-only hack 绕过问题。
6. 运行与任务相关的测试；必要时运行更广泛的验证，修复发现的直接相关问题。
7. 更新 `TODO-Px.md`：在完成任务标题前加 `[DONE]`，并填写或更新完成记录。
8. 如该任务出现在 `TODO.md` 索引中，同步相同 `[DONE]` 标记；仅在阶段级计划变化时更新 `PLAN.md`。
9. 更新本文件，记录关键步骤完成情况和最终验证结果。
10. 查看 git 状态和差异，提交本次任务涉及的所有未提交变更，提交信息包含任务编号并准确描述改动。
11. 停止，不继续下一个任务。

## 约束

- 若发现阻塞当前任务的缺失语言特性、规格不一致或实现边界，优先修复；若本次无法正确完成，则添加最小前置任务并同步索引后提交停止。
- 不回退或修改与当前任务无关的既有工作区变更。
- 所有手工文件编辑使用 `apply_patch`。

## 进度

- 计划已初始化。
- 已读取 `TODO.md` 与 `TODO-P7.md`，确认第一个未完成任务为 `P7-T02T`。
- 最新提交 `[P7-T02S] Fix refactor build blockers and add Task layout prerequisite` 与当前任务直接相关，按当前任务范围处理。
- 已实现 generic class instance layout handoff：`RefactorAbiQuery` 发布 concrete class instance layout，refactor/pass MIR class ctor 与 member access 消费 target/receiver concrete source type 生成的 class key。
- 已新增 `llvm::tests::refactor_class_ctor_uses_concrete_generic_instance_layout`。
- `task_atomic_claim_no_mutex_llvm.scoop` 已越过 `Task<T>` constructor/member layout blocker，当前停在后续 `P7-T02S` 的 `source-backed literal span` blocker。
- 已更新 `TODO-P7.md` 与 `TODO.md`，将 `P7-T02T` 标记为 `[DONE]`，并记录验证结果。

## 当前任务执行细化

1. 复现 `task_atomic_claim_no_mutex_llvm.scoop` 当前失败，记录具体 failing path 与诊断。
2. 定位 refactor LLVM class constructor lowering、class layout/descriptor materialization、`InstanceKey` / concrete type args 相关代码。
3. 找到已有 materialized type/class instance layout 数据源；若缺失，则在 P5/P6 handoff 或 ABI materializer 中补充稳定 keyed concrete field layout。
4. 修改 class ctor lowering，使 `Task<Int>` / `Task<(Int, Any)>` 等 generic instance 使用 concrete field ABI 存储字段，而不是 raw class declaration generic field type。
5. 确认 GC/type descriptor/field trace bitmap 继续依据 concrete field 类型生成，不擦除为 `Any`。
6. 增加或更新定向测试覆盖 generic class ctor concrete layout。
7. 运行任务要求验证命令，必要时补充格式化和 lint。
8. 更新 TODO 完成记录和本计划文件，提交本任务所有变更。

## 验证结果

- 通过：`cargo test -p scoopc --lib refactor_class_ctor_uses_concrete_generic_instance_layout`
- 通过：`cargo test -p scoopc --lib effect_lowered`
- 通过：`cargo test -p scoopc --lib llvm::codegen::effect_refactor`
- 通过：`cargo test -p scoopc --lib llvm::tests`
- 通过：`cargo clippy --all-targets -- -D warnings`
- 已运行：`cargo run -p scoop -- test --fixtures tests/fixtures/build/task_atomic_claim_no_mutex_llvm.scoop`；结果不再出现 `Task<T>` generic class layout 错误，当前推进到 `P7-T02S` 范围内的 `source-backed literal span` blocker。
