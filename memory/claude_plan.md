# Claude Plan

更新时间：2026-04-12

说明：按要求先记录“可共享”的执行思路摘要、步骤计划与进度日志。出于安全与隐私限制，这里不写逐词内部推理，而是写可审计的决策摘要、执行步骤、发现的问题与后续调整。

## 初始目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果发现其前置缺陷、规格不匹配或实现缺口，则先把阻塞项整理进 `TODO.md` / `PLAN.md`，提交后停止。

## 执行步骤

1. 检查最新一次提交的信息，确认是否提到已知问题、待补修复或未完成事项。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认当前计划与任务顺序是否一致。
4. 结合相关代码、测试、规范与最近提交，判断当前首个未完成任务是否可直接落地。
5. 如果任务过大或存在明确前置依赖：
   - 在 `PLAN.md` 中细化子任务；
   - 在 `TODO.md` 中调整顺序或拆分；
   - 本轮执行新的第一个子任务；
   - 若被阻塞，则记录原因、提交并停止。
6. 实现本轮目标，保持实现符合规范，不引入临时性绕过方案。
7. 运行相关验证：
   - 最小相关测试；
   - 必要时运行更大范围测试；
   - 至少确认无新增编译/静态检查问题，必要时跑 `cargo clippy --all-targets -- -D warnings`。
8. 更新文档状态：
   - 在 `TODO.md` 标记本轮任务完成或调整依赖；
   - 在 `PLAN.md` 更新当前状态；
   - 在本文件补充进度记录。
9. 使用清晰的 Git 提交信息提交本轮工作，然后停止。

## 进度日志

- 已创建本计划文件，下一步开始检查最新提交与任务列表。
- 已检查最新提交：提交信息为 `[T2003c0c1b] Support escape sibling non-resuming indirect site`，提交说明本身未附带额外“已知遗留问题”描述。
- 已定位本轮首个未完成任务：`T2003c0c1c`。
- 已完成范围审计：
  - `TODO.md` / `PLAN.md` 一致表明本轮目标是把 `escape-continuation + sibling non-resuming` 从 top-level single-site 扩到 richer site-matrix。
  - 当前 LLVM lowering 现状不是“只差放开门禁”，而是：
    - `codegen_handle_expr_immediate_resume_with_escape_and_nonresuming_siblings(...)` 仅放行 direct / indirect single-site，其余 matrix 形状直接报 `only top-level single-site supported`；
    - `codegen_handle_expr_immediate_resume_with_escape_sibling_site_matrix(...)` 已实现 escape site-matrix 主链路，但尚未接入 sibling non-resuming 的 dispatch / detach / restore 规则；
    - continuation step（resume 后继续 replay 的 step trampoline）也尚未接入 sibling non-resuming。
- 已确定本轮实现方案：
  1. 让 `escape + sibling non-resuming` 在非 single-site 时进入 site-matrix lowering，而不是直接报错。
  2. 在 site-matrix lowering 中补齐 sibling arm 分类（`Raise.raise` / custom non-resuming）与 op-tag dispatch 基础设施。
  3. main path：
     - state0 / state1 执行 body 时接入 sibling non-resuming dispatch；
     - indirect escape site 的 no-match fallback 改为先尝试 sibling dispatch，再外抛；
     - immediate arm body / escape arm body 执行期间保持 sibling self-capture 关闭。
  4. continuation step：
     - resume 后的 replay 路径接入 sibling non-resuming dispatch；
     - sibling arm body 在 step 中执行后正确 unpin state 并退出；
     - indirect no-match fallback 与 main path 保持一致。
  5. 新增至少两组 run-pass fixtures：
     - 一组覆盖 pre-immediate matrix + sibling `Raise.raise` 或 custom non-resuming；
     - 一组覆盖 nested / direct+indirect mixed + sibling custom non-resuming 或 `Raise.raise`。
  6. 运行格式化、相关测试、`cargo clippy --workspace --all-targets -- -D warnings`。
- 实现已完成：
  - 已把 `escape + sibling non-resuming` 的 non-single-site 路径接入 `codegen_handle_expr_immediate_resume_with_escape_sibling_site_matrix(...)`，不再只支持 top-level single-site。
  - 已为 site-matrix main path（state0/state1）补齐 sibling `Raise.raise` / custom non-resuming dispatch。
  - 已为 continuation step 补齐共享 sibling dispatch / cleanup，并让 indirect no-match 先尝试 sibling dispatch。
  - 已新增 run-pass fixtures：
    - `effect_resume_mixed_escape_pre_immediate_block_raise`
    - `effect_resume_mixed_escape_post_immediate_if_direct_indirect_custom_nonresuming`
- 验证已完成：
  - `cargo fmt --all`
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 文档状态已更新：
  - `TODO.md` 已将 `T2003c0c1c` 标记为完成并补充完成说明。
  - `PLAN.md` 已记录本轮完成内容，并把下一步调整到 `T2003c0c2`。
