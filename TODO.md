# TODO（Scoop：近期任务清单）

> 生成时间：2026-04-11  
> 说明：本文件是新的短版 TODO，只记录“接下来要做的新任务”。历史任务与已完成事项请看 `TODO-2.md` / `PLAN-2.md`。  
> 范围：本轮只覆盖 `ISSUES.md` 中确认仍存在的语言特性 / 编译器实现缺口，不把下一阶段的 stdlib 扩面混入主线。

## 约定

- 状态：
  - `[TODO]`：可立即实现与验收
  - `[BLOCKED]`：依赖未满足（例如缺文件/缺前置能力）
  - `[DONE]`：已完成（短版 TODO 一般不搬运历史 DONE）
- 每个任务包含：**描述 / 目标 / 验收 / 依赖**。
- 本轮优先级：
  - 主线：effect / continuation、语句语义 / 分号规则、`Task<T>`、lambda / 调用语义、泛型约束 / pattern / 值类型、`const fun` / MIR。
  - 末尾低优先级：annotation class、FFI / calling convention。

常用验收命令：

```bash
cargo test --all
cargo run -p scoop_tools -- spec-fixtures check
cargo run -p scoop -- test
```

LLVM 端到端（本机需 `clang` + `llvm-config`）：

```bash
cargo run -p scoop --features llvm -- test
```

---

## T20：Effect / Continuation 完整化

### T2001 [DONE] Effect：统一 `handle` arm 形态与 typecheck/HIR 不变量
- 描述：当前 `handle` 仍直接拒绝在同一个表达式里混用 `->`、`-> resume`、`, k ->` 等 arm 形态，导致语言语义被实现层的早期门禁截断。先收口 arm 的表示与兼容性检查，再推进后端链路。
- 目标：
  - typecheck 不再用“是否混用 arm 形态”作为直接拒绝条件，而是按 op 签名、resume 模式、binder 约束做真实兼容性检查。
  - HIR 为 handler arm 保留足够信息，能够区分 non-resuming / immediate-resume / continuation-binder 三类语义，而不是在 lowering 前折叠或拒绝。
  - 对不兼容组合给出稳定诊断，避免把语义错误延后成 LLVM/codegen 阶段崩溃。
- 验收：
  - 新增 typecheck / HIR fixtures：合法 mixed-arm、非法 mixed-arm、binder / resume 不匹配。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：无
- 完成说明：
  - 已移除 mixed-arm 的统一 early reject，并把 `handle` 结果类型改成按真实可返回路径确定。
  - 已补 mixed-arm typecheck/HIR fixtures，覆盖合法组合、返回类型不匹配、resume 语义冲突不可达。

### T2002a [DONE] Effect：non-resuming 单 payload ABI 泛化（direct + indirect perform）
- 描述：当前自定义 non-resuming effect 的 codegen 虽然已经能通过 flag-propagation + handler stack 跨函数分发，但 payload / handler binder 仍被硬编码为单个 word-sized `Int`。这使 `String`、引用类型以及含引用字段的聚合值在 non-resuming handler 上仍不可用。
- 目标：
  - 单 binder / 单 payload 的 non-resuming effect 不再要求 payload 为 `Int`；支持 scalar、`String` / ref，以及常见 aggregate payload。
  - direct perform 与通过函数/闭包触发的 indirect perform 共享同一套 payload encode/decode 语义，并具备正确的 GC rooting。
  - 保持既有 non-resuming dispatch 语义不回归：最近 handler 优先、arm body 在自身 handler scope 外执行、flag-propagation 仍可跨函数传播。
- 验收：
  - 新增 run-pass fixtures：non-resuming effect 传 `String` / `struct`，且至少一例经由函数或闭包的 indirect perform。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2001
- 完成说明：
  - runtime perform slot 已新增 `gc_ref` 通道，并在 slot 生命周期内负责 pin/unpin，避免 `String` / ref / boxed aggregate payload 成为 TLS roots hole。
  - LLVM codegen 已为 non-resuming perform / handler 引入共享 payload encode/decode helper，并让 `Continuation.resume` 复用同一套 ABI 规则。
  - 已新增 run-pass fixtures：`effect_nonresuming_payload_string_direct`、`effect_nonresuming_payload_struct_indirect`；`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2002b [DONE] Effect：escape continuation / CalleeSuspendState 恢复值 ABI 泛化
- 描述：`Continuation.resume` 本身已能传递 ref / compound payload，但 escape continuation 的间接 perform / call-site suspension 路径仍主要按 `resume_word` / 标量恢复值建模。top-level function 与 closure 的 CalleeSuspendState 还没有和 continuation 的双通道 payload 语义对齐。
- 目标：
  - top-level function / closure 的 CalleeSuspendState 不再只支持 `resume_word` 标量恢复值；间接 perform 的恢复值可覆盖 `String` / ref / aggregate。
  - 间接 perform + resume 的跨函数路径与 direct continuation step 共享同一套 payload encode/decode helper，而不是继续维护 `Int` 专用分支。
  - 对既有 `Continuation.resume(...)` lowering 做收口，确保 effect 路径和 continuation 路径的 payload 规则保持一致。
- 验收：
  - 新增 run-pass fixtures：间接 perform + `resume(String)`、间接 perform + `resume(struct)` 或等价 aggregate payload。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2002a
- 完成说明：
  - `CalleeSuspendState` 已统一为 `resume_word + resume_gc_ref + locals...` 形状，并让 GC trace 从 `resume_gc_ref` 起点覆盖恢复值与后续 GC locals。
  - top-level function / closure 的 callee-suspend resume path 已复用 `decode_abi_payload_transport`，恢复值不再局限于 `resume_word` 标量分支。
  - 间接 perform 的 escape continuation step 已把双通道 payload 写回 callee state，并对 `resume_gc_ref` 槽位走写屏障。
  - 已新增 run-pass fixtures：`effect_escape_continuation_indirect_perform_resume_string`、`effect_escape_continuation_indirect_perform_resume_struct_with_ref`。
  - `cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003a [DONE] Effect：immediate-resume 单-perform 路径的 `finally` cleanup 语义
- 描述：当前 immediate-resume 的 LLVM lowering 仍是“单个 `val x = perform` + 栈 state machine”的最小实现，并在 codegen 入口直接拒绝 `finally`。先在这个已经可运行的单 suspension 子集上补齐 cleanup 语义，再继续扩展控制流恢复。
- 目标：
  - 现有单个 `val x = perform` immediate-resume handle 可与 `finally` 组合。
  - `finally` 在正常 resume 完成、arm/body raise 向外传播时都恰好执行一次，不漏跑也不重复跑。
  - 不回归 `resume(value)` 的 one-shot 断言、handler inactive/active 切换与 handler scope 边界。
- 验收：
  - 新增 run-pass fixtures：immediate-resume + `finally` 的正常路径、raise 路径。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2002b
- 完成说明：
  - `codegen_handle_expr_immediate_resume` 已新增 `finally` / `finally_unwind` 收口：state0、arm、state1 内的 raise 统一先清理 handler frame，再执行 `finally` 并向外传播。
  - resumed computation 正常完成后会先退出 handler scope，再执行 `finally`，保持与既有 non-resuming / escape continuation 路径一致的 cleanup 顺序。
  - 已新增 run-pass fixtures：`effect_resume_finally_normal`、`effect_resume_finally_arm_raise`、`effect_resume_finally_body_raise_after_resume`。
  - `cargo test --all`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003b1 [DONE] Effect：immediate-resume 嵌套 block 中单 direct perform 的恢复
- 描述：`T2003b` 原始范围同时覆盖 block / branch / while 三类控制流，单轮改动和回归面过大。先收口最小但真实的“statement-position block 嵌套 + 单个 direct perform”路径，验证 immediate-resume 已不再局限于顶层 `val x = perform`。
- 目标：
  - immediate-resume 允许 `perform` 出现在嵌套 block 的语句列表中，而不再只接受 handle body 顶层局部绑定。
  - `resume(value)` 后可从该 block 内正确语句位置继续执行，并继续回到外层 handle body。
  - 对 if / while / value-position 嵌套 perform 先保留稳定诊断，不在本子任务里混入。
- 验收：
  - 新增 run-pass fixture：nested block 中 direct perform 的 immediate-resume。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003a
- 完成说明：
  - LLVM immediate-resume lowering 现已支持 statement-position nested block 中的单个 direct perform，不再只接受 handle body 顶层 `val x = perform`。
  - resume 后会先继续执行命中的 block tail，再回到外层 handle body；perform 前 block locals 的 slot 会跨 suspend/resume 复用。
  - 已新增 run-pass fixture：`effect_resume_nested_block_single_perform`；`cargo test --all`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003b2 [DONE] Effect：immediate-resume branch 内 direct perform 的恢复
- 描述：在 block 嵌套恢复打通后，再把 immediate-resume 扩到 `if` 分支中的 direct perform，补齐“分支命中 perform / 未命中 perform”两侧 CFG 与恢复后的合流。
- 目标：
  - immediate-resume 可覆盖 `if` then/else block 中的 direct perform。
  - resume 后能从命中的 branch 内正确位置继续执行，并在 branch 结束后回到外层后续语句。
  - 未命中 perform 的分支仍按普通控制流执行，不引入伪 suspension。
- 验收：
  - 新增 run-pass fixtures：then/else branch 中的 immediate-resume 组合。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003b1
- 完成说明：
  - immediate-resume 的路径扫描与 lowering 已从“仅 statement-position block”扩展到 statement-position `if` then/else branch，并继续保持“单 direct perform + 单次 resume”约束。
  - `state0` 现已区分“命中 perform 的分支”和“未命中 perform 的分支”：前者进入 arm/resume state machine，后者按普通分支控制流直接完成 handle，不再产生伪 suspension。
  - 已新增 run-pass fixtures：`effect_resume_if_then_branch_single_perform`、`effect_resume_if_else_branch_single_perform`。
  - `cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003b3 [DONE] Effect：immediate-resume while 内 direct perform 的恢复与诊断收口
- 描述：最后处理 loop 场景，把 direct perform 放进 `while` body，并对本阶段仍未覆盖的嵌套形状统一为稳定诊断。
- 目标：
  - immediate-resume 可覆盖 `while` body 中的 direct perform，并在 resume 后从循环体内正确位置继续执行。
  - loop locals / binder / one-shot resume 语义在多次迭代下保持稳定。
  - 对当前仍未支持的形状给出稳定诊断，而不是在 LLVM 阶段静默错编。
- 验收：
  - 新增 run-pass fixture：while body 中的 immediate-resume 组合。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003b2
- 完成说明：
  - immediate-resume 的 `resume_path` 与 lowering 现已支持 statement-position `while` body 中的单 direct perform；resume 后会先完成当前迭代尾部，再按原 `while` 条件决定是否进入下一次迭代。
  - 同一 `while` 内的 repeated direct perform 现会复用同一 binding slot / arm state machine，多次迭代下的 one-shot `resume(value)` 语义保持稳定。
  - 对 `while` condition 中的 perform 以及 `while` body 内更深层嵌套 perform，现会稳定报出 `unsupported_main_body`，不再依赖 LLVM 阶段的偶发失败。
  - 已新增 fixtures：run-pass `effect_resume_while_body_single_perform`、build-fail `effect_resume_while_nested_perform_is_error`。
  - `cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003c0a [DONE] Effect：LLVM 多 arm handle dispatch（immediate-resume + sibling non-resuming）
- 描述：审计 `T2003c` 时确认，LLVM `codegen_handle_expr` 仍在 `handle.arms.len() != 1` 时直接报 `handle arm count (only 1 supported)`。一次性把 immediate-resume、non-resuming、escape-continuation 三条 lowering 在同一 source-handle 下打通，风险过高；先落最小可运行子集：一个 immediate-resume arm + 若干 sibling non-resuming arms。
- 目标：
  - LLVM codegen 支持单个 `handle` 中的多 arm dispatch，不再把“一个 immediate-resume arm + sibling non-resuming arms”统一拒绝在 `UnsupportedMainBody`。
  - mixed-arm 下的 dispatch 与源码 arm 顺序保持一致；任一 arm body 执行期间，同一 source-handle 的 sibling arms 整组处于 handler scope 外，避免 sibling self-capture。
  - 覆盖 `Raise.raise` 与单 payload custom non-resuming effect 两类 sibling non-resuming arm；对 sibling escape-continuation arm 暂时给出稳定诊断。
- 验收：
  - 新增 run-pass / build-fail fixtures：覆盖 mixed-arm immediate-resume + non-resuming 的最小可运行路径，以及 mixed-arm immediate-resume + escape-continuation 的稳定诊断。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003b3
