## 执行计划

说明：出于安全与协作边界，这里记录可审阅的执行计划、决策依据摘要与进度更新，不记录不可审阅的内部推理细节。

1. 先读取 `TODO.md`，把它当作索引使用。
2. 按索引顺序读取对应的 `TODO-Px.md`，定位第一个标题未带 `[DONE]` 的详细任务。
3. 检查最近一次提交是否直接提到与该任务相关且未完成的问题；如果是，则将其视为当前任务的一部分或作为前置任务处理。
4. 阅读与当前任务相关的代码、测试、规范与任务约束，确认实现边界。
5. 实现当前任务，避免引入变通方案；若遇到阻塞，则在相应 `TODO-Px.md` 中增加最小必要前置任务，并同步 `TODO.md`。
6. 运行相关验证，包括必要的测试、`cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`（若范围允许且与任务相关）。
7. 更新详细任务文件的完成记录，并将任务标题标记为 `[DONE]`；如索引状态有变化，同步更新 `TODO.md`。
8. 仅在阶段计划发生真实变化时更新 `PLAN.md`。
9. 提交本次变更，提交信息使用当前任务编号。
10. 停止，不继续处理下一个任务。

## 进度日志

- 已创建本文件并写入初始计划。
- 已读取 `TODO.md` 与 `TODO-P6.md`，确认第一个未完成详细任务为 `P6-T02`：把 P5 的 `Step` / frame / continuation / resume-interface 合同下沉到 LLVM type/layout lowering。
- 已检查最新提交：`2175dc76 [P6-T01R] Review refactor LLVM stage boundary`。当前未发现需要先于 `P6-T02` 插入的新前置任务；继续按 `P6-T02` 执行。
- 已检查工作区：除当前 `memory/claude_plan.md` 外暂无未提交改动。
- 已阅读 P5/P6 相关代码与设计基线，当前实现状态判断如下：
  - `crates/scoopc/src/llvm/codegen/effect_refactor/` 目前只有占位 `mod.rs`，尚无独立 type/layout materialization。
  - P5 late-lowered IR 已稳定提供 `LateLoweredStepType`、`LateLoweredFrameSchema`、`LateLoweredResumeInterface`、`LateLoweredContinuationObject`、`LateLoweredDynamicInvokeEntry` 等结构，可直接作为 authoritative 输入。
  - 通用 LLVM `TypeId -> CgTy/LLVM type`、GC object header、指针类型与 enum/tuple/struct lowering helper 已存在，可复用。
- 当前执行方案细化为：
  1. 在 `crates/scoopc/src/llvm/codegen/effect_refactor/` 下新增独立 `types.rs` / `layout.rs`，建立 refactor LLVM contract materialization 与查询 API。
  2. 让该查询 API 只消费 P5 late-lowered program + `TypeStore` + 通用 LLVM helper，显式表达 `Step_F`、frame、continuation object、resume interface、dynamic/direct invoke 的 LLVM 形状。
  3. 补充定向单测，覆盖 `Step_F` canonical identity、frame/system slot 映射、continuation method 完整集、`Unit` 零载荷 ABI。
  4. 增加 build fixtures，锁定关键 LLVM IR 片段。
  5. 运行相关测试/fixture/clippy，随后回填 `TODO-P6.md` / `TODO.md` 并提交。
- 已完成：
  - `effect_refactor/types.rs` / `layout.rs` 已落地，refactor LLVM type/layout query API 已接到 `llvm/emit.rs` 的 refactor module build 路径。
  - `refactor_llvm_step_layout_*` / `frame_layout_*` / `continuation_layout_*` / `unit_abi_*` 单测已通过。
- 新发现的 blocker：
  - `scoop build --effect-pipeline refactor --emit-llvm` 主路径当前仍只稳定观察 entry-root production handoff；不可达 effectful helper 不会进入 ABI 物化范围。
  - 若为了让 helper 可达而在 `Pure main` 中引入 self-contained `handle`，当前生成的 `.ll` 会重新出现 legacy `scoop.effect.frame.*` lowering，而不是停留在 `P6-T01a` 规定的 fail-fast / refactor ABI shell 边界。
  - 这使得 `P6-T02` 设计的 build fixtures 目前无法在真实 refactor build 主路径上完成验证；已按规则新增前置任务 `P6-T01b`，并把 `P6-T02` 继续保持未完成。
- 已完成验证：
  - `cargo test -p scoopc refactor_llvm_`
  - `cargo test -p scoopc refactor_llvm_step_layout`
  - `cargo test -p scoopc refactor_llvm_frame_layout`
  - `cargo test -p scoopc refactor_llvm_continuation_layout`
  - `cargo test -p scoopc refactor_llvm_unit_abi`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
  - `cargo run -p scoop -- --effect-pipeline legacy test --fixtures tests/fixtures/build/effect_no_perform_no_handler_symbols_basic.scoop`
- blocker 复现命令摘要：
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_step_enum_single_case.scoop`：当前 build 主路径看不到不可达 effectful helper 的 ABI shell。
  - 把 helper 通过 self-contained `handle` 拉进 reachability 后，生成 `.ll` 会重新出现 legacy `scoop.effect.frame.*` lowering，因此不能作为 `P6-T02` 的合格验证路径。
