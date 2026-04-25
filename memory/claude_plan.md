## 当前轮执行计划（决策摘要）

注意：这里记录的是可审查的执行计划、依据和进度摘要，不写入逐字内部推理。

### 初始目标

本轮只处理 `TODO.md` 中第一个未完成任务；如果在检查、实现或测试中发现已有缺陷、规格不匹配或前置依赖缺失，则优先修复，或将其作为新的前置任务插入 `TODO.md` 并停止。

### 执行步骤

1. 检查最新一次 git 提交的信息，确认是否明确提到已有问题需要先修。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认该任务的上下文、依赖和已有拆分。
4. 评估任务规模：
   - 如果可在本轮完整交付，则直接实现。
   - 如果过大，则先把它拆分成更小的子任务，并更新 `PLAN.md` 与 `TODO.md`，本轮只做新的第一个子任务。
5. 实现任务时同时检查相关代码路径；任何现存 bug、回归、规格不匹配、未完成边界都视为本轮范围内问题。
6. 运行相关测试，并补充必要测试；同时执行质量检查，至少包括与本次改动相关的测试，以及在可行时运行 `cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`。
7. 更新 `TODO.md`、`PLAN.md` 和本文件，记录完成情况或阻塞依赖。
8. 提交一个清晰的 git commit，然后停止，不继续下一个任务。

### 当前已知约束

- 必须优先处理已有问题，不能用规避方案推进任务。
- 必须在修改前后持续更新本文件。
- 本轮结束前需要有 git commit。

### 当前进度

- 已创建本计划文件。
- 已检查最新提交、`TODO.md` 与 `PLAN.md`：
  - 最新提交为 `[T5000b3aR] Review call lowering boundaries`；
  - 提交说明本身未提出需要先修复的新旧缺陷；
  - 当前首个未完成任务为 `T5000b3b 拆出 intrinsics/ lowering 模块`。
- 当前判断：
  - 本轮目标是把 builtin/sysroot lowering 从 `crates/scoopc/src/llvm/codegen/mod.rs` 迁到 `llvm/codegen/intrinsics/`；
  - 这是边界整理任务，要求保持语义与错误边界不变；
  - 在迁移过程中若发现现存 bug、规格不匹配或缺失前置能力，必须先修复或把它前移为 TODO 前置任务。
- 已完成代码面梳理，并确定 `intrinsics/` 拆分形状：
  - `builtin.rs`：标量内建 / `print` / `toString` / `toInt` / `hash` / `sizeOf`
  - `sysroot.rs`：io/env/time/fs/process/path
  - `sync.rs`：mutex / condvar / once / destroy
  - `thread.rs`：thread / task transport / thread-specific intrinsics
  - `channels.rs`：channel send/recv/close
  - `containers.rs`：array builder / array get-set / array-like helper
  - `atomic.rs`：atomic int intrinsics
- 已开始实施：
  - 已新增 `crates/scoopc/src/llvm/codegen/intrinsics/` 及上述子模块文件；
  - 已在 `crates/scoopc/src/llvm/codegen/mod.rs` 注册 `mod intrinsics;`；
  - 已从 `codegen/mod.rs` 删除 builtin/sysroot lowering 主体实现块，仅保留非 intrinsics 主题与通用 helper。
- 进度校验：
  - 已确认 `codegen/mod.rs` 中不再残留 `codegen_sysroot_*` 主体实现，只剩 `scoop.unsafe` 的 funptr helper；
  - `cargo fmt --all` 已通过。
- 第一轮验证结果：
  - `cargo test -p scoopc llvm::` 已通过（148 tests passed）；
  - 暴露并已修复 1 个整理后遗留问题：`crates/scoopc/src/llvm/codegen/mod.rs` 中 `inkwell::AtomicOrdering` 导入未使用。
- 全量验证结果：
  - `cargo test --all` 已通过；
  - `cargo clippy --all-targets -- -D warnings` 已通过；
  - 未发现需要前插到 `T5000b3bR` 之前的现存缺陷任务。
- 文档状态：
  - 已将 `TODO.md` 中 `T5000b3b` 标记为完成并补充完成记录；
  - 已将 `PLAN.md` 补记 `T5000b3b` 的实现/验证结果，并将下一条待执行任务切换为 `T5000b3bR`。
- 下一步：检查工作区、准备本轮提交，并在提交后停止。
