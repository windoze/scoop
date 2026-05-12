# 执行计划

说明：不记录逐字内部推理；这里保留可审计的高层计划、关键判断依据与执行进度。

## 初始计划

1. 读取 `TODO.md`，按标题是否带有 `[DONE]` 识别首个未完成任务。
2. 查看最近一次提交信息，确认是否存在与该任务直接相关且明确未完成的事项；若有，将其视为当前任务的一部分或在 `TODO.md` 中补充为前置依赖。
3. 阅读当前任务在 `TODO.md` 中的完整要求、依赖、验证要求与完成记录；必要时再查看 `PLAN.md` 仅确认阶段级依赖是否需要调整。
4. 检查相关代码与测试位置，确认最小正确修改范围，避免引入规避性实现。
5. 实现当前任务；若发现阻塞当前任务且必须先修复的问题，则在 `TODO.md` 中添加最小必要前置任务，更新依赖顺序，并停止继续后续任务。
6. 运行任务要求的验证，以及必要的构建、测试、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`；若失败则立即修复。
7. 更新 `memory/claude_plan.md` 记录进度与关键决策。
8. 将当前任务在 `TODO.md` 中标记为 `[DONE]` 并补全完成记录；仅当阶段计划确实变化时更新 `PLAN.md`。
9. 按仓库约定创建一次 git 提交，提交信息包含任务编号；然后停止。

## 进度记录

- 已创建计划文件，下一步：读取 `TODO.md` 并识别首个未完成任务。
- 已读取 `TODO.md` 与最新提交信息，识别首个未完成任务为 `P0-T02B：清理剩余 stable-id 敏感 LLVM / pipeline 测试中的当前 callable symbol 字符串绑定`。
- 最新提交主题为 `[P0-T02R] Add prerequisite for callable symbol bindings`，说明当前任务已被明确前置化；暂无额外历史任务需要插入到它之前。
- 下一步：定位 `P0-T02B` 指向的测试与 helper，审查是否仍通过当前 callable symbol 文本定位函数/调用点，并据此设计最小且完整的迁移方案。
- 已审查到两类主要绑定：
  1. `llvm/tests.rs` 中通过 `function_ir_named("@a.*(")`、`ir.contains("define ... @a.*(")`、`find("@a.take(")` 等直接用当前 callable symbol 文本定位函数或调用点。
  2. `pipeline/llvm_codegen_stage.rs` 中 `refactor_llvm_function_abi_entry_shells_use_refactor_direct_entry` 直接锁定 `sample.effectEntry` 与 `__scoop_refactor_direct_invoke__/dynamic_invoke` 的当前 spelling。
- 具体实现策略：
  1. 在测试侧补少量 IR helper，用函数头/函数体结构匹配目标函数，并可从匹配结果提取当前实际 symbol 供“调用关系”断言使用，但不再把某个固定字符串当作金标准。
  2. 把 `llvm/tests.rs` 中所有同类顶层 callable 绑定一起迁移，覆盖任务列出的入口及相邻同类测试，避免留下同根问题。
  3. 为 `pipeline` 测试增加本地 IR matching helper，改为验证“dynamic shell 与 main wrapper 都只转发到语义上识别出的 direct-entry shell”。
  4. 扩大 source inventory，使其同时覆盖 `llvm/tests.rs` 与 `pipeline/llvm_codegen_stage.rs` 中本轮移除的硬编码 callable symbol 断言。
- 已完成代码改动：
  - `crates/scoopc/src/llvm/tests.rs`：
    - 把 `float_builtin_types_lower_to_llvm_scalars`、`direct_*`、`closure_call_without_outward_effect_*`、`direct_hir_reachability_*`、object/top-level init、managed explicit-frame、ctor factory、deferred/aggregate rebuild、hidden-sret、explicit-frame layout 等当前 callable symbol 绑定迁移为结构/调用关系断言。
    - 新增/扩展 IR helper：call target 解析、defined global 查询、descriptor -> offsets 解析、实际 symbol 提取、user-callable 角色判定、测试侧 LLVM ident sanitize。
    - 扩大 stable-id source inventory，现同时防回归 `llvm/tests.rs` 与 `pipeline/llvm_codegen_stage.rs` 的 callable symbol 硬编码断言。
  - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`：
    - 新增本地 IR matching / symbol / call-target helper。
    - `refactor_llvm_function_abi_entry_shells_use_refactor_direct_entry` 改为通过 defined-function call graph 识别 direct-entry shell 与 dynamic shell，不再锁定 `sample.effectEntry` / `__scoop_refactor_*` 当前 spelling。
- 调试记录：
  - 第一轮定向测试发现 2 个 matcher 过窄（latent wrapper 与 closure direct-call）。已改为基于参数形状、closure helper family、closure env / dynamic-entry family 等结构特征匹配。
  - 第一轮全量测试又暴露 5 个“误匹配到 user `a.main` 或过度依赖局部 IR 细节”的问题；已进一步收紧到返回形状、是否调用 `println`、ctor root marker、aggregate field extract、actual caller graph 等特征。
- 当前验证结果：
  - `cargo fmt`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id_source_inventory -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc direct_call_ -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc explicit_root_frame -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_function_abi_entry_shells_use_refactor_direct_entry -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
- 下一步：把 `P0-T02B` 在 `TODO.md` 标记为 `[DONE]` 并写回完成记录，然后创建提交并停止。
