# 执行计划与进度记录

## 约束说明

- 本文件用于记录本轮执行计划、关键决策、进度更新与必要的调整。
- 为避免写入未经验证的结论，初始版本先记录明确的执行步骤；在完成仓库检查、任务识别、实现、测试与提交后持续更新。
- 本轮目标是：只完成 `TODO.md` 中第一个未完成任务（如果需要，先拆分任务并更新 `PLAN.md`/`TODO.md`），完成后提交并停止。

## 初始执行计划

1. 检查最新一次 Git 提交，确认是否提到需要优先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 评估该任务是否可在本轮完整完成：
   - 如果可以，直接实现。
   - 如果过大或存在明确前置依赖，先更新 `PLAN.md` 与 `TODO.md`，拆分为更小的子任务，并以第一个子任务作为本轮执行对象。
4. 阅读与当前任务相关的代码、测试、规范和计划文件，确认实现边界与现状。
5. 实现任务所需代码变更；如果过程中发现任何与规范不符的既有缺陷或缺失能力，按要求先在 `TODO.md`/`PLAN.md` 中建模为前置任务，而不是绕过。
6. 运行相关验证：
   - 至少运行与改动直接相关的测试；
   - 如任务涉及公共基础设施或编译/运行路径，补充更高层级验证；
   - 在可行范围内运行 `cargo fmt`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 或与任务范围相称的子集。
7. 更新文档与任务状态：
   - 在 `TODO.md` 中标记本轮完成的任务；
   - 在 `PLAN.md` 中记录当前状态、剩余风险和后续顺序；
   - 同步更新本文件，记录实际执行结果。
8. 检查工作区改动，确保只包含本轮应提交内容，然后创建一次 Git 提交。
9. 停止，不继续处理下一个任务。

## 待确认项

- 最新提交是否声明了必须先修复的遗留问题。
- `TODO.md` 中第一个未完成任务的具体内容、复杂度与前置依赖。
- 当前工作区是否已有未提交改动需要避让。

## 进度

- 已创建本文件并写入初始计划。
- 已检查最新提交：`9a1edb040b6fb943e17876d839321088a3fc0f4c`，提交说明为 `[T4006S] 修复 lazy(None) print-like 类型传递`，未额外声明新的必须先修复遗留问题。
- 已检查工作区：当前只有本文件 `memory/claude_plan.md` 处于修改状态，暂无其它未提交改动需要避让。
- 已读取 `TODO.md` / `PLAN.md`：当前第一个未完成任务是 `T4006T`，内容是修复 `tests/fixtures/run-pass/gc_continuation_cross_thread_resume_with_objects.scoop` 在 LLVM codegen 阶段触发的 `scoop::llvm::unsupported_main_body: value coercion`。

## 当前任务判断

- 当前任务 `T4006T` 描述清晰，已有明确失败用例与验收目标，适合在本轮直接执行，不需要继续拆分 `TODO.md` / `PLAN.md`。
- 本轮只处理 `T4006T`，不会推进后续 `T4006R`。

## 当前细化计划

1. 复现 `gc_continuation_cross_thread_resume_with_objects.scoop` 的 build 失败，并尽量拿到更精确的报错栈或日志位置。
2. 阅读该 fixture 与相关 lowering / LLVM codegen 路径，定位触发 `value coercion` unsupported 的具体表达式形态。
3. 修复对应的 lowering / codegen 类型传递或 coercion 逻辑，确保实现走统一主线，而不是仅对该 fixture 加特判。
4. 新增或调整最小回归，覆盖这条 effect / continuation + GC object graph 跨线程恢复路径。
5. 运行定向验证，然后按影响范围补跑更高层验证，至少包括：
   - 目标 fixture build/run 或对应 fixtures root；
   - `cargo run -p scoop -- test`（确认不再被该夹具阻断）；
   - `cargo test --all`；
   - `cargo clippy --all-targets -- -D warnings`。
6. 更新 `TODO.md`、`PLAN.md` 与本文件，记录 `T4006T` 已完成。
7. 提交一次 Git commit，然后停止。

## 当前执行结果

- 已复现并定位根因：`gc_continuation_cross_thread_resume_with_objects.scoop` 与 `gc_continuation_escape_deep_object_graph.scoop` 的 `value coercion` 并不是 effect/GC 专属问题，而是 class ctor 实参求值过程污染调用者局部环境。
- 具体原因：
  - `ClassInit` side table 由独立 lowering pass 生成，ctor 参数 `SymbolId` 不与主 HIR 调用点 locals 共享同一编号空间。
  - 旧的 `codegen_class_ctor_eval_args` 会在“显式实参尚未全部求值完”时，就把 ctor 参数提前写入 `env`。
  - 当后续显式实参的本地 `SymbolId` 与这些 side-table 参数碰撞时，调用者局部会被误读成 ctor 参数，典型表现是 `return Node(name, t, value)` 中最后一个 `value:Int` 被读成 `name:String`，最终触发 `String -> Int` coercion 失败。
- 已实施修复：
  - class ctor / super ctor / `this(...)` delegation 的显式实参现在先在调用者环境中完整求值；
  - 之后才进入 ctor 参数作用域绑定这些显式值，并在该作用域中补齐默认值；
  - 这样默认值表达式仍能读取已提供参数，但显式实参求值不会再受 side-table 参数 `SymbolId` 干扰。

## 当前验证结果

- 已通过：
  - `cargo run -p scoop -- build tests/fixtures/run-pass/gc_continuation_escape_deep_object_graph.scoop -o /tmp/t4006t_gc_deep.out`
  - `/tmp/t4006t_gc_deep.out` 运行成功，stdout 与 golden 一致。
  - `cargo run -p scoop -- build tests/fixtures/run-pass/gc_continuation_cross_thread_resume_with_objects.scoop -o /tmp/t4006t_gc_cross.out`
  - `/tmp/t4006t_gc_cross.out` 运行成功，stdout 与 golden 一致。
  - 新增 focused regression `tests/fixtures/run-pass/class_ctor_arg_eval_scope_shadow_free_basic.scoop`，用于覆盖“helper 局部 + struct 临时值 + class ctor call”这条最小主线；单独 `cargo run -p scoop -- run ...` 可通过。
  - `cargo run -p scoop -- test --fixtures <临时 fixtures root（仅包含上述 3 条 run-pass）>` 结果为 `fixtures: ok (3)`。
  - `cargo test --all` 通过。
  - `cargo clippy --all-targets -- -D warnings` 通过。
- 全量 suite 状态：
  - `cargo run -p scoop -- test` 已越过原先的 `gc_continuation_cross_thread_resume_with_objects.scoop` 红线。
  - 继续向后会稳定失败在 `tests/fixtures/run-pass/top_level_val_recursive_init_is_error.scoop`，报“stdout 与 golden 不一致”。
  - 单独把该 fixture 拷到临时 fixtures root 后可通过，因此当前更像是 full-suite 顺序相关 / harness 级既有问题，而不是该 fixture 单独不可运行。

## 新发现的既有问题

- 已在 `TODO.md` / `PLAN.md` 中新增后续 blocker：
  1. `T4006U`：`top_level_val_recursive_init_is_error` 的 full-suite 顺序相关 stdout mismatch。
  2. `T4006V`：链式成员访问在非局部 receiver 上的解析 / codegen 缺口（例如 `node.tag.label` 当前仍会报 `member access target` unsupported）。

## 下一步

- 更新任务文件并提交本轮改动。
- 本轮到此停止；下一次调用应从新增的 `T4006U` 开始。
