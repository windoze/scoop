# 当前执行计划（初始）

## 目标

本轮只完成 `TODO.md` 中第一个尚未完成的任务；如果在执行前或执行过程中发现已有缺陷、回归、规范不匹配或实现边界缺失，则优先修复该问题，或将其整理为阻塞当前任务的前置任务并更新 `TODO.md` / `PLAN.md` 后停止。

## 可公开的推理摘要

1. 用户要求在执行任何命令前先写入计划文件，因此先创建本文件。
2. 接下来需要先检查最新提交信息，确认是否提到已知遗留问题；若有，则该问题优先于 `TODO.md` 中任务。
3. 然后读取 `TODO.md`，定位第一个未完成任务。
4. 如果该任务过大或存在前置缺陷，需要先拆分任务并更新 `PLAN.md` 与 `TODO.md`。
5. 在真正实现前，需要阅读相关代码、测试和规范上下文，避免引入变通方案或偏离规范。
6. 实现后必须运行足够测试，并补齐必要文档更新。
7. 最后要更新 `TODO.md`、`PLAN.md`、本文件，并提交一个清晰的 Git commit，然后停止，不继续下一个任务。

## 分步执行计划

1. 查看最新一次 Git 提交，确认是否明确提到待修问题。
2. 阅读 `TODO.md`、`PLAN.md`，找出当前最高优先级未完成项。
3. 评估该任务范围：
   - 如果可在本轮完整完成，直接进入实现。
   - 如果过大，先拆成更小的子任务，更新 `TODO.md` / `PLAN.md`，并以首个子任务作为本轮任务。
   - 如果被已有缺陷阻塞，先把该缺陷作为前置任务插入 `TODO.md` 中正确位置，更新 `PLAN.md`，提交后停止。
4. 阅读实现相关文件、测试和必要规范说明，确认当前行为与目标行为。
5. 修改代码实现任务，必要时补充或调整测试。
6. 运行相关验证，至少覆盖：
   - 与修改直接相关的测试
   - 必要的工作区测试 / lint / 格式检查
7. 更新进度文档：
   - 在 `TODO.md` 中标记该任务完成，或在阻塞情况下重排任务
   - 在 `PLAN.md` 中记录当前状态
   - 在本文件中追加关键进展与计划变化
8. 检查变更后状态并提交 Git commit。
9. 停止，等待下一次调用。

## 进展记录

- 已创建初始计划文件，尚未开始仓库检查。
- 已查看最新提交 `1b368259f6a198fdc929ea750a1d67ad87409860`，提交信息为 `[T5000e3aR] Review panic intrinsic boundary`；提交标题本身未显式提出新的待修遗留问题。
- 已读取 `TODO.md` / `PLAN.md` 并定位到第一个未完成任务为 `T5000e3b 删除待重做的早期 sysroot 模块与对应 tests/fixtures`。
- 已补读 `T5000e3` 上下文与 `T5000e3aR` review，当前没有看到必须先于 `T5000e3b` 插入的新阻塞缺陷；本轮主线确定为移除以下待重做 sysroot surface：
  - `scoop.channels`
  - `scoop.test`
  - `scoop.env`
  - `scoop.fs`
  - `scoop.io`
  - `scoop.net`
  - `scoop.path`
  - `scoop.time`

## 当前细化计划（针对 T5000e3b）

1. 盘点上述模块在以下位置的所有残留：
   - `sysroot/` 模块文件与任何 prelude / import 可见面；
   - `crates/scoopc/` 中相关 lowering、builtin/sysroot 分派、类型检查或其他桥接；
   - `stdlib/`、runtime 或工具代码中的配套桥接；
   - `tests/fixtures/**`、Rust 测试与文档中的引用。
2. 根据盘点结果删除这些 surface 本体，并同步移除所有仅为这些 surface 服务的实现路径。
3. 删除或改写依赖这些模块的 fixture / 测试；若存在仍应保留的语义覆盖，则把测试迁到当前仍受支持的主线，而不是保留旧 API。
4. 更新 `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`README.md`、`TODO.md`、`PLAN.md` 中相关口径。
5. 运行格式化、相关测试、全量测试和 `clippy`，修复所有暴露出的既有问题。
6. 完成后把 `T5000e3b` 标记为完成，提交 Git commit，然后停止。
## 2026-04-26 接续收尾计划

- 接管上一轮已启动的 `cargo test --all` 会话，确认删除早期 sysroot surface 后工作区仍全量通过。
- 在全量测试完成后重新运行 `cargo clippy --all-targets -- -D warnings`，确保本轮变更没有留下 warning。
- 若验证全部通过，则更新 `TODO.md`、`PLAN.md` 与本文件，记录 `T5000e3b` 的完成状态与验证结果。
- 最后检查工作区、提交本轮变更，提交信息使用 `[T5000e3b] Remove early sysroot surfaces pending redesign`，然后停止，不继续下一条任务。

## 最终结果（T5000e3b）

- `cargo test --all` 已完整通过，`cargo clippy --all-targets -- -D warnings` 也已通过，删除早期 sysroot surface 后工作区当前处于可提交状态。
- 本轮已完成的主体收口：
  - 删除 `scoop.channels`、`scoop.test`、`scoop.env`、`scoop.fs`、`scoop.io`、`scoop.net`、`scoop.path`、`scoop.time` 的 sysroot / stdlib surface；
  - 删除编译器中只为这些 surface 服务的 LLVM lowering、runtime ABI / symbol 声明与 runtime C 导出；
  - 删除对应 fixtures，并把仍有价值的覆盖迁回 `scoop.sync` / `scoop.thread` / `require(...)` 等仍受支持主线。
- 收尾验证时顺手修复的既有问题：
  - `crates/scoop/src/commands/build.rs` 仍引用已删除的 `std_channels_basic.scoop`；
  - `runtime/c/scoop_runtime.c` 在 surface 删除后残留未使用符号，导致 warning；
  - HIR/MIR golden 因 `TypeId` 重排而失配，已批量重生快照。
- 已回写 `TODO.md` 与 `PLAN.md`：`T5000e3b` 现已标记为完成，下一条待执行任务为 `T5000e3bR Review：确认 sysroot surface 已实质缩回到仍承诺维护的最小集合`。
- 下一步只剩检查最终 diff、提交 `[T5000e3b] Remove early sysroot surfaces pending redesign`，然后停止。
