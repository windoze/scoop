# 执行记录

## 约束说明

- 按要求先写入本文件，再执行仓库检查与实现工作。
- 不记录原始逐字思维过程；改为记录可公开的决策摘要、执行计划、进度与变更原因。
- 本次目标是：先检查最新提交是否提到需先修复的既有问题；若有，优先修复。否则读取 `TODO.md`，完成第一个未完成任务，然后更新 `TODO.md` / `PLAN.md`，提交 git commit，并停止。

## 初始执行计划

1. 查看最新一次提交信息，确认是否明确提到已有缺陷、回归或待优先修复的问题。
2. 读取 `TODO.md` 与 `PLAN.md`，识别第一个未完成任务，并确认其依赖关系与当前计划是否一致。
3. 如果首个未完成任务过大，先把它拆成更小的可执行子任务，并同步更新 `TODO.md` 与 `PLAN.md`；本次只执行拆分后的第一个子任务。
4. 在实现前检查相关代码、测试、规格或夹具，确认不存在被该任务触发的既有问题；一旦发现，优先修复或将其以前置任务形式插入 `TODO.md`。
5. 实现当前任务，保持实现符合规格，不采用绕过缺陷的临时方案。
6. 运行与改动相关的格式化、测试、必要的 lint / clippy，修复发现的问题直到通过，或在无法继续时把阻塞项显式前移到 `TODO.md`。
7. 更新 `memory/claude_plan.md`、`TODO.md`、`PLAN.md` 记录结果。
8. 使用清晰的提交信息提交本次改动，然后停止，不继续下一个任务。

## 进度

- 已完成：初始化计划文件。
- 已完成：检查最新提交、`TODO.md`、`PLAN.md`。
- 已确认：
  - 最新提交 `[T5000b3R] Review codegen theme split boundary` 的提交信息没有显式声明需要先修复的既有缺陷。
  - 当前第一个未完成任务是 `T5000b4 继续拆分 MainCodegen 为 module / function / cache / effect emitter 上下文`。
  - `T5000b4` 单轮范围过大，需要先拆分为更小子任务。
  - 拆分依据：
    - `crates/scoopc/src/llvm/codegen/mod.rs` 仍有 7185 行；
    - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 仍有 5923 行，且存在多处手动保存/恢复 `MainCodegen` 状态；
    - `layout.rs` / `ty.rs` 仍独占 `type_layout_cache`、`option_niche_cache`、`enum_cg_layout_cache`、`class_init_layout_cache`、`pack_field_indices` 等缓存，适合作为第一个独立切口。
- 当前计划调整：
  1. 先把 `T5000b4` 拆成更小的 `T5000b4a` / `T5000b4b` / `T5000b4c` 子任务，并更新 `TODO.md` / `PLAN.md`。
  2. 本轮执行第一个子任务：抽出编译单元级共享 layout / suspend-analysis cache。
  3. 代码完成后运行 `cargo fmt --all`、`cargo test -p scoopc llvm::`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
  4. 若测试通过，则回写 `memory/claude_plan.md`、`TODO.md`、`PLAN.md` 的完成记录，提交 commit 后停止。
- 已完成：将 `T5000b4` 拆成 `T5000b4a` / `T5000b4b` / `T5000b4c`，并回写 `TODO.md` / `PLAN.md`。
- 已完成：实现 `T5000b4a`。
  - 关键改动：
    - 在 `crates/scoopc/src/llvm/codegen/mod.rs` 中新增 `SharedCodegenCaches`；
    - `CompilationUnitCodegenCx` 现持有编译单元级共享 cache；
    - `MainCodegen` 已删除 layout / suspend-analysis cache 字段；
    - `layout.rs`、`ty.rs`、`effect/state_machine_plan.rs`、`codegen/mod.rs` 的相关读写路径均已切换到共享 cache；
    - `cg_enum_layout(...)` 改为返回从共享 cache 克隆出的 `CgEnumLayout`，以适配 `RefCell` 化后的缓存访问。
- 已完成：验证当前实现。
  - `cargo fmt --all`
  - `cargo test -p scoopc llvm::`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - 结果：全部通过。
- 下一步：
  - 更新 git 状态并提交本轮改动。
  - 本轮停止；下一次调用应从 `T5000b4aR` 开始。
