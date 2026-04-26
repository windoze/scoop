# 本轮执行计划

## 说明

用户要求先写计划文件再执行命令。这里记录的是可执行计划、检查顺序、关键决策点与进度更新，不包含逐字内部推理。

## 初始步骤

1. 查看最新一次 Git 提交，确认提交信息是否提到任何已知问题、回归、未完成边界或需先修复的事项。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 评估该任务是否过大：
   - 如果可直接完成，则进入实现。
   - 如果过大或被前置缺陷阻塞，则先更新 `PLAN.md` 与 `TODO.md`，拆分为更小子任务或补入前置修复任务，并在本轮只处理新的首个任务。

## 执行原则

1. 任何在检查、测试、实现过程中发现的既有问题都视为立即在范围内。
2. 不接受规避式实现；如果遇到规范缺口、实现边界或回归，必须先修复，或者把修复任务插入到 `TODO.md` 中当前任务之前。
3. 本轮最多完成一个任务，然后停止。

## 实施步骤

1. 收集上下文：
   - 最新提交信息
   - `TODO.md`
   - `PLAN.md`
   - 与首个任务相关的代码与测试
2. 如有需要，细化任务并更新计划文件。
3. 实现任务或前置修复。
4. 运行相关验证：
   - 最小相关测试
   - 必要时扩大到工作区测试
   - `cargo fmt`
   - `cargo clippy --all-targets -- -D warnings`
5. 更新文档与任务状态：
   - 在 `TODO.md` 中标记完成或重排依赖
   - 更新 `PLAN.md`
   - 视进展同步更新本文件
6. 提交 Git commit，提交信息明确对应任务。

## 进度记录

- 已创建本计划文件，并完成最新提交、`TODO.md`、`PLAN.md` 的初始检查。
- 已确认本轮首个未完成任务是 `T5000e1b0a1R Review：确认 member direct-call 已真正消费 lambda-derived effect-row facts`。
- 已完成代码复核：
  - `crates/scoopc/src/typecheck/expr/call.rs` 的 member direct-call 单候选与多候选路径均已检查；
  - `crates/scoopc/src/typecheck/expr/ops.rs` 中 `collect_member_method_signatures_from_index(...)` 的 effect-row 事实已确认被调用点闭环消费。
- 已新增 review regression：
  - `typecheck::expr::infer::tests::member_direct_call_overload_keeps_effect_generic_lambda_candidate_alive`
  - 目的：覆盖“成员重载 + lambda + 显式 perform”风险形状，验证 effect-generic 候选不会被默认 `Pure` 提前过滤。
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc member_direct_call_ -- --nocapture`
  - `cargo test -p scoopc typed_hir_keeps_effect_generic_member_type_apply_on_direct_call_path -- --nocapture`
  - `cargo test -p scoopc monomorph_rewrites_effect_generic_extension_call_to_concrete_instance -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 结论：
  - 未发现需要插入到当前任务之前的新前置缺陷；
  - 本轮任务可标记完成，下一条将切换到 `T5000e1b0aR`。
