# T4003T 执行记录

## 当前任务

- 目标任务：`T4003T` 收口局部 `val` pattern binding 的可执行 lowering / codegen。
- 约束：只完成 `TODO.md` 中当前第一个未完成任务，完成后更新 `TODO.md`、`PLAN.md`、`memory/claude_plan.md`，提交 git commit 后停止。
- 当前判断：AST / resolver / typecheck 已支持局部 `val` destructuring，剩余问题集中在 HIR lowering 产物进入 LLVM codegen 时的匿名局部绑定与 struct field 投影。

## 已完成实现

1. 为 lowering 生成的临时局部增加唯一 synthetic local 分配逻辑，避免匿名局部在 LLVM codegen 阶段直接失败。
2. 将 block / stmt lowering 改为支持一个 AST 语句展开成多个 HIR 语句，以承载局部 pattern destructuring 展开。
3. 为局部 `val` pattern binding 增加 lowering：
   - 先把 RHS 求值保存到临时 subject；
   - 对 variant pattern 合成运行期检查；
   - 为每个 binder 合成独立局部 `ValDecl`；
   - tuple / struct binder 使用成员投影；
   - variant binder 使用合成 `when` 提取。
4. 在 typecheck 阶段把局部 pattern binder 的推断类型记录回 side table，供 HIR lowering 读取。
5. 修复普通 `val` lowering 在无显式类型注解时过度退化成 `Any` 的问题，优先复用 initializer 的 typechecked type，避免 LLVM `value coercion` 失败。
6. 新增 HIR 与 run-pass fixtures，覆盖 tuple / struct / variant 的局部 destructuring 与 mismatch 行为。

## 结果更新

- `T4003T` 已完成实现并通过定向验证。
- 本轮最终收口内容：
  1. 局部 `val` pattern binding 现会在 HIR lowering 阶段展开成：
     - 单次求值的 synthetic subject；
     - 必要的 variant 运行期校验；
     - 每个 binder 的独立命名 `ValDecl`；
     - tuple / struct / variant 的统一投影 / 提取表达式。
  2. `typecheck` 现会把局部 pattern binder 的推断类型写回 side table，供 lowering 读取。
  3. `lower_val_decl` 现优先复用 initializer 的 typechecked type，避免局部 subject / 普通 `val`
     因 HIR `VarRef.ty = Any` 退化到错误 codegen。
  4. `collect_struct_layouts(...)` 与 `collect_generic_struct_instantiation_layouts(...)`
     现会把 struct body 中真正拥有 backing field 的 property 一并写入布局；这修复了
     `struct Point { val x: Int; val y: Int }` 一类 body-property struct 的字段投影失败。
  5. tuple literal lowering 现优先使用 `typechecked_expr_ty(span)`，避免
     `(noneValue, 7)` 这类字面量在 build/test 路径下把元素静态类型退化成 `Any/Ref`。

## 验证结果

- 已通过：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/local_val_destructuring_tuple_struct_variant_basic.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/local_val_destructuring_nested_variant_mismatch_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/hir`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 额外观察：
  - 尝试执行全量 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 时，
    现有仓库里的 `gc_continuation_cross_thread_resume_with_objects.scoop` 仍以
    `scoop::llvm::unsupported_main_body: value coercion` 失败；
  - 该失败与本轮新增的局部 destructuring 回归无关，本轮未继续展开该独立问题。

## 本轮执行计划

1. 检查当前工作树与相关代码位置，确认未提交修改和最新失败点。
2. 重新运行 struct/tuple/variant destructuring 的真实用例，确认 body-property struct layout 修复是否已打通执行路径。
3. 若仍有失败，继续沿 lowering / LLVM codegen 路径定位剩余问题。
4. 运行真实验证：
   - 单文件 `scoop run` 覆盖 tuple / struct / variant destructuring；
   - HIR fixture 校验；
   - 相关 fixture 子集；
   - `cargo test --all`；
   - `cargo clippy --all-targets -- -D warnings`。
5. 更新 `TODO.md`、`PLAN.md`、本文件，并提交 `[T4003T] ...` 风格 commit。
6. 停止，等待下一次调用。

## 执行原则

- 不接受 workaround，不通过修改 fixture 规避真实实现缺口。
- 只在本轮完成 `T4003T`，不推进下一个任务。
- 所有手工文件编辑使用 `apply_patch`。
