## 可共享执行思路摘要

目标：严格按照仓库约定与 `TODO.md` 的顺序推进，只完成首个未完成任务，然后停止。

当前预定执行顺序：
1. 检查最新一次提交，确认提交信息是否提到已知问题、遗留修复或需要先处理的事项。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`、相关源码与测试，评估该任务是否可在本轮完整落地。
4. 如果任务过大或存在前置缺口，则先把任务拆分并更新 `TODO.md` / `PLAN.md`，本轮只执行新的第一个子任务。
5. 实现该任务，补充或调整测试。
6. 运行必要的格式化、静态检查与测试，至少覆盖本次改动的直接范围，并尽量满足仓库要求中的 `clippy`/测试约束。
7. 更新 `memory/claude_plan.md` 记录进展，更新 `TODO.md` 与 `PLAN.md` 的任务状态。
8. 用清晰的提交信息提交本轮变更，然后停止，不进入下一个任务。

执行原则：
- 不接受规避性实现、夹带临时兼容或只为夹具通过的 hack。
- 如果遇到规范缺口、编译器/运行时 bug、缺失语言特性或其它阻塞项，先把问题转成更前置的 `TODO.md` 任务，并更新 `PLAN.md` 说明依赖关系，然后提交并停止。
- 不回退或覆盖与当前任务无关的现有改动。
- 在关键步骤完成后，持续更新本文件，便于检查进展。

## 进展记录

- 已创建初始计划文件，下一步检查最新提交与任务清单。
- 已检查最新提交：提交信息仅为 `Update plan`，未声明需要先修复的遗留问题。
- 已读取 `TODO.md` / `PLAN.md`，本轮首个未完成任务为 `T4001`：泛型约束、参数化超类型与 star projection。
- 已定位当前缺口：
  - `where` 子句在 `crates/scoopc/src/typecheck/where_clause.rs` 中仍拒绝带类型实参的 bound。
  - `TypeEnv` 当前只保存 direct supertypes 的 FQN，不保存带 type args 的原始超类型信息。
  - `assignable` 当前只在“目标 nominal 不带实参”时才走上转，无法处理 `MyIter<Int> -> Iterable<Int>`。
  - `star projection` 在通用 lowering 中被降成 `Any`，在 enum-pattern 专用 lowering 中仍直接拒绝。
- 当前实现判断：
  - 该任务可以在本轮直接落地，不需要先拆新子任务。
  - 需要同时覆盖 typecheck 与 HIR/LLVM 可执行链路，否则 run-pass 仍会在类型擦除或 side table 上裂开。

## 当前实施方案

1. 在 `TypeEnv` 中新增“保留原始 type args 的 direct supertype 定义”存储，同时保留原有 FQN-only 索引以兼容现有调用方。
2. 在 `TypeLowering` 中为“具体 nominal 实例化后的 direct supertypes”建立缓存：
   - 在 `Foo<A, B>` 被 lowering 成 `TypeId` 时，立刻按声明处 type params 做 substitution，得到具体 direct supertype `TypeId` 列表。
   - 后续 `assignable` 直接沿这个具体化缓存做 DFS，而不是只看裸 FQN。
3. 放开 `where` 子句对 generic nominal bound 的拒绝，并复用新的参数化 assignable 规则检查约束满足性。
4. 为 star projection 引入独立的 typecheck 内部表示：
   - typecheck 阶段保留 `*` 身份，用于区分“只读视图”与普通 `Any?`。
   - HIR 重建阶段把 `*` 擦除为其运行时读视图（预期是 `Any?`），避免后端需要理解新的源级语义节点。
   - assignable 规则中要求：
     - `Ref` / `Option<Ref>` 可上转到 `*` 读视图；
     - 值类型（如 `Int`）不能隐式上转到 `*`，避免绕过显式 boxing 语义。
     - `*` 不能作为普通可写目标类型接受任意值写入。
5. 补充回归：
   - `where T: Bound<Arg>` 通过/失败 fixtures。
   - 参数化超类型赋值/调用 fixtures，例如 `MyIter<Int> -> Iterable<Int>`。
   - star projection typecheck / run-pass fixtures，至少覆盖：
     - `Array<String>` / `Iterable<String>` 到 `*` 视图可读。
     - `Array<Int>` 到 `*` 视图继续拒绝，要求显式 boxing。
6. 跑格式化、相关测试、`clippy`，然后更新 `TODO.md` / `PLAN.md` 并提交。
# 本轮执行计划（T4001）

说明：按要求先记录执行计划。这里记录的是可审计的执行步骤与决策，不包含模型内部推理细节。

1. 检查当前工作区状态，确认上一轮已经完成的修改范围仍然存在，并确认首个未完成任务仍是 `TODO.md` 中的 `T4001`。
2. 先修复当前代码中因引入 `TypeKind::StarProjection` 造成的编译缺口，重点检查：
   - RTTI / layout / 导出路径中的穷尽匹配；
   - 新旧超类型信息在 typecheck、HIR、LLVM 之间的衔接；
   - 所有新增分支是否仍保持“仅在 typecheck 内部保留 star，导出时擦除为读视图”的设计。
3. 运行 `cargo check -p scoopc`，根据实际报错继续补齐遗漏，直到 `scoopc` 编译通过且无新增 warning。
4. 为 `T4001` 增加或修复测试，覆盖至少三类行为：
   - 泛型约束允许参数化 nominal bound；
   - 参数化超类型参与赋值/上转判定；
   - star projection 的可读视图语义，以及值类型不能隐式视为 `*`。
5. 运行相关测试，再运行更完整的校验（至少包含本任务相关测试集、`cargo clippy --all-targets -- -D warnings`）。
6. 如果在实现过程中发现规范不匹配或新的前置缺口，按要求先把阻塞项写入 `TODO.md` / `PLAN.md`，并停止在正确的依赖位置。
7. 若 `T4001` 完成，则更新：
   - `TODO.md`：仅标记 `T4001` 完成；
   - `PLAN.md`：记录完成情况与后续任务关系；
   - 本文件：记录关键步骤完成情况与最终测试结果。
8. 提交一次清晰的 git commit，然后停止，不继续处理后续任务。

当前已知重点：
- `TypeKind::StarProjection` 需要在 RTTI / 导出 / 代码生成等路径补齐分支。
- 参数化超类型目前在 typecheck 侧已有一半实现，需要实测是否覆盖运行时/导出路径。
- 必须避免以 workaround 通过测试；若发现真实缺口，需要把缺口显式前置到 `TODO.md`。

## 本轮新增进展

- 已重新运行 `cargo check -p scoopc`，确认当前首批阻塞为：
  - `crates/scoopc/src/rtti/type_desc.rs`
  - `crates/scoopc/src/rtti/mod.rs`
  中 `TypeKind::StarProjection` 的非穷尽匹配；
  - `where_clause.rs` 与 `assignable.rs` 的现有 warning。
- 已完成上述首批修复：
  - RTTI / layout 中把 `StarProjection` 按其 `read_ty` 视图处理；
  - `type_rtti()` 中暂按引用语义导出 `RttiKind::Ref`；
  - 清理了 `where_clause.rs` 无用导入与 `assignable.rs` 未使用变量。
- 下一步：再次运行 `cargo check -p scoopc`，确认是否还存在更深层的 `StarProjection` 穷尽匹配遗漏或真实语义错误。

- `cargo check -p scoopc` 已通过。
- 已新增 `T4001` 回归 fixture 草案，覆盖：
  - 参数化 where bound 的通过/失败；
  - 参数化超类型上转 + interface dispatch；
  - `Array<String> -> Array<*>` 与 `Array<String?> -> Array<*>` 的读视图；
  - `Array<Int> -> Array<*>` 的拒绝。
- 已同步修正文档注释：`sysroot/collections.scoop` 不再声称“参数化 interface 上转不支持”。
- 下一步：逐个运行新增 fixture；若触发真实缺口，先修实现，再扩大测试范围。

- 在运行 `star_projection_array_read_view.scoop` 时发现后端缺失一条真实 coercion：
  - 需要把 pointer-like 的 ref / `String` / `Option<Ref>` 映射到 star 读视图擦除后的 `Option<Any>`；
  - 已在 LLVM `coerce_value` 中补上“pointer niche nullable-ref enum” coercion helper。

## 本轮收尾结果

- `T4001` 已完成，未新增需要前置拆出的 blocker 任务。
- 已完成的验证：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck` → `fixtures: ok (326)`
  - T4001 定向 run-pass（参数化超类型 interface dispatch + `Array<String> -> Array<*>` 读视图）→ `fixtures: ok (2)`
  - `cargo test --all` → 通过
  - `cargo clippy --all-targets -- -D warnings` → 通过
- 已更新：
  - `sysroot/collections.scoop`：移除过时的“参数化 interface 上转不支持”注释。
  - `TODO.md`：仅将 `T4001` 标记为完成。
  - `PLAN.md`：记录当前轮完成情况与下一项 `T4001R`。
  - `ISSUES.md`：第 5 条改为“已收口”状态。
