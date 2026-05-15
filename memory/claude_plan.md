## 本轮执行计划（P4-T01l）

按照 `PROMPT.md` 规范完成 `TODO.md` 中第一个未完成任务 **P4-T01l**：解锁 `@Intrinsic struct/class` body method 在 builtin scalar receiver / `ToString.toString` interface dispatch 上的可达性。

### 任务确认

- 上一次会话把 P4-T01 主任务的 sysroot 改写回退，并新增 P4-T01l 作为前置 / 双轨可达性机制。
- P4-T01l 的"必须实现内容"包含三块：
  1. typecheck 层把 builtin scalar / `String` 的 nominal FQN 提取统一进 member-call 主线；
  2. 让 `ToString.toString` interface dispatch 在 `@Intrinsic struct/class` override 引入后仍能被 monomorphization / late lowering 正确发布 callable body；
  3. HIR / MIR / LLVM 收口受体作为第 0 个 arg 的传递；

  并保留所有现有 by-name 路径作为过渡 surface（删除留给 P4-T01）。

### 本轮已完成

- **typecheck FQN 提取统一**（要求 1）：在 `crates/scoopc/src/typecheck/expr/ops.rs` 新增 `try_extract_member_call_receiver_fqn_and_args`，识别 `ValueTypeKind::{Bool, Char, Int, UInt, Float32, Float64}` / `RefTypeKind::String` 并映射到 `scoop.core.<X>` FQN（无 type args）。
- 在 `late_resolve_direct_member_fun_fqn_from_receiver_ty` 与 `infer_member_call_expr_type` 主线（line ~6275）替换调用 `try_extract_nominal_fqn_and_args` 的两个点为新 helper。
- **回归基线全部干净**：
  - `cargo test -p scoopc`：861 passed, 0 failed。
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`：400/400 PASS。
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`：437/437 PASS。
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build`：42/42 PASS（含既有 `intrinsic_sysroot_overlay_array_mutablearray_body_methods_basic`、`sysroot_overlay_core_array_interface_bridge`）。
  - `cargo clippy --all-targets -- -D warnings` clean。

### 本轮未完成（owner test 与下游 gap）

为反复验证语义，本会话试做了 sysroot overlay fixture（`tests/fixtures/build/intrinsic_sysroot_overlay_scalar_tostring_basic*`）覆盖 sysroot overlay 把六个 builtin scalar 改写为 `@Intrinsic struct/class : Hashable, ToString { override fun toString(): String { ... } }` 的真实场景。基于这份探针得到三类观察：

1. **dual-track 简化形态（仅类型上加 `@Intrinsic`，不加 override body）通过**：说明 `@Intrinsic` 注解在 builtin scalar 类型上不会与 layout 内置识别冲突；新 helper 在该形态下不会被命中（typecheck 早 short-circuit 仍生效）。
2. **dual-track 完整形态（同时声明 override body 与 by-name extension）触发 P4-T01l 任务体里事先描述的 gap 2**：`println(<value>)` 报 `LLVM stage handoff 缺少 reachable callable scoop.core.ToString.toString 的 published late-lowered body`。说明 builtin scalar override 引入后，`ToString.toString` interface 的 default body / itable 入口未被 monomorphization / late lowering 收集发布。
3. **真正"new path"形态（在 `@Intrinsic struct Bool` 上加一个非 by-name 名字的 method，比如 `negate`）触发 gap 1**：`true.negate()` 在 typecheck 通过后，MIR codegen 报 `pass MIR direct call arity mismatch`，`scoop.core.Bool.negate` 的 MIR `Direct` 调用 args=[]——HIR → MIR 的 receiver-prefix 仍未把 builtin scalar receiver 作为第 0 个 arg 注入。

为不污染 mainline fixture 集，最后**已删除该探针 fixture**（typecheck FQN 提取的 helper 改动本身被既有 fixture 间接覆盖：原本 nominal-only 的两条 call 路径现在能正确处理 builtin scalar receiver，对默认 sysroot 不改变行为）。

下游 gap 1 / gap 2 的修复涉及：

- **gap 1**：HIR `lower_canonical_call_expr` 看到 builtin scalar receiver 进入 path 597+ 路径时，`type_kinds.get("scoop.core.Bool")` 当前返回 None / 让 path 597+ closure 落入 fallback。需要让 HIR 知道 sysroot `@Intrinsic struct Bool { ... }` 已经把 `scoop.core.Bool` 注入 `type_kinds`，并保证 receiver 一定经 `CanonicalCallLoweringRequest::receiver = Some(...)` 进入。
- **gap 2**：emit / late-lowering 链路在 builtin scalar override 引入后没有把 `scoop.core.ToString.toString` 的 default body / itable 入口正确发布，导致 `has_published_body` 检查失败。修复需要审视 monomorphization 的 reachable callable 收集与 late-lowered program 发布通道。

由于这两个 gap 是 P4-T01l 的 "必须实现内容" 里的 #2 与 #3 子项（任务体早已显式列出 gap 1 / gap 2），按 `PROMPT.md` "Default to finishing the current task as written" 的硬约束，**不在本轮把 P4-T01l 拆解为新前置子任务**——但本轮的 helper 改动已经把 gap 1 / gap 2 的现场缩窄到上述两条具体的下游通道，下次会话承接时不需再重做 typecheck 层的探查。

### 顺序

1. ✅ 阅读 `try_extract_nominal_fqn_and_args` / `late_resolve_direct_member_fun_fqn_from_receiver_ty` 主线，定位需要扩展的两处。
2. ✅ 实现 helper、接通 typecheck。
3. ✅ 用 sysroot overlay fixture 反复验证三类形态分别命中哪条 gap。
4. ❌ HIR / MIR receiver-prefix 收口（gap 1，下次会话承接）。
5. ❌ `ToString.toString` published late-lowered body 收口（gap 2，下次会话承接）。
6. ❌ owner test 端到端通过、`cargo test -p scoopc` / fixture 全量再跑、提交。

### 风险点

- `gap 1` 涉及 HIR `lower_canonical_call_expr` 与 `type_kinds` 的 sysroot 类型注入；如果 sysroot 类型未被 `collect_type_decl_kinds` 完整收集，需要先补该路径。
- `gap 2` 涉及 monomorphization 与 late-lowering 的 reachable callable 收集逻辑；若需要新增 itable 发布通道，可能比 helper 改动大很多。
- 既有 by-name 路径必须保留作为过渡 surface（任务体硬约束），所以 gap 修复必须采取"add 新机制 + 不破坏旧机制"的策略。

### 完成状态

- 未完成：P4-T01l 仍为 `[TODO]`；本轮提交内容仅包括 helper（typecheck FQN 提取）与对应 typecheck 主线接通，加 plan 与执行记录更新。Gap 1 / gap 2 留给下次会话承接。
- 待提交：
  - `crates/scoopc/src/typecheck/expr/ops.rs`（新 helper）
  - `crates/scoopc/src/typecheck/expr/call.rs`（接通 helper 到两个调用点）
  - `memory/claude_plan.md`
