## 当前执行计划

说明：不记录详细私有推理；以下为可审阅的高层执行计划与进度日志。

1. 读取 `TODO.md`，严格按标题是否含 `[DONE]` 判定完成状态，定位第一个未完成任务。
2. 检查最近一次提交信息是否直接指出与该任务相关的未完成事项；若是，则将其视为当前任务的一部分或在 `TODO.md` 中补充为前置任务。
3. 阅读与当前任务直接相关的代码、测试、规范和任务记录，确认依赖、约束和验证要求；避免进行与当前任务无关的开放式排查。
4. 实现当前任务要求的改动；若遇到阻塞当前任务的真实缺陷、缺失特性或规范不匹配，不做变通处理，而是在 `TODO.md` 中添加最小必要前置任务并停止。
5. 运行与当前任务相关的验证，包括必要的定向测试，以及任务要求的更广泛检查（如适用）。若出现失败，立即修复并重新验证。
6. 更新文档记录：
   - 在 `TODO.md` 中将当前任务标题前缀改为 `[DONE]`，并补充完成记录。
   - 仅当阶段计划或依赖结构发生变化时更新 `PLAN.md`。
   - 在本文件中追加关键进度和计划变更。
7. 按仓库约定创建一次 git 提交，提交信息包含任务编号；然后停止，不继续处理下一个任务。

## 进度日志

- 已创建本计划文件并记录初始执行步骤。
- 已读取 `TODO.md` 并确认首个未完成任务为 `P4-T02R`（`P5-T01` 之前的 review 任务不可跳过）。
- 已检查最近提交：最新提交主题为 `Fix doc tests`，未直接指出与 `P4-T02R` 相关的未完成事项，因此不改当前任务顺序。
- 已完成与 `P4-T02R` 直接相关的代码/测试入口定向复核：
  - `AbiMangler` 规则位于 `crates/scoopc/src/stable_id.rs`，导出命名空间为 `__scoop_abi0_{fun|global|type}__<readable>__h<hash128>`。
  - `PrivateSymbolMangler` 规则位于 `crates/scoopc/src/stable_id.rs`，私有 helper 命名空间为 `__scoop_priv0__<role>__h<hash128>`。
  - `crates/scoopc/src/llvm/codegen/mod.rs` 中 `declare_classified_llvm_function(...)` 已把 declaration path 按 `ExportedAbi` / `RuntimeOrNativeImport` / `CompilerPrivateHelper` 分类，并显式要求 exported/runtime 保持 `External`，private helper 使用显式 `Internal`/`Private`/（少量显式 external 的 helper）linkage。
  - `exported_abi_symbol_for_hir_fun(...)` / `exported_abi_symbol_for_materialized_fun(...)` 已统一通过 `AbiMangler` 生成导出 symbol，并对 `main` 保留固定例外。
  - `declare_top_level_fun*`、`declare_materialized_top_level_fun_with_symbol(...)`、`declare_materialized_mir_plain_fun_with_symbol(...)`、effect-lowered plain callable layout 均已接入分类后的 declaration path；closure/object-init/effect helper 继续走 compiler-private helper 路径。
  - 现有测试已覆盖：exported symbol object 审计、`main` 固定 external surface、`@Extern`/runtime import、跨 checkout 路径稳定性、virtual cone collision，以及 source inventory 防止旧 raw callable/private spelling 回流。
- 下一步：运行 `P4-T01/P4-T02` 要求的 path-stability / multi-cone / full `cargo test -p scoopc` 验证，并补跑 `main`/`@Extern`/runtime 例外相关定向测试；若无 blocker，则只更新 `TODO.md` 与本文件并提交。
- 已完成验证：`external_symbol_audit_top_level_and_materialized_generic_smoke`、`external_symbol_audit_closure_effect_and_hidden_init_helpers_smoke`、`refactor_llvm_extern_global`、`function_declaration_helpers_emit_explicit_linkage`、P4 相关 path-stability / multi-cone / ABI namespace 定向测试、`cargo test -p scoopc`、`cargo clippy -p scoopc --all-targets -- -D warnings` 全部通过，未发现需要插入的新前置任务。
- 已将 `TODO.md` 中 `P4-T02R` 标记为 `[DONE]` 并补全完成记录；接下来只需检查工作树、按任务编号提交，然后停止。
