# 执行计划

> 说明：本文件记录可审计的执行计划与进度更新，不包含隐私推理链。

## 当前计划

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 判断第一个未完成任务。
2. 读取该任务相关上下文，必要时查看 `PLAN.md`、最新提交和相关源码/测试，但不做开放式历史问题扫描。
3. 按任务要求实现或修复；若发现阻塞当前任务的规范/实现缺口，则在 `TODO.md` 插入最小必要前置任务并停止。
4. 运行与当前任务直接相关的测试；如有必要再运行更广范围验证，修复由当前任务引入或暴露且阻塞任务的失败。
5. 更新 `TODO.md`：完成时在任务标题加 `[DONE]` 并填写完成记录；仅当阶段计划变化时更新 `PLAN.md`。
6. 检查工作区差异，提交本次任务相关变更，然后停止，不继续下一个任务。

## 进度

- 已写入初始计划，下一步读取 `TODO.md` 确认第一个未完成任务。
- 已确认第一个未完成任务为 `P8-T05：迁移 string/native helper 边界并收窄 runtime allowlist`。
- 下一步检查最新提交与工作区状态，然后梳理 `runtime/c/scoop_runtime.c`、`sysroot/lib/scoop.lang.string/src/lang_string.scoop` 和 runtime allowlist 中的 string helper 归属。
- 最新提交 `[P8-T04] Migrate scoop.thread native implementation` 未声明直接相关未完成事项；工作区除本计划文件外无其它修改。
- P8-T05 实施边界：保留 runtime core 中的 canonical `String` descriptor、基础分配/字节访问 substrate、compiler/`scoop.core` 仍需的 primitive-to-string、concat/equality/slice；将 `scoop_string_from_byte_array`、`scoop_string_from_char_array`、`scoop_string_from_string_array` 迁到 `scoop.lang.string` native cone；删除未使用的 `scoop_string_to_float64` runtime-core export。
- 已实施核心迁移草案：新增 `scoop.lang.string/native/scoop_lang_string.c` 和该 cone 的 `[native-build]`；runtime core 暴露最小 String/MutableArray substrate accessors；runtime allowlist 移除数组转字符串和 unused `scoop_string_to_float64`，新增必要底层 accessors。
- 定向验证通过：runtime allowlist 单测、runtime string substrate integration test、`lang_string_builder_basic`、`stdlib_string_basic`、`stdlib_string_methods_extended`、`lang_string_helpers_auto_prelude`、`fstring_desugar_basic`、P8 normal-link symbol regression。
- 全量验证通过：`cargo fmt`、`cargo build`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo run -p scoop -- test`。
- 已更新 `TODO.md`：`P8-T05` 标记为 `[DONE]`，任务索引与当前状态改为下一任务 `P9-T01`，并写入完成记录；`PLAN.md` 未变更。
- 已检查工作区差异，准备提交本次 `P8-T05` 相关变更。