- 完成说明：
  - `codegen_handle_expr` 现已在多 arm 场景下分流到新的 mixed-arm lowering，不再统一报 `handle arm count (only 1 supported)`。
  - LLVM mixed-arm lowering 现已支持“一个 immediate-resume arm + sibling non-resuming arms”的最小子集：`Raise.raise` 与单 payload custom non-resuming effect 都能在同一个 source-handle 内参与 dispatch。
  - immediate-resume arm body 与 sibling non-resuming arm body 执行期间，会把同一 source-handle 的 custom sibling handler frames 从 TLS handler stack 中摘除，避免 sibling self-capture；resume 后再恢复 body 阶段所需的 dispatch scope。
  - 已新增 fixtures：run-pass `effect_resume_mixed_custom_nonresuming_dispatch`、`effect_resume_mixed_raise_dispatch`，build-fail `effect_resume_mixed_escape_is_error`。
  - `cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003c0b1 [DONE] Effect：LLVM 多 arm handle dispatch（sibling escape-continuation，single direct site）
- 描述：`T2003c0b` 原始范围同时覆盖 direct/indirect perform、多 perform 点以及更复杂 mixed 组合，单轮风险过高。先收口“一个 immediate-resume arm + 一个 sibling escape-continuation arm”的最小可运行直连路径。
- 目标：
  - mixed-arm immediate-resume + 单个 sibling escape-continuation arm 可在 LLVM 端到端路径运行，至少覆盖 direct perform、单 perform 点、top-level statement-position。
  - escape-continuation arm 的 suspension / captured handler stack / sibling self-capture 语义与既有单-arm 子集保持一致。
  - 对 indirect perform、多 perform 点以及本阶段仍不支持的复杂 mixed 组合给出稳定诊断，不再回退到通用的 arm-count 门禁。
- 验收：
  - 新增 run-pass / build-fail fixtures：覆盖 mixed-arm immediate-resume + sibling escape-continuation 的最小可运行路径，以及至少一个剩余明确不支持的组合。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0a
- 完成说明：
  - LLVM mixed-arm lowering 现已支持“一个 immediate-resume arm + 一个 sibling escape-continuation arm”的最小 direct single-site 子集；当前要求 immediate site 与 escape site 都是 top-level `val = perform` 形态。
  - sibling escape-continuation 的 continuation step 现会恢复 pre-escape 的 outer/body captures，并在 `resume(...)` 后继续执行 escape perform 之后的 top-level tail。
  - immediate arm body 与 escape arm body 执行期间，sibling escape handler frame 都会脱离当前 TLS handler stack，避免同源 self-capture；对 multiple direct perform points 等 richer mixed 组合已改为稳定诊断。
  - 已新增 fixtures：run-pass `effect_resume_mixed_escape_direct`；build-fail `effect_resume_mixed_escape_is_error` 已改为覆盖“multiple direct perform points not yet supported”。
  - `cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003c0b2a [DONE] Effect：LLVM 多 arm handle dispatch（sibling escape-continuation，single indirect site）
- 描述：`T2003c0b2` 原始范围同时覆盖 indirect perform 与 richer mixed 组合，单轮风险仍偏大。先收口“一个 immediate-resume arm + 一个 sibling escape-continuation arm + 一个 top-level indirect call site”的最小可运行子集。
- 目标：
  - sibling escape-continuation 不再局限于 direct single-site；至少支持 top-level `val x = f(...)` 形态的单个 indirect perform call site。
  - 复用既有单-arm indirect continuation 的 callee-suspend / captured-handler-stack 语义，并保持 sibling self-capture 语义稳定。
  - 对 multiple indirect sites、direct+indirect 多 site，以及本阶段仍不支持的 richer mixed 组合给出稳定诊断。
- 验收：
  - 新增 run-pass / build-fail fixtures：覆盖 mixed-arm immediate-resume + sibling escape-continuation 的单 indirect site 路径，以及至少一个剩余明确不支持的组合。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0b1
