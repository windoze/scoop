# 本轮执行计划

## 约束说明

- 本轮只处理 `TODO.md` 中当前排在最前面的未完成任务，完成后立即停止。
- 在开始任务前，先检查最新提交是否提到任何既有问题；若提到，则先修复这些问题。
- 如果在探查、实现、测试过程中发现任何既有缺陷、规格不匹配、未完成实现边界或依赖缺口，必须优先修复；若当前无法直接修复，则需要先把该问题作为前置任务写入 `TODO.md` 并更新 `PLAN.md`，随后提交并停止。
- 不接受规避性实现、夹具专用 hack、缩小范围或改变建模来绕过问题。

## 初始步骤

1. 检查最新一次 Git 提交，确认提交说明中是否提到需要先处理的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认当前计划与 `TODO.md` 是否一致。
4. 判断该任务是否足够小且可在本轮内完整完成：
   - 如果可以，直接实施。
   - 如果不可以，先把任务拆成更小的子任务，更新 `PLAN.md` 与 `TODO.md`，并把新的第一个子任务作为本轮目标。

## 实施步骤

1. 阅读与目标任务直接相关的代码、测试、规格或文档。
2. 在不引入变通方案的前提下实现任务。
3. 运行相关测试、格式化和必要的静态检查；若发现问题，立即修复。
4. 若执行过程中计划发生变化，或关键步骤完成，及时更新本文件。

## 收尾步骤

1. 将已完成任务在 `TODO.md` 中标记完成。
2. 更新 `PLAN.md`，反映当前状态、已完成内容和后续顺序。
3. 检查工作区改动，确认仅包含本轮应提交内容。
4. 使用清晰的 Git 提交信息提交改动。
5. 提交后停止，不继续处理下一个任务。

## 当前状态

- 已完成：创建本轮计划文件。
- 已完成：检查最新提交；最新提交说明只有 `Update plan`，未额外声明需要先修复的既有问题。
- 已完成：读取 `TODO.md` 与 `PLAN.md`；当前首个未完成任务为 `T5000a 建立编译器性能与 codegen 边界基线`，任务规模可在本轮直接完成，无需先拆分。
- 已完成：收集 baseline 证据，当前已确认的关键事实包括：
  - `crates/scoopc/src/llvm/codegen/mod.rs` 约 17759 行；
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 约 10322 行；
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 约 5923 行；
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_segments.rs` 约 5085 行；
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_transform.rs` 约 4988 行；
  - `crates/scoopc/src/llvm/mod.rs` 约 3835 行。
  - `MainCodegen::new` 不只在模块入口构造，还在 wrapper/object init/top-level immutable init/closure lowering 等路径重复构造。
  - `-O0` 路径仍固定执行 `function(sroa),rewrite-statepoints-for-gc`，同时 `run_pass_pipeline` 还固定开启 `verify_each(true)`。
  - `debug_assertions` 下 `build_unified_lowering_contract` 会做额外校验与 round-trip 检查。
  - reachability 在 `collect_reachable_top_level_funs` 之后，还会做 struct member eager inclusion 与 generic member monomorphized eager inclusion，并再次扫描 `fun_index`。
  - 单态目标解析、outward-effect/suspendability 判断、handle/state-machine planning 仍直接挂在 LLVM codegen 查询路径上。
- 已完成：已将 baseline 与 guardrail 固化到 `OPTIMIZATION.md` 第 0、10、11 节。
- 已完成：已更新 `PLAN.md` 当前进度，明确下一条待执行任务为 `T5000aR`。
- 已完成：已在 `TODO.md` 中将 `T5000a` 标记完成，并记录 baseline 的归档位置。
- 已完成：运行 `cargo test --all`，全量测试通过。
- 已完成：运行 `cargo clippy --all-targets -- -D warnings`，零 warning 通过。
- 已完成：检查工作区差异，当前仅包含 `OPTIMIZATION.md`、`PLAN.md`、`TODO.md` 与本文件的本轮改动。
- 进行中：准备提交本轮改动，提交后立即停止。

## 下一步

1. 提交本轮改动并停止。
