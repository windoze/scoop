# 执行计划

说明：我不能写出“完整内部思维链”，但会在此文件持续记录可审阅的执行计划、关键判断依据、进度和变更。

## 当前目标

按 `TODO.md` 的顺序完成第一个未完成任务；如果发现前置缺陷、规范不匹配或任务过大，则先调整 `TODO.md` / `PLAN.md`，提交后停止。

## 初始步骤

1. 检查最新一次 Git 提交，确认提交信息或上下文里是否提到已知遗留问题。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`，核对该任务的上下文、依赖和现有计划。
4. 评估该任务是否可以在本轮完整落地。
   - 如果过大：把任务拆成更小子任务，更新 `PLAN.md` 和 `TODO.md`，本轮执行第一个子任务。
   - 如果被规范缺口或实现缺陷阻塞：先把阻塞项显式写入 `TODO.md` 并调整顺序，更新 `PLAN.md`，提交后停止。
5. 若可执行：实现任务并补充/修正必要测试。
6. 运行相关验证，至少覆盖直接相关测试；如任务影响面较大，再补跑更广的测试与 `cargo clippy --all-targets -- -D warnings`。
7. 更新文档与计划：
   - 在 `TODO.md` 中标记任务完成或重排阻塞依赖。
   - 在 `PLAN.md` 中记录当前状态与后续影响。
   - 在本文件中记录关键进展。
8. 用清晰的 Git 提交信息提交本轮变更，然后停止。

## 执行记录

- 已创建本文件并写入初始计划。
- 已检查最新提交 `79867e80b1e51d0deb9e4e964508ac82d0a0b3a6`，提交信息未显式声明新的遗留缺陷。
- 已定位 `TODO.md` 首个未完成任务原为 `T2003c0b2c3d`。
- 经过代码审计，确认该任务当前同时跨越三类不同 CFG / replay 问题：
  1. 同一个 top-level nested block 语句中的 mixed direct / indirect site 续跑；
  2. 同一个 `if` 语句中的 mixed direct / indirect site 合流；
  3. 同一个 `while` 语句中的 mixed direct / indirect site + loop re-entry。
- 现有 `mixed_escape_matrix` 还存在两类具体门禁：
  1. `escape_site_pcs_by_stmt_idx` 分类阶段直接拒绝 `multiple sites per top-level statement`；
  2. state0/state1/step 对同一语句索引默认只处理一种 site 类型。
- 已据此把 `T2003c0b2c3d` 拆为 `T2003c0b2c3d1` / `d2` / `d3`，本轮执行 `T2003c0b2c3d1`：
  same-top-level nested block 中的 single direct + single indirect 共存。
- `T2003c0b2c3d1` 当前实现思路：
  1. 保留 richer mixed 形状的稳定诊断，只放开同一个 nested block 语句里的 single direct + single indirect。
  2. 在 mixed escape site 分类阶段保留源码顺序，并记录“当前 block-site 的下一个同 stmt site”。
  3. 为 state0/state1/step 增加 block 专用续跑 helper：从当前 nested block site 继续走到同 stmt 的下一个 mixed site。
  4. 新增 pre-immediate / post-immediate run-pass fixtures，验证 direct-first 与 indirect-first 的最小闭环。

## 接手续跑记录

- 已接手上一轮未完成状态，当前仍以 `T2003c0b2c3d1` 为唯一执行目标。
- 已知最新阻塞不是语法/类型问题，而是 LLVM IR 在 `__scoop_mixed_escape_matrix_step__main_0` 中出现无终结符 basic block。
- 上一轮已经在 `step trampoline` 的 `current indirect -> next direct` 分支上引入 `current_site_escaped = true`，意图让该路径在命中下一个 direct site 后不再落入当前 site 的 no-escape 尾收口。
- 我本轮的执行顺序：
  1. 先重新格式化并重新编译 `effect_resume_mixed_escape_pre_immediate_block_indirect_direct.scoop`，验证无终结符问题是否已消失。
  2. 如果首个样例通过，再编译 `effect_resume_mixed_escape_post_immediate_block_direct_indirect.scoop`，检查另一种顺序是否仍有 IR 或行为问题。
  3. 若编译通过，则运行两个新样例，生成并写入对应 `.stdout`。
  4. 移除 `crates/scoopc/src/llvm/mod.rs` 中的临时 IR 调试输出，避免污染正常失败路径。
  5. 跑相关验证：至少覆盖新样例、相关 `scoop -- test`、`cargo test --all` 与 `cargo clippy --workspace --all-targets -- -D warnings`。
  6. 全部通过后，更新 `TODO.md` / `PLAN.md` / 本文件，标记 `T2003c0b2c3d1` 完成并提交。
- 进展更新：
  1. 已修复 step trampoline 中 `current indirect -> next direct` 路径在已有 terminator 后继续发射后续 stmt IR 的问题；最小 `pre` 样例已不再触发 LLVM verifier 的无终结符错误。
  2. `post` 样例随后暴露出更细的 body-lift 缺口：第二个 sibling site 为 indirect 时，恢复该 site 需要 replay “前一个 direct 之后到当前 indirect 之前”的 block 前缀，但原分析没有把这段前缀用到的 locals 纳入 `body_lift_ids`。
  3. 已新增静态分析 helper，把上述 block-prefix 依赖并入 `body_lift_ids`；`post` 样例第二次恢复现已能正确 replay `after_direct` 前缀并继续进入 `fetch_resume`。
  4. 两个新 fixture 已改成与现有 multi-step mixed-escape 回归一致的 `Cell.k` 观测方式，避免把“step 函数写回主函数栈上局部变量”这一独立语义混入当前任务验收面。
  5. 已补新 fixture 的 `.stdout`，并移除了 `crates/scoopc/src/llvm/mod.rs` 中的临时 IR 调试输出。当前进入完整测试 / lint / 文档更新阶段。
  6. 全量验证已通过：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 均成功。
  7. `T2003c0b2c3d1` 现可视为完成；下一轮首个未完成任务应为 `T2003c0b2c3d2`。
