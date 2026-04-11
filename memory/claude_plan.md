# 执行计划与决策摘要

## 约束

- 本次只处理 `TODO.md` 中第一个未完成任务，完成后即停止。
- 在开始实际检查代码与执行命令前，先记录本计划文件。
- 若执行过程中发现任务过大，需要先拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`。
- 若发现最新提交提到的遗留问题，需先修复这些问题，再进入 `TODO.md` 的首个未完成任务。

## 初始步骤

1. 检查最新一次 Git 提交内容，确认是否提到已知问题、遗留缺陷或待补修复项。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认当前计划与 `TODO.md` 是否一致。
4. 结合代码结构评估该任务是否足够小且可在本轮完整交付。

## 执行策略

1. 如果最新提交提到的遗留问题存在：
   - 先定位相关代码与测试。
   - 修复问题并补充验证。
   - 再回到 `TODO.md` 的首个未完成任务。
2. 如果首个未完成任务过大：
   - 将其拆成更小的子任务。
   - 更新 `PLAN.md`。
   - 更新 `TODO.md`，让第一个子任务成为新的当前任务。
   - 本轮只实现第一个子任务。
3. 如果任务规模合适：
   - 直接实现。
   - 运行相关格式化、测试、lint/clippy。
   - 更新文档与任务状态。
   - 提交 Git commit。

## 验证清单

- 必要代码改动已完成。
- 相关测试通过。
- `cargo fmt` 已检查或执行。
- `cargo clippy --all-targets -- -D warnings` 通过，且无新增告警。
- `TODO.md` 与 `PLAN.md` 已同步更新。
- 如有必要，更新 `README.md` 或相关内联注释。
- 生成清晰的 Git 提交。

## 执行中更新规则

- 当我完成关键检查、决定拆分任务、开始实现、完成测试、准备提交时，会继续更新本文件。
- 若实际情况与初始计划不一致，会在此记录原因与调整后的执行顺序。

## 当前进展（2026-04-11）

- 已检查最新提交：`62612ec [T0152] 支持 safe member access 的 ref receiver 与 extension property`。
- 最新提交 message 本身未提到额外遗留问题；后续仍会在实现当前任务前做必要的针对性回归，避免把已有回归带入下一步。
- 已读取 `TODO.md` 与 `PLAN.md`。
- 当前 `TODO.md` 中第一个未完成任务是：
  - `T0153 [TODO] Higher-order：receiver function type 的局部函数值调用`
- 当前判断：`T0153` 范围可控，不需要先拆分。主要改动面集中在：
  1. `typecheck/expr/call.rs`：放开 receiver function value call，并把 receiver 按“第 0 个实参”检查。
  2. `llvm/codegen/mod.rs`：放开 receiver closure/function value 的间接调用 ABI，并让 receiver 作为 env 之后的第一个 LLVM 实参传递。
  3. receiver lambda 最小 codegen 适配：lambda 本体签名包含 receiver，但仍保持现阶段“不为 lambda body 注入新的 this 绑定”的约束。
  4. fixtures：新增 run-pass 与 typecheck fail 回归，覆盖直接调用、higher-order 传递、arity mismatch、receiver mismatch。

## 实施结果（2026-04-11）

- 已完成代码实现：
  - `typecheck/expr/call.rs` 已支持 receiver function value 调用，receiver 按第 0 个实参参与 arity / 类型检查。
  - `llvm/codegen/mod.rs` 已支持 receiver closure/function value 的 `env + receiver + params` 间接调用 ABI。
  - receiver lambda codegen 已适配 receiver 槽位与显式参数偏移。
- 已新增回归：
  - run-pass：`receiver_function_value_call_basic`
  - typecheck fail：`receiver_function_value_call_arity_mismatch_is_error`
  - typecheck fail：`receiver_function_value_call_receiver_mismatch_is_error`
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test --all`
  - `cargo run -p scoop -- build tests/fixtures/run-pass/receiver_function_value_call_basic.scoop -o /tmp/receiver_function_value_call_basic && /tmp/receiver_function_value_call_basic`
  - `cargo run -p scoop -- test`
  - `cargo clippy --workspace --all-targets --message-format short -- -D warnings`
- 当前剩余收尾：
  - 更新 `TODO.md` / `PLAN.md` 状态。
  - 检查 diff。
  - 提交本轮变更并停止。
