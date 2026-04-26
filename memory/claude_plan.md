# 执行计划与决策记录

## 约束说明

- 本轮目标：只完成 `TODO.md` 中第一个未完成任务，然后停止。
- 优先级规则：任何在最新提交说明、代码审查、测试、探测过程中发现的既有问题，都必须先修复，或先在 `TODO.md` 中以前置任务的形式插入，再停止。
- 过程要求：在执行过程中持续更新本文件，记录当前计划、关键发现、阻塞原因、已完成步骤与后续动作。
- 说明：这里记录的是可审计的执行计划、依据和决策摘要，不包含逐字原始思维流。

## 初始执行计划

1. 检查最新一次 Git 提交，确认是否明确提到尚未解决的问题、已知缺陷、临时方案或待补修复项。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解当前计划顺序、依赖关系和任务背景。
4. 如第一个未完成任务过大或边界不清，拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`，然后只执行拆分后的第一个子任务。
5. 在开始实现前，检查任务相关代码、测试、规范和最近改动，确认是否存在会阻塞该任务的既有问题。
6. 如果发现既有问题：
   - 先直接修复；或者
   - 若无法在本轮直接修复，则将其作为当前任务的前置任务插入 `TODO.md`，更新 `PLAN.md`，提交并停止。
7. 实现当前目标任务，保证实现符合规范，不引入 workaround、fixture-only hack 或规避性改写。
8. 运行相关测试，并根据影响范围补充验证：
   - 最小相关测试；
   - 必要的全集成测试；
   - `cargo fmt`；
   - `cargo clippy --all-targets -- -D warnings`；
   - 其他本任务相关命令。
9. 更新文档与计划：
   - 在 `TODO.md` 标记任务完成；
   - 在 `PLAN.md` 反映当前状态、依赖变化和后续顺序；
   - 如实现中产生关键决策，也记录到本文件。
10. 提交本轮改动，提交信息清晰描述任务编号和内容。
11. 停止，不继续处理下一个任务。

## 待检查项

- 最新提交是否提到未修复问题。
- `TODO.md` 第一个未完成任务是什么。
- `PLAN.md` 是否已反映该任务及其前置依赖。
- 当前工作区是否存在用户未提交改动，且是否影响本轮任务。

## 进度记录

- 已完成：创建本计划文件并写入初始执行框架。
- 已完成：检查最新提交 `7dd3ca2e54678105684b26e5e2db84bd8d362be5`（`[T5000e2aR] Review compilation-unit materialization boundary`）。
  - 结论：提交说明本身未声明仍待修复的已知问题；
  - 同步核对 `TODO.md` / `PLAN.md` 中对应 review 记录，结论均为“未发现需要插入到下一任务之前的新前置缺陷任务”。
- 已完成：定位 `TODO.md` 中第一个未完成任务为 `T5000e2b 让编译单元 MIR instance collection 覆盖 owner/nominal specialization`。
- 已完成：核对 `PLAN.md` 中该阶段背景。
  - `T5000e2` 已被拆分为 `T5000e2a`～`T5000e2c`；
  - 当前轮到执行 `T5000e2b`，目标是把 owner/nominal specialization 的实例身份与发现逻辑迁入 MIR instance collection，而不是继续依赖 HIR 扫描 `TypeStore`。
- 当前判断：
  - `T5000e2b` 目前看起来可以直接实现，不需要先拆分。
  - 但已确认存在一个真实的前端/实例收集缺口：generic owner 下的非泛型 member fun / getter 根本不会进入 MIR materialization 请求集合。
- 下一步：
  1. 为 `generic owner + member fun/getter` 补最小回归测试，锁定当前缺口。
  2. 修复 typecheck 侧请求记录：让 member direct-call 在 `MonomorphKey` / `TopLevelFunCallBinding` 中带上 owner-specialization 所需的 concrete args，而不是只记录函数自身 type args。
  3. 修复 MIR template catalog：generic owner 下即使成员函数本身没有 `<T>` / `<eff E>`，也要把 owner type params 作为可 materialize 的实例维度；值类型 computed property getter 同理。
  4. 运行最小测试、相关 crate 测试、`cargo fmt --all`、`cargo clippy --all-targets -- -D warnings`。
  5. 若验证通过，再更新 `TODO.md` / `PLAN.md`、提交并停止。

## 关键发现

- `T5000e2b` 的旧 HIR 路径残留明确存在：
  - `crates/scoopc/src/hir/lower/util.rs` 的 `collect_generic_member_fun_instantiations(...)` 仍通过扫描 `TypeStore` 中的具体 nominal 类型来生成 owner-specialized member/getter HIR。
- 当前 MIR 路径存在两个直接缺口：
  - `crates/scoopc/src/mir/materialize.rs` 的 `collect_generic_template_infos(...)` 只把“函数自身带 type/effect params”的成员函数加入 template catalog，导致 `class Box<T> { fun get(): T }` / generic owner getter 根本没有对应 template root；
  - `crates/scoopc/src/typecheck/expr/call.rs` 在记录 `MonomorphKey` / `TopLevelFunCallBinding` 时只写入函数自身 `instantiated.type_args` 与 `eff_args`，不会把 receiver owner 的 concrete args 纳入请求，因此 `Box<Int>.get()` 这类调用不会产出任何实例请求。
- 实测结果：
  - 对 `class Box<T> { fun get(): T }` 的 `box.get()` 与 generic owner getter `box.doubled` 运行 `cargo run -q -p scoop -- dump-ir ...`，当前 `MaterializedMir.instance_keys` 和 `file.items` 都为空，证明 owner-specialized 成员实例没有进入 MIR 主线。
# 2026-04-26 当前轮执行记录（T5000e2b）

## 已知上下文
- 本轮目标是完成 `TODO.md` 中第一个未完成任务：`T5000e2b 让编译单元 MIR instance collection 覆盖 owner/nominal specialization`。
- 上一轮分析已确认最新提交 `[T5000e2aR]` 没有额外需要先插入的新前置缺陷任务。
- 已完成的核心实现方向：
  - `typecheck` 在记录 member/getter 实例请求时，开始把 generic owner 的 concrete type args 以前缀形式并入 `type_args`。
  - `mir/materialize.rs` 的 template catalog 现在能把 owner type params 纳入 generic member/getter template key。
  - materialization 新增 request-root direct-call 扫描，用请求源文件中的 generic root 函数补种 owner-specialized getter / nested direct call 实例。
  - 已新增并跑通过两条定向测试，覆盖 owner-specialized effect generic member call 与 owner-specialized getter seeding。

## 当前已知阻塞 / 必须先修复的问题
- `cargo clippy --all-targets -- -D warnings` 失败：
  - 位置：`crates/scoopc/src/mir/materialize.rs`
  - 原因：`MirInstanceMaterializer::new(...)` 命中 `clippy::too_many_arguments`
- 该问题属于已存在的质量问题，必须先修复后才能继续完成本任务。

## 当前执行计划
1. 检查 `mir/materialize.rs` 中 `MirInstanceMaterializer::new(...)` 的当前签名和调用点。
2. 把构造参数收口到单独的输入 struct，消除 `too_many_arguments`，同时保持现有 request-driven materialization 设计不变。
3. 复查 `call.rs` / `materialize.rs` 是否还存在 owner-specialized instance 请求遗漏，必要时补齐。
4. 运行格式化与定向验证：
   - `cargo fmt --all`
   - 两条新增 materialize 定向测试
5. 运行全量质量门：
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
6. 若通过，更新 `TODO.md`、`PLAN.md`、本文件，并提交一个只覆盖本轮任务的 commit，然后停止。

## 执行原则
- 不引入 workaround，不回退到扫描 `TypeStore` 的 eager materialization 路径。
- 继续保持 owner-specialized 实例 key 的参数顺序为：`owner args + fun args`。
- 如果验证中发现新的既有缺陷，优先修复；若无法在本轮直接修复，则按要求前插到 `TODO.md` 并停止。

## 进展更新
- 已完成步骤 1-2：
  - 检查并确认 `MirInstanceMaterializer::new(...)` 的 8 参数签名正是当前 `clippy::too_many_arguments` 唯一已知触发点。
  - 已将 `template_infos` / `request_root_fun_keys` / site-binding 加载输入 / `typecheck_types` 收口到 `MaterializerConstructionInputs`，`new(...)` 改为接收单一 construction input。
- 已完成一步额外复查：
  - 扫描 `typecheck/expr/call.rs` 中所有 `record_top_level_fun_call_binding(...)` 调用点，owner-specialized member direct-call 分支已统一改为通过 `combined_member_instance_type_args(...)` 记录 `owner args + fun args`。
- 下一步：
  - 跑 `cargo fmt --all`
  - 跑 `cargo clippy --all-targets -- -D warnings`
  - 再做定向测试与全量测试

## 验证进展
- `cargo fmt --all`：已通过。
- `cargo clippy --all-targets -- -D warnings`：已通过；`MirInstanceMaterializer::new(...)` 的参数收口已消除当前唯一已知告警。
- 下一步进入测试验证：
  - 两条 `materialize` 定向测试
  - `cargo test --all`

## 最终状态
- 两条新增定向测试均已通过：
  - `cargo test -p scoopc typechecked_compilation_unit_materialization_handles_owner_specialized_effect_generic_member_calls -- --nocapture`
  - `cargo test -p scoopc typechecked_compilation_unit_materialization_seeds_owner_specialized_getter_from_request_roots -- --nocapture`
- `cargo test --all` 已通过。
- 本轮未发现新的需要前插到 `TODO.md` 的前置缺陷任务。
- 已完成的任务结论：
  - 编译单元级 MIR instance collection 现已覆盖 owner-specialized member/getter；
  - 请求键与 template identity 已统一承载 `owner args + fun args`；
  - getter / nested direct call 的实例发现已进入 request-root direct-call seeding 主线；
  - build/frontend 仍在使用的 HIR eager materialization 主路径保留给后续 `T5000e2c` 收口。
- 文档状态：
  - `TODO.md` 已准备把 `T5000e2b` 标记为完成；
  - `PLAN.md` 已准备记录本轮结果，并把下一条待执行任务切换为 `T5000e2bR`。
- 剩余收尾：
  - 检查 diff / git status
  - 提交本轮改动
  - 停止
