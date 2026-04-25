# 执行计划

## 约束说明

- 按要求先记录执行计划，再执行仓库检查与命令。
- 详细的内部推理不写入仓库文件；此处仅保留可审阅的执行计划、关键判断和进度更新。
- 本次调用只处理 `TODO.md` 中第一个未完成任务；若发现阻塞该任务的既有问题，则先修复问题或把问题作为前置任务插入 `TODO.md` 后停止。

## 初始步骤

1. 查看最新一次 Git 提交信息，确认是否明确提到需要先修复的遗留问题。
2. 读取 `TODO.md`、`PLAN.md`、必要时读取 `PROMPT.md` 与相关说明，定位第一个未完成任务。
3. 判断该任务是否需要拆分；若需要，先更新 `PLAN.md` 和 `TODO.md`，提交后停止，等待下次调用执行第一个新子任务。
4. 若无需拆分，则检查实现上下文与相关代码、测试、规范，确认是否存在阻塞任务的既有问题。

## 执行步骤

1. 实现当前目标任务，必要时同步补充注释或整理模块边界，但不做无关重构。
2. 运行与改动直接相关的测试；若任务影响范围较大，则扩大到必要的集成测试。
3. 运行格式化与质量检查，至少覆盖：
   - `cargo fmt --check`（如失败则先 `cargo fmt`）
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
4. 若测试或检查暴露既有问题：
   - 先判断是否为当前任务引入的问题；
   - 若为仓库中已存在且阻塞当前任务的问题，优先修复，或者把它作为前置任务写入 `TODO.md` / `PLAN.md` 后停止。

## 收尾步骤

1. 更新 `TODO.md`，将本次完成的任务标记为已完成；若任务被拆分或重排，确保依赖顺序正确。
2. 更新 `PLAN.md`，反映当前状态、关键决策、遗留依赖与下一步。
3. 更新本文件，记录完成情况、测试结果和任何计划变更。
4. 使用清晰的 Git 提交信息提交本次修改。
5. 停止，不继续处理下一个任务。

## 进度日志

- 已创建本计划文件。
- 已检查最新提交 `42d41463622b80f3fd2ff40d42d4f8188da186e2`（`[T4014a] Make ordinary @Extern effect-impermeable`）。提交本身未额外声明需要先于 `TODO.md` 处理的新遗留缺陷，但 `ISSUES.md` / `TODO.md` 已明确当前主线从 `T4014a` 转入 `T4014b`。
- 已读取 `TODO.md`、`PLAN.md`、`ISSUES.md`、`PROMPT.md`。当前第一个未完成任务为 `T4014b`：完善 stable handle 的 FFI / reactor 合同，并把 `Pinned` 收口为短时裸地址借出。
- 已初步审阅 `sysroot/core.scoop`、`sysroot/unsafe.scoop`、`SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`typecheck` / `LLVM` / `runtime GC` 中 `GcHandle` / `Pinned` 相关实现。
- 当前判断：
  - `GcHandle` 已具备 `raw: UIntPtr` 的稳定 token 表示，runtime 也已有 `scoop_handle_new/get/drop`；
  - `Pinned` 仍是 `struct Pinned(val value: Any)`，本质上不能作为 ordinary `@Extern` ABI token；这与“仅短时裸地址借出”的目标一致；
  - 已有 runtime 回归覆盖 stable token round-trip 与 stale token error，但尚未看到针对 ordinary `@Extern` signature 的正反两向回归：例如 `GcHandle` / `UIntPtr` 可经 ABI 往返，而 `Pinned` 不可作为 ABI surface。
- 下一步：
  1. 运行与 `GcHandle` / `Pinned` / ordinary `@Extern` ABI 直接相关的定向测试，确认当前缺口是否已经体现在回归里。
  2. 若缺口仅在文档/回归层，则补齐文档与 fixture。
  3. 若测试暴露真实实现缺陷，则优先修复该缺陷，并把变更限定在 `T4014b` 范围内。
- 已实施中的改动：
  - 把 `@Extern` GC-free 诊断文案改为显式区分两条桥接路径：长期 opaque token 使用 `GcHandle.raw: UIntPtr`，短时裸地址借出使用 `GC.pin/unpin` + `scoop.unsafe.Ptr<T>`；
  - 修正文案里过时的 `Pinned<引用类型>` 说法为 `Pinned` handle；
  - 更新 `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`sysroot/core.scoop`、`ISSUES.md`，把 stable handle / `Pinned` 的职责分离写成统一叙事；
  - 新增 typecheck 回归：
    - `tests/fixtures/typecheck/extern_fun_gc_handle_raw_token_roundtrip_ok.scoop`
    - `tests/fixtures/typecheck/extern_fun_signature_with_pinned_is_error.scoop`
- 已验证：
  - 新增/相关 typecheck 定向 fixtures：`fixtures: ok (4)`；
  - 相关 runtime GC 定向 fixtures：`fixtures: ok (3)`；
  - `cargo fmt --check` 初次执行暴露仓库既有未格式化代码，已通过 `cargo fmt` 收口，随后 `cargo fmt --check` 通过；
  - `cargo run -p scoop_tools -- spec-fixtures check` 通过。
- 正在执行：
  - 无
- 待执行：
  - 无
- 最终结果：
  - `target/debug/scoop test` 通过，结果为 `fixtures: ok (1202)`；
  - `cargo test --all` 通过；
  - `cargo clippy --all-targets -- -D warnings` 通过。
- 额外说明：
  - 首次执行 `cargo run -p scoop -- test` 时，我在其运行中并行启动了 `cargo test --all`，随后看到一次 `run_pass_cone/float_multi_file_literal_basic` 的 transient 失败；
  - 该 case 单独复现通过，随后在无并行 Cargo 重负载干扰的情况下重跑全量 `target/debug/scoop test`，结果全绿；
  - 因此该信号判定为执行时干扰，不是稳定可复现的仓库回归。
