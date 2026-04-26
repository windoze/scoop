# 当前执行思路摘要

根据本轮要求，我会先记录一份可审阅的执行思路摘要与步骤计划，再开始任何仓库探查或命令执行。这里记录的是可共享的分析摘要与执行方案，不包含不可共享的原始内部推理。

## 目标

本轮只完成 `TODO.md` 中“第一个未完成任务”，并在完成后停止。

## 强约束

1. 先检查最新提交是否提到已有问题；若提到，先修复这些问题。
2. 任何在检查、测试、实现过程中发现的既有缺陷、规约不匹配、回避式实现、实现边界不完整，都必须立即纳入当前范围。
3. 不能通过变通、缩小范围、替换表示方式、弱化测试形状等方式绕过问题。
4. 若当前任务过大，必须先把它拆分到 `TODO.md` / `PLAN.md`，本轮只执行拆分后的第一个子任务。
5. 完成后必须更新 `TODO.md`、`PLAN.md`，运行相关验证，提交 Git commit，然后停止。

## 初始执行计划

1. 查看最新一次 Git 提交信息，确认是否明确提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解当前计划和任务上下文。
4. 结合任务内容检查相关代码、测试、规格或文档，判断该任务是否可在本轮完整完成。
5. 如果任务过大：
   - 拆成更小子任务；
   - 更新 `PLAN.md`；
   - 更新 `TODO.md` 中的任务顺序和依赖；
   - 提交这些计划调整并停止。
6. 如果任务可执行：
   - 直接实现；
   - 增加或调整测试；
   - 运行必要的格式化、测试、`clippy` 等验证；
   - 修复实现中暴露的既有问题；
   - 更新 `TODO.md` / `PLAN.md`；
   - 提交改动并停止。

## 执行过程中的动态更新规则

- 如果我发现阻塞当前任务的既有缺陷，我会先把它记录为前置任务并调整 `TODO.md` / `PLAN.md`。
- 如果实现方案发生变化，我会及时更新本文件，说明变更原因和当前进度。
- 如果某一步已完成，我会在本文件中追加进度记录，便于核查。

## 当前状态

- 已完成：按要求先写入执行思路摘要与计划。
- 已完成：检查最新提交、`TODO.md` 与 `PLAN.md`。
- 结论：
  - 最新提交 `0e96fcf8 [T5000e1b] Update execution memory after commit` 仅更新 `memory/claude_plan.md`，未声明新的待修复既有问题。
  - `TODO.md` 中第一个未完成任务是 `T5000e1bR Review：确认 effect-row 实参已成为 InstanceKey / materializer 的一等维度`。
- 当前执行计划细化：
  1. 阅读 `T5000e1b` 相关实现、测试与最近提交 diff。
  2. 针对 review 验收点检查：
     - `eff_args` 是否从 typecheck 请求进入 `InstanceKey`、template substitution、instance cache 与 debug 输出；
     - effect-row 参数是否在 lowering/template 阶段被保留，而非提前塌缩；
     - effect-only generic 与“同 type args 不同 effect row”是否被稳定区分。
  3. 运行相关测试与必要的全量验证。
  4. 若发现既有问题，先修复并补测试，再完成 review 记录。
  5. 更新 `TODO.md` / `PLAN.md` / 本文件并提交。
- 关键进展：
  - 已确认 `eff_args` 已进入 `MonomorphKey`、`InstanceKey`、site binding、instance substitution 与 `instance_fqn(...)`。
  - 已通过临时 `dump-ir` 复现实例发现一个真实缺口：
    - 场景：generic wrapper 中调用 effect-generic extension fun（`x.forward<eff E>()`），再实例化为 `wrap<eff Boom>`；
    - 现象：`dump-ir` 输出里只 materialize 出 `forward::<eff Pure>`，`wrap::<eff Boom>` 体内保留 `MemberAccess + FunValue` 路径，没有收口到 `forward::<eff Boom>`；
    - 初步原因：
      1. 显式 `<eff ...>` 的 extension call 没有在 HIR lowering 阶段像普通 extension call 一样降为顶层 direct call；
      2. extension call typecheck 只记录了 `MonomorphKey`，没有写入 `top_level_fun_call_bindings`，导致 materializer fixed-point 阶段拿不到 site binding 的 `eff_args`。
- 当前决定：
  - 先修复上述既有缺口并补回归测试；
  - 修复后继续检查 member method 路径是否也存在同类问题；
  - 若发现更深层 implementation boundary 缺口，则按要求把它前插为新的 TODO 前置任务。
- 已完成的实现：
  - `crates/scoopc/src/hir/lower/expr.rs`：新增 `transparent_call_callee(...)`，让 call 位置的 `TypeApply` 继续走 extension/member direct-call 降糖。
  - `crates/scoopc/src/typecheck/expr/call.rs`：为 extension/member direct-call 补齐 `TopLevelFunCallBinding` 写回，并让扩展调用路径优先消费显式 `eff_arg`。
  - `crates/scoopc/src/typecheck/expr/ops.rs`：成员方法签名收集不再丢弃 `eff_param` 与 effect facts。
  - `crates/scoopc/src/mir/materialize.rs`：扩展 generic template catalog，使 type-body / companion object generic member fun 也能建立 request lookup groundwork。
  - `crates/scoopc/src/monomorph/lower.rs`：新增回归测试 `monomorph_rewrites_effect_generic_extension_call_to_concrete_instance`。
- 已完成验证：
  - `cargo check -p scoopc`
  - `cargo test -p scoopc monomorph_ -- --nocapture`
  - `cargo clippy --all-targets -- -D warnings`
  - 结论：当前已完成的“call-site/request binding 层修复”全部通过。
- 新发现的更深阻塞：
  - 对 `class Box { fun <eff E = Pure> forward(): Int / E { ... } }` 的 probing 显示：
    - `dump-mir` 仍把 type declarations 输出为 `Todo { kind: "type" }`；
    - generic MIR file 中没有 `fixtures.monomorph.Box.forward` root；
    - 因此 `dump-ir` 会继续报 `missing_mir_root_for_template`。
  - 这说明：type-body generic member fun 还没有进入 generic MIR template 主线，当前不能把 `T5000e1bR` 直接判定完成。
- 当前执行结论：
  - 已把工作拆成新的前置任务：
    1. `T5000e1b0a`：修复 extension/member direct-call 的 request binding 与显式 type-apply 透传（本轮已完成）；
    2. `T5000e1b0b`：让 generic MIR template / dump-ir 收录 type-body generic member fun roots（待后续执行）。
  - 原 `T5000e1bR` 仍保持未完成，并改为依赖新的 member-method 前置任务。
- 下一步：更新 `TODO.md` / `PLAN.md` 为新顺序，提交本轮已完成的 `T5000e1b0a`，然后停止。
