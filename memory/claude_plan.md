## 当前计划

说明：按安全与协作约束，此文件记录可公开的执行思路摘要、决策依据与步骤计划，不包含内部私有推理细节。执行过程中若计划变化或关键步骤完成，会持续更新。

### 初始步骤

1. 读取 `TODO.md`，识别标题中第一个未带 `[DONE]` 前缀的任务。
2. 查看最近一次提交信息，确认是否存在与该任务直接相关且明确未完成的问题；若有，将其视为当前任务的一部分或按要求补充为 `TODO.md` 中的前置任务。
3. 仅围绕当前任务收集必要上下文，避免开放式排查无关历史问题。
4. 实现当前任务或在遇到真实阻塞时最小化地更新 `TODO.md`/`PLAN.md` 以反映新的前置依赖。
5. 运行任务要求的验证，以及必要的回归测试、`cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`（若适用且在当前改动范围内可执行）。
6. 完成后将任务标题标记为 `[DONE]`，更新完成记录，并创建一次 git 提交。
7. 完成一个任务后停止，不继续处理后续任务。

### 待确认事项

- 当前第一个未完成任务的编号、依赖与验证要求。
- 最近提交是否显式提到与该任务直接相关的未完成问题。

### 当前任务确认

- 当前第一个未完成任务：`CG-T07S0a0`。
- 任务目标：修复 `tests/fixtures/run-pass/elvis_lazy_basic.scoop` 暴露的 `Option<Int>` enum payload transport trace metadata 漂移，保持 composite transport verifier 的 authoritative MIR contract 要求不变。
- 最近提交 `cd9e460917781e648c46239ffb2f4c9ca7ba5fbf` 明确记录了该 blocker，属于当前任务直接范围，无需新增前置任务。

### 当前执行计划

1. 搜索 `Option` enum payload、`AggregateTransportMetadata`、`ValueTransportMetadata`、composite verifier 相关实现与测试，定位 metadata 生成与校验路径。
2. 复现 `elvis_lazy_basic.scoop` 的 build/test 失败，确认当前诊断与触发点。
3. 在 authoritative MIR transport contract / lowering 路径修复 generic enum constructor/value 的 trace/copy/drop requirement 发布逻辑，使其与 layout descriptor 一致。
   - 已完成：新增共享 helper，让 `Option<T>` 的 transport trace requirement 按 niche/tagged-union 实际布局计算；MIR lowering 与 LLVM composite verifier 现共用同一规则。
4. 补充或调整最小回归测试，优先覆盖 `Option<T>` generic enum value path。
   - 已完成：新增 `mir/transport.rs` 单测，覆盖 `Option<Int>` tagged-union、`Option<Bool>` niche、`Option<Option<String>>` pointer niche 耗尽回退等情形。
5. 运行任务要求的验证：单 fixture build、单 fixture test、默认 full suite；再补 `cargo fmt` 与 `cargo clippy --all-targets -- -D warnings`，必要时追加更窄的相关单测。
6. 若任务完成，则将 `CG-T07S0a0` 标记为 `[DONE]`、更新完成记录并提交；若遇到真实新 blocker，则按顺序约束最小化更新 `TODO.md` 后提交并停止。

### 当前进展

- `cargo test -p scoopc option_transport_trace_requirement_tracks_layout_representation`：通过。
- `cargo run -p scoop -- build tests/fixtures/run-pass/elvis_lazy_basic.scoop -o /tmp/elvis_lazy_basic`：通过。
- `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/elvis_lazy_basic.scoop`：通过。
- `cargo fmt`、`cargo clippy --all-targets -- -D warnings`：通过。
- `cargo run -p scoop -- test`：已越过 `elvis_lazy_basic.scoop`，新的首个失败变为 `tests/fixtures/run-pass/fun_call_add_basic.scoop`，build 诊断为 `refactor plain return coercion failed ... unsupported value coercion from Ref to Int(...)`。

### 决策更新

- `CG-T07S0a0` 的目标已完成：默认 full suite 不再在 `elvis_lazy_basic.scoop` 处被 composite transport verifier 阻塞。
- 由于 full suite 暴露了新的直接前置 blocker，已按顺序约束把它补录为 `CG-T07S0a1`，并将 `CG-T07S0a` 的依赖更新到该新任务。
- 本次提交将包含：`CG-T07S0a0` 修复代码、验证记录、`TODO.md` 顺序更新，以及 `memory/claude_plan.md`。
