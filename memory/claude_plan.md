## 执行计划

说明：我不会记录或暴露详细的内部思维过程，但会在此维护可审阅的高层计划、关键判断、执行进度与变更原因。

1. 检查最新一次 Git 提交，确认是否提到了任何已知问题、回归、待修复项或阻塞事项；若有，先修复这些问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 评估该任务是否过大：
   - 如果可直接完成，进入实现；
   - 如果过大，则先更新 `PLAN.md` 与 `TODO.md`，将其拆分为更小的前置子任务，本次只执行新的第一个子任务。
4. 在实现过程中，如果发现任何既有缺陷、规格不匹配、实现边界缺失或测试/运行时回归：
   - 先修复该问题；
   - 若当前无法在本次直接修复，则把它作为前置任务插入 `TODO.md` 当前任务之前，并更新 `PLAN.md` 说明阻塞关系，然后停止。
5. 完成当前首个任务后，运行相关验证：
   - 至少执行与改动直接相关的测试；
   - 若适用，执行 `cargo fmt`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`，或给出无法执行的明确原因。
6. 更新文档与计划：
   - 在 `TODO.md` 标记任务完成；
   - 在 `PLAN.md` 记录当前状态、依赖变化与后续顺序；
   - 必要时同步更新本文件中的执行进度。
7. 提交 Git，提交信息使用清晰描述并尽量带任务号。
8. 本轮只完成一个任务，然后停止。

## 当前进度

- 已创建执行计划文件。
- 已检查最新提交 `30b179ddacd42073cee6df8bb6db3c0803b63aea`（`[T5000b1R] Review llvm mod root boundary`）。
  - 提交内容只是在 `TODO.md` / `PLAN.md` / `memory/claude_plan.md` 中记录 review 结论；
  - 未提到任何需要优先修复的既有缺陷；
  - review 结论明确写明：未发现必须插入到 `T5000b2` 之前的新前置缺陷任务。
- 已确认 `TODO.md` 当前首个未完成任务为 `T5000b2 提炼 MainCodegen 共享编译单元上下文与 child-codegen 构造路径`。
- 已完成对 `MainCodegen::new` 现有构造点的勘察：
  - `crates/scoopc/src/llvm/emit.rs` 中有 3 组编译单元级输入重复拼装：
    - 顶层声明阶段；
    - reachable top-level function body 发射阶段；
    - 入口 `main` exit-code lowering 阶段。
  - `crates/scoopc/src/llvm/codegen/mod.rs` 中有 4 处 child/nested codegen 手写 `MainCodegenInputs { ... }`：
    - effect-call wrapper body；
    - top-level immutable value init；
    - closure body lowering；
    - object init lowering。
- 当前实现策略：
  1. 新增一个共享的编译单元上下文类型，承接稳定只读输入与跨 child-codegen 共享的编译单元级状态；
  2. 将 `known_effect_instances_by_effect_fqn` 的构建上移到共享上下文，避免每次 child-codegen 重新扫描；
  3. 让 `MainCodegen` 持有对共享上下文的引用，并提供统一的 child-codegen 工厂方法；
  4. 收敛 `emit.rs` 中的入口构造方式，使编译单元输入只在一处集中拼装；
  5. 运行格式化、测试与 clippy；
  6. 若验证通过，再更新 `TODO.md` / `PLAN.md` / 本文件并提交。
- 已完成 `T5000b2` 实现：
  - `crates/scoopc/src/llvm/codegen/mod.rs` 中新增 `CompilationUnitCodegenCx` / `CompilationUnitCodegenInputs`，把稳定编译单元输入、共享 `effect_op_tags`、共享 `known_fun_call_suspend_cache` 与预计算的 `known_effect_instances_by_effect_fqn` 集中到共享层；
  - `MainCodegen` 已改为持有 `shared: &CompilationUnitCodegenCx`，并新增 `fresh_child_codegen()`，收口了 effect-call wrapper、top-level immutable init、closure body lowering、object init lowering 4 处 child/nested codegen 构造路径；
  - `crates/scoopc/src/llvm/emit.rs` 已改为只在一个位置构造编译单元上下文，并通过 `fresh_main_codegen()` 复用到顶层声明、reachable top-level function body 发射和入口 `main` exit-code lowering。
- 实现过程中出现过两个局部编译问题，均已当场修正：
  - `Deref` 的关联类型暴露了过窄可见性的共享上下文类型，已把 `CompilationUnitCodegenCx` 调整为 `pub(crate)`；
  - 共享层里误残留了 `current_source_id`，已移回函数级 `MainCodegen`。
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc llvm::`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - 结果：全部通过。
- 下一步：检查文档改动与代码 diff，提交本轮 `T5000b2` 结果，然后停止；后续待执行任务应切换为 `T5000b2R`。