- 完成说明：
  - mixed-arm 入口现已区分 sibling escape-continuation 的 direct / indirect 子集：direct 继续走 `T2003c0b1` lowering，single indirect site 走新的 dedicated lowering。
  - LLVM mixed-arm lowering 现已支持“一个 immediate-resume arm + 一个 sibling escape-continuation arm + 一个 top-level indirect call site”的最小子集；continuation step 会写回 callee suspend state 的双通道 resume payload，并在 `resume(...)` 后重新调用 callee 再继续执行 source-handle 的 top-level tail。
  - 已对 richer mixed 组合补稳定诊断：`direct + indirect sites not yet supported`、`multiple indirect call sites not yet supported`、`indirect perform before immediate site not yet supported`。
  - 已新增 fixtures：run-pass `effect_resume_mixed_escape_indirect`，build-fail `effect_resume_mixed_escape_direct_indirect_is_error`。
  - `cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003c0b2b0 [DONE] Effect：LLVM immediate-resume tail 中嵌套 `handle` 结果表达式
- 描述：为实现 `T2003c0b2b1`，尝试把 mixed-arm body 重写成“outer immediate-resume handle + inner single-arm escape-continuation handle tail”。但手写等价程序当前也会在 LLVM codegen 报 `value coercion`，说明“immediate-resume tail 中把嵌套 `handle` 作为结果表达式继续 lowering”本身还是缺口，需要先补。
- 目标：
  - `codegen_handle_expr_immediate_resume` 的 resumed tail / final value 路径可接受嵌套 `handle` 结果表达式，不再在手写等价程序上报 `value coercion`。
  - immediate-resume + nested escape-handle 的结果值、`finally` cleanup 与 raise 传播语义保持稳定。
  - 为后续 `T2003c0b2b1` 提供可复用的 lowering primitive，而不是在 mixed-arm 专用路径里重复实现一套多 suspension state machine。
- 验收：
  - 新增 run-pass 或等价回归：手写“outer immediate-resume + inner escape handle tail”最小样例。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0b2a
- 完成说明：
  - `codegen_immediate_resume_top_level_tail_and_finalize` 现已为 tail 中的表达式语句显式传递 expected type：最后一个表达式使用 `Some(out_ty)`，非最后表达式使用 `Some(Unit)`，避免嵌套 `handle` 在 LLVM codegen 中丢失结果类型上下文。
  - 已新增 run-pass 回归：`effect_resume_nested_escape_handle_tail`，覆盖“outer immediate-resume + inner single-arm escape handle tail”的手写等价程序。
  - `cargo test --all`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003c0b2b0c [DONE] Effect：LLVM single-arm escape-continuation 多 direct site 的非 `Unit` 结果 lowering
- 描述：尝试实现 `T2003c0b2b1` 时发现，计划复用的“inner single-arm escape handle tail”本身还缺一个前置能力：single-arm escape-continuation 在 top-level multiple direct sites 且 `handle` 结果不是 `Unit` 时，最小样例仍会在 LLVM codegen 报 `unknown local value` / `value coercion`。在 mixed-arm 路径继续下沉 tail 之前，必须先把这个 primitive 打通。
- 目标：
  - single-arm escape-continuation 在“多个 top-level direct perform sites + 非 `Unit` handle 结果”场景下可稳定通过 LLVM codegen，不再在最小样例上报 `unknown local value` / `value coercion`。
  - 第一次 suspend 返回 arm result 给 caller、后续 `resume(...)` 继续推进剩余 direct sites 的既有 multi-perform 语义保持不回归。
  - 为 `T2003c0b2b1` 提供可直接复用的 inner tail primitive，而不是在 mixed-arm 专用路径里另写一套结果值协议。
- 验收：
  - 新增 run-pass fixture：single-arm escape-continuation 的 multiple direct sites + 非 `Unit` 结果最小样例。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0b2b0
- 完成说明：
  - single-arm escape-continuation 的 multi-perform step trampoline 现已把 pointer-like enum outer capture 视为 `gc_ref` 存储类别；outer/body capture 的筛选、state field 形状、zero-init、restore 与 write-back 路径已统一复用该协议。
  - 修复了嵌套 tail 里的 inner escape handle 在第二次 direct perform 从 step trampoline 再次进入 arm 时，因 `String?` / `Continuation<T>?` 这类 pointer-like enum 外层局部未恢复进 `cg.env` 而触发的 `unknown local value`。
  - 已新增 run-pass 回归：`effect_resume_nested_escape_handle_tail_multi_perform_nonunit`，覆盖“outer immediate-resume tail + inner single-arm escape handle + multiple direct sites + non-`Unit` 结果 + pointer-like enum capture”。
  - `cargo test --all`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003c0b2b1 [DONE] Effect：LLVM 多 arm handle dispatch（sibling escape-continuation，post-immediate multiple direct sites）
- 描述：`T2003c0b2b` 原始范围同时要求处理 `multiple direct perform points`、`multiple indirect call sites`、`direct + indirect sites` 与 `perform before immediate site`。其中“escape site 出现在 immediate site 之后，且全部为 top-level direct perform”计划通过把 tail 下沉为 single-arm escape-continuation handle 复用既有 multi-perform lowering 来落地；但这一步还依赖 `T2003c0b2b0c` 先补齐 single-arm 多 direct site 的非 `Unit` 结果值 primitive。
- 目标：
  - 在“一个 immediate-resume arm + 一个 sibling escape-continuation arm”的前提下，支持 immediate site 之后的多个 top-level direct escape sites。
  - 多个 direct escape sites 在每次 `resume(...)` 后都能继续命中后续 sibling escape arm，不再停留在 `multiple direct perform points not yet supported` 诊断。
  - sibling escape arm 的 self-capture、captured locals 与 `finally` cleanup 语义保持与既有 single-arm multi-perform escape-continuation 一致。
- 验收：
  - 新增 run-pass fixture：multiple direct sites（all post-immediate）。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0b2b0c
- 完成说明：
  - mixed-arm direct sibling escape lowering 现已支持 immediate site 之后的多个 top-level direct escape sites，不再在该子集上回退到 `multiple direct perform points not yet supported`。
  - mixed escape state 现已新增 `pc` 字段；step trampoline 会按 direct site 序号恢复当前 perform 结果、继续执行 top-level tail，并在命中后续 sibling escape site 时再次分配 continuation。
  - direct mixed-arm 的 outer/body capture 现已统一复用 `EscapeCaptureStorageKind` 的 `word / gc_ref` 存储协议，因此 step trampoline 中的 arm/body 也能恢复 pointer-like enum 外层捕获。
  - 已移除旧的 build-fail 回归 `effect_resume_mixed_escape_is_error`，并新增 run-pass 回归 `effect_resume_mixed_escape_direct_multi`，覆盖 post-immediate 两个 direct site、第一次 escape 恢复值跨第二次 suspension 的 body-lift，以及 arm 内 pointer-like enum outer capture。
  - `cargo test --all`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003c0b2b2 [DONE] Effect：LLVM 多 arm handle dispatch（sibling escape-continuation，post-immediate indirect/direct+indirect site matrix）
- 描述：在 post-immediate multiple direct sites 打通后，继续补齐“一个 immediate-resume arm + 一个 sibling escape-continuation arm”在 immediate site 之后的 remaining top-level site matrix：multiple indirect call sites，以及 direct + indirect 共存。
- 目标：
  - 支持 immediate site 之后的多个 indirect escape sites。
  - 支持 immediate site 之后 direct + indirect escape sites 共存，不再依赖 `direct + indirect sites not yet supported` / `multiple indirect call sites not yet supported` 诊断。
  - direct/indirect 混合 site 在 captured-handler-stack、one-shot continuation 与 tail replay 上保持稳定一致。
- 验收：
  - 新增 run-pass fixtures：multiple indirect sites、direct+indirect 共存（all post-immediate）。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0b2b1
- 完成说明：
  - mixed-arm escape sibling lowering 现已新增统一的 post-immediate site matrix 路径，把 top-level direct / indirect escape sites 接到同一条 continuation step trampoline 上。
  - 该路径现已支持：multiple indirect sites、direct→indirect、indirect→direct；并继续保留 pre-immediate site 的稳定诊断。
  - 已新增 run-pass 回归：`effect_resume_mixed_escape_indirect_multi`、`effect_resume_mixed_escape_direct_indirect`、`effect_resume_mixed_escape_indirect_direct`；同时把旧的 post-immediate 负例替换为 `effect_resume_mixed_escape_pre_immediate_direct_indirect_is_error`。
  - `cargo test --all`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003c0b2b3 [DONE] Effect：LLVM 多 arm handle dispatch（sibling escape-continuation，pre-immediate top-level sites）
- 描述：`perform before immediate site` 不是单纯的 site 扫描问题，而是 continuation step 在恢复后仍需重新命中 sibling immediate-resume state machine 的控制流缺口。把它单独拆出来，避免和 post-immediate site matrix 耦合。
- 目标：
  - 支持 escape site 位于 immediate site 之前的 top-level direct/indirect 组合。
  - continuation step 在恢复 pre-immediate escape site 之后，仍可正确进入后续 immediate-resume site，并保持 sibling handler-scope / one-shot 语义。
  - `perform before immediate site not yet supported` / `indirect perform before immediate site not yet supported` 仅在更深层 body 形状中保留。
- 验收：
  - 新增 run-pass fixture：escape-before-immediate（top-level direct/indirect 至少各一例或等价覆盖）。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0b2b2
- 完成说明：
  - mixed-arm site-matrix lowering 现已支持 pre-immediate top-level direct / indirect escape sites，不再把这类 top-level 组合回退到 `perform before immediate site not yet supported` / `indirect perform before immediate site not yet supported`。
  - continuation step trampoline 现可在恢复 pre-immediate escape site 之后重新命中 sibling immediate-resume site，并在 immediate arm `resume(...)` 后继续 replay 后续 top-level tail 与 post-immediate escape sites。
  - 已新增 run-pass 回归：`effect_resume_mixed_escape_pre_immediate_direct`、`effect_resume_mixed_escape_pre_immediate_indirect`；旧的 top-level pre-immediate build 负例已替换为 nested-shape 负例 `effect_resume_mixed_escape_pre_immediate_nested_is_error`。
  - `cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --check` 通过。

### T2003c0b2c1a [DONE] Effect：LLVM 多 arm handle dispatch（sibling escape-continuation，nested block 的 direct site）
- 描述：在 mixed-arm site matrix 仍是 top-level-only 的前提下，最小可安全扩展的 nested direct 子集是 statement-position `block`。它只要求顺序前缀/尾部 replay，不涉及 if 的双分支合流或 while 的迭代重入。
- 目标：
  - sibling escape-continuation 支持 nested block 中的 direct perform site，不再要求 direct site 必须是 top-level `val = perform`。
  - direct site 位于 immediate site 之前或之后时，resume 后都能先完成 block tail，再继续进入后续 top-level immediate-resume / sibling escape dispatch。
  - 对 if branch、while body 与 nested indirect call site 继续保留稳定诊断。
- 验收：
  - 新增 run-pass fixtures：覆盖 nested block 中的 mixed-arm escape direct 组合（至少一例 pre-immediate、一例 post-immediate 或等价覆盖）。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0b2b3
- 完成说明：
  - mixed-arm escape sibling lowering 现已支持 statement-position nested block 中的 direct perform site，覆盖 immediate site 之前与之后两侧的 replay。
  - nested block 中 perform 前声明且在 resume 后继续使用的 local 现已纳入 body capture/lift 分析，并在 continuation step 中恢复。
  - 在 `T2003c0b2c1a` 完成时，if branch / while body / nested indirect 仍维持稳定诊断；其中 if branch 子集已由后续 `T2003c0b2c1b` 承接。
  - 已新增 run-pass 回归：`effect_resume_mixed_escape_pre_immediate_block`、`effect_resume_mixed_escape_post_immediate_block`。
  - `cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --all`、`cargo run -p scoop --features llvm -- test` 通过。

### T2003c0b2c1b [DONE] Effect：LLVM 多 arm handle dispatch（sibling escape-continuation，if branch 的 direct site）
- 描述：在 nested block direct site 打通后，继续扩展到 if then/else branch。该子集需要处理双分支拦截、未命中分支顺序执行与 branch 后合流，复杂度高于纯 block，因此单列。
- 目标：
  - sibling escape-continuation 支持 if then/else branch 中的 direct perform site，不再要求 direct site 必须是 top-level `val = perform`。
  - direct site 位于 immediate site 之前或之后时，resume 后都能回到命中分支 tail，并在 if 结束后继续执行后续 top-level body。
  - 对 while body 与 nested indirect call site 继续保留稳定诊断。
- 验收：
  - 新增 run-pass fixtures：覆盖 if then/else branch 中的 mixed-arm escape direct 组合。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0b2c1a
- 完成说明：
  - mixed-arm site-matrix lowering 现已支持 statement-position `if` then/else branch 中的 direct sibling escape site：命中分支会在 direct perform 处创建 continuation，未命中分支按普通顺序执行，并在 `if` 结束后回到统一的 top-level tail。
  - step trampoline、pre-immediate `state0` 与 post-immediate `state1` 现已共享新的 if-branch dispatch helper，可在 then/else 两个候选 site 之间按运行时条件选择命中的分支，并保持 branch tail replay 与 after-if merge 的控制流一致。
  - nested body decl / used-after / capture 分析现已覆盖 if branch；旧的 if 边界 build 负例 `effect_resume_mixed_escape_pre_immediate_nested_is_error` 已替换为 while 负例 `effect_resume_mixed_escape_while_is_error`，继续锁住 `T2003c0b2c2` 之前仍不支持的 while body 子集。
  - 已新增 run-pass 回归：`effect_resume_mixed_escape_pre_immediate_if`、`effect_resume_mixed_escape_post_immediate_if`。
  - `cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test` 通过。

### T2003c0b2c2a [DONE] Effect：LLVM 多 arm handle dispatch（sibling escape-continuation，while body 的 flat direct site）
- 描述：审计 `T2003c0b2c2` 后确认，“while body 的 direct site”至少跨了两个难度层级：一类是 direct site 直接位于 while body 的 statement 序列中（flat site），另一类是 site 继续嵌在 while 内的 block / if 中。二者在 replay / re-entry 上都需要 while 重入，但后者还要再叠一层 nested path 分派。先收口 flat site，避免把两个状态机问题耦合。
- 目标：
  - sibling escape-continuation 支持 while body 中直接位于 statement 序列的 direct perform site，不再要求 top-level val-bound site。
  - resume 后能正确完成当前迭代尾部、重新检查 loop condition，并在后续迭代中保持 sibling handler-scope / one-shot continuation 语义稳定。
  - 对 while 内继续嵌套 block / if 的 direct site，以及 nested indirect call site 继续保留稳定诊断。
- 验收：
  - 新增 run-pass fixtures：覆盖 while body flat direct site 的 mixed-arm escape direct 组合。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0b2c1b
- 完成说明：
  - mixed-arm escape direct-site 扫描、body decl / used-after / capture 分析现已扩展到 flat while body，并显式把 while 内 nested direct site 留给后续 `T2003c0b2c2b`。
  - matrix lowering 的 `state0`、`state1` 与 continuation step trampoline 现已新增 while-body helper：`resume(...)` 后会先完成当前迭代尾部、重新检查 loop condition，并在后续迭代中再次命中同一个 sibling escape site。
  - 已新增 run-pass 回归：`effect_resume_mixed_escape_pre_immediate_while`、`effect_resume_mixed_escape_post_immediate_while`；旧的 while 负例已改为 nested direct 负例 `effect_resume_mixed_escape_while_is_error`，锁住 `T2003c0b2c2b` 之前仍不支持的 while nested path。
  - `cargo fmt --all --check`、`cargo test --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo run -p scoop --features llvm -- test` 通过。

### T2003c0b2c2b [DONE] Effect：LLVM 多 arm handle dispatch（sibling escape-continuation，while body 的 nested direct site）
- 描述：在 flat while-body direct site 打通后，继续扩展到 while 内再嵌 block / if 的 direct site。该子集除了 while 重入外，还需要让 continuation step 在恢复后重新回到命中的 nested path，并在 loop re-entry 中继续参与分派。
- 目标：
  - sibling escape-continuation 支持 while body 中嵌套 block / if 的 direct perform site。
  - continuation step 在恢复后能先 replay 命中 nested path 的尾部，再继续当前迭代剩余语句、loop condition 与后续迭代。
  - 对 nested indirect call site 继续保留稳定诊断。
- 验收：
  - 新增 run-pass / build-fail fixtures：覆盖 while body nested direct site 的 mixed-arm 组合，以及至少一个本阶段仍不支持的更深层形状。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0b2c2a
- 完成说明：
  - mixed-arm while direct-site 扫描现已接受最小 nested path 子集：`while -> block -> block*` 与 `while -> if-branch -> block*`；更深层 `while -> block -> if` / nested while 仍保留稳定诊断 `deeper nested direct site in while body not yet supported`。
  - while-body 的 intercept 与 tail replay 现已复用新的 nested helper：首次命中 direct site 时可先走 while-body 前缀，再进入 nested block / if path；`resume(...)` 后会先 replay 命中的 nested path 尾部，再执行当前迭代剩余语句、loop condition 与后续迭代。
  - 已新增 run-pass 回归：`effect_resume_mixed_escape_pre_immediate_while_nested_block`、`effect_resume_mixed_escape_post_immediate_while_nested_if`；已有 build-fail `effect_resume_mixed_escape_while_is_error` 已更新为锁定更深层 nested while 形状的稳定诊断。
  - `cargo fmt --all`、`cargo test --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test` 通过。

### T2003c0b2c3a [DONE] Effect：LLVM 多 arm handle dispatch（sibling escape-continuation，nested block 的 indirect site）
- 描述：继续审计 `T2003c0b2c3` 后确认，nested indirect 至少跨三类 CFG 形状：statement-position nested block、if/branch、while body。其中 nested block 主要是顺序前缀 / 尾部 replay，适合作为第一步先落地。
- 目标：
  - sibling escape-continuation 支持 statement-position nested block 中的 indirect call site，覆盖 pre-immediate / post-immediate 两侧。
  - continuation step 在 `resume(...)` 后可重新进入命中的 nested block，重放 call-site 之后的 block tail，再继续后续 top-level tail。
  - if / while 中的 nested indirect 继续保留稳定诊断。
- 验收：
  - 新增 run-pass / build fixtures：覆盖 nested block indirect 的 mixed-arm 组合，以及至少一个仍未支持的 if / while nested indirect 形状。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0b2c2b
- 完成说明：
  - mixed-arm escape `site matrix` 现已为 indirect site 保留 `resume_path`，并新增 nested-block-only 的 scanner / prefix / tail helper；top-level `val = call(...)` 与 statement-position nested block 中的 `val = call(...)` 现在共享同一套 state0 / state1 / continuation-step lowering。
  - continuation step 在恢复 nested block indirect site 时，现会先 replay block 前缀、重新调用 callee、再继续 block tail 与后续 top-level tail；pre-immediate / post-immediate 两侧都已接入同一条路径。
  - if branch / while body 的 nested indirect 当时已统一改成稳定诊断，不再被旧的 top-level-only 扫描器静默漏掉；后续在 `T2003c0b2c3b` 已把 if branch 子集打通，当前由 build 负例 `effect_resume_mixed_escape_while_indirect_is_error` 继续锁住 while 边界。
  - 已新增 run-pass 回归：`effect_resume_mixed_escape_pre_immediate_block_indirect`、`effect_resume_mixed_escape_post_immediate_block_indirect`。
  - `cargo fmt --all`、`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003c0b2c3b [DONE] Effect：LLVM 多 arm handle dispatch（sibling escape-continuation，if branch 的 indirect site）
- 描述：在 nested block indirect 打通后，继续扩到 if then/else branch。该子集除了 call-site suspension 外，还要处理分支命中、resume 后 branch tail replay，以及 after-if CFG 合流。
- 目标：
  - sibling escape-continuation 支持 if then/else branch 中的 indirect call site。
  - continuation step 在恢复后会先完成命中的 branch tail，再统一继续 after-if top-level tail。
  - while body 的 nested indirect 继续保留稳定诊断。
- 验收：
  - 新增 run-pass / build fixtures：覆盖 if branch indirect 的 mixed-arm 组合，以及至少一个仍未支持的 while nested indirect 形状。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0b2c3a
