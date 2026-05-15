## 本轮执行计划（P4-T01）

按照 `PROMPT.md` 规范完成 `TODO.md` 中第一个未完成任务 **P4-T01**：以标量 `toString` 为 tracer bullet，删除第一批字符串名字特判。

### 任务确认

- `TODO.md` 中所有 P4 前置（`P4-T01a/b/c-pre1/c/d/e/f/g/h/i/j/k`）已 [DONE]；下一条是 `P4-T01`。
- 最近一次提交 `[P4-T01k]` 后 `cargo test -p scoopc` 已 0 failed。

### 实现方案

将以下 6 个内建类型从 declaration-only 形式改写为 `@Intrinsic struct/class : Hashable, ToString { override fun toString(): String { ... } }` 形式：
- `Int.toString` — body 调 `scoopAbiIntToString(this)` bridge（已由 P4-T01f 提供）
- `Char.toString` — body 调 `scoopAbiCharToString(this)`
- `Float64.toString` — body 调 `scoopAbiFloat64ToString(this)`
- `Float32.toString` — body 调 `scoopAbiFloat32ToString(this)`
- `Bool.toString` — body `if (this) "true" else "false"`
- `String.toString` — body `return this`

清理以下 by-name 特判（删除而不是保留双轨）：
- resolver: `crates/scoopc/src/resolve/scopes.rs` 中 Int/Bool/Char/Float `toString` allowlist
- typecheck: `crates/scoopc/src/typecheck/expr/call.rs` 内建 `toString` synthetic 直接返回 String 的 contract
- HIR: `crates/scoopc/src/hir/lower/expr.rs::should_keep_member_call_as_member_access` 的 `toString` keep-list
- LLVM intrinsics: `try_codegen_tostring_iface_builtin` / `codegen_sysroot_to_string_ext`
- effect_lowered: `lower_builtin_to_string_call` / `lower_refactor_core_to_string_call` / `refactor_core_print_to_string`
- mir_body: `codegen_mir_transport_to_string` 按 `CgTy` 派生 runtime helper 名字的分支

`ToString.toString` interface dispatch 对用户类型仍可用（P4-T01a/b/c 已保证）。

### 顺序

1. 阅读现有 `sysroot/core.scoop` 中六个类型的 declaration-only 形式与 `sysroot/scalar_string_bridge.scoop` 的 bridge 暴露面。
2. 改写 sysroot：把内建类型从 declaration-only 形式改为 `@Intrinsic struct/class { override fun toString(): String { ... } }`，body 用 Scoop 写出。
3. 跑 `cargo build -p scoopc` 确认 sysroot 形态被 typecheck 接受。
4. 跑 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`，看 tracer bullet 之前的旧路径是否还能 pass（保留旧路径作为过渡时期的 safety 期望，即新路径落地后旧路径会被精确删除而不是双轨）。
5. 删除上面列出的 by-name 特判，逐一删除并验证（先验证用户类型的 `ToString.toString` 仍能 work 后再删）。
6. 检查 `print/println` 链路。
7. 加 fixture：scalar `toString` 端到端 run-pass。
8. 跑 `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 cargo run -p scoop -- test --fixtures <fixture>`、`cargo test -p scoopc`、`cargo clippy --all-targets -- -D warnings`。
9. 写 `[DONE] P4-T01` 完成记录并提交。

### 风险点

- 这是一个非常大的重构，可能会触及 6+ 个 codegen 文件。
- 改 `sysroot/core.scoop` 可能引发 layout / @Intrinsic 解析的 corner case；需要从最小变化开始（先一个类型 Bool 试点），逐个加上。
- 删除旧路径必须在新路径完整能跑之后；中间状态可能 fail。
- 若发现某条特判路径"用户类型也走过"的证据不存在（必须实现 4 的硬约束），则不能删除该路径，需要先补 fixture/owner-test 锁定用户类型路径再删。
- 若某个旧路径与新机制有 ABI surface 冲突（如 `@Intrinsic struct String : Hashable, ToString { ... }` 被 layout 内置识别破坏），那本任务可能需要拆出更小前置。

### 进展更新

- 改写 sysroot：`Bool/Char/Int/Float64/Float32/String` 改为 `@Intrinsic struct/class : Hashable, ToString { override fun toString(): String { ... } }`，删除冗余 `fun *.toString(): String` 顶层声明。
- 删除 typecheck 内 `Int/Bool/Char/Float*.toString` 的 synthetic 直接返回类型短路；`Int/Char/Float* hash/toInt/abs/...` 等其它 by-name 路径保留。
- 修改 HIR `should_keep_member_call_as_member_access`：从 `Int/Bool/Char/Float*` keep-list 中移除 `toString`。
- 引入新 helper `try_extract_member_call_receiver_fqn_and_args`，让 builtin scalar 也能进入 nominal member-call 主线。
- **遇到阻塞**：
  - **gap 1**：`true.toString()` 即使在 typecheck 通过后，HIR → MIR 阶段也没有把 receiver 作为第 0 个 arg 传递，触发 `pass MIR direct call arity mismatch`；本任务原定不动 HIR/MIR 的 receiver-prefix 逻辑，但实际证明 `@Intrinsic struct/class` body method 主线在 builtin scalar receiver 上还需要补齐 receiver-prefix。
  - **gap 2**：`println(<value>)` 在 sysroot 改写后报 `LLVM stage handoff 缺少 reachable callable scoop.core.ToString.toString 的 published late-lowered body`；说明 `ToString.toString` 在 builtin scalar override 引入后没有被 monomorphization / late lowering 正确发布 callable，与既有 `try_codegen_tostring_iface_builtin` 形成"双轨互相缺失"。
- 按 `PROMPT.md` "Missing or Incomplete Language Features" 规则，把这两条阻塞拆为新前置 `P4-T01l` ([TODO])，回退当前会话内的 sysroot / typecheck / HIR 改动，等 `P4-T01l` 完成后再启动 `P4-T01` 的删除动作。
- 已更新 `TODO.md`：顶部任务索引追加 `P4-T01l`；P4 前置 I 顺序约束行更新为十二个前置；`P4-T01l` 任务体说明 gap 1 / gap 2 与必须实现内容；`P4-T01` 依赖项更新为 `P4-T01l`。
- 验证当前回退状态：
  - `cargo build -p scoopc` 通过；
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`：400 fixtures 全过；
  - 工作树仅剩 `TODO.md` / `memory/claude_plan.md` 待提交。

### 完成状态

- 未完成：`P4-T01` 仍是 `[TODO]`；本轮提交只增加 `P4-T01l` 前置任务并回退 P4-T01 主任务的代码改动；
- 待提交：`TODO.md`、`memory/claude_plan.md`。
