## 执行计划

说明：我不会写入内部完整推理过程，但会在此持续记录可审计的执行计划、关键判断、进度更新与阻塞信息。

1. 读取 `TODO.md`，定位第一个标题未标记为 `[DONE]` 的任务。
2. 读取该任务相关说明、依赖、验证要求，并检查最新提交是否存在与该任务直接相关的未完成事项。
3. 仅围绕当前任务收集必要上下文，避免开放式问题排查。
4. 实现当前任务；若遇到阻塞当前任务的真实缺陷或缺失特性，则先修复，或在 `TODO.md` 中插入最小必要前置任务并停止。
5. 运行任务要求的验证，以及必要的 `cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`（若范围过大或任务另有明确验证要求，则按任务要求执行并记录结果）。
6. 更新 `memory/claude_plan.md` 记录关键进度与结果。
7. 将当前任务在 `TODO.md` 中标记为 `[DONE]` 并填写完成记录；仅在阶段计划确有变化时更新 `PLAN.md`。
8. 按仓库提交风格创建一次 git 提交，然后停止，不继续下一个任务。

## 进度记录

- 已创建本计划文件，下一步读取 `TODO.md` 确认当前执行任务。
- 已确认首个未完成任务为 `P1-T01`：新增共享 `stable_id` 模块，收口 canonical encoder 与 shared hash helper。
- 当前执行步骤：
  1. 读取 `STABLE_ID.md` 中 §6、§7、§8.1、§9 Phase 1 的要求。
  2. 审查 `crates/scoopc/src/lib.rs` 与仓库内现有 hash / type display / canonical encoding 相关实现，确定最小落地点。
  3. 新增 `crates/scoopc/src/stable_id.rs` 与必要导出，先落 canonical type/effect encoder、shared hash helper、版本化前缀与 dump label 基础 API。
  4. 为 `stable_id` 模块补单元测试，覆盖顺序稳定性、前缀隔离、pretty-text 不参与 canonical 主体。
  5. 运行定向测试，再视结果补充格式化/质量验证。
- 实现进展：
  - 已在 `crates/scoopc/src/lib.rs` 导出 `pub mod stable_id;`。
  - 已新增 `crates/scoopc/src/stable_id.rs`，包含：
    - canonical type/effect encoder；
    - `StableHashScope` + 版本化 SHA-256 helper；
    - 128-bit hex / 64-bit 截断 helper；
    - `stable_dump_label` 基础 API；
    - 显式 `StableTypeParamResolver`，避免 type-param 编码退回到 pretty/path/span。
  - 已补单元测试，覆盖 required shapes、顺序稳定性、前缀隔离、type-param key 显式化、dump label scope 与缺失 resolver 错误路径。
- 验证结果：
  - `cargo fmt`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc canonical_ -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
- 待办：把 `P1-T01` 标记为 `[DONE]`，回写完成记录，并创建 git 提交。
