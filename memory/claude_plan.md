# 本轮执行计划

说明：不记录逐字内部思维，仅记录可审计的执行计划、决策依据和进度更新。

## 初始计划

1. 查看最新一次 Git 提交，确认提交说明中是否提到尚未修复的问题；如果有，先定位并修复这些问题。
2. 阅读 `TODO.md`，识别第一个未完成任务。
3. 阅读 `PLAN.md`，确认该任务是否已有更细的执行安排，以及当前整体优先级是否一致。
4. 评估该任务复杂度：
   - 如果任务可以在本轮完整交付，则直接实现。
   - 如果任务过大，则把它拆成更小的可执行子任务，并更新 `PLAN.md` 与 `TODO.md`，确保第一个子任务成为新的当前任务。
5. 实现当前任务所需代码改动，并在必要时补充或整理相关模块、注释与文档。
6. 运行相关验证：
   - 至少运行与改动直接相关的测试。
   - 若条件允许，运行 `cargo test --all`。
   - 按要求运行 `cargo clippy --all-targets -- -D warnings`，确保无警告。
7. 更新项目记录：
   - 在 `TODO.md` 中标记当前任务完成，或在无法直接完成时按依赖关系重排任务。
   - 在 `PLAN.md` 中更新状态、拆分结果或阻塞原因。
   - 在本文件记录关键进展与计划变更。
8. 检查工作区改动，确认不误伤用户已有修改。
9. 提交本轮改动，提交信息聚焦当前任务。
10. 完成本轮后停止，不继续处理下一个任务。

## 进度记录

- 已创建本文件并写入初始计划；下一步将检查最新提交与任务列表。
- 已检查最新提交 `bf1af21128a1332e08b8645487eb976438a3a0b8`（`[T0147c-2a] Refactor lowering resolve and LLVM helper inputs`）。提交说明未显式提到需先修复的遗留问题，因此继续按 TODO 顺序执行。
- 已读取 `TODO.md` / `PLAN.md`，确认当前首个未完成任务为 `T0147c-2b`：清理 `typecheck/annotations.rs`、`override_effects.rs`、`properties.rs`、`val_pat.rs` 中的 `too_many_arguments`。
- 当前策略：先复现并枚举这四个文件中的 clippy 告警，再查看相关函数和调用链，优先用局部参数对象或上下文 struct 收口参数，避免使用 `#[allow]`。
- 已确认该任务可在本轮完整完成，不需要继续拆分到更细的子任务，因此 `TODO.md` / `PLAN.md` 暂不做任务重排。
- 已完成代码改动：
  - `annotations.rs`：新增 `AnnotationCheckContext`，把 `source/file/index/env` 合并为只读上下文，相关注解检查 helper 统一改为消费上下文 + `TypeLowering`。
  - `override_effects.rs`：新增 `TypeInterfaceImplTarget`，收口 interface impl effect 检查目标参数。
  - `properties.rs`：新增 `DelegatedPropertySignatureCheck`，收口 delegated property `getValue/setValue` 签名检查参数。
  - `val_pat.rs`：新增 `ValPatChecker`，把递归 pattern 检查的共享状态收入口部 checker。
- 已完成验证：
  - `cargo fmt --all` 通过。
  - 严格 clippy 复核确认这四个文件不再出现 `too_many_arguments`。
  - 全局剩余 `too_many_arguments` 数量为 53（符合从上一轮 65 减去本轮 12 的预期），剩余告警集中在 `typecheck/expr/**` 与后续 `T0147c-2c/T0147c-2d`。
  - `cargo test --all` 通过。
  - `cargo run -p scoop -- test` 通过（`fixtures: ok (852)`）。
- 已完成收尾文档更新：`TODO.md` 已将 `T0147c-2b` 标记为完成并写入完成说明；`PLAN.md` 已同步记录本轮的结构化输入收口方案与验证结果。
- 当前仅剩最后一步：检查工作区、提交本轮改动并停止。
