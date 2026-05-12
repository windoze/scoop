## 本次执行计划

1. 读取 `TODO.md`，按标题是否带有 `[DONE]` 确定首个未完成任务。
2. 检查最近一次提交信息是否直接提到与该任务相关且尚未完成的问题；若是，则将其视为当前任务的一部分或在 `TODO.md` 中补成前置任务。
3. 阅读当前任务要求、依赖、验证标准，以及必要的相关代码与测试。
4. 以最小且正确的改动完成该任务；若发现阻塞且无法按规格继续，则先在 `TODO.md` 中补充最小前置任务并停止。
5. 运行该任务要求的验证，以及必要的 `cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`（若作用范围合理且可执行）。
6. 更新 `TODO.md`：将完成的任务标题加上 `[DONE]`，补充完成记录；仅在阶段计划确实变化时更新 `PLAN.md`。
7. 提交本次所有相关改动，提交信息使用当前任务编号。

## 进展记录

- 已创建计划文件，下一步读取 `TODO.md` 并锁定首个未完成任务。
- 已锁定首个未完成任务：`P1-T02R`（Review `stable_id` 基础设施，确认后续阶段已有唯一 authoritative API）。
- 最近一次提交为 `[P1-T02] Unify stable-id keys and hash helpers`，提交信息未显式记录新的未完成阻塞项；本次按 `P1-T02R` 的 review 要求继续审阅。
- 下一步：检查 `stable_id.rs`、`lib.rs`、`rtti/*`、`llvm/codegen/mod.rs`、`itable.rs`，并结合精确搜索确认是否仍有分叉 hash/helper 或 identity 来源未脱离 `ConeId` / path/span / `TypeId`。
- 审阅结论：`stable_id.rs` 已提供后续 P2-P6 所需的唯一 key/hash/mangler/label authoritative API；未发现必须在 `P1-T02R` 前新增的 blocker/prerequisite。
- 允许暂留的旧结构边界已确认并准备回写 `TODO.md`：
  - `TemplateKey` / `InstanceKey` 仅保留为 MIR materialization 内部键，不再承担 exported identity。
  - `StableConeKey::for_virtual_source_path(...)` 仅限单文件 dump / 测试 / manifest-less 虚拟源路径。
  - RTTI closure env 仍使用 `ClosureId` 形状，后续由 `P6-T01` 收尾。
  - `cone/archive.rs` 的 `SOURCES_SHA256` 属于内容 fingerprint，不属于 stable-id 协议 helper。
- 验证完成：
  - `cargo test -p scoopc stable_id -- --nocapture`
  - `cargo test -p scoopc canonical_ -- --nocapture`
  - `cargo test -p scoopc materialize_for_dump_keeps_set_alias_receiver_overload_targets_distinct -- --nocapture`
  - `cargo test -p scoopc`
  - `cargo clippy -p scoopc --all-targets -- -D warnings`
- 精确搜索结论（`crates/scoopc/src`）：
  - `fn stable_hash64` 仅剩 `stable_id.rs`
  - `Sha256::digest` 为 0 命中
  - `stable_template_symbol_suffix` 仅剩 shared helper、两个调用模块与审计/单元测试引用
- 当前下一步：提交 `TODO.md` 与 `memory/claude_plan.md` 的 review 结论，结束本次 invocation。
