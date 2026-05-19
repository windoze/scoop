# 当前执行计划

## 约束
- 只处理 `TODO.md` 中第一个标题未带 `[DONE]` 的任务，完成后停止。
- `TODO.md` 是任务顺序、依赖、验证要求和完成记录的唯一权威来源。
- 不做开放式历史问题扫描；只处理当前任务直接相关或阻塞的问题。
- 如遇到阻塞当前任务的缺失功能、规格不符或实现边界，先在 `TODO.md` 中插入最小必要前置任务并停止。
- 完成后需要更新 `TODO.md`，运行相关验证，并提交 Git commit。

## 步骤
1. 读取 `TODO.md`，定位第一个标题没有 `[DONE]` 前缀的任务。
2. 查看该任务的依赖、验收标准和完成记录；必要时查看最近提交是否提到与该任务直接相关的未完成问题。
3. 基于任务内容读取相关代码、测试和文档，确认最小正确实现范围。
4. 实现当前任务；如果发现阻塞任务的规格缺口或前置依赖，更新 `TODO.md` 并停止。
5. 添加或更新最贴近该任务的测试/fixture。
6. 运行任务要求的验证命令和必要的补充测试；如失败则修复并重跑。
7. 将当前任务标题标记为 `[DONE]`，更新完成记录。
8. 检查工作区差异，确保只包含本任务相关变更。
9. 按仓库风格创建包含本任务变更的 Git commit。
10. 停止，不继续下一个任务。

## 进度
- 已写入初始执行计划。
- 已读取 `TODO.md`，第一个未完成任务为 `P2-T02`：有 body 的 `@CallingConvention` 生成 object-level native callable symbol。
- 当前只围绕 `P2-T02` 检查相关近期提交、实现入口、测试和验证命令。
- 设计决策：`@CallingConvention` body 函数继续保留普通 managed Scoop ABI 入口；LLVM 后端额外生成一个 C ABI wrapper/object symbol，wrapper 直接调用普通入口，不插入 `enter_native` / `leave_native`。
- 待实现项：新增 HIR side table 记录 `@CallingConvention` body 函数；typecheck 对 effect/generic/GC ref native surface 做 gate；LLVM 根据 side table emit wrapper；添加 IR、typecheck negative 和 cone-local C link/run fixture。
- 已完成初版代码改动：新增 `NativeCallableFunIndex`，lowering 收集 `@CallingConvention` body metadata，typecheck 支持 `name` + `convention` 参数并添加 native surface/effect/generic gate，LLVM emit C ABI wrapper symbol。
- 已添加定向 fixture：IR symbol 检查、GC ref/effect negative、cone-local C object 链接引用 wrapper symbol。
- 定向验证已通过：HIR side-table 单测、`calling_convention_body_symbol_emit_llvm` build fixture、完整 `tests/fixtures/typecheck/`、`run_pass_cone/c_sources_calling_convention_body_link`。
- 当前进入仓库级验证：`cargo build`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、完整 fixture suite。
- 仓库级验证完成：`cargo build`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo run -p scoop -- test` 均通过；首次 `cargo test --all --all-targets` 使用 120s 超时不足，已用 600s 重跑通过。
- 下一步：回写 `TODO.md`，将 `P2-T02` 标记 `[DONE]` 并记录完成范围和验证结果。
- 已回写 `TODO.md`：`P2-T02` 标记为 `[DONE]`，任务索引状态更新为 `[DONE]`，当前状态更新为下一任务 `P2-T03`，并补充完成记录。
- 下一步：检查 git diff/status，确认变更范围后提交。
- 最终补充验证完成：后端 duplicate-symbol 诊断调整后，定向 build/run-pass-cone fixture、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo run -p scoop -- test` 已全部重跑通过。