- 完成说明：
  - `scan_mixed_escape_indirect_sites` 现已允许 if then/else branch 中的 statement-position val-bound indirect call site，并为 if path 保留 `resume_path`；同分支多 site 仍会给出稳定诊断。
  - mixed-arm escape `site matrix` 现已新增 if-indirect 分类与 lowering：state0、state1 与 continuation step 都会共享新的 if-branch prefix / tail helper，在恢复后先 replay 命中的 branch tail，再统一继续 after-if top-level tail。
  - 已新增 run-pass 回归：`effect_resume_mixed_escape_pre_immediate_if_indirect`、`effect_resume_mixed_escape_post_immediate_if_indirect`；旧的 build 负例 `effect_resume_mixed_escape_if_indirect_is_error` 已替换为 while 边界负例 `effect_resume_mixed_escape_while_indirect_is_error`。
  - `cargo fmt --all`、`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003c0b2c3c [DONE] Effect：LLVM 多 arm handle dispatch（sibling escape-continuation，while body 的 indirect site）
- 描述：while body indirect 需要把 nested call-site suspension 与 loop re-entry 结合起来：resume 后既要继续当前迭代尾部，又要重新检查 condition，并允许后续迭代再次命中同一 sibling indirect site。
- 目标：
  - sibling escape-continuation 支持 while body 中的 flat / nested indirect call site。
  - continuation step 在恢复后能先 replay 当前迭代中命中的 nested path 尾部，再执行剩余 body、loop condition 与后续迭代。
  - 更深层 nested while 或本阶段之外的 richer loop 形状继续保留稳定诊断。
- 验收：
  - 新增 run-pass / build fixtures：覆盖 while body indirect 的 mixed-arm 组合，以及至少一个更深层 loop 形状的稳定诊断。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0b2c3b
- 完成说明：
  - `scan_mixed_escape_indirect_sites` 现已允许 while body 中的 flat / nested indirect site，并对 deeper nested while 保持稳定 `unsupported_main_body` 诊断。
  - mixed-arm escape `site matrix` 已新增 while-indirect 分类与 lowering：state0、state1 与 continuation step 现会共享 while-body indirect 的 prefix / current-tail / loop re-entry helper，后续迭代可再次命中同一 sibling indirect site。
  - 已新增 run-pass 回归：`effect_resume_mixed_escape_pre_immediate_while_indirect`、`effect_resume_mixed_escape_post_immediate_while_nested_if_indirect`；既有 build 负例 `effect_resume_mixed_escape_while_indirect_is_error` 已更新为更深层 nested while 诊断。
  - `cargo fmt --all`、`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003c0b2c3d1 [DONE] Effect：LLVM 多 arm handle dispatch（sibling escape-continuation，nested block 的 direct / indirect 共存）
- 描述：审计 `T2003c0b2c3d` 时确认，当前 mixed-arm matrix 在“同一个 top-level nested block 语句里同时出现 nested direct / indirect site”时，会先被 `multiple sites per top-level statement not yet supported` 挡住；且 step/state replay 也缺少“从当前 nested block site 继续走到同 stmt 的下一个 site”的专用路径。先收口最小可运行子集：同一个 nested block 语句中的 single direct + single indirect。
- 目标：
  - mixed-arm immediate-resume + sibling escape-continuation 支持同一个 top-level nested block 语句里的 single direct + single indirect site，且两者可按源码顺序共存。
  - 无论先命中 direct 还是 indirect，resume 后都能继续在同一个 nested block 语句内命中后续 site，再回到 top-level tail。
  - richer nested block 组合（例如同 stmt 超过两个 site 或同类 site 重复）继续保留稳定诊断，避免过早放宽到未实现形状。
- 验收：
  - 新增 run-pass fixtures：覆盖 nested block 中 single direct + single indirect 的 pre-immediate / post-immediate 组合。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0b2c3c
- 完成说明：
  - mixed-arm `site matrix` 现已允许同一个 top-level nested block 语句里的 single direct + single indirect site 共存，并继续对“同 stmt 超过两个 site”或“同类 site 重复”保持稳定诊断。
  - state0、state1 与 continuation step 已共享新的 block-pair replay helper：既能从当前 nested block site 续跑到同 stmt 的后续 sibling site，也能在 direct-first / indirect-second 两种顺序下分别处理 next-site / prev-site 语义。
  - 针对 direct-first / indirect-second 形状，body-lift 分析现会额外纳入“前一个 direct 之后到当前 indirect 之前”的 replay 前缀依赖 locals，避免第二次恢复时遗漏 `direct` 等 block-local。
  - 已新增 run-pass 回归：`effect_resume_mixed_escape_pre_immediate_block_indirect_direct`、`effect_resume_mixed_escape_post_immediate_block_direct_indirect`。
  - `cargo fmt --all`、`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003c0b2c3d2 [DONE] Effect：LLVM 多 arm handle dispatch（sibling escape-continuation，if branch 的 direct / indirect 共存）
- 描述：在 nested block 的同 stmt mixed site 续跑语义打通后，再把 direct / indirect 共存扩到同一个 `if` 语句的 then/else branch，补齐 branch 条件、命中分支 tail 与 after-if 合流下的 mixed replay。
- 目标：
  - 同一个 `if` 语句内的 nested direct / indirect site 可共存，不再被 `multiple sites per top-level statement not yet supported` 提前拒绝。
  - resume 后会先继续命中的 branch 内剩余路径，再在必要时命中 sibling mixed site，最后统一回到 after-if tail。
  - richer branch 组合若仍未覆盖，继续保留稳定诊断。
- 验收：
  - 新增 run-pass fixtures：覆盖 if branch 中 direct + indirect 共存的 mixed-arm 组合。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0b2c3d1
- 完成说明：
  - mixed-arm `site matrix` 现已支持同一个 `if` 语句里的 direct / indirect mixed site：分类阶段会为 then/else branch 建立独立 mixed route，并在同分支 direct↔indirect 间维护 next/prev replay 关系，不再统一回退到 `multiple sites per top-level statement not yet supported`。
  - state0、state1 与 resumed main tail 现已共享新的 if-branch mixed helper；current-site 恢复后可先 replay 命中 branch 的 tail，再在需要时继续命中同分支 sibling mixed site，并在最终完成后统一回到 after-if tail。
  - direct-first / indirect-second 路径已补上 if-branch 的 used-between body-lift 与 branch-scope re-entry，修复 post-immediate 续跑时的 lifted local 缺口（例如 `x` / `label` / `direct`）。
  - 已新增 run-pass 回归：`effect_resume_mixed_escape_pre_immediate_if_indirect_direct`、`effect_resume_mixed_escape_post_immediate_if_direct_indirect`。
  - `cargo fmt --all`、`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003c0b2c3d3 [DONE] Effect：LLVM 多 arm handle dispatch（sibling escape-continuation，while body 的 direct / indirect 共存）
- 描述：最后把同 stmt mixed site replay 扩到 `while` body，让 direct / indirect 共存在 loop re-entry、当前迭代 tail 与后续迭代再命中的语义下保持一致。
- 目标：
  - 同一个 `while` 语句内的 nested direct / indirect site 可共存，resume 后既能继续当前迭代剩余路径，也能在需要时命中同 stmt 的后续 mixed site。
  - loop condition、body tail、后续迭代与 callee suspend state / resume payload 语义保持稳定。
  - 更深层 while mixed 组合若仍未覆盖，继续保留稳定诊断。
- 验收：
  - 新增 run-pass / build fixtures：覆盖 while body 中 direct + indirect 共存的 mixed-arm 组合，以及至少一个剩余未实现边界。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0b2c3d2
- 完成说明：
  - mixed-arm `site matrix` 现已支持同一个 `while` 语句里的 direct / indirect mixed site：分类阶段会为同 stmt 的 while mixed route 建立 `next/prev` 关系，并对“不同 while-body stmt 共存”保留稳定诊断 `only same-body-stmt direct / indirect coexistence in while body supported`。
  - state0、state1 与 continuation step 现已共享新的 while mixed helper；恢复后既能继续当前迭代尾部，也能在需要时命中同 stmt 的 sibling mixed site，并在 direct→indirect 的场景下于后续迭代重新从 sibling direct site 进入，而不会把它错误地当成普通 replay 语句。
  - 已新增 run-pass / build 回归：`effect_resume_mixed_escape_pre_immediate_while_indirect_direct`、`effect_resume_mixed_escape_post_immediate_while_direct_indirect`、`effect_resume_mixed_escape_while_direct_indirect_separate_stmt_is_error`。
  - `cargo fmt --all`、`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003c0c1a [DONE] Effect：LLVM 多 arm handle dispatch（escape-continuation + sibling non-resuming，top-level direct single-site）
- 描述：审计 `T2003c0c1` 后确认，当前 escape + sibling non-resuming 组合横跨 direct-site、single indirect-site 与 site-matrix 三条独立 lowering；若整包推进，会把 dispatch、continuation step 与 nested replay 三层改动耦合。先收口最小可运行子集：top-level direct single-site 的 sibling escape + sibling non-resuming。
- 目标：
  - immediate-resume arm + sibling escape-continuation arm + 若干 sibling non-resuming arms 可在 top-level direct single-site 组合下共存。
  - 主 body / resumed main path 中，`Raise.raise` 与 custom non-resuming effects 能参与同一 source-handle 的 dispatch。
  - immediate arm body、escape arm body 与 direct continuation step 执行期间，会把同源 sibling non-resuming handler frames 摘出当前 handler scope，避免 sibling self-capture。
- 验收：
  - 新增 run-pass fixtures：top-level direct single-site 的 `immediate-resume + escape + raise`、`immediate-resume + escape + custom non-resuming`。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0b2c3d3
- 完成说明：
  - `codegen_handle_expr_multi_arm` 不再把 `escape-continuation + sibling non-resuming` 一刀切拒绝；当前已新增 `T2003c0c1a` 的 direct single-site 分流，只对 single indirect / richer site-matrix 保留稳定诊断，交给后续 `T2003c0c1b` / `T2003c0c1c`。
  - top-level single direct escape site 的 mixed-arm lowering 现已支持 sibling `Raise.raise` 与 custom non-resuming：主 body、resumed main path 与单-site continuation step 都已接入 op-tag dispatch / catch blocks。
  - immediate arm body、escape arm body，以及 sibling raise/custom catch body 现在都会把同源 sibling non-resuming 路由到 `finally_unwind` 或 step cleanup 路径，避免 sibling self-capture。
  - 已新增 run-pass 回归：`effect_resume_mixed_escape_raise_direct_single_site`、`effect_resume_mixed_escape_custom_nonresuming_direct_single_site`。
  - `cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003c0c1b [DONE] Effect：LLVM 多 arm handle dispatch（escape-continuation + sibling non-resuming，single indirect site）
- 描述：在 direct single-site 子集打通后，再把 sibling non-resuming 接到 single indirect-site 的 escape lowering。该路径需要同时处理 callee suspend state、dispatch no-match 与 continuation step 恢复后的 handler-scope。
- 目标：
  - immediate-resume arm + sibling escape-continuation arm + 若干 sibling non-resuming arms 可覆盖单个 top-level indirect call site。
  - indirect call-site suspension、resume payload 写回与 no-match dispatch 期间，sibling non-resuming 的 dispatch / detach / restore 语义保持稳定。
  - immediate arm body、escape arm body 与 indirect continuation step 期间继续避免 sibling self-capture。
- 验收：
  - 新增 run-pass fixtures：single indirect-site 的 `immediate-resume + escape + raise` 或等价 custom non-resuming 组合。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0c1a
- 完成说明：
  - `codegen_handle_expr_immediate_resume_with_escape_sibling_indirect` 的 source-handle main path、indirect call-site no-match dispatch 与 continuation step 现已统一接入 sibling non-resuming 的 dispatch / detach / cleanup 逻辑，覆盖 `Raise.raise` 与 custom non-resuming 两条路径。
  - LLVM codegen 的 effect `op_tag` 状态已提升为整个编译单元共享，修复了跨函数 perform 在 caller / callee / step trampoline 之间因局部分配顺序不同而发生的 tag 漂移。
  - 已新增 run-pass 回归：`effect_resume_mixed_escape_raise_indirect_single_site`、`effect_resume_mixed_escape_custom_nonresuming_indirect_single_site`。
  - `cargo test --all`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003c0c1c [DONE] Effect：LLVM 多 arm handle dispatch（escape-continuation + sibling non-resuming，site-matrix）
- 描述：最后把 sibling non-resuming 扩到 richer escape site matrix，包括 pre/post-immediate、多 site、nested block/if/while，以及 direct/indirect mixed。该阶段才统一处理 matrix state0/state1/continuation step 的 sibling detach/restore。
- 目标：
  - site-matrix 形态下的 escape-continuation 与 sibling non-resuming 共存可运行，不再被 “escape + non-resuming not supported” 门禁截断。
  - pre/post-immediate、多 site、nested path replay 与 loop re-entry 期间，non-resuming、immediate-resume、escape-continuation 三类 arms 都遵守一致的 handler-scope / sibling self-capture 规则。
  - `Raise.raise` 与 custom non-resuming effects 可与 richer escape site matrix 组合运行，不因 nested replay 或 resume 路径而丢失 dispatch 优先级。
- 验收：
  - 新增 run-pass fixtures：至少覆盖一例 pre/post-immediate 的 matrix 组合，以及一例 nested / direct+indirect mixed 与 sibling non-resuming 共存的组合。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0c1b
