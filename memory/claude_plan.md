# 执行计划与进度记录

## 说明

按要求先记录执行计划、关键判断依据和后续进度。这里提供的是可审计的高层计划与决策摘要，不包含不可共享的内部详细推理。

## 当前目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果在执行前发现最新提交遗留问题或规格不匹配，则先修复这些前置问题，必要时更新 `TODO.md` / `PLAN.md` 后停止。

## 初始步骤

1. 检查最新一次提交，确认是否明确提到已知问题、待修复项或回归。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务的上下文、依赖和既有分解。
4. 如任务过大或存在前置缺口：
   - 将任务拆分为更小的可执行子任务；
   - 更新 `PLAN.md`；
   - 调整 `TODO.md` 的顺序和依赖；
   - 本轮只执行拆分后的第一个子任务。
5. 实现任务并补充/调整测试。
6. 运行相关验证，优先确保：
   - 相关测试通过；
   - `cargo fmt` 通过；
   - `cargo clippy --all-targets -- -D warnings` 无告警；
   - 必要时运行更广的 `cargo test --all` 或目标化测试。
7. 更新文档与跟踪文件：
   - 在 `TODO.md` 中标记已完成任务；
   - 在 `PLAN.md` 中更新状态；
   - 在本文件记录关键进展和计划变化。
8. 提交一次 git commit，然后停止，不继续下一个任务。

## 进度日志

- 2026-04-13：已创建本计划文件，准备开始检查最新提交与任务列表。
- 2026-04-13：已检查最新提交 `0e366d52d9a11485e06ce200452ccb9ec5c8ff2d`。提交说明仅包含 `T2003u1` 统一状态机设计定稿，没有额外注明需要先修的遗留缺陷。
- 2026-04-13：已确认 `TODO.md` 中第一个未完成任务是 `T2003u2 Effect：实现统一的 suspension-aware state machine plan`。
- 2026-04-13：已审阅 `PLAN.md`、`docs/effect_unified_state_machine.md` 与现有 `crates/scoopc/src/llvm/codegen/effect/*`。判断本轮可以直接完成 `T2003u2`，不必再拆子任务；范围聚焦于“统一 plan builder + pretty dump + 覆盖性单元测试”，暂不切换 LLVM 主 emitter（那是 `T2003u3`/`T2003u4` 的范围）。

## 本轮实现细化

1. 在 effect codegen 模块内新增统一 `HandleStateMachinePlan` 数据结构。
2. 实现 plan builder：
   - 统一扫描 `handle` body 的 direct perform、可能挂起的调用点、control-flow（`if` / `while` / block）、nested handle；
   - 为 handle arms 建立统一 dispatch / arm-plan 元数据；
   - 建模 frame layout、resume target、cleanup/finally scope、loop re-entry 与 branch merge。
3. 提供稳定的 pretty dump 输出，便于 golden/字符串断言测试。
4. 新增单元测试，至少覆盖：
   - direct perform；
   - effectful call / indirect suspend 边界；
   - `if`；
   - `while`；
   - nested handle；
   - multiple arms / mixed arm kinds。
5. 更新 `TODO.md` / `PLAN.md`，将 `T2003u2` 标记为完成并记录落地结果。
6. 运行格式化、测试和 lint。
7. 提交 git commit 后停止。

## 当前已完成的关键实现

- 已新增统一 plan builder：`crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`。
- 已实现统一 `HandleStateMachinePlan` 输出，包含：
  - `states`
  - `suspend_sites`
  - `arm_plans`
  - `cleanup_scopes`
  - `frame_layout`
  - `dispatch_plan`
  - nested-handle 子计划
- 已实现 pretty dump，便于直接做字符串断言。
- 已补 3 组单元测试，覆盖：
  - direct perform + `if` / `while` / `finally`
  - effectful callee call-site + 本地函数值 indirect call-site
  - nested handle + multiple arms
- 已完成首轮定向验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc plan_dump_ --lib`
- 已完成完整验证：
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`

## 待完成事项

1. 检查最终 diff，确认 `TODO.md` / `PLAN.md` / 代码变更一致。
2. 提交 git commit 并停止。
