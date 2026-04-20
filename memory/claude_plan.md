# Claude Plan

## 目标
本轮只完成 `TODO.md` 中第一个未完成任务；但在开始该任务前，必须先检查最新提交是否提到任何既有问题，并优先修复这些问题。

## 当前已知执行计划
1. 查看最新提交信息，确认是否显式提到待修复问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对现有计划、依赖与任务顺序。
4. 如首个未完成任务过大，则拆分为更小子任务，并同步更新 `TODO.md` 与 `PLAN.md`。
5. 实现当前应执行的首个任务或子任务。
6. 运行相关测试，并补充必要测试，直到相关检查通过。
7. 更新文档状态：在 `TODO.md` 标记完成，在 `PLAN.md` 记录进展与调整。
8. 复核工作区改动，使用清晰提交信息提交本轮变更。
9. 停止，不继续下一个任务。

## 执行原则
- 不使用规避方案、兼容性垫片或仅为测试通过的临时实现。
- 若发现规格缺口、实现边界或前置依赖缺失，必须先在 `TODO.md` / `PLAN.md` 中显式建模，再提交并停止。
- 过程中如计划发生变化，或关键步骤完成，我会继续更新本文件。

## 备注
- 由于尚未读取仓库当前状态，以上为初始计划；在检查最新提交、`TODO.md`、`PLAN.md` 后会细化为针对当前任务的具体步骤。

## 当前进展（2026-04-20）
- 已检查最新提交 `5fb2099da083ae3fdf022b088899beab0727dc28`，提交主题为 `[T4010b0] Add struct ctor-call blocker before value defaults`。该提交没有留下“已知未修复但未入 TODO”的额外问题；它做的就是把一个既有 blocker `T4010b0` 显式插入任务序列。
- 已阅读 `TODO.md` 与 `PLAN.md`，确认第一个未完成任务为 `T4010b0`：收口 `struct` ctor call 与 struct literal 的统一构造语义。
- 当前尚未判断是否需要把 `T4010b0` 再次拆分；先检查 parser / resolver / typecheck / lowering / codegen 中 struct literal 与 direct ctor call 的现状，若发现横跨多套独立基础设施且无法在本轮完整收口，再按要求拆分并先只执行第一个子任务。
- 已复现实例：
  - `struct Point(val x: Int, val y: Int); Point(1, 2)` 当前在 typecheck 阶段报 `scoop::typecheck::callee_not_callable: Point`。
  - `struct Point { val x: Int; val y: Int }; Point(1, 2)` 也同样失败，而 `Point { x: 1, y: 2 }` 可正常 build。
- 结论：`T4010b0` 不能只把“带 primary ctor 参数的 struct”接进 class ctor 主线；否则 body-property 风格 struct 仍会与 struct literal 分裂，继续违背 spec §2.3.1 的等价承诺。

## 针对 T4010b0 的具体执行路线
1. 把 resolver / constructor 索引里的 struct 构造入口改为“按 direct field 列表合成”的统一表示：
   - 覆盖 primary ctor 参数；
   - 覆盖 body-property 风格字段；
   - 避免把 struct secondary ctor 当作可执行入口继续扩大语义面。
2. 在 typecheck `call` 主线中把 struct ctor call 作为 nominal value construction 处理，而不是只识别 class ctor。
3. 在 HIR lowering 中把 struct ctor call 直接降到与 struct literal 共用的 `StructLit` 表示，确保后端复用已有 struct aggregate codegen。
4. 补充最小回归：
   - resolver/typecheck：body-property struct 与命名参数都能进入构造主线；
   - build/run-pass：`Point(...)` 与 `Point { ... }` 等价；
   - HIR lowering：struct ctor call 不再保留为普通 `Call`，而是稳定收口为 `StructLit`。
5. 跑定向测试，再跑受影响的更广测试与 `cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
6. 更新 `TODO.md` / `PLAN.md` 标记 `T4010b0` 完成，并提交本轮改动后停止。

## 完成记录（2026-04-20）
- 已完成 `T4010b0`，实现结果：
  - resolver / `Index::constructors` 现为 `struct` 合成 direct-field constructor：primary ctor 参数与 body-property 字段会按统一顺序进入同一组构造参数。
  - `struct` secondary ctor 不再进入 direct constructor candidate，避免把 class-only 初始化执行体暴露成 value-construction 入口。
  - typecheck 的 unresolved call 主线现统一处理 nominal constructor call，class ctor 与 struct field constructor 共用命名/位置参数绑定与 overload 选择。
  - HIR lowering 现把 struct ctor call 直接收口为 `StructLit`，后端复用既有 struct aggregate codegen。
- 已新增回归：
  - resolver 单测：`struct_field_constructor_call_is_collected_as_candidate`
  - build：`struct_ctor_call_minimal_ok.scoop`
  - run-pass：`struct_ctor_call_literal_equivalence_basic.scoop`
  - typecheck：`struct_ctor_call_ok.scoop`、`struct_ctor_call_unknown_named_arg_is_error.scoop`
  - HIR：`struct_ctor_call_lowering.scoop` / `.hir`
- 已验证：
  - `cargo test -q -p scoopc struct_field_constructor_call_is_collected_as_candidate -- --nocapture`
  - `cargo run -q -p scoop -- build tests/fixtures/build/struct_ctor_call_minimal_ok.scoop -o /tmp/struct_ctor_call_minimal_ok.out`
  - `/tmp/struct_ctor_call_minimal_ok.out`（退出码 `3`）
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (344)`）
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/hir`（`fixtures: ok (18)`）
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/run-pass`（`fixtures: ok (361)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 当前剩余动作：
  1. 复核 `git diff` 与工作区状态。
  2. 提交本轮改动，提交信息使用 `T4010b0`。
  3. 停止，等待下一次调用处理 `T4010b0R`。
