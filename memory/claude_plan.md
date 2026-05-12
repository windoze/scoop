## 当前执行计划

1. 先读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务；不做开放式问题排查。
2. 查看最近一次提交信息，确认是否有与该任务直接相关且明确未完成的事项；若有，将其视为该任务内容或前置依赖。
3. 阅读当前任务要求、依赖、验证标准，以及实现所需的最小相关代码与测试文件。
4. 实现该任务；若遇到阻塞当前任务的真实缺陷或缺失能力，不做绕行，而是在 `TODO.md` 中加入最小必要前置任务并停止。
5. 运行与该任务直接相关的验证；必要时补充或修正测试，直到结果满足任务要求。
6. 更新 `TODO.md`：将已完成任务标题标记为 `[DONE]`，并填写或更新完成记录；仅在阶段计划确有变化时更新 `PLAN.md`。
7. 检查工作区改动，按要求创建一次 git 提交，然后停止，不继续下一个任务。

## 记录约定

- 在识别出当前任务后，补充任务编号、目标、相关文件和验证命令。
- 在关键步骤完成或计划发生变化时，及时更新本文件。
- 这里记录的是简明执行依据与步骤，不记录无关的开放式排查内容。

## 当前任务

- 任务编号：`P1-T02`
- 任务标题：落地 stable key / mangler / label API，并收口仓库内分叉 hash 实现
- 来自 `TODO.md` 的直接要求：
  - 在 `crates/scoopc/src/stable_id.rs` 中补齐 stable key、`AbiMangler`、`PrivateSymbolMangler` 与 stable local label API。
  - 让 `StableConeKey` 基于 cone 名称 / 版本，而不是 `ConeId`。
  - 让 `StableTemplateKey` / `StableInstanceKey` 脱离 `TemplateKey { fqn, source_path, decl_span }` 与 `TypeId` 作为 exported identity。
  - 删除或迁移 `rtti/mod.rs`、`rtti/type_desc.rs`、`llvm/codegen/mod.rs`、`itable.rs` 中分叉的 hash helper。
  - 固定 ABI/private symbol 命名模式并补测试。
- 最近提交检查：`[P1-T01] Add shared stable-id primitives`，提交说明中未显式记录与 `P1-T02` 直接相关的未完成 blocker。

## 预期相关文件

- `crates/scoopc/src/stable_id.rs`
- `crates/scoopc/src/lib.rs`
- `crates/scoopc/src/frontend.rs`
- `crates/scoopc/src/mir/materialize.rs`
- `crates/scoopc/src/rtti/mod.rs`
- `crates/scoopc/src/rtti/type_desc.rs`
- `crates/scoopc/src/llvm/codegen/mod.rs`
- `crates/scoopc/src/itable.rs`
- `TODO.md`

## 预期验证

- `cargo test -p scoopc stable_id -- --nocapture`
- 任务要求中的精确搜索：`fn stable_hash64`、`Sha256::digest`、`stable_template_symbol_suffix`
- 视改动面补跑 `cargo test -p scoopc`
- `cargo clippy -p scoopc --all-targets -- -D warnings`

## 当前实现策略

1. 先在 `crates/scoopc/src/stable_id.rs` 增加统一 stable key trait/struct、mangler、local label API，以及 shared overload-suffix helper。
2. 把 `rtti/mod.rs`、`rtti/type_desc.rs`、`llvm/codegen/mod.rs`、`itable.rs` 的分叉 `stable_hash64` 全部切到 shared `stable_id::stable_hash64(scope, ...)`。
3. 把 `hir/lower/util.rs` 与 `mir/materialize.rs` 的 `stable_template_symbol_suffix` 迁到 shared helper，并让其输入改为 `StableTemplateKey`，不再从 `source_path + decl_span` 构造 exported/stable identity。
4. 为了让 overload suffix 真正使用 cone 名/version，把 `StableConeKey` 从 manifest-aware 调用链传到 HIR lowering / MIR materialize；dump / 单文件 helper 使用虚拟 cone key（文件 stem + `0.0.0`），显式 cone 路径从 `Cone.toml` 提供真实值。
5. 补充 `stable_id` 单元测试与必要调用点测试，最后运行格式化、测试、clippy、grep，并回写 `TODO.md` / git commit。

## 当前状态

- `stable_id` 基础设施已落地，并已接入 HIR overload suffix、MIR materialization、RTTI/itable/runtime type id、LLVM codegen 相关 shared hash 调用点。
- manifest-aware 路径（frontend / `.cone` export / pre-specialize）已显式传递真实 `StableConeKey`；单文件 dump/test helper 使用虚拟 cone key。
- 验证已完成：
  - `cargo fmt`
  - `cargo test -p scoopc stable_id -- --nocapture`
  - `cargo test -p scoopc canonical_ -- --nocapture`
  - `cargo test -p scoopc materialize_for_dump_keeps_set_alias_receiver_overload_targets_distinct -- --nocapture`
  - `cargo test -p scoopc`
  - `cargo clippy -p scoopc --all-targets -- -D warnings`
- grep 状态已收口：
  - `fn stable_hash64` 仅剩 shared `stable_id.rs`
  - `Sha256::digest` 在 `crates/scoopc/src` 中为 0 命中
  - `stable_template_symbol_suffix` 仅剩 shared helper、两个生产调用点和审计测试引用
- 待完成：检查 git diff / git status，创建本任务提交，然后停止。
