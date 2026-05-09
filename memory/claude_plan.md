# 执行计划

## 当前任务
- 本次调用起始任务是 `P8-T04`：在“只有新主线存在”的条件下重跑完整回归矩阵，并锁定最终收口状态。
- 当前结论：`P8-T04` 被一个新发现且未跟踪的前置 blocker 阻塞；本次调用将按规则把该 blocker 写成新的前置任务 `P8-T03a`，更新 `TODO-P8.md` / `TODO.md`，提交 git commit，然后停止。

## 已知约束
- 必须先检查最新提交是否声明了与 `P8-T04` 直接相关的未完事项；若有，需并入当前任务或写入前置依赖。
- 必须按任务要求运行完整矩阵，且若中途修复问题，最终至少再完整重跑一遍整个矩阵。
- 不允许通过恢复 legacy selector、恢复旧主线、添加 hidden fallback、缩小验证范围或改写 fixture 形状来绕过问题。
- 只有在阶段级计划/依赖变化时才更新 `PLAN.md`；常规任务记录只更新 `TODO` 与本文件。

## 执行步骤
1. 运行 `git log -1` 与 `git status --short`，确认最新提交、当前工作区状态以及是否存在需要一并纳入的未提交续作。
2. 如果最新提交信息暴露出与 `P8-T04` 直接相关的未完 blocker，先判断是否需要在 `TODO-P8.md` 中补充前置任务；否则直接继续执行 `P8-T04`。
3. 按 `TODO-P8.md` 要求的顺序运行完整回归矩阵：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `cargo run -p scoop_tools -- spec-fixtures check`
   - `cargo clippy --all-targets -- -D warnings`
   - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
   - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`
4. 若任一步失败：定位根因，做最小正确修复；若问题表明当前任务被新的真实前置缺口阻塞，则在 `TODO-P8.md` / `TODO.md` 中插入最小前置任务并停止。
5. 当前已确认阻塞来自“默认单文件 LLVM artifact/helper 仍走 materialized-HIR entry-main 路径，而 raw materialized MIR backend 不支持 `TerminatorKind::Handle`”。
6. 将该 blocker 写成新的前置任务 `P8-T03a`，把 `P8-T04` 依赖改到该任务，并记录触发证据与验证入口。
7. 撤回本次为验证 blocker 做的试探性代码改动，避免把未完成方案混入提交。
8. 如阶段计划未变化，不改 `PLAN.md`；最后提交 `TODO` / `memory` 更新并停止，等待下一次调用执行 `P8-T03a`。

## 进展记录
- 已读取 `TODO.md` 索引，确认首个未完成任务为 `P8-T04`。
- 已读取 `TODO-P8.md` 中 `P8-T04` / `P8-T04R` 条目，明确本次只执行 `P8-T04`，且需要完整矩阵、最终再跑一遍整矩阵、并附搜索摘要。
- 已在未运行任何 bash/git/cargo 命令前更新本文件，满足“先写计划再执行命令”的要求。
- 已检查最新提交 `2ca6ab3264f517fb1b753790e1e898f33327bfdd [P8-T03R] Close docs cleanup review gaps`；提交信息未声明需要先插入的 `P8-T04` 直接 blocker。
- 已检查工作区状态：当前未提交改动仅为本文件，可在此基线上继续执行完整回归矩阵。
- 第一轮完整矩阵在首步 `cargo test --all` 即失败；失败集中在 `crates/scoopc/src/llvm/tests.rs` 的 effectful/handle 相关测试，统一报 `UnsupportedMainBody { kind: "HIR handle lowering removed; use refactor MIR lowering" }`。
- 根因定位：`emit_minimal_main_ir`、`emit_minimal_main_obj_to_file`、`emit_minimal_main_asm_to_file` 与 `build_minimal_main_module_with_opt_level` 虽然会准备 `materialized_lowered_hir`，但仍只调用 `build_main_module_from_codegen_entry(... from_materialized_lowered_hir ...)`；可达普通函数可经 MIR bridge 发射，而入口 `main` 在缺少 late-lowered stage handoff 时仍回到 `codegen_main_exit_code` 的 HIR body lowering，因此 `main` 中一旦出现 `handle/try` 就会触发已删除的 HIR handle lowering 路径。
- 进一步验证表明：即使把 C `main` 改成调用包级 `pkg.main`，`pkg.main` 自身仍会因为 raw materialized MIR backend 明确不支持 `TerminatorKind::Handle` 而回退到已删除的 HIR handle lowering；说明 blocker 不只是 entry wrapper，而是“默认单文件 LLVM 入口仍建在 materialized-HIR/raw-MIR path 上”。
- 已据此在 `TODO-P8.md` / `TODO.md` 中插入新的前置任务 `P8-T03a`，要求先迁移默认单文件 LLVM artifact 入口与默认测试 helper 到 refactor LLVM stage，再回到 `P8-T04` 重跑完整矩阵。
- 已撤回为验证 blocker 做的试探性代码改动，避免把未完成方案混入 blocker 提交。
- 下一步：检查工作区，仅保留 `TODO` / `memory` 更新，创建 `[P8-T03a]` blocker 提交，然后停止。
