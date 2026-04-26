# 当前执行记录

## 工作思路摘要

- 先建立一份可追踪的执行计划文件，后续在关键步骤完成或计划调整时持续更新。
- 优先检查最新一次 Git 提交，确认提交说明或相关改动中是否提到既有问题；如果发现已有问题，先修复该问题，再考虑 `TODO.md` 中的计划任务。
- 读取 `TODO.md`，定位第一个未完成任务。
- 如果该任务过大，则拆分为更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`，本轮只执行拆分后的第一个子任务。
- 对本轮目标任务执行实现、测试、文档更新、提交，完成后立即停止。

## 分步计划

1. 检查最新 Git 提交，确认是否存在被明确提到但尚未修复的既有问题。
2. 阅读 `TODO.md`，识别第一个未完成任务及其依赖关系。
3. 视复杂度决定是否需要拆分任务，并在必要时更新 `PLAN.md` 与 `TODO.md`。
4. 阅读与目标任务直接相关的代码、测试、规格或文档，确认当前实现边界。
5. 实现目标任务，避免通过变通方式绕过已有缺陷；若遇到阻塞性既有问题，先修复或将其作为前置任务写回 `TODO.md`。
6. 运行相关测试与必要的质量检查，至少覆盖本次变更涉及的路径；若需要，补充或修复测试。
7. 更新 `memory/claude_plan.md`、`TODO.md`、`PLAN.md`，记录完成情况与计划变化。
8. 提交本轮变更，提交信息对应本轮完成的任务，然后停止。

## 当前状态

- 已创建执行记录文件。
- 已检查最新提交：`56376503f351a0e05783e580f78880a9b36c532d [T5000e3d] Materialize direct-call targets before LLVM codegen`。
- 已读取 `TODO.md` / `PLAN.md`，当前首个未完成任务是 `T5000e3dR Review：确认 LLVM backend 已退出单态目标猜测主职责`。

## 当前 review 聚焦点

- 核对 HIR lowering 是否已在 codegen 主路径上把 standalone / member / extension direct-call target 统一物化为稳定实例 FQN。
- 核对 LLVM backend 中普通静态 direct-call 是否已只按完整实例 FQN 命中 `fun_index`，不再保留 `try_resolve_monomorphized_*` 一类现场补救逻辑。
- 核对仍保留的模板名消费路径是否已经收口为窄边界，例如 sysroot/builtin special-case、vtable / itable dispatch。
- 核对 via-MIR compilation-unit lowering 是否确实以 `InstanceKey` 驱动实例收集与 member 实例生成，而不是重新退回 backend 猜测。

## 当前观察

- `hir/lower/expr.rs` 中 direct-call lowering 已统一经 `materialized_top_level_fun_call_target_fqn(...)` 回放 typecheck `TopLevelFunCallBinding`。
- `llvm/codegen/call/dispatch.rs` 中普通静态 direct-call 已直接按完整 FQN 查 `fun_index`；模板名归一化 helper 目前只用于 sysroot/builtin special-case 与 vtable / itable 路径。
- `hir/lower/mod.rs` 的 via-MIR compilation-unit lowering 已改为 `ExplicitMirInstances` 模式下按 `InstanceKey` 生成单态 fun/member 实例。
- `llvm/emit.rs` 中仍可见少量模板名导向的 generic member eager inclusion 兜底，但它不再参与 ordinary static direct-call 的目标解析主线；当前 review 未发现它构成 `T5000e3dR` 前置阻塞缺陷。

## 本轮验证结果

- 已新增 LLVM 回归测试 `frontend_codegen_consumes_materialized_generic_direct_call_instances`，同时从 via-MIR frontend lowering 和生成的 LLVM IR 两层锁定 `id::<Int>` / `Box.memberId::<Int>` 的实例身份。
- 已通过定向测试：
  - `cargo test -p scoopc compilation_unit_via_mir_instances_materializes_non_intrinsic_direct_call_targets -- --nocapture`
  - `cargo test -p scoopc typed_hir_dump_keeps_generic_direct_calls_as_template_targets -- --nocapture`
  - `cargo test -p scoopc lowered_hir_codegen_accepts_materialized_generic_sysroot_direct_calls -- --nocapture`
  - `cargo test -p scoopc frontend_codegen_consumes_materialized_generic_direct_call_instances -- --nocapture`
- 已通过质量检查：
  - `cargo fmt --all`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

## 当前状态

- `T5000e3dR` 的 review 结论已经形成，未发现需要插入到下一条任务之前的新前置缺陷任务。
- 正在回写 `TODO.md` / `PLAN.md` 并准备提交本轮变更。
