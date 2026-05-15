## 本轮执行计划（P4-T01g）

按照 `PROMPT.md` 规范完成 `TODO.md` 中第一个未完成任务 **P4-T01g**：解锁 subclass-typed receiver 调用 inherited body method 与访问 inherited 字段。

### 任务确认

- 检查 `TODO.md` 顶部任务索引，在 `P4-T01f` 已 [DONE] 之后第一个未完成任务确实是 `P4-T01g`（这是新增的两条 P4 前置 IV 子任务之一）。
- 检查最近一次 commit `[P4-T01f] Finalize execution log` 等，无遗留未完成项需要并入。

### 实现方案要点

- **resolver 改动**：在 `crates/scoopc/src/resolve/scopes.rs` 内 member-name 解析未命中"自身声明面"时，沿 class supertype 链（先 direct superclass、再 interfaces，按声明顺序，含已 inherited interface chain）查找 inherited body method / 字段。命中后挂回 member-call AST node，typecheck 阶段视同自身 method/field 一样消费。
- **typecheck 改动**：在 `crates/scoopc/src/typecheck/expr/call.rs::infer_member_call_expr_type` / 字段访问路径中，沿继承链查找的命中要正确处理：
  - `this` 隐含 receiver 同样适用（subclass body 内 `this.inheritedField` / `this.inheritedMethod()`）；
  - generic supertype 实例化要把 `T` 在 base 的位置映射到 subclass 实参。
- **HIR lowering**：复用现有 `<OwnerFqn>.<methodName>` top-level 调用 + receiver 上转；不新增 IR path。
- **不删除任何现有 by-name 特判 / 扩展函数 fallback**（删除留给 P4-T01）。
- 不引入"subclass-typed receiver 解析时优先访问 base 实现而绕过 vtable"这类回退；virtual dispatch 仍是唯一的 dispatch 来源，本任务只解决"前端可见性"。

### 顺序

1. 复现 gap：用最小 scoop 代码跑通 baseline 失败信号（不入库）。
2. 浏览 `resolve/scopes.rs`、`typecheck/expr/call.rs`、`hir/lower/expr.rs`、`typecheck/override_effects.rs::direct_superclass`，找到 inherited 查找的最小插入点。
3. 实现 resolver / typecheck 路径；保持 lowering 不变。
4. 加 fixture：subclass-typed receiver 调用 base ordinary body method、未 override interface default body、`this.inheritedField`、多级继承、`@Intrinsic class<T>` inherited default。
5. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`、`cargo test -p scoopc -- --nocapture`、`cargo clippy --all-targets -- -D warnings`。
6. 写 `[DONE] P4-T01g` 完成记录，提交 commit。

### 风险点

- 若 inherited generic supertype 实例化要重写 monomorphization key，可能涉及面比预期大；遇到此类阻塞按 PROMPT.md 处理：拆出更小前置或继续整体推进，但不引入 workaround。
- 若 base class 的 `val` 字段 access 当前完全通过 layout-based 路径走，则可能 resolver 已经拿不到字段元数据，需要在 layout / member meta 表上补 inherited 字段 expose。

### 进展更新

- **resolver / index 改动已落地**：在 `Index` 中新增 `direct_supertypes` 并通过 `collect_direct_supertypes` 在 `add_file_in_cone` 全部完成后单独构建；`scopes.rs` 中加入 `resolve_inherited_member`，在 `resolve_member_access_on_value_receiver` 现有所有 fallback 之后、落入 `unresolved_member` 之前按 BFS 沿继承链查找；命中规则限制为 "可见 + has_body 的 fun" 或 "可见 value"，避免 `Hashable.hash` default body 抢占现有 extension fun fallback。
- **HIR / typecheck 零改动**：实测既有 `late_resolve_direct_member_fun_fqn_from_receiver_ty` 与 `find_member_owner_nominal_instantiation` 已经能够沿继承链定位 owner 实例化，因此本任务在 typecheck / HIR lowering 上零改动。
- **新增 5 个 fixture**：
  - `inherited_member_call_base_class_body_method_basic.scoop`
  - `inherited_member_call_interface_default_body_basic.scoop`
  - `inherited_member_field_access_basic.scoop`
  - `inherited_member_call_multi_level_chain_basic.scoop`
  - `inherited_member_call_intrinsic_generic_class_basic.scoop`
- **回归确认**：
  - 上述 5 个新增 fixture 全部通过；
  - `member_call_*` / `interface_default_method_dispatch_basic.scoop` / `intrinsic_*_body_method_basic.scoop` / `intrinsic_named_*_basic.scoop` 既有 fixture 全部通过；
  - 全量 `tests/fixtures/run-pass` 与本任务介入前一致：唯二既存失败 `extern_native_aggregate_return_direct_indirect_parity.scoop`、`sync_gc_release_task_like_object_basic.scoop` 已在 `P4-T01i` 中显式登记；
  - 全量 `tests/fixtures/typecheck` 唯一失败 `extern_fun_gc_handle_raw_token_roundtrip_ok.scoop` 同样属 `P4-T01i`；
  - `cargo clippy --all-targets -- -D warnings` 通过；
  - `cargo test -p scoopc` 全量 9 个失败已在 `git stash` 复核后确认全部为 `@Unsafe @Extern` 与 failure-policy 行号 drift 的预先存在 issue，已在 `P4-T01i` 中显式登记，不归本任务范畴。
- **TODO.md 改动**：
  - 顶部任务索引追加 `P4-T01i`（清理 P2-T02 之后仍残留 `@Unsafe @Extern` 的 fixture / 单测 / failure-policy 行号常量）；
  - "P4 前置 I" / "P4 前置 IV" 顺序约束行更新为九个前置；
  - `P4-T01g` 标 `[DONE]` 并补完 改动范围 / 核心决策 / 验证结果 / 与 `PLAN.md` / `MANAGED_ABI.md` 的对应闭合记录；
  - `P4-T01i` 任务体说明三类清理：fixture-only / inline-source 单测 / failure-policy 行号 sentinel。

### 完成状态

- 已完成：实现、回归、`[DONE]` 记录、`P4-T01i` 登记、`memory/claude_plan.md` 刷新；
- 待提交：`git status` 中的本任务 deltas（`crates/scoopc/src/resolve/{mod,scopes}.rs` / `tests/fixtures/run-pass/inherited_member_*.scoop` / `TODO.md` / `memory/claude_plan.md`）。
