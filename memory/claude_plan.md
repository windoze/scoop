# 本轮执行计划

## 约束说明

- 按用户要求，本文件在任何进一步仓库检查或实现动作前创建。
- 这里记录可公开的分析摘要、执行计划、关键决策与进度。
- 不记录逐字内部思维链路，但会记录足够详细的步骤、依据与后续更新。

## 目标

完成 `TODO.md` 中第一个未完成任务，然后停止。本轮还需要先检查最新提交中是否提到既有问题；若有，需先修复这些问题，再进入 `TODO.md` 的首个任务。

## 初始执行步骤

1. 检查最新一次 git 提交信息与相关改动，确认是否明确提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认当前计划与 `TODO.md` 是否一致。
4. 若首个未完成任务过大，拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`，本轮只执行拆分后的第一个子任务。
5. 实现该任务所需改动。
6. 运行相关测试，并补充必要测试；同时运行格式化、lint 与必要的全量或定向校验，确保无警告。
7. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成状态或阻塞原因。
8. 以清晰的提交信息提交本轮改动。
9. 停止，不继续处理下一个任务。

## 决策分支

### 若最新提交提到既有问题

- 优先定位并修复这些问题。
- 修复后运行对应测试。
- 只有在这些问题被修复后，才继续处理 `TODO.md` 的首个未完成任务。

### 若首个任务无法在当前实现边界内按规范完成

- 不采用绕过方案。
- 精确识别缺失能力或规范不匹配点。
- 在 `TODO.md` 中插入新的前置修复任务，并调整依赖顺序。
- 在 `PLAN.md` 与本文件中记录阻塞原因。
- 提交这些计划性变更后停止。

## 预期检查项

- `cargo fmt`
- `cargo test --all`
- `cargo clippy --all-targets -- -D warnings`
- 若任务涉及夹具或规范同步，补充：
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop_tools -- spec-fixtures check`

## 进度日志

- 2026-04-12：已创建本文件并写入初始计划，下一步将检查最新提交与任务列表。
- 2026-04-12：已检查最新提交 `016d2e30f016c21a977b2a9df382addd5ac6e7ef`，提交信息未显式声明需要先修复的既有问题，因此继续按 `TODO.md` 主线推进。
- 2026-04-12：已定位首个未完成任务为 `T2003c0c1`（LLVM 多 arm handle dispatch：escape-continuation + sibling non-resuming）。

## 范围审计结论

- `T2003c0c1` 当前范围过大，不适合单轮直接实现。
- 依据：
  - 现有代码把 “immediate-resume + sibling escape-continuation” 分成三条独立 lowering：
    - direct-site 路径
    - indirect-site 路径
    - site-matrix 路径（pre/post-immediate、多 site、nested block/if/while、direct/indirect mixed）
  - 当前门禁位于 `codegen_handle_expr_multi_arm`，但真正需要改动的不是单一点，而是这三条 lowering 及其 continuation step 中的 dispatch / handler-stack 摘除与恢复逻辑。
  - sibling non-resuming arms 不仅要在主 body / resumed main path 中参与 dispatch，还要在 immediate arm body、escape arm body、continuation step 执行期间遵守“同源 sibling handler 处于 scope 外”的规则，意味着需要分路径分别处理。

## 拆分方案

- 计划把 `T2003c0c1` 拆成三个子任务：
  1. `T2003c0c1a`：top-level direct single-site 的 escape + sibling non-resuming。
  2. `T2003c0c1b`：single indirect-site 的 escape + sibling non-resuming。
  3. `T2003c0c1c`：site-matrix（pre/post-immediate、多 site、nested block/if/while、direct/indirect mixed）上的 escape + sibling non-resuming。
- 本轮只执行拆分后的第一个子任务 `T2003c0c1a`。

## 当前执行步骤

1. 更新 `TODO.md` 和 `PLAN.md`，把 `T2003c0c1` 拆成子任务并调整依赖顺序。
2. 在 direct single-site 的 mixed-arm escape lowering 中接入 sibling non-resuming dispatch。
3. 补充最小 run-pass 回归，至少覆盖：
   - immediate-resume + escape + `Raise.raise`
   - immediate-resume + escape + custom non-resuming effect
4. 运行针对性测试、全量测试与 lint。
5. 更新计划文件并提交。

## 实施结果

- 已完成 `T2003c0c1a`。
- 入口分流：
  - `codegen_handle_expr_multi_arm` 已新增 `escape + sibling non-resuming` 的子路径选择器。
  - 当前只对 top-level post-immediate single direct escape site 放行；single indirect-site 与 richer site-matrix 继续留给 `T2003c0c1b` / `T2003c0c1c`。
- lowering 改动：
  - top-level single direct escape-site 路径已支持 sibling `Raise.raise` 与 custom non-resuming。
  - 主 body、resumed main path、以及 single-site continuation step 已接入 sibling non-resuming dispatch。
  - immediate arm body、escape arm body，以及 sibling raise/custom catch body 期间，custom sibling direct perform 现会走 cleanup / unwind 路径，避免同源 sibling self-capture。
- 新增回归：
  - `tests/fixtures/run-pass/effect_resume_mixed_escape_raise_direct_single_site.scoop`
  - `tests/fixtures/run-pass/effect_resume_mixed_escape_custom_nonresuming_direct_single_site.scoop`

## 验证结果

- 已通过：
  - `cargo run -p scoop --features llvm -- test`
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`

## 下一步

- 下一轮应从 `T2003c0c1b` 开始：把 sibling non-resuming 扩展到 single indirect-site 的 escape lowering。
