# 本轮执行计划

## 当前任务：T3004d — Cleanup scope、嵌套 handle 与 emitter 完善

### 分析

T3004d 需要补齐三类 emitter 残留 stub：

1. **CleanupEnter terminator** — 当前 `ret void`，应改为 branch 到 cleanup entry state
2. **CleanupEdgeComplete / ReturnToEnclosingExpression ops** — 语义标记，保持 no-op 是正确的
3. **NestedHandle / NestedHandleBoundary ops** — 当前 `ret void` 中断，应委托递归 codegen

### 实现计划

#### 1. CleanupEnter terminator
Cleanup scope 在 plan builder 中的结构：
- body_end_state 的 terminator 是 `CleanupEnter { scope_id, next_state: cleanup_entry }`
- cleanup_entry → cleanup_end 是 finally block 的 stmts
- cleanup_end 有 `CleanupEdgeComplete` op + `Goto(cleanup_exit)`
- cleanup_exit 有 `ReturnToEnclosingExpression` op + `ReturnHandle`

所有 cleanup states 已经在 step function 的 state_bb_map 中有对应的 basic block。
`CleanupEnter` 只需要 unconditional branch 到 `next_state` 对应的 bb 即可。

#### 2. CleanupEdgeComplete / ReturnToEnclosingExpression
保持 no-op，更新注释说明这是设计如此而非 placeholder。

#### 3. NestedHandle / NestedHandleBoundary
两种场景：
- `NestedHandle { nested_id, expr }`: 不会 suspend 的嵌套 handle → 直接委托 `codegen_expr_in_expected_context(expr)` 递归进入 `codegen_handle_expr`
- `NestedHandleBoundary { site_id, nested_id, expr }`: 可能 suspend 的嵌套 handle → 同样委托 `codegen_expr_in_expected_context(expr)`；state machine 已经安排了 Suspend terminator 来处理内部 perform 冒泡

#### 4. 验证
- `cargo check -p scoopc` 零 warning
- `cargo clippy --all-targets -- -D warnings` 通过
- `cargo test --all` 通过

### 执行状态
- [x] 分析代码
- [ ] 实现 CleanupEnter terminator
- [ ] 更新 cleanup op 注释
- [ ] 实现 NestedHandle / NestedHandleBoundary
- [ ] 验证
- [ ] 更新 TODO.md / PLAN.md 并提交
