## 本次执行计划

1. 读取 `TODO.md`，确认第一个未完成任务（仅标题前缀带 `[DONE]` 的任务才算完成）。
2. 查看最近一次提交，判断是否存在与该任务直接相关且明确标注未完成的内容；如有，则将其视为当前任务的一部分或在 `TODO.md` 中补充为前置依赖。
3. 阅读当前任务及其依赖涉及的代码、测试、文档，确认约束、验收标准与现状。
4. 实现当前任务；若遇到阻塞当前任务且必须先修复的问题，则在 `TODO.md` 中以最小必要粒度添加前置任务并停止在该前置整理完成处。
5. 运行与当前任务直接相关的验证，包括必要的测试、格式化、lint；修复执行中发现的直接相关问题。
6. 更新 `memory/claude_plan.md` 记录关键进展与计划变更。
7. 按要求更新 `TODO.md`：将已完成任务标题加上 `[DONE]`，补全完成记录；仅当阶段计划确有变化时才更新 `PLAN.md`。
8. 检查工作区变更，使用清晰的提交信息创建一次 git 提交，然后停止，不进入下一个任务。

## 进度记录

- 已创建本次执行计划。
- 已读取 `TODO.md`，确认首个未完成任务为 `P7-T01B：收口剩余 sanitize/type-display/TypeId 驱动的 private LLVM type/global 命名`。
- 已检查最近一次提交：`[P7-T01R] Add private metadata naming prerequisite`。该提交用于把当前任务显式补为前置依赖，当前无需再新增前置任务即可进入实现分析。
- 下一步：阅读 `P7-T01B` 涉及的代码入口与 stable-id/private mangler 基础设施，确认现有命名来源与可复用 helper，再实施最小但成组闭合的改动。
- 已完成实现主线改动：
  - 为 `PrivateSymbolMangler` 增加可复用的 private type-name 生成能力，并引入 `CanonicalTextKey` 作为 ad-hoc canonical key 包装。
  - 已将 boxed-enum、class/object runtime metadata、itable/vtable、MIR capture/value box、enum boxed payload、composite transport descriptor 的 private LLVM type/global naming 改到 stable semantic key + hashed private family。
  - 已同步更新一批 LLVM/pipeline/fixture 测试，使其验证 private family / 结构语义，而不再绑定旧 sanitize/type-display/TypeId 拼写。
- 已完成验证：
  - `cargo fmt`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id_source_inventory -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc runtime_type_primitives -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc composite_transport -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc object_member_call_uses_gc_managed_singleton_receiver -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_value_boxing_transport -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_enum_payload_transport -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc external_symbol_audit_closure_effect_and_hidden_init_helpers_smoke -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
- 已回写 `TODO.md`：`P7-T01B` 已标记为 `[DONE]`，并补全改动范围、核心决策、验证结果与对应闭合说明。
- 下一步：检查最终 diff，按任务要求提交本次所有未提交改动，然后停止。
