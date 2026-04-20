# 执行记录与计划

## 说明

按要求先记录执行思路与步骤。这里记录的是可审计的决策摘要与执行计划，不包含冗长的内部推理细节。

## 当前目标

完成 `TODO.md` 中第一个未完成任务，并在完成后停止。

## 总体步骤

1. 检查最新一次 Git 提交，确认是否提到了已知问题、遗留修复或未完成事项。
2. 如果最新提交中存在明确提到且尚未修复的问题，先将这些问题纳入当前工作范围并优先修复。
3. 阅读 `TODO.md`，定位第一个未完成任务。
4. 评估该任务是否可以在一次迭代内完整交付。
5. 如果任务过大，则拆分为更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`，本次只执行拆分后的第一个子任务。
6. 实施该任务，必要时补充或调整代码结构与注释。
7. 运行相关测试、格式化、静态检查，至少覆盖与本次改动直接相关的范围；如果任务本身影响较广，则扩展验证范围。
8. 若发现规范不匹配、实现缺口或依赖前置问题，不做规避；改为在 `TODO.md`/`PLAN.md` 中新增前置任务、调整顺序，并在本次提交后停止。
9. 如果任务完成，则更新 `TODO.md` 与 `PLAN.md` 的状态记录。
10. 提交 Git commit，提交信息对应当前任务。
11. 停止，不继续处理下一个任务。

## 初始假设

- 仓库可能已有未提交修改，处理时不能回退不属于本次工作的用户改动。
- `memory/claude_plan.md` 需要在执行过程中持续更新，记录关键进展、计划调整与完成情况。
- 如果遇到编译警告或 Clippy 告警，应一并修复到当前任务涉及范围可接受为止；若告警暴露更早的实现问题，则按依赖关系处理。

## 待完成检查点

- [x] 最新提交检查
- [x] 识别第一个未完成任务
- [x] 评估是否需要拆分
- [x] 实施改动
- [x] 运行验证
- [x] 更新 `TODO.md`
- [x] 更新 `PLAN.md`
- [x] 更新本文件进度
- [ ] 提交 Git commit

## 当前进展

- 已检查最新提交 `12af7b10a0b3d8730c2048f1f7c845900ea6b6ce`，提交标题为 `[T4010b0] Unify struct ctor calls with struct literals`。提交信息本身未附带额外需要优先修复的遗留 issue 描述。
- 已定位本轮第一个未完成任务为 `T4010b0R`：复审 `struct` ctor call 与 struct literal 的统一构造主线。
- 当前复审重点：
  1. resolver 是否只保留一套面向 `struct` 的 direct construction 参数模型。
  2. typecheck 是否让 `StructName(...)` 与 `StructName { ... }` 在命名参数 / 泛型 / 重载选择上得出一致结果。
  3. HIR lowering 是否把 struct ctor call 收口为既有 `StructLit`，而不是保留另一条 codegen 专用路径。
  4. 用最小 probe 复验是否存在“literal 可通过、ctor call 失败/语义不同”的实际裂缝。

## 复审发现

- 最小 probe `struct Wrapper<T>(val box: Box<T>)` 暴露出一条真实裂缝：
  - `val wrapped: Wrapper<Int> = Wrapper { box: Box(41) }` 会在 typecheck 阶段报字段类型不匹配；
  - `val wrapped: Wrapper<Int> = Wrapper(Box(41))` 会在 typecheck 阶段报 `no_matching_overload`。
- 根因不是测试写法，而是两条主线都没有把嵌套泛型字段 `Box<T>` 完整具体化：
  - generic struct literal 只会把“字段类型本身就是 `T`”的情况替换成具体类型，像 `Box<T>` 这样的嵌套 nominal 不会递归替换。
  - ctor overload 匹配只会从“形参类型本身就是 `T`”的情况收集 type arg，像 `Box<T>` 这样的嵌套形参不会参与推断。
- 在修完上述 typecheck 裂缝后，probe 继续暴露出一个更底层、此前未登记的 HIR/LLVM 布局缺口：
  - generic struct/enum layout 生成时只支持裸 type param / tuple / nullable，像 `Box<T>` 这样的嵌套 nominal 字段拿不到 concrete `TypeId`/layout key，导致 build 阶段报 `scoop::llvm::unsupported_main_body: struct field type`。

## 已实施修复

- `crates/scoopc/src/typecheck/expr/infer.rs`
  - 为 generic struct literal 增加按 type param 名称递归收集/替换的逻辑，确保 `Box<T>`、`((T))`、`T?` 等嵌套字段类型都能在 expected type 下被具体化。
- `crates/scoopc/src/typecheck/expr/call.rs`
  - ctor overload 匹配改为先递归收集嵌套泛型约束，再实例化具体形参类型后做验参；不再只识别顶层 `T`。
- `crates/scoopc/src/hir/lower/util.rs` 与 `crates/scoopc/src/hir/lower/mod.rs`
  - generic struct/enum instantiation layout 现在会在声明处解析 nominal 路径并递归替换 type args，给嵌套 nominal 字段保留 concrete `TypeId` 与 mangled layout key。
- 已新增正式回归夹具：
  - `tests/fixtures/run-pass/struct_ctor_call_nested_generic_equivalence_basic.scoop`
- 已用两个最小 probe 复验：
  - struct literal 版本可 build，并以退出码 `42` 结束。
  - struct ctor call 版本可 build，并以退出码 `42` 结束。

## 验证结果

- `cargo fmt`
- `cargo run -q -p scoop -- build tests/fixtures/run-pass/struct_ctor_call_nested_generic_equivalence_basic.scoop -o /tmp/t4010b0r_fixture.out`
- `/tmp/t4010b0r_fixture.out` 退出码 `0`
- `cargo run -q -p scoop -- test --fixtures tests/fixtures/run-pass` → `fixtures: ok (362)`
- `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck` → `fixtures: ok (344)`
- `cargo test --all` → 通过
- `cargo clippy --all-targets -- -D warnings` → 通过

## 当前状态

- `T4010b0R` 已完成并已在 `TODO.md` / `PLAN.md` 中结案。
- 下一轮首个未完成任务将是 `T4010b`。
