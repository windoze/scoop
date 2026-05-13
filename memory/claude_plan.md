## 当前执行计划

说明：不写入内部推理细节，改为维护可审阅的执行计划、关键决策、阻塞项与完成进度。

1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务；以它作为本次唯一执行目标。
2. 查看最近提交信息，确认是否存在与该任务直接相关且明确未完成的事项；若存在且会阻塞当前任务，则将其视为当前任务一部分或在 `TODO.md` 中补成前置任务。
3. 阅读当前任务条目中的要求、依赖、验证标准与完成记录，并检查相关代码与测试位置。
4. 实现该任务要求的改动；如遇到会阻塞任务完成的真实缺陷或缺失能力，不采用变通方案，而是在 `TODO.md` 中补充最小必要前置任务并停止继续推进。
5. 运行与当前任务直接相关的验证；若任务本身或仓库规范要求更广泛验证，则补充运行对应命令，并修复发现的问题。
6. 更新文档记录：
   - 在 `TODO.md` 中将已完成任务标题改为带 `[DONE]`，并补全完成记录。
   - 仅当阶段计划真的变化时才更新 `PLAN.md`。
   - 持续更新本文件，记录进展、关键决策、验证结果与阻塞信息。
7. 按仓库约定提交本次改动，提交信息以当前任务号开头。
8. 完成首个未完成任务后立即停止，不继续处理后续任务。

## 进度日志

- 已初始化执行计划，下一步读取 `TODO.md` 与最近提交信息。
- 已确认首个未完成任务为 `P3-T01`：`收口 raw MIR terminator/call-kind/PerformResult route policy`。
- 最近提交 `21e2b84f [P2-T03] Close materialized MIR handoff gaps` 未显式记录与 `P3-T01` 直接相关的未完成事项，因此本次直接按 `P3-T01` 执行。
- 下一步：阅读 `PIPELINE_GAPS.md` 中 `§3.1`、`§3.2`、`§3.3`、`§3.6` 以及 `crates/scoopc/src/llvm/codegen/mir_body.rs`、raw-route gate 测试入口，定位 production path 上仍然晚期失败或默认值兜底的分支。
- 实现方案已收敛为：
  1. 在 `crates/scoopc/src/llvm/codegen/mir_body.rs` 增加 raw MIR route verifier，扫描 body 中会进入 raw emitter 的 terminator / call-kind / `PerformResult` 形状，并在 body emission 之前以 `BackendGate` 拒绝。
  2. 删除 raw `PerformResult` 默认值路径，并把 raw `Perform` / `Virtual` / `Interface` / `Resume` / `Handle` / `ResumeUnwind` / raw `Todo terminator` 的晚期 `UnsupportedMainBody` 改成 backend-gate impossible-state guard，避免继续向用户暴露“尚未支持”。
  3. 补充 raw-route 定向单测，覆盖 unsupported call kind、`PerformResult`、effect/control terminator 的 gate 行为。
  4. 回写 `crates/scoopc/src/llvm/codegen_gap_inventory.rs`、`PIPELINE_GAPS.md`、`TODO.md`，并按测试结果同步必要的审计基线（例如 failure-policy `UnsupportedMainBody` 计数）。
- 已完成实现与验证：
  - raw-route gate、inventory 回写、gap 文档回写、failure-policy/pipeline-gap audit 基线同步均已完成。
  - 已通过验证：`cargo test -p scoopc refactor_llvm_raw_route_gate -- --nocapture`、`cargo test -p scoopc raw_mir_effect_control_route -- --nocapture`、`cargo test -p scoopc codegen_gap_inventory`、`cargo test -p scoopc pipeline_gap_audit`、`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`、`cargo test -p scoopc refactor_llvm_backend_gate`、`cargo run -p scoop -- test --fixtures tests/fixtures/build/emit_llvm_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/build/effect_refactor_direct_handle_resume_emit_llvm.scoop`、`cargo clippy --all-targets -- -D warnings`。
  - `TODO.md` 已把 `P3-T01` 标记为 `[DONE]` 并写入完成记录。
- 下一步：检查工作区 diff，整理提交信息，以 `P3-T01` 为前缀创建本次提交，然后停止。
