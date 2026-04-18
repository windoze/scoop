# 本轮执行计划与决策摘要

## 约束说明

- 按用户要求，先在此文件记录本轮计划，再执行仓库检查与实现工作。
- 此文件记录的是可审计的执行计划、判断依据摘要和进度更新，不包含原始内部推理。
- 本轮目标：只完成 `TODO.md` 中第一个未完成任务（如需先处理最新提交中提到的遗留问题，则先处理该问题），完成后测试、更新 `TODO.md`/`PLAN.md`、提交 git，然后停止。

## 初始步骤

1. 检查最新一次 git 提交信息，确认是否明确提到已有问题需要先修复。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 判断该任务是否过大：
   - 若可直接完成，则进入实现。
   - 若过大，则先拆分子任务，更新 `PLAN.md` 与 `TODO.md`，并只执行拆分后的第一个子任务。
4. 在实现过程中，如发现任何规范不匹配、缺失特性或现有 bug 会阻塞正确实现，则：
   - 先在 `TODO.md` 中补入前置修复任务并调整顺序；
   - 在 `PLAN.md` 与本文件记录阻塞原因；
   - 提交后停止，不做绕过式实现。
5. 完成当前任务后：
   - 运行相关测试与必要的质量检查；
   - 更新 `TODO.md`、`PLAN.md` 和本文件；
   - 生成单独 git 提交；
   - 停止。

## 计划中的验证

- 至少运行与改动直接相关的测试。
- 如改动影响公共编译/运行路径，补充运行更高层级验证。
- 若时间与范围允许，运行 `cargo clippy --all-targets -- -D warnings` 以满足无 warning 要求；若范围过大或与现有仓库状态冲突，则在记录中说明实际验证边界。

## 进度记录

- [x] 已写入本轮初始计划，尚未开始仓库检查。
- [x] 已检查最新提交 `86e1f3bc9552bcebd4a8d1e9cba7ebb72e26da8a`、`TODO.md` 与 `PLAN.md`。
- [x] 最新提交信息未明确标注额外待修复遗留问题；当前首个未完成任务为 `T4003R`。
- [x] 已完成 `T4003R` 复审，并定位到既有后端裂缝：顶层 direct call、vtable member call 与 itable member call 仍未复用命名实参绑定主线。
- [x] 已将 LLVM 侧调用参数绑定收口为共享辅助函数，并让 direct call、vtable、itable、function-value、funptr 共用同一套映射/求值逻辑。
- [x] 已把顶层泛型 direct call 的 monomorph FQN 解析切到同一套命名实参映射。
- [x] 已新增 run-pass 回归：
  - `tests/fixtures/run-pass/top_level_generic_named_args_basic.scoop`
  - `tests/fixtures/run-pass/member_call_virtual_named_args_basic.scoop`
  - `tests/fixtures/run-pass/member_call_interface_named_args_basic.scoop`
- [x] 已完成验证：
  - `cargo run -p scoop -- test --fixtures /tmp/t4003r-run-pass` -> `fixtures: ok (6)`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck` -> `fixtures: ok (327)`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- [ ] 待更新 git 提交记录并结束本轮。