- 完成说明：
  - `codegen_handle_expr_immediate_resume_with_escape_and_nonresuming_siblings` 不再把 non-single-site matrix 组合统一拒绝；现已在 richer site-matrix 下接入 sibling non-resuming lowering。
  - site-matrix 的 state0 / state1 / continuation step 现已共享 sibling dispatch：普通 replay、indirect no-match fallback、以及 nested block/if/while 的 direct+indirect mixed replay 都会先尝试 sibling `Raise.raise` / custom non-resuming，再按既有 escape/outer unwind 语义继续。
  - immediate arm body、escape arm body，以及 matrix 下 sibling raise/custom catch body 现已统一导向 `finally_unwind` 或 step cleanup，保持 handler-scope 与 sibling self-capture 规则一致。
  - 已新增 run-pass 回归：`effect_resume_mixed_escape_pre_immediate_block_raise`、`effect_resume_mixed_escape_post_immediate_if_direct_indirect_custom_nonresuming`。
  - `cargo fmt --all`、`cargo test --all`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003c0c2 Effect：LLVM 多 arm handle dispatch（multiple resuming arms / 无 immediate-resume 的 multi-arm）
- 描述：审计 `codegen_handle_expr_multi_arm` 后确认，当前入口同时硬编码了三类结构性门禁：`multiple immediate-resume arms`、`multiple escape-continuation arms`、以及 `multi-arm without immediate-resume`。这三类缺口分别对应 non-resuming dispatch、escape continuation lowering、immediate-resume state machine 三条不同实现路径，单轮一起推进风险过高，因此拆为 `T2003c0c2a`～`T2003c0c2d`。
- 总目标：
  - 同一个 `handle` 最终支持多个 immediate-resume arms、多个 escape-continuation arms，以及不含 immediate-resume 的 multi-arm 组合。
  - 多个 resuming arms 的 dispatch 顺序、resume target 选择、captured-handler-stack / continuation 生命周期语义与源码 arm 顺序一致。
  - multi non-resuming、escape-only、escape+non-resuming、multi-immediate、multi-escape 等当前实现层门禁全部转成真实 lowering；仅对真正非法的语义冲突保留诊断。
- 拆分顺序：
  - `T2003c0c2a`：无 immediate-resume 的 pure non-resuming multi-arm。
  - `T2003c0c2b`：无 immediate-resume 的 escape-only / escape+non-resuming multi-arm。
  - `T2003c0c2c`：multiple immediate-resume arms（暂不混入 multiple escape）。
  - `T2003c0c2d`：multiple escape-continuation arms 与 richer multi-resuming mixed-arm 收口。
- 依赖：T2003c0c1c

### T2003c0c2a [DONE] Effect：LLVM 多 arm handle dispatch（无 immediate-resume 的 pure non-resuming multi-arm）
- 描述：现有 `codegen_handle_expr_multi_arm` 会把“没有 immediate-resume 的 multi-arm handle”统一拒绝，但对纯 non-resuming 子集来说，所需 primitive 已大体存在：single-arm `Raise.raise` / custom non-resuming lowering 已可运行，`immediate + sibling non-resuming` 路径也已实现多 non-resuming dispatch、handler detach/restore 与 `finally` 收口。先把这条最小可复用子集独立打通。
- 目标：
  - 支持不含 immediate-resume / escape-continuation 的 multi-arm handle；至少覆盖 multiple custom non-resuming，以及 `Raise.raise` + custom non-resuming 共存。
  - direct perform 与通过函数/闭包触发的 indirect perform 都能按源码 arm 顺序分发到匹配的 non-resuming arm。
  - 任一 non-resuming arm body 执行期间，同一个 source-handle 的 sibling handler scope 整体脱离当前 dispatch 栈，避免 sibling self-capture，并保持 `finally` 恰好执行一次。
- 验收：
  - 新增 run-pass fixtures：multiple custom non-resuming、`Raise.raise` + custom non-resuming、至少一例 indirect perform 命中 multi-arm。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0c1c
- 完成说明：
  - `codegen_handle_expr_multi_arm` 现已在“无 immediate-resume / 无 escape-continuation”的 multi-arm 组合下分流到新的 pure non-resuming lowering，不再统一报 `handle multi-arm without immediate-resume not yet supported`。
  - LLVM codegen 已新增 `codegen_handle_expr_nonresuming_multi_arm`：支持多个 custom non-resuming arms，以及 `Raise.raise` + custom non-resuming 共存；body dispatch、catch-arm detach 与 `finally_unwind` 现已统一按 active slot `op_tag` 向外传播。
  - non-resuming arm body 执行期间，同一个 source-handle 的 sibling custom handlers 与 Raise handler 都会整体脱离当前 dispatch scope，避免 sibling self-capture；`finally` 在 body no-match、catch 返回、arm body 向外 re-perform / re-raise 时都恰好执行一次。
  - 已新增 run-pass 回归：`effect_multi_nonresuming_custom_indirect`、`effect_multi_nonresuming_raise_custom_finally`。
  - `cargo fmt --all`、`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003c0c2b Effect：LLVM 多 arm handle dispatch（无 immediate-resume 的 escape-only / escape+non-resuming）
- 描述：审计 `T2003c0c2b` 后确认，无 immediate-resume 的 escape 路径并不是单一门禁，而是至少跨三个独立 lowering：top-level single direct site、single indirect site，以及 richer site-matrix（多 site / nested block / if / while / direct+indirect mixed）。若整包推进，会把 multi-arm dispatch、continuation step 与 nested replay 一次性耦合，因此继续拆成 `T2003c0c2b1`～`T2003c0c2b3`。
- 总目标：
  - 支持“一个 escape-continuation arm + 0..N 个 sibling non-resuming arms”的无-immediate multi-arm handle，并让 sibling non-resuming 在 arm body / continuation step 中遵守一致的 detach / restore / self-capture 规则。
  - escape continuation 的 captured-handler-stack / one-shot 生命周期 / resume 后 tail replay 与既有单-arm、以及已完成的 immediate mixed-arm 子集保持一致。
  - richer site-matrix 打通后，再把当前 `handle multi-arm without immediate-resume not yet supported` 收口为真实 lowering；multiple escape-continuation arms 仍留给 `T2003c0c2d`。
- 拆分顺序：
  - `T2003c0c2b1`：无 immediate-resume 的 single direct escape site（允许 sibling non-resuming）。
  - `T2003c0c2b1a`：修正 indirect escape-continuation arm binder 的真实类型与 payload decode。
  - `T2003c0c2b1b`：补 single-arm indirect escape-continuation 的 callee tail-perform resume path。
  - `T2003c0c2b2`：无 immediate-resume 的 single indirect escape site（允许 sibling non-resuming）。
  - `T2003c0c2b3`：无 immediate-resume 的 richer escape site-matrix（多 site / nested / direct+indirect mixed）。
- 依赖：T2003c0c2a

### T2003c0c2b1 [DONE] Effect：LLVM 多 arm handle dispatch（无 immediate-resume，single direct escape site）
- 描述：先收口最小可运行子集：top-level、`val` 绑定、single direct escape site。该阶段聚焦“一个 escape-continuation arm + sibling non-resuming arms”的无-immediate direct 单站点组合；true multiple escape-only 仍留给 `T2003c0c2d`。
- 目标：
  - `codegen_handle_expr_multi_arm` 不再把“无 immediate-resume + single direct escape site”统一拒绝；至少支持一个 escape arm，且可选 sibling `Raise.raise` / custom non-resuming arms。
  - escape continuation 的 captured-handler-stack、one-shot 生命周期、resume 后 top-level tail replay 与现有 single-arm direct escape 语义一致。
  - sibling non-resuming 在 escape arm body 与 continuation step 中都遵守一致的 handler-scope / self-capture 规则。
- 验收：
  - 新增 run-pass fixtures：无 immediate-resume 的 `escape + Raise.raise` direct single-site、`escape + sibling custom non-resuming` direct single-site。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0c2a
- 完成说明：
  - `codegen_handle_expr_multi_arm` 现已在“无 immediate-resume + 单个 top-level direct escape site”场景下分流到新的 no-immediate escape lowering；single indirect site 与 richer site-matrix 仍保留稳定诊断，交给后续 `T2003c0c2b2` / `T2003c0c2b3`。
  - LLVM codegen 已新增 `codegen_handle_expr_escape_with_nonresuming_siblings_direct`：支持一个 escape-continuation arm + sibling `Raise.raise` / custom non-resuming arms 的 no-immediate direct single-site 子集。
  - handle 主 body 的 pre-escape prefix 与 continuation step 现已共享 sibling op-tag dispatch / catch blocks；escape arm body 与 sibling catch body 继续把同源 sibling non-resuming 导向 `finally_unwind` 或 step cleanup，避免 self-capture。
  - 已新增 run-pass 回归：`effect_multi_escape_raise_direct_single_site`、`effect_multi_escape_custom_nonresuming_direct_single_site`。
  - `cargo fmt --all`、`cargo test --all`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003c0c2b1a [DONE] Effect：indirect escape-continuation arm binder 的真实类型与 payload decode
- 描述：在尝试实现 `T2003c0c2b2` 时发现，现有 single-arm indirect escape-continuation 本身还有一个前置缺口：arm binder 在 LLVM codegen 中没有以真实 op 参数类型 materialize。最小变体里，`println(key)` 会报 `sysroot print/println arg type`，`key + 1` 会报 `integer binary op lhs`，说明 indirect perform → arm binder 的 local typing / payload decode 还不正确，而且并非 multi-arm 特有问题。
- 目标：
  - single-arm indirect escape-continuation 的 arm binder 在 arm body 中具备真实 `CgTy` 与可用值语义，不再停留在“可声明但不可直接使用”。
  - 至少覆盖一个 `Int` binder 的直接打印或算术，以及一个 `String` / ref binder 的直接使用或等价覆盖。
  - 为 `T2003c0c2b2` 提供可复用的 binder materialization 基线，避免 multi-arm indirect path 复制当前错误语义。
- 验收：
  - 新增 run-pass fixtures：single-arm indirect escape-continuation 直接使用 arm binder（至少一例 `Int`；若实现允许，再补 `String` / ref 或等价覆盖）。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0c2b1
- 完成说明：
  - AST/typecheck/HIR 之间已新增 `binding span -> TypeId` side table；typecheck 在计算 handle arm binder 类型时写回该表，HIR lowering 在无显式 binder 注解时会优先读取 typecheck 结果，不再退回 `Any`。
  - `codegen_handle_expr_escape_continuation_indirect` 的 arm binder 读取路径现已统一走 `perform_slot_read_u64 + perform_slot_read_gc_ref + decode_abi_payload_transport`，不再停留在仅支持 `Int` 的手写 decode 分支。
  - single-arm indirect escape-continuation 的 arm binder 现已在 LLVM codegen 中按真实 payload 类型 materialize；`println(key)` 与 `key + 1` 不再分别报 `sysroot print/println arg type` / `integer binary op lhs`。
  - 已新增 run-pass 回归：`effect_escape_continuation_indirect_perform_binder_int_use`、`effect_escape_continuation_indirect_perform_binder_string_use`。
  - 在为上述回归尝试“callee 直接以 perform 作为尾返回”的最小形状时，确认还存在独立的 tail-resume 缺口，已另拆为后续 `T2003c0c2b1b` 跟踪，不再把该额外形状混入当前 binder materialization 任务。
  - `cargo fmt --all`、`cargo test --all`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003c0c2b1b [DONE] Effect：single-arm indirect escape-continuation 的 callee tail-perform resume path
- 描述：在为 `T2003c0c2b1a` 补 binder-use 回归时，尝试使用 `fun fetch() { Ask.ask(...) }` / `fun compute() { Ask.get(...) }` 这类“callee 直接以 perform 作为尾返回”的间接 perform 形状，发现 handle 会在打印 arm/result 后提前退出，`resume(...)` 没有继续执行 callee/handle body tail。既有 single-arm indirect escape 回归主要覆盖“callee 先把 perform 绑定到局部，再在 resume 后继续使用该局部”的形状，因此该 tail-return 子集仍是独立缺口。
- 目标：
  - single-arm indirect escape-continuation 支持 callee body 直接以 perform 作为 tail return，不再在 `resume(...)` 后提前退出。
  - tail-return 子集与既有 local-bound indirect path 在 continuation capture、callee suspend state、handle body tail replay 上保持一致语义。
  - 为 `T2003c0c2b2` 提供更完整的 single-arm indirect escape 基线，避免 multi-arm indirect path 继承未跟踪的 tail-resume 缺口。
- 验收：
  - 新增 run-pass fixtures：single-arm indirect escape-continuation 的 tail-return indirect perform（至少一例 `Int` 或 `String`，并覆盖 `resume(...)` 后继续执行 callee/handle body tail）。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0c2b1a
- 完成说明：
  - `scan_for_callee_suspend` 现已识别 `val x = perform(...)` 之外的两类 tail-return 形状：block 尾表达式 `perform(...)` 与 `return perform(...)`。
  - top-level function / closure 的 suspendable resume path 现已区分“恢复值绑定到局部后继续执行”与“恢复值直接成为当前返回值”两类模式；tail-return 子集不再在 resume 后走默认返回或空状态。
  - closure expression-body 形状（如 `{ Ask.ask(key) }`）现会合成最小 block 进入同一套 callee-suspend 扫描与 resume lowering，不再漏掉 tail-perform 路径。
  - 已新增 run-pass 回归：`effect_escape_continuation_indirect_perform_tail_return_int`、`effect_escape_continuation_indirect_perform_closure_tail_return_string`。
  - `cargo fmt --all`、`cargo test --all`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003c0c2b2 [DONE] Effect：LLVM 多 arm handle dispatch（无 immediate-resume，single indirect escape site）
- 描述：在 direct 单站点打通、`T2003c0c2b1a` 修正了 indirect escape arm binder materialization，且 `T2003c0c2b1b` 收口了 single-arm indirect escape 的 tail-return resume path 之后，再把无-immediate 的 escape 子集扩到 top-level single indirect call site，并让 callee suspend state replay 与 sibling non-resuming dispatch 对齐。
- 目标：
  - 无 immediate-resume 的一个 escape arm + 0..N sibling non-resuming arms 支持 top-level single indirect call site。
  - continuation step 会把 resume payload 写回 callee suspend state，并在 no-match / resume replay 时保留 sibling non-resuming 的 dispatch 优先级。
  - escape-only 与 escape+non-resuming 的 indirect 单站点都不再回退到统一门禁。
- 验收：
  - 新增 run-pass fixtures：无 immediate-resume 的 escape-only indirect single-site、escape + sibling non-resuming indirect single-site。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0c2b1b
- 完成说明：
  - `codegen_handle_expr_escape_with_nonresuming_siblings` 现已确认会在“无 immediate-resume + 单个 top-level indirect escape site”场景下分流到 dedicated no-immediate indirect lowering，不再回退到统一门禁。
  - no-immediate indirect continuation step 已对齐既有 single-arm indirect escape 的 callee suspend state replay：`resume(...)` 会把恢复值写回 callee state，重新调用 callee，并在 no-match / replay 路径中保留 sibling `Raise.raise` / custom non-resuming 的 dispatch 优先级。
  - 已新增 run-pass 回归：`effect_multi_escape_indirect_single_site`、`effect_multi_escape_custom_nonresuming_indirect_single_site`、`effect_multi_escape_raise_indirect_single_site`。
  - `cargo fmt --all --check`、`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003c0c2b3 Effect：LLVM 多 arm handle dispatch（无 immediate-resume，escape site-matrix，已拆分）
