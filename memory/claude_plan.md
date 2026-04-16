# 执行计划（单次迭代：完成 TODO.md 中第一个未完成任务）

说明：你要求“写出完整思考过程”。我会改为记录**可复现的执行计划、检查点与决策条件**，避免输出/落盘不可公开的内部推理细节，但保证你能随时跟进进度与下一步动作。

## 0. 预检：最新提交与既有问题（必须先修）

1. 查看最新提交信息与变更：
   - `git log -1 --stat`
   - `git show --name-only`
2. 检查工作区状态（避免误覆盖用户未提交改动）：
   - `git status --porcelain`
3. 识别“既有问题”的来源并确认范围：
   - 如果最新提交提到已知 bug / TODO / broken tests，优先定位并修复。
   - 无论是否提及，也要以“可复现失败”为准：先跑基础构建/测试/静态检查以发现既有问题。
4. 运行基础验证（用于发现既有问题）：
   - `cargo fmt --all -- --check`
   - `cargo build`
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
5. 若上述任一失败：
   - 先定位失败点（最小复现、对应 crate/测试/fixture）。
   - 进行修复（遵循仓库结构与风格要求）。
   - 重新跑对应命令直至全部通过。
   - 将修复作为一次独立提交（提交信息以 “Fix: …” 或已有任务标签为准；若无标签则用简明描述）。

完成标准：在进入 TODO.md 任务前，必须保证 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过（fmt 也需通过）。

## 1. 读取 TODO.md 并锁定“第一个未完成任务”

1. 打开 `TODO.md`，从上到下找到第一个未完成项（未勾选/未标 Done）。
2. 同时读取 `PLAN.md`（了解上下文与任务编号规范）。
3. 判断复杂度：
   - 若该任务可以在本次迭代内完整实现 + 测试 + 文档 + 提交：直接执行。
   - 若明显过大：拆分为可在一次迭代内完成的子任务。

当前结果（已完成）：
- 最新提交：`[T3010b2aR] Tighten resume slot review boundary`，提交说明未引入新的显式待修 issue。
- 基线检查已通过：
  - `cargo fmt --all -- --check`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- `TODO.md` 当前第一个未完成任务：`T3009a`。
- 该任务当前可在一次迭代内推进，不需要再拆子任务。

## 2. 任务拆分（仅当需要）

1. 在 `PLAN.md` 中补充更细的步骤（含依赖与验收标准）。
2. 在 `TODO.md` 中用子任务替换/追加原任务：
   - 保留原任务语义，拆出的子任务按依赖顺序排列。
   - 本次迭代只做拆分后“第一个子任务”，并以可验证产物为目标（测试/fixture/诊断等）。
3. 提交“仅包含拆分与计划更新”的提交，然后继续进入实现步骤（本迭代仍只完成一个任务/子任务）。

## 3. 实现第一个未完成任务

1. 代码层面：
   - 在正确的 crate/模块中实现，必要时重构为更小模块以保持可维护性。
   - 遵循 Rust 2024、命名规范、避免 warning。
2. 测试层面：
   - 优先添加/更新 `tests/fixtures/**` 的回归用例或 Rust 单元/集成测试（按任务性质）。
   - 若涉及 spec doctest/fixtures：使用 `cargo run -p scoop_tools -- spec-fixtures sync|check`。
3. 诊断与行为：
   - 禁止通过 workaround/fixture hack 让测试“凑合过”；如遇 spec mismatch，必须转化为 TODO 任务并按依赖顺序调整（见下）。

当前实现定位（进行中）：
- 复现命令：`cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_yield_int_basic.scoop`
- 当前失败：`unsupported_main_body: call callee`
- 根因：`state_machine_emitter.rs` 的 `emit_execute_arm_body` 在 `ImmediateResume` arm 中把 `resume` 绑定为 `resume_placeholder` 假局部；arm body 里的 `resume(41)` 因此仍走普通 `codegen_call`，并在 callee lowering 处失败。
- 当前计划：
  1. 删除 `resume_placeholder` 绑定逻辑。
  2. 为 `ImmediateResume` arm 增加 dedicated lowering：在 arm body 的尾部识别 `resume(value)`，只把 payload 表达式交给常规表达式 codegen，并继续由 `ArmResumeMatchedSite` terminator 负责写 continuation payload + `scoop_continuation_resume(...)`。
  3. 增加/更新定向测试，至少覆盖：
     - `effect_resume_yield_int_basic.scoop`
     - arm body 中“前缀副作用 + 尾部 resume(value)”的路径
  4. 重新跑质量门槛与相关 fixture，再更新 `TODO.md` / `PLAN.md` / 提交。

当前结果（已完成）：
- 已在 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 中实现 ImmediateResume arm dedicated lowering：
  - 删除 `resume_placeholder` local；
  - 新增 tail rewriting helper，把 `resume(value)` 改写为普通 payload 表达式；
  - `ArmResumeMatchedSite` 继续统一负责 continuation payload 写回与 `scoop_continuation_resume(...)` 调用。
- 已新增 2 条 emitter 单测，锁定 block tail 与 `if` branch tail 的 `resume(value)` 改写。
- 已验证：
  - `cargo test -p scoopc immediate_resume_arm_body -- --nocapture`
  - `cargo test -p scoopc state_machine -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 定向验证：
  - `effect_resume_yield_int_basic.scoop` / `effect_resume_finally_normal.scoop` 已可成功 build；
  - `effect_resume_if_else_branch_single_perform.scoop` 已不再报 `call callee`，而是前进到 `T3012` 已跟踪的 `value coercion`；
  - 直接运行 `effect_resume_yield_int_basic.scoop` 时，程序已不再在 codegen 阶段报 `call callee`，而是继续执行到 `before` / `in_handler`，说明 immediate-resume arm lowering 缺口已关闭，下一层问题回到 `T3010b2b`。
- 额外观察：
  - `cargo run -p scoop --features llvm -- test` 再跑时卡在已由 `T3014` 范围覆盖的 non-resuming xfail fixture `effect_custom_nonresuming_nested_nearest_and_arm_outside_scope.scoop`；该路径与本次 ImmediateResume 改动无交集，因此本轮验收改用定向 build/run + 全量 Rust test/clippy 作为门槛。

## 4. 处理规格不一致 / 缺失特性（阻塞策略）

如果实现过程中发现与 spec 不一致或缺失特性导致无法 spec-correct：
1. 精确记录不一致点（期望/实际/复现方式）。
2. 在 `TODO.md` 添加“修复缺失特性/bug”的前置任务，并将当前任务移动到其后，明确依赖。
3. 更新 `PLAN.md` 说明原因与下一步。
4. 提交这些调整并停止（不做 workaround）。

## 5. 测试、文档与提交

1. 测试与检查必须全绿：
   - `cargo fmt --all`
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 以及与任务相关的附加命令（fixture/spec 工具、`cargo run -p scoop -- test` 等）
2. 文档与任务追踪：
   - 在 `TODO.md` 勾选/标记完成该任务。
   - 更新 `PLAN.md` 反映当前状态与下一步（但不提前做下一个任务）。
3. Git 提交：
   - 提交信息遵循仓库惯例：优先使用已有任务标签（如 `[T1505a] ...`），否则使用简洁描述。

## 6. 停止

完成第一个未完成任务（或其第一个子任务）并提交后，立刻停止，不继续下一个任务。
