# Claude Plan

## 说明

按本次任务要求，这里先记录一份可审计的执行计划、判断依据摘要与后续进度日志。内容会在执行过程中持续更新。

## 初始目标

1. 检查最新提交是否提到任何既有问题；若有，优先修复。
2. 读取 `TODO.md`，确定第一个未完成任务。
3. 如果该任务过大，先拆分任务并更新 `PLAN.md` / `TODO.md`，然后只执行拆分后的第一个子任务。
4. 在执行当前任务的过程中，任何发现的既有缺陷、回归、规范不匹配、未完成实现边界或临时规避路径，都立即视为当前范围内问题：
   - 若能直接修复，则先修复。
   - 若阻塞当前任务且无法在本轮直接完成，则在 `TODO.md` 中把该修复任务插入到被阻塞任务之前，更新 `PLAN.md` 说明依赖关系，然后提交并停止。
5. 对本轮实际执行的唯一任务完成以下闭环：
   - 实现
   - 测试（包含相关测试、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`，以及必要的专项测试）
   - 更新 `TODO.md` / `PLAN.md`
   - Git 提交
   - 停止

## 执行步骤

1. 查看最新提交信息与改动摘要，确认是否有明确提及待修问题。
2. 读取 `TODO.md`、`PLAN.md`，确认当前优先级最高的未完成任务。
3. 评估任务规模和前置依赖：
   - 如果任务清晰且可在本轮闭环，直接实施。
   - 如果任务过大或存在未覆盖前置能力，先拆分并更新计划文件。
4. 阅读相关代码与测试，定位实现点和潜在既有问题。
5. 修改代码并补充/调整测试。
6. 运行格式化、静态检查和相关测试，修复发现的问题。
7. 更新 `TODO.md` / `PLAN.md` / 本文件的进度日志。
8. 检查工作区改动，形成单一逻辑提交并停止。

## 进度日志

- 2026-04-25：已创建计划文件，尚未开始代码与仓库内容检查。
- 2026-04-25：已检查最新提交 `57026f324c184c96963a6dcea261a6fe0716bd72`（`[T5000b3dR] Review codegen root boundary`）。
  - 提交说明本身未直接声明新的待修既有问题；
  - 但该提交属于 `T5000b3dR` review，因此当前仍需按 `TODO.md` 继续执行下一条 review 任务，并在审阅中把任何暴露出的既有边界问题立即纳入范围。
- 2026-04-25：已读取 `TODO.md` / `PLAN.md`。
  - 当前第一个未完成任务是 `T5000b3R Review：确认 llvm/codegen/mod.rs 的主题拆分是真正的边界整理`；
  - 当前执行策略：先审阅 `llvm/codegen` 根模块与各主题模块边界、确认是否存在跨主题倒灌或残留主体实现；若发现既有问题则先修复，否则完成 review、更新文档并提交。
- 2026-04-25：已完成 `llvm/codegen` 主题边界审阅的代码证据收集。
  - `call/`、`intrinsics/`、`closure/`、`class_ctor.rs`、`enum_lowering.rs`、`object_init.rs` 的主体入口均位于各自主题模块；
  - `codegen/mod.rs` 中对应的少量同名函数目前主要是薄委托或表达式层统一分派入口，而不是主题主体实现本身；
  - `codegen_addressable_place` 仅被 `intrinsics/atomic.rs` 复用，当前更像通用 lvalue bridge；`lookup_pure_unit_closure_type` 已位于 `closure/`，仅被 `sync/thread` 借用作 expected-function-type 桥接。
- 2026-04-25：审阅中发现一个需立即修正的既有问题。
  - `crates/scoopc/src/llvm/codegen/mod.rs` 顶部模块说明仍描述“早期最小子集 / 不支持 if/loop”等旧口径，与当前主题拆分后的真实职责边界不符；
  - 这属于本轮 review 直接暴露的文档错配，需要先修正，再把 review 结论回写到 `TODO.md` / `PLAN.md`。
- 2026-04-25：已修正 `crates/scoopc/src/llvm/codegen/mod.rs` 顶部模块说明。
  - 现已改为描述当前真实边界：根模块承接共享上下文、generic lowering 与跨主题 helper；
  - `call/`、`intrinsics/`、`closure/`、`class_ctor.rs`、`enum_lowering.rs`、`object_init.rs` 等主题模块的职责边界已明确写入注释；
  - 下一步进入格式化、测试与 review 记录回写。
- 2026-04-25：验证已完成。
  - `cargo fmt --all`：通过；
  - `cargo test -p scoopc llvm::`：通过；
  - `cargo test --all`：通过；
  - `cargo clippy --all-targets -- -D warnings`：通过。
- 2026-04-25：任务文档已回写。
  - `TODO.md` 已将 `T5000b3R` 标记为完成，并记录本轮 review 结论与修正的文档问题；
  - `PLAN.md` 已追加 `T5000b3R` 进度记录，并将下一条待执行任务切换为 `T5000b4`；
  - 下一步：检查 diff，提交本轮改动，然后停止。
