# 执行计划与决策记录

说明：我不会写入原始的内部推理全文，但会持续记录足够详细的执行计划、关键判断、进展与变更原因，便于审查当前工作状态。

## 初始目标

本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。若在执行过程中发现前置缺陷、规范不匹配或任务过大，需要先更新计划与任务分解，再按新的首个可执行子任务推进。

## 初始步骤计划

1. 检查最新一次提交，确认提交信息是否提到遗留问题；若有，先修复这些问题。
2. 读取 `TODO.md`，识别第一个未完成任务。
3. 读取 `PLAN.md`，理解现有路线、依赖关系与任务上下文。
4. 判断该任务是否足够小且可在本轮完整完成。
5. 如果任务过大：
   - 细化为更小的子任务；
   - 更新 `PLAN.md`；
   - 更新 `TODO.md`，将原任务替换或补充为有序子任务；
   - 选择新的第一个子任务作为本轮目标。
6. 实现本轮目标任务。
7. 运行相关验证：
   - 最小必要测试；
   - 相关集成/回归测试；
   - 如适用，运行格式化与 `cargo clippy --all-targets -- -D warnings`。
8. 若测试或验证暴露规范缺陷、实现缺口或依赖问题：
   - 不采用规避方案；
   - 在 `TODO.md` 中新增或重排前置修复任务；
   - 更新 `PLAN.md` 记录阻塞原因；
   - 提交这些计划调整后停止。
9. 若任务完成：
   - 更新 `TODO.md` 勾选完成；
   - 更新 `PLAN.md` 反映当前状态；
   - 视需要更新本文件中的进展记录；
   - 提交变更并停止。

## 当前状态

- 已检查最新提交：`[T1510c2] Add stackmap statepoint smoke blocker`。
- 已读取 `TODO.md` / `PLAN.md`。
- 已确认当前首个未完成任务为 `T1510c2`：修复 runtime stackmap statepoint smoke 在 extern/native leaf lowering 后失效。
- 该任务同时满足“最新提交提到的既有问题”和“`TODO.md` 首个未完成任务”两个条件，因此本轮直接处理它。

## 进展记录

- 已创建本文件，作为本轮执行期间的计划与状态记录位置。
- 已确认无需先做新的任务分解：先阅读 smoke fixture、相关 runtime helper、LLVM lowering 与 build regression，再判断是否需要在实现过程中补拆子任务。
- 已完成方案收敛：不回退 `T1510c1` 的 extern/native leaf 合同；改为新增 sysroot 内部 smoke helper，把 stackmap smoke 调用点重新放回 ordinary managed runtime call 主线，以恢复真实 statepoint record。
- 已开始代码修改：
  - `sysroot/core.scoop`：新增 `__scoop_stackmap_statepoint_smoke()` 声明与注释；
  - `crates/scoopc/src/llvm/codegen/{gc.rs,runtime_abi.rs,runtime_symbols.rs}`：新增 intrinsic lowering 与 runtime symbol 声明；
  - `tests/fixtures/run-pass/stackmap_registry_statepoint_smoke.scoop`：切换到内部 helper；
  - `tests/fixtures/build/stackmap_registry_statepoint_smoke_managed_call.scoop`：新增 IR 回归；
  - `runtime/c/scoop_test.c`：更新 helper 注释，明确不能再走 `@Extern` leaf 路径。
- 已完成实现与验证：
  - 手动 `build + run tests/fixtures/run-pass/stackmap_registry_statepoint_smoke.scoop` 输出 `1`；
  - 手动 `--emit-llvm` 检查显示 `@scoop_test_stackmap_statepoint_smoke` 调用点重新被重写为 `llvm.experimental.gc.statepoint`；
  - `extern_enter_native_no_statepoint_writeback.scoop` 的 IR 仍保持 `call void @scoop_enter_native` / `call void @scoop_test_gc_collect_in_native` / `call void @scoop_leave_native`，未回退 leaf/no-statepoint 合同；
  - `cargo run -p scoop -- test`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 通过。
- 待收尾：
  - 已更新 `TODO.md` / `PLAN.md` 标记 `T1510c2` 完成；
  - 下一步整理 `git status`、提交本轮变更，然后停止。
