## 当前执行计划

说明：按安全与协作要求，这里记录可审计的执行计划、关键判断与进度更新，不写出冗长的内部推理细节。

1. 读取 `TODO.md`，将其仅作为任务索引。
2. 按索引顺序读取对应的 `TODO-Px.md`，定位第一个标题未标记 `[DONE]` 的详细任务。
3. 检查最近提交是否直接提到与该任务相关且未完成的问题；若该问题构成当前任务前置条件，则先按详细 TODO 规则处理。
4. 阅读当前任务涉及的代码、测试、规范与依赖，确认实现边界。
5. 实现当前任务；若遇到阻塞当前任务的真实缺陷或缺失能力，不做规避，而是在相应 `TODO-Px.md` 中添加最小前置任务，并同步 `TODO.md`。
6. 运行与当前任务直接相关的验证；若任务落地后需要更广验证，再补充执行格式化、测试与无 warning 检查。
7. 更新 `TODO-Px.md` 的完成记录，并将任务标题改为 `[DONE]`；如有必要同步 `TODO.md`，仅在阶段计划变化时更新 `PLAN.md`。
8. 检查工作区状态，按要求提交当前任务相关的全部未提交改动，并停止，不继续下一个任务。

## 进度更新

- 已创建本文件。
- 已读取 `TODO.md` 索引并确认首个未完成详细任务为 `TODO-P6.md` 中的 `P6-T02g`：为 callable carrier 发布到 canonical dynamic entry 的 authoritative refactor contract。
- 已检查最近提交；最新提交正是该 blocker 的跟踪提交，当前没有更早且更高优先级的未完成前置问题覆盖该任务顺序。
- 已确认当前缺口：`RefactorAbiQuery` 只发布了 callable/dynamic invoke query，但没有把 closure/vtable/itable 实际绑定到 refactor target；`closure/mod.rs`、`mir_body.rs`、`gc.rs` 仍把 carrier 指向普通 ABI 符号。
- 计划中的最小实现：
  1. 在 refactor ABI materializer 内预发布 callable carrier entry shell，并把 `root_fqn -> published carrier target symbol` 缓存在编译单元共享状态里。
  2. 让 closure object、pass MIR `MakeClosure`、class vtable、interface itable 在 refactor contract 已启用时改写为这些已发布 target；若缺少 authoritative mapping，则显式报错而不是回退到普通 ABI。
  3. 补充定向单元测试与 build fixture，验证 carrier target 已切换，且缺失 published target 时会 fail fast。
  4. 运行任务要求的测试、fixture 与 `clippy -D warnings`，然后回填 `TODO-P6.md` / `TODO.md` 并提交。

- 已完成代码实现：
  - 在 `llvm/codegen/mod.rs` 增加 refactor callable carrier contract 的共享 cache/lookup；
  - 在 `llvm/codegen/effect_refactor/layout.rs` 预发布 closure/vtable/itable dynamic-entry shell；
  - 在 `llvm/codegen/{closure/mod.rs,mir_body.rs,gc.rs}` 中把 carrier target 切到已发布 shell。
- 已新增验证资产：
  - 单元测试：`refactor_llvm_dynamic_entry_publication_*`
  - fixture：`tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop`
- 已完成验证：
  - `cargo test -p scoopc refactor_llvm_dynamic_invoke_query`
  - `cargo test -p scoopc refactor_llvm_callable_carrier_layout`
  - `cargo test -p scoopc refactor_llvm_dynamic_entry_publication`
  - 3 个 refactor build fixtures（含新 carrier publication fixture）
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
- 已回填 `TODO-P6.md` 与 `TODO.md`，下一步只剩检查工作区并按任务要求提交一次 git commit，然后停止。