- 描述：继续审计后确认，这个 richer site-matrix 不是单一路径门禁，而是至少跨四类独立实现：multiple top-level direct、nested direct、indirect site-matrix、direct+indirect mixed。若继续整包推进，会把多站点 pc 状态机、nested replay、callee suspend replay 与 same-stmt mixed next/prev 关系耦合在一起，因此继续拆成 `T2003c0c2b3a`～`T2003c0c2b3d`。
- 总目标：
  - 无 immediate-resume 的一个 escape arm + 0..N sibling non-resuming arms 最终支持 richer site-matrix，不再停留在 direct / indirect 单站点子集。
  - pre/post site replay、多 site、nested path 与 direct+indirect mixed 下的 captured locals / handler-scope / sibling self-capture 语义保持稳定。
  - 为后续 `T2003c0c2c` / `T2003c0c2d` 提供已收口的无-immediate escape lowering 基线。
- 拆分顺序：
  - `T2003c0c2b3a`：multiple top-level direct escape sites。
  - `T2003c0c2b3b`：nested direct escape sites（block / if / while）。
  - `T2003c0c2b3c`：indirect escape site-matrix（top-level multiple + nested block / if / while）。
  - `T2003c0c2b3d`：direct + indirect mixed site-matrix。
- 依赖：T2003c0c2b2

### T2003c0c2b3a [DONE] Effect：LLVM 多 arm handle dispatch（无 immediate-resume，multiple top-level direct escape sites）
- 描述：当前 no-immediate multi-arm direct lowering 仍只支持一个 top-level direct escape site。先收口最小但真实的 site-matrix 子集：多个 top-level direct sites；nested direct / indirect / mixed 继续留给后续子任务。
- 目标：
  - 无 immediate-resume 的一个 escape arm + 0..N sibling non-resuming arms 支持多个 top-level direct escape sites。
  - continuation step 通过 `pc` / replay 继续命中后续 top-level direct site，不再在第一个 direct site 之后提前完成。
  - body-lift、captured locals、sibling dispatch / detach / cleanup 与既有 single-arm multi-perform direct escape 语义保持一致。
- 验收：
  - 新增 run-pass fixtures：无 immediate-resume 的 multiple top-level direct escape-only，以及 multiple top-level direct + sibling non-resuming。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0c2b2
