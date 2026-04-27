# 本轮执行计划

## 目标

本轮只处理 `TODO.md` 中的第一个未完成任务。开始任务前，先检查最新提交是否提到既有问题；如果发现既有问题、回归、规格不一致或实现边界缺口，优先修复或把它作为前置任务插入 `TODO.md` 后停止。

## 执行步骤

1. 查看最新提交信息和变更内容，确认是否提到或引入需要优先处理的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读相关的 `PLAN.md`、规格文档和代码，确认任务边界、依赖和预期行为。
4. 如果任务过大或依赖缺失：
   - 将任务拆分为更小的可执行子任务；
   - 更新 `TODO.md` 和 `PLAN.md`；
   - 提交规划变更并停止。
5. 如果任务可直接完成：
   - 按现有代码风格实现；
   - 添加或更新最小但充分的测试；
   - 运行相关测试，必要时扩展到 `cargo test --all`、fixture 测试或 clippy；
   - 修复测试暴露出的所有相关问题。
6. 完成后更新 `TODO.md` 标记该任务完成，并同步更新 `PLAN.md`。
7. 查看工作区差异，确认只包含本轮相关改动。
8. 提交 Git commit，提交信息使用任务标签和清晰描述。
9. 停止，不继续处理后续任务。

## 过程记录

- 已创建本计划文件，后续在关键发现、计划调整、实现完成、测试完成和提交前继续更新。
- 初始检查已开始：最新提交标题为 `[T5000h2] Enable caller-side MIR pass body rewrites`，提交正文为空，当前尚未发现提交信息中点名的既有问题。
- 已读取 `TODO.md` 任务标题索引；第一个未完成任务定位为 `T5000h3 接入 DirectCallOnly + known provenance 的高阶 wrapper 摊平`。
- 下一步将展开 `T5000h3`、相邻 `PLAN.md` 记录、最新提交中相关改动，以及 MIR pass / summary / pass view 代码，确认实现边界后再编辑代码。
- 已确认 `T5000h3` 的主要落点在 `crates/scoopc/src/mir/inline.rs`：
  - 当前 inliner 只展开 `DirectCall` callee，且 callee body 只能含 direct call；
  - `ParamUseSummary::DirectCallOnly` 已由 `mir/summary.rs` 产生；
  - production pass MIR lowering 当前只支持 `DirectCall`，caller-side pass body 是否发布仍必须由 `pass_publishable_caller_body(...)` 保守把关。
- 实现计划调整为：
  1. 在 inlining pass 内增加 block-local callable provenance，识别 `TopLevelRef`、`MakeClosure`、`Use` 传播，以及 direct-call result summary 中的简单 provenance；
  2. 对 direct-call callee 若存在 `DirectCallOnly` 参数，要求对应调用点实参具备 known direct function 或 known closure provenance；
  3. 展开 callee body 时，把对该参数的 `FunValue` 调用改写为 `DirectCall` 或结构化 `Closure` 调用；
  4. 添加 MIR pass 测试覆盖 direct function provenance 的 production-visible 摊平，以及 known closure provenance 的结构化 rewrite；
  5. 添加 LLVM production 测试确认 direct-function 高阶 wrapper 不再在生成 IR 中保留 wrapper/id 调用边界；
  6. 运行 targeted tests、`cargo test --all` 与 clippy 后更新任务文档并提交。
- 实现已完成：
  - `mir/inline.rs` 增加 basic-block 局部 callable provenance；
  - `DirectCallOnly` 参数只有在调用点 provenance 可知时才触发高阶 wrapper 摊平；
  - wrapper 内部 `FunValue` 参数调用会被改写为 `DirectCall` 或结构化 `ClosureCall`；
  - 对顶层函数值产生的 non-capturing forwarding closure，pass 会归一化成 direct function provenance，并清理随之变成死代码的 closure 构造。
- 验证已完成：
  - `cargo fmt --all`
  - `cargo test -p scoopc mir::inline -- --nocapture`
  - `cargo test -p scoopc production_codegen_observes_direct_call_only_provenance_wrapper_flattening -- --nocapture`
  - `cargo test -p scoopc --no-default-features`
  - `cargo test --all`
  - `cargo run -p scoop -- test`（`fixtures: ok (1201)`）
  - `cargo clippy --all-targets -- -D warnings`
- 已更新 `TODO.md` / `PLAN.md`，将 `T5000h3` 标记完成；下一步复查 diff 并提交。
