## 当前执行计划

1. 读取 `TODO.md`，确认详细任务文件映射关系与任务顺序。
2. 按索引顺序读取对应的 `TODO-Px.md`，定位第一个标题未带 `[DONE]` 的详细任务。
3. 检查最近一次提交信息是否存在与该任务直接相关且未完成的问题；若存在，将其视为当前任务组成部分或前置依赖。
4. 阅读当前任务涉及的实现、约束、验证要求与完成记录，确认需要修改的代码、测试与文档范围。
5. 实现当前任务；若遇到阻塞当前任务且不能规避的真实缺口，则在对应 `TODO-Px.md` 中加入最小前置任务并同步 `TODO.md`，必要时更新 `PLAN.md`。
6. 运行与当前任务直接相关的验证；随后运行要求的质量检查，至少包括相关测试，以及在可行范围内执行 `cargo fmt`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
7. 将完成情况写回对应 `TODO-Px.md`，在任务标题前添加 `[DONE]`；若任务索引或顺序变化，同步更新 `TODO.md`。
8. 复查工作区中与本次任务相关的改动，按要求创建一次 git 提交，然后停止，不进入下一个任务。

## 记录约定

- 这里记录执行计划、关键决策、阻塞信息与已完成步骤。
- 不记录内部推理细节，只记录可审计的行动计划与结果。

## 当前进展

- 已读取 `TODO.md` 与 `TODO-P5.md`，确认首个未完成详细任务为 `P5-T05R`。
- 已检查最近一次提交主题为 `[P5-T05] Materialize late-lowered step/boundary contracts`；其主题与当前 review 直接相关，但未额外声明新的未完成问题，因此当前目标仍是完成 `P5-T05R` review 本身。
- 已初步复核以下实现位置：
  - `crates/scoopc/src/effect_lowered/ir.rs`
  - `crates/scoopc/src/effect_lowered/builder.rs`
  - `crates/scoopc/src/effect_lowered/materialize.rs`
  - `crates/scoopc/src/effect_lowered/dump.rs`
- 当前观察到的关键事实：
  - `LateLoweredDynamicInvokeEntry` 作为显式 `invoke(args_tuple) -> Step_F` 入口存在于 `LateLoweredCallable` 中；
  - `LateLoweredContinuationContract` 显式区分 `surface_ty` 与 `out_step_schema`；
  - `materialize_continuation_object(...)` 为 step schema 的全部 case 生成 `surface_resumes` 与 `methods`，不可达路径以 `LateLoweredContinuationResumeBody::Unreachable` 表达，没有直接删方法；
  - one-shot 重复 `resume` 目前显式收口到 `LateLoweredOneShotPolicy::OrdinaryRuntimeErrorOutward`。

## 下一步

1. 运行 `P5-T05R` 要求的额外关键字搜索，复核是否仍存在 TLS / snapshot / bridge / erased signal 依赖。
2. 运行 `P5-T05` 要求的定向测试与 `clippy`，确认 review 验证矩阵通过。
3. 若验证通过，则更新 `TODO-P5.md`、同步 `TODO.md`、补写 review 完成记录并提交；若发现问题，则先修复问题再执行上述步骤。

## 已完成步骤

- 已完成关键字搜索：
  - `crates/scoopc/src/effect_lowered/**` 仅命中 canonical MIR `snapshot` 文档/错误信息；未发现 `handler_stack` / `tls` / `bridge` / `Signal {` 语义依赖。
  - `crates/scoopc/src/effect_refactor_pipeline/**` 中的 `snapshot` 命中均属于 stage 输入绑定与 dump 文档；未发现把 TLS handler snapshot/bridge 当 correctness 前提的新路径代码。
  - `crates/scoopc/src/llvm/codegen/effect/**` 仍有 legacy handler-stack / bridge 相关实现，但 review 关注点是确认 P5 新路径未依赖这些 legacy 后端位置；当前复核结果满足该约束。
- 已完成 `P5-T05` 记录中的全部定向验证：`cargo fmt --all`、全部列出的 `cargo test -p scoopc --no-default-features ...` 命令，以及 `cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings` 均通过。
- 已更新 `TODO-P5.md` 与 `TODO.md`，将 `P5-T05R` 标记为 `[DONE]` 并写入 review 完成记录。

## 待收尾

1. 运行提交前的 git 检查，确认本次提交范围。
2. 以 `P5-T05R` 为主题创建提交。
3. 停止，不进入 `P5-T06`。