- 完成说明：
  - `codegen_handle_expr_escape_with_nonresuming_siblings` 现已在“无 immediate-resume + 无 indirect site + 全部为 top-level direct site”场景下分流到统一的 no-immediate direct lowering，不再把 multiple top-level direct sites 统一拒绝。
  - no-immediate direct lowering 的 continuation state 现已新增 `pc` 字段；step trampoline 会按 `pc` 恢复当前 direct site 的返回值，继续 replay top-level tail，并在命中后续 direct site 时重新分配 continuation。
  - 已把 top-level body-lift 分析扩到多 site：较早 direct site 的结果与其他 pre-site locals 现在可跨后续 suspension 保留到最终 tail；sibling `Raise.raise` / custom non-resuming 的 dispatch、detach 与 cleanup 语义保持不回归。
  - 已新增 run-pass 回归：`effect_multi_escape_direct_multi`、`effect_multi_escape_custom_nonresuming_direct_multi`。
  - `cargo fmt --all`、`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003c0c2b3b [TODO] Effect：LLVM 多 arm handle dispatch（无 immediate-resume，nested direct escape sites）
- 描述：在 top-level direct site-matrix 打通后，再扩到 nested direct sites。该阶段需要把 block / if / while 的 prefix / tail replay 与 loop re-entry 接到 no-immediate multi-arm direct lowering。
- 目标：
  - 无 immediate-resume 的一个 escape arm + 0..N sibling non-resuming arms 支持 nested block / if / while 中的 direct escape site。
  - `resume(...)` 后会先 replay 命中的 nested path 尾部，再继续当前 top-level tail 或 loop re-entry。
  - sibling non-resuming 在 nested replay / arm body / continuation step 中保持一致的 dispatch 与 self-capture 语义。
- 验收：
  - 新增 run-pass / build fixtures：覆盖至少一例 nested direct，以及至少一个仍未支持的 indirect / mixed 边界。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0c2b3a

### T2003c0c2b3c [TODO] Effect：LLVM 多 arm handle dispatch（无 immediate-resume，indirect escape site-matrix）
- 描述：在 direct site-matrix 打通后，再把 no-immediate multi-arm escape 扩到 indirect site-matrix，包括 top-level multiple indirect，以及 nested block / if / while 的 indirect call site。
- 目标：
  - 无 immediate-resume 的一个 escape arm + 0..N sibling non-resuming arms 支持 richer indirect site-matrix。
  - continuation step 会把恢复值写回 callee suspend state，并在 replay / no-match 路径中保持 sibling non-resuming dispatch 优先级。
  - nested indirect 与 top-level multiple indirect 共享稳定的 replay / callee-resume 语义。
- 验收：
  - 新增 run-pass / build fixtures：覆盖至少一例 multiple indirect 或 nested indirect，以及至少一个仍未支持的 direct+indirect mixed 边界。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0c2b3b

### T2003c0c2b3d [TODO] Effect：LLVM 多 arm handle dispatch（无 immediate-resume，direct + indirect mixed site-matrix）
- 描述：最后收口 no-immediate richer matrix 中 direct + indirect mixed 的组合，包括 top-level mixed 与 nested same-stmt mixed。该阶段统一处理 next/prev replay、same-stmt 路由与 sibling dispatch。
- 目标：
  - 无 immediate-resume 的一个 escape arm + 0..N sibling non-resuming arms 支持 direct + indirect mixed site-matrix，不再停留在同类 site 子集。
  - same-stmt mixed、pre/post site replay、captured locals 与 sibling self-capture 语义保持稳定。
  - 完成后再进入 `T2003c0c2c` / `T2003c0c2d`，避免它们继续依赖 no-immediate escape 的结构性门禁。
- 验收：
  - 新增 run-pass fixtures：至少一例 direct+indirect mixed + sibling non-resuming。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0c2b3c

### T2003c0c2c [TODO] Effect：LLVM 多 arm handle dispatch（multiple immediate-resume arms）
- 描述：当前 immediate-resume lowering 整体以“单个 distinguished immediate site + 单个 arm state machine”为中心组织。multiple immediate-resume arms 需要把 perform-site 扫描、arm dispatch、resume target 选择与 `finally` cleanup 从“单个 op”扩到“按 op tag / 源码顺序选择多个 immediate arm”，但暂不把 multiple escape 一起混入。
- 目标：
  - 同一个 handle 支持多个 immediate-resume arms；至少覆盖 top-level direct site，并保持源码 arm 顺序与 one-shot `resume(value)` 语义稳定。
  - multiple immediate-resume arms 可与 sibling non-resuming 组合，不再被 `handle mixed immediate-resume arms (only 1 supported)` 门禁截断。
  - `finally`、arm-body re-perform、resume 后 tail continuation 与现有单 immediate / mixed-arm non-resuming 子集一致。
- 验收：
  - 新增 run-pass fixtures：multiple immediate arms、multiple immediate + sibling non-resuming。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0c2b3d

### T2003c0c2d [TODO] Effect：LLVM 多 arm handle dispatch（multiple escape-continuation arms / richer multi-resuming mixed-arm）
- 描述：最后收口 multiple escape-continuation arms，以及 immediate-resume + multiple escape-continuation 的 richer multi-resuming mixed-arm。该阶段需要统一多个 continuation 的 dispatch、resume target、captured locals 与 nested replay 规则。
- 目标：
  - 同一个 handle 支持多个 escape-continuation arms，以及 immediate-resume + multiple escape-continuation 的 mixed-arm 组合。
  - 多个 resuming arms 的 dispatch 顺序、resume target 选择、captured-handler-stack / continuation 生命周期语义与源码 arm 顺序一致。
  - 把 `handle mixed escape-continuation arms (only 1 supported)` 等剩余结构性门禁全部转成真实 lowering；仅对真实非法语义保留诊断。
- 验收：
  - 新增 run-pass fixtures：multiple escape arms、multiple immediate + multiple escape 的最小可运行组合。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0c2c

### T2003c [TODO] Effect：perform / handler 完整组合回归矩阵
- 描述：在 `T2003c0*` 的 mixed-arm LLVM dispatch 全部落地后，补 full matrix 回归，确保 `T2003` 的终态不是“剩余组合继续稳定诊断”，而是合法的 perform/handler 组合都已有实现与回归覆盖。
- 目标：
  - 为 single/multiple direct perform、single/multiple indirect perform、direct+indirect 共存、multi-arm kind 任意组合、nested handle、GC stress 补齐端到端回归。
  - 相关用例在 `SCOOP_GC_STRESS=1` 下稳定，且 handler stack、continuation pinning、captured locals / cleanup 语义不回归。
  - `T2003` 完成时，不再存在“因为 lowering 尚未实现而拒绝某类合法 mixed 组合”的长期 TODO；若仍有限制，必须是语言语义层面的真实非法组合。
- 验收：
  - 新增 run-pass fixtures：覆盖 full mixed-arm / nested handle / GC stress matrix。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003c0c2d

### T2004 [BLOCKED] 前端：裸 block-only 方案已改由 Rust 风格分号 / expression statement 语义统一承接
- 描述：原计划是单独补 statement-position 裸 `{ ... }` block 语法，并把 effect fixtures 里的 `@Safe` nested-block workaround 切回普通 `{ ... }`。现确认 Scoop 不采用 Kotlin / Swift 式 whitespace-sensitive 语句边界；同时由于 trailing lambda 已支持多段出现，若只补“裸 block 优先级”而不统一收口语句终止与 expression statement 语义，歧义仍会散落在 call / lambda / block tail 各处。
- 目标：
  - 不再单独推进“保持当前可选分号语义，仅补 naked block”的方案。
  - 由 `T2201`～`T2204` 统一承接：显式分号边界、Rust 风格 expression statement / block tail 语义、effect fixtures 去 `@Safe` workaround，以及规范 / 文档同步。
- 验收：
  - `T2201`～`T2204` 进入主线落地顺序；`T2004` 不再作为独立实现项继续展开。
- 依赖：T2003c

## T22：前端语句语义 / Rust 风格分号规则收口

### T2201 [TODO] Parser / AST：引入 Rust 风格语句终止规则与 `{}` / trailing lambda 消歧
- 描述：当前 parser 把 `;` 当作可选分隔符，表达式语句默认尽量向右吞 postfix，因此 `{ ... }` 在表达式起始位置会优先落入 lambda，而在 call 之后又会被解释成 trailing lambda。Scoop 既不保留换行语义，又已支持多个 trailing lambdas，继续沿用 Kotlin-like optional-semicolon 规则会让 statement-position block 与 trailing lambda 的边界越来越不透明。
- 目标：
  - 明确 `;` 是语句终止符而不是“纯可选装饰”；以前一条表达式结束、且下一条以 `{` 或其它仍可能续接 postfix 的 token 起始时，必须依赖显式 `;` 切断。
  - statement boundary 之后的 `{ ... }` 可作为新的 block 起始；`callee { ... }`、`callee(args) { ... }`、多个 trailing lambdas 仍按调用后缀解析，不因新规则回退。
  - parser / AST 为表达式语句保留“是否以 `;` 终止”的信息，供后续 block tail / expression statement 语义使用。
- 验收：
  - 新增 parser fixtures：`foo(); { ... }` 解析为两条语句、`foo() { ... }` 仍为 trailing lambda、多个 trailing lambdas + `;` fence、缺少 `;` 时的稳定诊断或歧义分流。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：无

### T2202 [TODO] Typecheck / HIR：引入 Rust 风格 expression statement 与 block tail value 语义
- 描述：当前 block 的值语义只看“最后一条是不是 `StmtKind::Expr`”，并不区分该表达式是否由 `;` 终止。这与 Rust 风格“无分号 tail expr 才产生 block 值；有分号则是 expression statement，结果视为 `Unit`”并不一致，也会让后续 naked block / lambda 消歧缺乏统一语义基础。
- 目标：
  - block 仅在最后一个表达式语句未以 `;` 终止时才产生 tail value；`expr;` 作为 expression statement，其结果不再参与 block 值推导。
  - `if` / `when` / `handle` / lambda block body / 局部 block 的值语义统一按上述规则工作，避免“parser 看起来像 Rust，typecheck 仍按旧规则取尾值”。
  - HIR / MIR / diagnostics 对“tail expr”和“terminated expr stmt”保持可区分形状，不再依赖 AST 阶段的隐式约定。
- 验收：
  - 新增 typecheck / HIR fixtures：`{ 1 }` 得 `Int`、`{ 1; }` 得 `Unit`、控制流 / handler / lambda body 的 tail expr 与 terminated expr stmt 区分、相关稳定诊断。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：T2201

### T2203 [TODO] 回归迁移：effect nested-block fixtures 切到 plain block + semicolon fence
- 描述：`T2003b1`、`T2003c0b2c1a`、`T2003c0b2c2b` 等回归当前借 `@Safe { ... }` 表达 statement-position nested block；这在 HIR lowering 后虽然会退化成普通 `Block`，但源码层并没有真正覆盖新的语句边界语义。引入 Rust 风格分号规则后，需要把这些回归切回 plain block，并显式补上必要的 `;` fence。
- 目标：
  - 把仅用于制造 nested block 的 `@Safe` workaround 切回普通 `{ ... }`，并在前一条 expr-ended statement 之后按新规则补显式 `;`。
  - 真正依赖 safe-region 语义的测试继续保留 `@Safe`，避免把 unsafe 语义回归误混进 parser / stmt 语义任务。
  - 锁定 trailing lambda / multiple trailing lambdas 与 block statement 并存时的行为，避免 effect fixture 迁移顺手改变调用语义。
- 验收：
  - 更新 effect fixtures：`effect_resume_nested_block_single_perform`、`effect_resume_mixed_escape_pre_immediate_block`、`effect_resume_mixed_escape_post_immediate_block`、`effect_resume_mixed_escape_pre_immediate_while_nested_block`、`effect_resume_mixed_escape_while_is_error` 改为 plain block + 必要分号，行为或诊断保持不变。
  - 新增 parser / typecheck / HIR / run-pass 回归：multiple trailing lambdas 与后续 block statement 的 fence 规则。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2202, T2003c

### T2204 [TODO] 规范 / 文档：同步 Rust 风格分号、expression statement 与 trailing-lambda 消歧规则
- 描述：当前 `SCOOP_FULL_SPEC.md` 对 trailing lambda 采用 Kotlin-like 表述，但尚未把“非 whitespace-sensitive + Rust 风格分号 / expression statement 语义”的最终语法边界写实；若只改实现不改规范，后续 fixtures 与文档示例会持续漂移。
- 目标：
  - 更新 `SCOOP_FULL_SPEC.md` 的 statements / block expression / trailing lambda / lambda body 相关章节，明确 `;` 的语义、tail expr 规则、statement-position block 的 fence 规则，以及 multiple trailing lambdas 的优先级。
  - 同步改写规范内 doctest / fixture 示例，以及仓库内仍使用旧叙述的说明文档（至少 `README.md` / `PLAN.md` / `TODO.md` 中相关描述）。
  - 若规范代码块发生变更，补 `spec-fixtures sync/check` 所需的生成文件与说明，保证规范与回归继续一致。
- 验收：
  - `SCOOP_FULL_SPEC.md` 与相关文档更新完成；必要时运行 `cargo run -p scoop_tools -- spec-fixtures sync` 后再 `cargo run -p scoop_tools -- spec-fixtures check`。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：T2203

## T21：Structured Concurrency / `Task<T>`

### T2101 [TODO] 并发：`spawn` / `join` 的 typecheck 与 HIR 去 `Int` 硬编码
- 描述：`spawn { ... }` 目前仍要求 body 可赋给 `Int`，`join` lowering 也仍写死为 `Int` 句柄与 `Int` 结果。先把前端表示与类型系统改成真实的 `Task<T>`。
- 目标：
  - `spawn { body }` 从 body 推导结果类型 `T`，表达式类型为 `Task<T>`，不再要求 body 可赋给 `Int`。
  - HIR 对 `spawn` / `join` 保留任务结果类型与必要的运行期元信息，不再把 handle/result 擦成 `Int`。
  - 已确认仍缺失的后端功能由后续任务显式承接，而不是继续用前端 `Int` 特判掩盖：
    - T2102：HIR lowering / sysroot glue 仍写死 `__scoop_task_spawn_int` / `__scoop_task_join_int` 与 `Task<Int>` 表面；
    - T2103：LLVM codegen 仍只支持 `scoop_task_spawn_int` / `scoop_task_join_int`；
    - T2104：runtime executor 仍是 `ScoopTaskU64` / `result_u64` / `resume_u64` 单载荷模型。
- 验收：
  - 新增 typecheck / HIR fixtures：`Task<Int>`、`Task<String>`、`Task<Struct>`、`Task<Task<Int>>`。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：无

### T2102 [TODO] 并发：`spawn` / `join` 语法糖与 sysroot glue 去 `_int` 专用路径
- 描述：即使 T2101 把前端类型改成 `Task<T>`，当前 lowering 仍固定生成 `__scoop_task_spawn_int` / `__scoop_task_join_int`，`wrap_task_spawn_int_call` 与 sysroot `task/core` 表面也仍把结果类型收窄为 `Task<Int>`。这属于明确缺失的功能，不应被描述成“后端边界”。
- 目标：
  - HIR lowering / block rewrite 不再依赖 `__scoop_task_spawn_int` / `__scoop_task_join_int` 和 `wrap_task_spawn_int_call` 这类 `_int` 专用入口。
  - `sysroot/core.scoop` 与 `sysroot/task.scoop` 中供 `spawn` / `join` / `await` 路径使用的 internal glue 不再只暴露 `Task<Int>` / `Continuation<Int>` / `Executor.await(Task<Int>)`。
  - 语法糖 desugar 后的 HIR 仍能保留任务结果类型，为后续 LLVM / runtime 泛型化提供稳定输入。
- 验收：
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - 新增 HIR fixtures：`spawn { "x" }`、`spawn { Struct(...) }`、`join task` 的泛型结果类型保持不被擦除。
- 依赖：T2101

### T2103 [TODO] 并发：LLVM codegen 去 `scoop_task_*_int` 专用路径
- 描述：当前 LLVM 侧对 `spawn/join` 的支持仍硬编码在 `scoop_task_spawn_int` / `scoop_task_join_int`，并显式要求 `CgTy::Int` / `i64` 值。只要这条路径不改，`Task<T>` 就仍然只是假类型，不能执行 `Int` 之外的结果类型。
- 目标：
  - codegen dispatch 不再只识别 `__scoop_task_spawn_int` / `__scoop_task_join_int`，而是支持与 `Task<T>` 对齐的统一 task intrinsic / helper 路径。
  - `spawn` 结果保存、`join` 结果取回可覆盖 scalar / ref / aggregate / 泛型实例，不再把结果压扁回 `i64`。
  - task payload transport 与 continuation payload 方案保持一致，避免维护独立的 task-only ABI。
- 验收：
  - 新增 run-pass fixtures：`spawn` 返回 `String`、tuple/struct、class ref、嵌套泛型值。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2102

### T2104 [TODO] 并发：runtime executor / `Task<T>` 完成回调泛型化
- 描述：`runtime/c/scoop_task_executor.c` 当前仍是 `ScoopTaskU64` + `result_u64` + `on_complete_resume_u64` 模型，sysroot `task.scoop` 也仍把 `taskCreate` / `await` / `map` / `andThen` 固定在 `Task<Int>`。这不是“实现细节”，而是当前明确缺失的运行期功能。
- 目标：
  - runtime task 状态机、executor job、completion waiter 不再只支持 `u64` 结果与 `resume_u64`，而是支持与编译器 ABI 对齐的泛型 payload。
  - `Task<T>.onComplete`、`Executor.await`、`map`、`andThen` 等 glue 不再固定在 `Task<Int>` / `Continuation<Int>`。
  - ref / aggregate payload 在 pinning、GC stress、跨线程或跨 executor 恢复时语义稳定。
- 验收：
  - 新增 `crates/scoop_runtime/tests/*` 或等价 runtime 测试：泛型 task result、onComplete 恢复、ref payload rooting。
  - 至少一组 `SCOOP_GC_STRESS=1` run-pass 覆盖 `Task<String>` 或 `Task<StructWithRef>`。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2103

### T2105 [TODO] 并发：结构化并发回归矩阵与语义锁定
- 描述：`Task<T>` 泛型化后，需要用真实并发场景锁定语义边界，避免回归只覆盖“单任务 + Int 返回值”的最小路径。
- 目标：
  - 覆盖 nested `spawn` / `join`、控制流中的 `join`、多任务交错与 join 顺序等场景。
  - 验证 `Task<T>` 在错误传播、取消前置准备、GC 压力下的最小语义边界；暂不扩展到下一阶段 executor / stdlib API。
  - 把当前阶段明确不支持的并发组合写成稳定诊断或注释化限制，避免语义漂移。
- 验收：
  - 新增 run-pass fixtures：nested spawn/join、多任务交错、控制流 join。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2104

## T23：Lambda 推断与调用语义补齐

### T2301 [TODO] Lambda：expected function type 向任意参数个数传播
- 描述：当前 lambda 的 expected-type 向下传播只覆盖 0/1/2 参数；一旦没有 expected function type，未标注类型的参数会直接报错。需要先把最常见的上下文推断补齐，再把真正不可推断的场景留给稳定诊断。
- 目标：
  - expected function type 的传播覆盖任意参数个数，而不是只支持 0/1/2 参数 lambda。
  - 变量初始化、返回语境、调用实参、集合/构造器上下文等常见入口都能把 expected type 传给 lambda。
  - 对确实无法推断的场景保留清晰、稳定的错误信息，而不是依赖零散 early error。
- 验收：
  - 新增 typecheck fixtures：3+ 参数 lambda、多上下文 expected-type 传播、无法推断时的诊断。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：无

### T2302 [TODO] Lambda：receiver lambda 体内 `this` 与成员解析
- 描述：receiver function type 已经进入类型系统，但 lambda body 里当前还不会自动注入 `this`，导致“类型可表达、语义不可用”的断层。
- 目标：
  - receiver lambda 进入 typecheck / lowering 时自动建立 `this` 绑定与成员查找环境。
  - receiver lambda 中的成员访问、扩展调用、闭包捕获与普通 lambda 保持一致的局部作用域规则。
  - 相关 HIR / codegen 不再把 receiver 仅当作普通首参处理，避免 `this` 语义在后续阶段丢失。
- 验收：
  - 新增 typecheck / run-pass fixtures：receiver lambda 直接访问 `this`、调用成员、捕获外层局部。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2301

### T2303 [TODO] 调用语义：统一函数值 / funptr / ctor delegation 的实参匹配
- 描述：当前函数值调用、函数指针调用、`super(...)` / `this(...)` 构造器委托调用仍各自带有命名实参或 receiver function type 的早期门禁，调用规则没有真正统一。
- 目标：
  - 函数值调用支持命名实参，参数匹配规则与普通函数调用保持一致。
  - 函数指针调用支持命名实参，并解除对 receiver function type 的不必要早期拒绝，或在更合理的阶段统一降格/诊断。
  - `super(...)` / `this(...)` 构造器委托调用改用同一套实参匹配逻辑，不再只允许位置参数。
- 验收：
  - 新增 typecheck / run-pass fixtures：函数值命名实参、funptr 命名实参、ctor delegation 命名实参。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2301

## T24：泛型约束 / Pattern / 值类型能力补齐

### T2401 [TODO] 泛型：`where` nominal bound 支持类型实参与 instantiated supertype 满足性
- 描述：当前 `where` 约束一旦写成带类型实参的 nominal bound（例如 `where T: Box<Int>`）就会被直接拒绝；即便后续把 bound 本身 lower 成实例化类型，类/接口经由“已实例化的 supertype”满足该 bound 的场景也还没有被显式承接。需要把这类 bound 贯通到解析、解析后表示、检查、子类型关系与诊断。
- 目标：
  - 支持带类型实参的 nominal `where` bound，并正确解析到已实例化的 bound type。
  - 类/接口若通过已实例化 supertype 满足该 bound（例如 `Sub : Base<Int>` 满足 `where T: Base<Int>`），实例化处检查可正确通过，而不是只接受“exact same nominal type”。
  - 实例化处 bound 检查、函数体内成员分发、错误消息都基于实例化后的 bound，而不是回退到未参数化 nominal type。
  - 对不满足或不可解析的 bound 给出稳定诊断。
- 验收：
  - 新增 typecheck fixtures：正例 `where T: Box<Int>`、经由 instantiated supertype 满足 `where T: Base<Int>`、反例 `where T: Box<String>`、体内通过 bound 调方法。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：无

### T2401a [TODO] 泛型：`where` nominal bound 的子类型满足性回归矩阵
- 描述：当前 use-site `where` 检查已通过 `is_type_assignable` + `direct_supertypes` 支持最小 nominal 上转，但仓库里还没有专门锁定“接口/类继承链上的实例满足 generic bound”这条语义。像 `interface Sub : Base`、`class Impl : Sub`、`fun <T> f(x: T) where T: Base` 这样的路径，不应只依赖当前实现细节“碰巧有效”。
- 目标：
  - 为当前已支持的非参数化 nominal bound 语义补齐回归：类实现接口、类继承基类、子接口及其实现类型传给父接口 bound。
  - 覆盖变量持有子类型实例、参数透传、泛型 passthrough 等常见入口，而不只断言字面量/构造表达式直传。
  - 对 builtin/value 类型通过 boxing 满足 interface bound 的既有语义也补专门回归，避免后续 `assignable` 调整回退。
- 验收：
  - 新增 typecheck / run-pass fixtures：`Sub : Base`、`Impl : Sub` 满足 `where T: Base`，以及 `Int` / `Bool` 满足 `Hashable` 或等价 interface bound。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：无

### T2401b [TODO] 泛型：`where` bound 驱动的方法分发补齐接口继承链与多 bound 歧义
- 描述：当前 `where` bound 驱动的方法分发只会从声明的 bound 本身构造 `Bound.method` 并在首个命中的 bound 上提前返回，没有沿接口 supertype 链查找继承成员，也不会对多个 bounds 提供的同名成员做歧义诊断。这会把合法语义错判成“方法不存在”，或者把本该报歧义的场景静默绑定到声明顺序。
- 目标：
  - `where T: Sub` 可调用 `Base` 上声明的方法，接口继承链上的成员对 bound receiver 可见。
  - `where T: A, T: B` 下的同名成员遵守明确的候选集/歧义规则，不再按遍历顺序抢先返回。
  - 对“继承成员可见”“多 bound 歧义”“确实不存在该成员”三类场景给出稳定回归与诊断。
- 验收：
  - 新增 typecheck / run-pass fixtures：通过 `where T: Sub` 调用 `Base` 方法、多 bound 同名成员的歧义报错。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：T2401a

### T2401c [TODO] 调用/泛型：成员方法签名收集与 `where` bound 分发对齐 richer generic/effect 调用
- 描述：当前共享的成员方法签名收集路径仍会直接跳过“2 个以上 type params”与带 `<eff E>` 的成员方法；`where` bound 驱动的方法分发还会忽略显式类型实参。这使 `x.method<U>()`、多 type param 成员方法、effect-generic 成员方法在普通 member call 与 bound member call 上都存在实现层缺口。
- 目标：
  - 普通成员调用与 `where` bound 驱动的成员调用都支持显式类型实参，不再把 `member<T>(...)` silently 降格成“只能靠推断”。
  - 成员方法签名收集、实例化与诊断支持 2+ type params 与 `<eff E>` 成员方法，不再在候选收集阶段直接跳过。
  - 成员方法调用的 type arg / effect arg 规则与顶层函数调用尽量对齐，避免 shared helper 与 top-level call 各走一套缩水语义。
- 验收：
  - 新增 typecheck / run-pass fixtures：普通 member call 与 `where` bound member call 的显式类型实参、2+ type params 成员方法、带 `<eff E>` 的成员方法。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：T2401b

### T2402 [TODO] Pattern：顶层 `val` 支持 pattern binding
- 描述：局部 destructuring 已逐步落地，但顶层 `val (a, b) = ...` 仍在声明头检查阶段被直接拒绝，导致同一套 pattern 语法在顶层与局部语义不一致。
- 目标：
  - 顶层 tuple / struct / enum destructuring 复用既有 pattern binding 规则，而不是单独保留“顶层只允许标识符”的限制。
  - 顶层符号安装、初始化顺序、多文件可见性与循环引用诊断保持稳定。
  - 对当前仍不支持的递归或歧义 pattern 给出明确报错，而不是统一 early reject。
- 验收：
  - 新增多文件 fixtures：顶层 tuple/struct 解构、跨文件引用、非法 pattern 诊断。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：无

### T2403 [TODO] 值类型：`struct` 字段支持 `var` 与默认值
- 描述：当前 `struct` 字段同时禁止 `var` 与默认值，值类型声明能力明显弱于目标语言语义。需要先收口字段模型，再决定更新与构造路径如何共享实现。
- 目标：
  - `struct` 声明支持 `var` 字段与默认值，声明头、构造参数、布局与初始化规则保持一致。
  - 默认值在构造调用、`with` 更新与常量/编译期路径中有统一语义，不引入额外特判。
  - 字段可变性与值语义冲突处给出明确约束和诊断。
- 验收：
  - 新增 run-pass fixtures：带默认值的 `struct` 构造、省略默认参数、`var` 字段更新。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：无

### T2404 [TODO] 值类型：`with` 更新扩展到更完整的值类型语义
- 描述：当前 `with` 更新只支持 `struct` 且显式拒绝嵌套字段路径更新，无法覆盖更接近 record-style 的值对象写法。
- 目标：
  - `with` 的 base 类型不再局限于当前最小 `struct` 子集，而是对齐本轮支持的值类型模型。
  - 嵌套字段路径更新 lower 成稳定的 copy-update 链，而不是在 typecheck 阶段直接拒绝。
  - 诊断能够区分“字段不存在 / 字段不可更新 / 类型不匹配 / base 非值类型”等不同错误。
- 验收：
  - 新增 HIR / run-pass fixtures：单层 `with`、嵌套路径 `with`、非法更新诊断。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2403

### T2405 [TODO] Pattern：`when` 的 or-pattern 支持共享 binder
- 描述：当前 or-pattern 已能做简单判别，但一旦在 `A(x) | B(x)` 里引入 binder 就会被直接拒绝。需要把 binder 集一致性与类型合流规则补齐。
- 目标：
  - 当各分支 binder 集、名称与类型兼容时，允许 or-pattern 引入共享 binder。
  - `when` arm 的局部环境合并稳定，不再依赖“or-pattern 不能绑定名字”的早期限制。
  - 对 binder 数量、名称或类型不一致的分支给出具体诊断。
- 验收：
  - 新增 typecheck / run-pass fixtures：合法 binder or-pattern、非法 binder mismatch。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：T2402

## T25：`const fun` 与 MIR 完整化

### T2501 [TODO] `const fun`：放宽纯签名门禁并后置 effect 验证
- 描述：当前声明头检查会直接拒绝 `const fun` 上的非 `Pure` effect row 与任何 `eff` 参数，使编译期函数模型停留在最保守的纯函数子集。
- 目标：
  - `const fun` 的声明层不再 blanket reject 非 `Pure` effect row / `eff` 参数，而是允许表达后再在语义阶段判定是否可在编译期执行。
  - const evaluator / 调用检查能区分“编译期可执行”“语义可声明但当前未实现”“运行期 effect 不允许进入 const”三类情况。
  - 相关诊断后置到更合理的阶段，并保留清晰的 unsupported reason。
- 验收：
  - 新增 const fixtures：effect row / `eff` 参数的正反例与诊断。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：无

### T2502 [TODO] MIR：常见表达式 lowering 去 `Todo`
- 描述：MIR 路径里，struct literal、tuple literal、interpolated string、member access、call、cast、type check 等表达式仍大量直接降成 `Todo`，当前更像“结构回归视图”而不是可依赖的中端表示。
- 目标：
  - struct/tuple literal、interpolated string、member access、call、cast、type check 等常见表达式都 lower 成真实 MIR，而不是 `Todo`。
  - 新增 dump-mir fixtures 覆盖每一类表达式，确保结构稳定可回归。
  - 已触达路径不再残留 `Todo("...")` 占位。
- 验收：
  - 新增 MIR fixtures：struct/tuple/interpolation/member/call/cast/type-check。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：无

### T2503 [TODO] MIR：`perform` / `handle` / 控制流 lowering 去占位
- 描述：effect 相关语义与部分控制流在 MIR 中仍是非常粗糙的占位结构。如果后续要把 MIR 当作验证、优化或解释执行的基础，这一块必须先去掉“看起来有节点，实际不可用”的假完整性。
- 目标：
  - `perform` / `handle` 在 MIR 中有最小但真实的结构化表示，可表达 handler、resume/continue 边界与 effect 控制流。
  - MIR 中的相关控制流节点与前端 / LLVM 语义保持可对照，不再退回统一 `Todo`。
  - 不破坏现有 LLVM 主路径；MIR 的增强先服务于可验证性与后续优化入口。
- 验收：
  - 新增 dump-mir fixtures：`perform`、`handle`、嵌套控制流与 effect 组合。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：T2003、T2502

## T26：低优先级（Annotation / FFI ABI）

### T2601 [TODO] Annotation：annotation class 从 data-only 子集扩到 richer model
- 描述：annotation class 当前只接受“主构造参数承载数据”的最小子集，不支持继承 / 实现接口，也不支持类型体。该能力不阻塞当前主线，因此放在本轮末尾。
- 目标：
  - annotation class 支持更完整的声明模型，包括 supertypes / interfaces 与类型体保留。
  - typecheck / HIR 能保留 richer annotation 元信息，避免在前端直接截断。
  - 对仍未实现的 runtime 或反射相关能力给出明确边界，而不是继续用 data-only 子集冒充完整支持。
- 验收：
  - 新增 parser / typecheck / HIR fixtures：带 supertype、带 body 的 annotation class 及相关诊断。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：无

### T2602 [TODO] FFI：`@CallingConvention` 与 extern side table 扩展到非 C ABI
- 描述：当前 extern side table 明确只支持 C ABI，`@CallingConvention` 也只接受 `"c"` / `"cdecl"`。这条线属于系统互操作增强，不是当前阶段主线，因此放在所有语言特性任务之后。
- 目标：
  - `@CallingConvention` 接受除 C ABI 之外的目标 calling convention，并在不支持的 target 上给出明确 gate/诊断。
  - extern side table / HIR / LLVM codegen 为符号保存 calling convention 信息，不再写死为 C ABI。
  - 至少用 compile-only / emit-llvm fixtures 锁定 calling convention 的前端与后端映射。
- 验收：
  - 新增 fixtures：非 C ABI extern 声明、目标不支持时的诊断、`--emit-llvm` calling convention 检查。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：无
