## 本次执行计划

### 说明

按你的要求，我会持续更新这份文件，记录本次任务的执行计划、关键决策摘要、进度与验证结果。
出于协作与安全边界考虑，这里记录的是可审计的执行思路与决策摘要，而不是不可压缩的内部逐词推理。

### 初始步骤

1. 读取 `TODO.md`，严格按标题是否带有 `[DONE]` 判断完成状态，找出第一个未完成任务。
2. 查看最近一次提交，确认是否存在与该任务直接相关且明确标注未完成的问题；若有，则按要求将其视为当前任务的一部分或在 `TODO.md` 中补充为前置任务。
3. 阅读该任务及其依赖涉及的代码、测试、规范和记录文件，确认实现边界与验收条件。
4. 如任务可直接完成，则实施最小正确修改；如遇阻塞当前任务的真实缺口或规格不匹配，则先在 `TODO.md` 中补充最小前置任务并停止。
5. 运行该任务要求的验证命令，以及必要的回归测试、`cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`（若适用且可在合理时间内完成）。
6. 更新 `memory/claude_plan.md` 记录结果；将对应任务在 `TODO.md` 中标记为 `[DONE]` 并补全完成记录；仅当阶段计划发生变化时才更新 `PLAN.md`。
7. 按仓库约定创建一次 git 提交，然后停止，不继续处理下一个任务。

### 当前状态

- 状态：已读取 `TODO.md` 并识别当前执行单元。
- 当前任务：`CG-T07S0a2`。
- 任务摘要：修复 `tests/fixtures/run-pass/gc_array_class_elements_cross_function.scoop` 中 `println::<String>` 参数 lowering 把 `String` 值误判成 `Ref`，导致 LLVM 前端准备阶段报 `unsupported value coercion from Ref to String`。
- 最新提交检查：最近一次提交标题为 `[CG-T07S0a1] Fix tail expected-type lowering and record String blocker`，已明确把当前问题登记为后续 blocker；`TODO.md` 中也已按顺序加入 `CG-T07S0a2`，因此无需再拆分新任务。

### 下一步

1. 复现 `gc_array_class_elements_cross_function.scoop` 的构建失败，收集完整诊断与调用栈线索。
2. 搜索并阅读 `refactor pure assignment`、direct-call lowering、`println::<String>` 参数准备、`String` surface transport 相关代码。
3. 判断问题是否发生在 authoritative HIR/MIR lowering、frontend prepare、还是 LLVM backend gate；若是当前任务范围内 bug，则直接修复；若暴露新的真实前置缺口，则按要求回写 `TODO.md` 后停止。
4. 以最小正确改动补回 regression test，并执行任务要求的 build / fixture / full-suite 验证，再运行格式化与 `clippy`。
5. 更新 `TODO.md` 与本计划文件，提交 git commit，然后停止。

### 当前判断

- 失败已复现于 `refactor pure assignment ... callee_fqn: "scoop.core.println::<String>" ... unsupported value coercion from Ref to String`。
- generic `dump-mir` 显示 `printArray` 中 `xs.get(i)` 对应的 MIR local 仍是 `String`，说明问题不是 fixture/HIR 直接把表达式降成了 `Any`。
- 进一步阅读发现 raw materialized MIR path 会显式使用 `pass_view.materialized().types` 解释 MIR body/local 类型，而 refactor plain callable body lowering 仍把 canonical materialized MIR body 交给 `source_types`。这会让 plain body 的 slot/type 推导与 canonical MIR type store 脱节，足以解释 `String` surface 被误判成 `Ref`。
- 计划修复：先把 refactor plain callable body 的 MIR contract 验证、返回类型推导、local slot 建立和 `RefactorValuePrimitives` 的 MIR type 解释统一切到 `pass_view.materialized().types`，再补一个 `Array<String>` -> `println(String)` 的 production LLVM regression test 验证该路径。

### 阶段结果更新

- 已完成第一轮修复：refactor plain callable body 的 canonical MIR contract 验证、返回类型推导、local slot 建立和 value lowering 已切到 `pass_view.materialized().types`。
- 已新增最小 LLVM regression test：`refactor_plain_array_string_get_keeps_string_surface_for_println`；该测试已通过。
- 但真实 fixture `gc_array_class_elements_cross_function.scoop` 仍在失败，说明还有更具体的一条 `String` surface 漂移未被该最小复现覆盖。
- 已进一步确认：问题根源不在 LLVM slot/load，而在 canonical materialized MIR 本身。新检查测试发现真实 fixture 的 `main` 中存在 `println::<String>` call-site 直接把 `arg local0`（`Array<String>` / `arr1`）绑定成了 `String` print 调用，说明 `arr1.size()` 这类 site 在 materialization/call binding 阶段发生了错绑。
- 已定位具体成因：`site_instance_binding_for_callee()` 在 lookup exact miss 后会退到 enclosing site binding；对 `Array.size/get/set` 又额外允许在 remap 失败时直接复用该 enclosing binding。这样字符串插值里的内层 `arr1.size()` 在没有 exact binding 时，会错误继承外层 `println` 的 binding，并被 materialize 成 `println::<String>`。
- 下一步修复：收紧 array intrinsic 的 enclosing-binding fallback，只接受真实匹配或可安全 remap 的 binding；同时补一条针对“字符串插值里嵌套 array intrinsic 不得继承外层 println binding”的 materialization regression test。

### 当前结果

- `CG-T07S0a2` 已完成。
- 已修复两处问题：
  1. refactor plain callable body lowering 现在对 canonical MIR body 一律使用 `pass_view.materialized().types`。
  2. materialized direct-call rewrite 不再让 `scoop.core.size/get/set` 在 remap 失败时继承无关 enclosing binding。
- 新增回归：
  - `llvm::tests::refactor_plain_array_string_get_keeps_string_surface_for_println`
  - `llvm::tests::materialized_gc_array_fixture_keeps_string_locals_for_println_string_sites`
- 验证结果：
  - 通过：`cargo test -p scoopc refactor_plain_array_string_get_keeps_string_surface_for_println`
  - 通过：`cargo test -p scoopc materialized_gc_array_fixture_keeps_string_locals_for_println_string_sites`
  - 通过：`cargo run -p scoop -- build tests/fixtures/run-pass/gc_array_class_elements_cross_function.scoop -o /tmp/gc_array_class_elements_cross_function`
  - 通过：`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/gc_array_class_elements_cross_function.scoop`
  - 通过：`cargo fmt`
  - 通过：`cargo clippy --all-targets -- -D warnings`
  - 默认 full-suite：`cargo run -p scoop -- test` 已越过 `gc_array_class_elements_cross_function.scoop`，新的下一处失败为 `tests/fixtures/run-pass/gc_trace_task_field_basic.scoop`
- 文档更新：
  - `TODO.md` 已把 `CG-T07S0a2` 标记为 `[DONE]`
  - 已新增下一前置任务 `CG-T07S0a3`，并让 `CG-T07S0a` 依赖它

### 收尾步骤

1. 检查 git 状态、差异和最近提交，拟定提交信息。
2. 提交本次改动。
3. 停止，等待下一次调用处理 `CG-T07S0a3`。
