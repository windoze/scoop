# 执行计划与进度记录

## 约束与目标

- 本轮只处理 `TODO.md` 中第一个未完成任务，完成后立即停止。
- 在开始任何终端命令前，先建立本文件，记录计划与后续关键进展。
- 必须先检查最新提交是否提到需要先修复的既有问题；若存在，则这些问题优先于 `TODO.md` 任务。
- 若当前首个未完成任务过大，需要先拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`，然后仅执行拆分后的第一个子任务。
- 实现后必须运行相关测试，并尽量满足无警告构建/检查要求。
- 需要同步更新 `TODO.md`、`PLAN.md`，并创建 Git 提交。

## 初始执行计划

1. 查看最新一次 Git 提交，确认提交信息中是否指出尚未修复的既有问题。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md` 以及与该任务直接相关的代码/规范上下文，判断任务边界与依赖。
4. 若任务过大或被缺失特性/规格不匹配阻塞：
   - 拆分或重排 `TODO.md`；
   - 更新 `PLAN.md` 说明原因、依赖与新顺序；
   - 若本轮只能完成拆分/重排，则提交这些变更并停止。
5. 若任务可直接实施：
   - 修改代码实现任务；
   - 补充或调整测试；
   - 运行相关验证，必要时迭代修复；
   - 更新 `TODO.md` 与 `PLAN.md`；
   - 提交本轮变更并停止。

## 记录规范

- 每完成一个关键步骤，就在本文件追加“进度更新”。
- 若计划调整，记录调整原因、影响范围与新的执行顺序。

## 进度更新

- 已创建计划文件，下一步将检查最新提交信息与待办列表。
- 已检查最新提交 `adf733aa1b7cd2160e0da85cc1d453cc94a4ecfe`（`[T3011] Fix unified frame slot mutability metadata`），未发现提交信息里额外声明一个需要先于 `TODO.md` 继续处理的新既有问题。
- 已确认 `TODO.md` 中首个未完成任务为 `T3011R`，属于 review 任务；无需再拆分，直接审查 unified contract 中 frame slot mutability / capture 元数据的数据流与消费面。
- 已完成生产代码复审：
  - `state_machine_plan.rs` 中 `build_stmt(Val)` 会以声明点为准覆盖同 `SymbolId` 的旧 slot metadata，不保留先前 fallback 占坑的 `mutable: false` / `seed_from_outer_scope: true` 残值。
  - `collect_outer_scope_slots()` 与 `authoritative_local_slot()` 都从 `known_local_metadata` 读取权威 mutability/ty；`build_unified_lowering_contract()` 会先基于当前 codegen env 建立 context，再补入当前 `handle` 自身的 local metadata，保证 nested handle / arm capture / finally 路径能看到声明点信息。
  - `state_machine_emitter.rs` 中 `populate_frame_slots_in_env()`、`emit_read_local_from_frame()`、`emit_execute_arm_body()`、outer-scope seeding/writeback 都直接消费 unified frame slot metadata；赋值仍统一走 `codegen_assign_stmt()`，未发现 effect-only mutable 特判。
- 已完成定向验证：
  - `cargo test -p scoopc declared_handle_local_overwrites_placeholder_slot_metadata -- --nocapture`
  - `cargo test -p scoopc handle_context_extension_recovers_nested_handle_outer_var_mutability -- --nocapture`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_resume_unit.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_outer_var_writeback_basic.scoop`
- 已完成全量质量门槛验证：
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop --features llvm -- test` 结果仍只停在已跟踪的 stale `EXPECT: fail`：`tests/fixtures/run-pass/continuation_resume_continuation.scoop`（`T3017`），未出现新的更早失败点。
- 当前结论：`T3011R` 无新增生产代码问题；下一步只需把审查结论同步回 `TODO.md` / `PLAN.md`，然后提交并停止。
