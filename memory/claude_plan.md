# 当前执行计划

## 目标
- 按照 `TODO.md` 的顺序，只完成第一个未完成任务，然后停止。
- 在推进该任务前，先检查最近一次提交是否提到已有问题；若提到，则先修复该问题。
- 在执行过程中，若发现任何现存缺陷、规格不匹配、实现边界缺口或回避性做法，立即将其视为当前范围内的问题优先处理，必要时先更新 `TODO.md`/`PLAN.md` 后停止。

## 约束与执行原则
- 不以变通方案、特判、缩小测试形状或规避路径的方式推进任务。
- 如果当前任务过大，需要先拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
- 完成任务后必须：
  - 更新 `TODO.md`
  - 更新 `PLAN.md`
  - 运行相关测试与质量检查
  - 提交 Git commit
  - 停止，不继续下一个任务

## 初始步骤
1. 查看最近一次提交信息，确认是否提到已有问题需要优先修复。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务上下文、依赖和是否需要拆分。
4. 结合代码与测试现状判断：
   - 若存在最近提交提到的已有问题，先修复该问题。
   - 若首个未完成任务过大，则拆分任务并更新 `TODO.md`/`PLAN.md`，本轮只执行拆分后的第一个子任务。
   - 若执行中发现阻塞性的既有缺陷，则先修复；若本轮无法直接修复，则把缺陷作为前置任务插入 `TODO.md`，更新 `PLAN.md`，提交后停止。

## 本轮预期产出
- 一个完成的首个未完成任务（或其拆分后的首个子任务），以及对应代码、测试、文档和提交。
- 或者：若发现阻塞性既有问题，则新增前置任务并提交任务重排结果后停止。

## 进度记录
- 已完成：创建本计划文件，后续会在关键步骤完成后持续更新。
- 2026-04-26 进展：
  - 已检查最新提交：`[T5000e1b0a1] Queue member lambda-eff direct-call fix before review`。
  - 已阅读 `TODO.md` / `PLAN.md` 并确认：
    - 第一个未完成任务是 `T5000e1b0a1 修复 effect-generic member direct-call 对 lambda 实参的 overload matching / eff_arg 推断闭环`；
    - 该任务正是最新提交显式排到 review 之前的前置修复项，因此必须先完成它，不能跳到后续 review。
  - 下一步：
    1. 阅读 `T5000e1b0a1` 的任务说明与最近相关实现；
    2. 构造或定位能稳定复现问题的 case；
    3. 修复 overload matching / `eff_arg` 推断闭环；
    4. 运行相关测试、更新 `TODO.md` / `PLAN.md`、提交并停止。
- 2026-04-26 进一步进展：
  - 已用最小 case 复现当前失败：`box.lift({ perform Boom.ping(); 1 })` 会报 `NoMatchingOverload`。
  - 继续定位后确认真正的前置阻塞不是 member overload matcher 本身，而是更早的既有规格缺口：
    - `SCOOP_FULL_SPEC.md` 明确支持 `perform E.op(...)`；
    - 但当前 parser 尚未把 `perform` 接进表达式前缀，导致 lambda body 内的 `perform Boom.ping()` 落成 `StmtKind::Missing`，在 expected-context typecheck 时先报 `block expression（missing stmt）`，从而让 member direct-call 候选被误丢弃。
  - 由于这是阻塞当前任务的既有问题，已直接纳入本轮修复，而不是绕过成 `Boom.ping()` 形式。
  - 已完成的代码修改：
    1. 在 `crates/scoopc/src/parser/expr.rs` 为显式 `perform` 增加前缀解析，按 effect-op call 语法糖处理，并把外层 span 扩到包含 `perform` 关键字；
    2. 新增 `crates/scoopc/src/typecheck/expr/infer.rs` 回归测试，覆盖 typed receiver 成员 direct-call + lambda + 显式 `perform`；
    3. 新增 `crates/scoopc/src/mir/materialize.rs` 回归测试，覆盖 lambda-derived member direct-call 的 `TopLevelFunCallBinding` / monomorph key 保留非 `Pure` `eff_args`。
  - 下一步：
    1. 运行格式化与定向测试，确认显式 `perform` 已打通当前任务路径；
    2. 若定向测试通过，再运行任务要求的更完整检查；
    3. 更新 `TODO.md` / `PLAN.md`、提交并停止。
- 2026-04-26 收尾状态：
  - 已完成验证：
    - `cargo fmt --all`
    - `cargo test -p scoopc member_direct_call_infers_effect_row_from_lambda_with_explicit_perform -- --nocapture`
    - `cargo test -p scoopc dump_materialization_inputs_keep_eff_args_for_member_direct_call_binding -- --nocapture`
    - `cargo test -p scoopc typed_hir_keeps_effect_generic_member_type_apply_on_direct_call_path -- --nocapture`
    - `cargo test -p scoopc monomorph_rewrites_effect_generic_extension_call_to_concrete_instance -- --nocapture`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
  - 已更新 `TODO.md` / `PLAN.md`：
    - `T5000e1b0a1` 已标记为完成；
    - 下一条待执行任务已切换为 `T5000e1b0a1R Review：确认 member direct-call 已真正消费 lambda-derived effect-row facts`。
  - 下一步：
    1. 检查工作区状态与 diff；
    2. 提交本轮改动；
    3. 停止，等待下一次调用。
