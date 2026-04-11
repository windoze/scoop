# Scoop：近期计划（从 2026-04-11 起）

> 说明：本文件是新的短版计划，只记录“接下来要做的新任务”。历史计划与已完成事项请看 `PLAN-2.md` / `TODO-2.md`。  
> 范围：本轮只聚焦 `ISSUES.md` 中确认仍存在的语言特性 / 编译器实现缺口，不扩 `stdlib/` / `sysroot/` 的下一阶段 API 面。

## 0. 工作原则（短版）

- 先补语义主链路，再做体验与优化；禁止只在 parser/typecheck 放行而 HIR/MIR/LLVM 仍靠 `Todo` 或早期门禁兜底。
- 本轮不把 `fs/io/net/path/collections/channels/task` 等 stdlib 完整性问题混入主线；只有直接阻塞语言语义落地的 runtime/ABI 变更才进入计划。
- 以 fixtures 为主要验收方式：每项任务至少补一组正/反向回归；effect / task / GC 组合场景必须补 `--gc-stress` 或等价压力覆盖。
- 默认不新增“仅为库方便而设”的编译器 intrinsic；如需新增 runtime 或 ABI 入口，必须证明它解决的是语言语义缺口。
- 任务按“小步可回归”拆分：优先收口表示层与不变量，再扩展 lowering/codegen/runtime，最后补端到端回归矩阵。

## 0.1 常用验收命令

```bash
cargo test --all
cargo run -p scoop_tools -- spec-fixtures check
cargo run -p scoop -- test
```

LLVM 端到端（本机需 `clang` + `llvm-config`）：

```bash
cargo run -p scoop --features llvm -- test
```

## 1. Effect / Continuation 完整化（T20）

- 背景：当前 effect / continuation 仍是“能跑部分样例”的 v0 子集，`handle` arm 形态、non-resuming payload ABI、escape continuation 的 call-site suspension ABI，以及 immediate-resume + `finally` 组合语义都还有明显门禁。
- 目标：
  - 收口 `handle` arm 的类型系统与 HIR 表示，允许合法的 non-resuming / immediate-resume / continuation-binder 组合，不再靠“先拒绝混用”维持实现简单。
  - 让 non-resuming effect 与 escape continuation 的恢复值传递不再硬编码在 word-sized `Int` 分支；跨函数路径与 aggregate/reference payload 走统一语义。
  - 补齐 immediate-resume 在表达式位置、控制流位置、`finally` 组合下的行为，并用复杂 fixtures 锁住语义。
- 当前进展：
  - T2001 已完成：typecheck/HIR 已允许 mixed arms，`handle` 结果类型按真实返回路径检查，HIR/fixtures 已补齐三类 arm 的稳定回归。
  - 已对原 `T2002` 做范围审计：它同时覆盖 non-resuming payload、escape continuation 的 CalleeSuspendState、以及 `resume(...)` ABI 收口，单轮实现风险过高，已拆为 `T2002a` / `T2002b` 两步推进。
  - T2002a 已完成：runtime perform slot 已增加 `gc_ref` 通道并负责 pin/unpin；non-resuming perform / handler 与 `Continuation.resume` 已共享一套 payload encode/decode helper；新增 String/struct direct+indirect run-pass 回归已通过。
  - T2002b 已完成：`CalleeSuspendState` 已升级为双通道 payload；top-level function / closure 的 resume path 与 escape continuation 间接 perform step 均已对齐 `decode_abi_payload_transport` / `resume_gc_ref` 语义；新增间接 `resume(String)` / `resume(struct with ref)` 回归已通过。
  - 已对原 `T2003` 做范围审计：它同时包含 immediate-resume 的单-perform cleanup、控制流内 perform 恢复，以及 mixed-arm / nested handle / GC stress 回归矩阵，单轮实现与验收面过大，现拆为 `T2003a` / `T2003b*` / `T2003c`。
  - T2003a 已完成：现有单-perform immediate-resume lowering 已支持 `finally`，并在正常 resume、arm raise、resume 后间接 raise 三条路径上保持 cleanup 恰好一次。
  - 已进一步审计 `T2003b`：block / branch / while 的 CFG 形状差异明显，单轮同时推进会把扫描、恢复点、诊断三类改动耦合在一起；现再拆为 `T2003b1`（nested block）、`T2003b2`（if/branch）、`T2003b3`（while + 诊断收口）。
  - T2003b1 已完成：immediate-resume 现已支持 statement-position nested block 中的单 direct perform；resume 会先继续 block tail，再回到外层 handle body，并复用 perform 前 block locals。
  - T2003b2 已完成：immediate-resume 现已支持 statement-position `if` then/else branch 中的单 direct perform；命中分支会进入 arm/resume state machine，未命中分支会按普通控制流直接完成 handle。
  - T2003b3 已完成：immediate-resume 现已支持 statement-position `while` body 中的单 direct perform；resume 后会完成当前迭代尾部并可在后续迭代再次拦截同一 perform，且对 `while` condition / nested perform 形状给出稳定 `unsupported_main_body` 诊断。
  - 审计 `T2003c` 时发现新的前置缺口：LLVM `codegen_handle_expr` 仍在 `handle.arms.len() != 1` 时直接报 `handle arm count (only 1 supported)`，因此 mixed-arm immediate-resume 还不是“仅缺回归”的状态。
  - 已将原 `T2003c` 拆成 `T2003c0` + `T2003c`：先补多 arm LLVM dispatch 最小能力，再补 mixed-arm / nested handle / GC stress 回归矩阵。
  - 进一步审计 `T2003c0` 后确认，它同时跨越 immediate-resume 栈 state machine、non-resuming dispatch，以及 escape-continuation suspension/captured-handler-stack 三条 lowering；单轮一并落地风险过高，因此继续拆成 `T2003c0a` / `T2003c0b`。
  - T2003c0a 已完成：mixed-arm lowering 已支持“一个 immediate-resume arm + sibling non-resuming arms”的最小子集；`Raise.raise` 与单 payload custom non-resuming effect 都可以在同一个 source-handle 内参与 dispatch，且 arm body 执行期间会把同一 source-handle 的 custom sibling handler frames 从 TLS handler stack 中摘除，避免 sibling self-capture。
  - 已进一步审计 `T2003c0b`：它同时覆盖 direct/indirect perform、多 perform 点与 richer mixed 组合；若继续整包推进，风险面会再次跨越 direct interception、flag-dispatch 与 continuation captured-handler-stack 三层实现，因此继续拆成 `T2003c0b1` / `T2003c0b2`。
  - T2003c0b1 已完成：mixed-arm lowering 现已支持“一个 immediate-resume arm + 一个 sibling escape-continuation arm”的 direct single-site 子集；当前要求 immediate site 与 escape site 都是 top-level `val = perform`，并让 continuation step 恢复 pre-escape outer/body captures、在 `resume(...)` 后继续执行 escape site 之后的 top-level tail。
  - 已进一步审计 `T2003c0b2`：它仍同时覆盖 indirect 单站点与 richer mixed 组合；现继续拆成 `T2003c0b2a` / `T2003c0b2b`，先落地 indirect single-site，再扩展更复杂 mixed 形状。
  - T2003c0b2a 已完成：mixed-arm lowering 现已支持“一个 immediate-resume arm + 一个 sibling escape-continuation arm + 一个 top-level indirect call site”的最小子集；continuation step 会把双通道 resume payload 写回 callee suspend state，并在 `resume(...)` 后重新调用 callee、继续执行 source-handle 的 top-level tail。
  - 同时已补稳定诊断：`direct + indirect sites not yet supported`、`multiple indirect call sites not yet supported`、`indirect perform before immediate site not yet supported`。
  - 审计 `T2003c0b2b` 后确认它其实跨了两类不同难度的问题：一类是 immediate site 之后的 top-level site matrix（multiple direct / multiple indirect / direct+indirect），另一类是 pre-immediate escape site，它要求 continuation step 在恢复后仍能重新进入 sibling immediate-resume state machine。继续整包推进会把两个状态机问题耦合在一起，因此再拆成 `T2003c0b2b1` / `T2003c0b2b2` / `T2003c0b2b3`。
  - T2003c0b2b0 已完成：`codegen_immediate_resume_top_level_tail_and_finalize` 现已为 tail 中表达式显式传递 expected type（最后一个表达式为 `Some(out_ty)`，其它表达式为 `Some(Unit)`），因此“outer immediate-resume + inner single-arm escape handle tail”的手写等价程序不再在 LLVM codegen 报 `value coercion`。
  - 已新增 run-pass 回归 `effect_resume_nested_escape_handle_tail`，锁住 nested escape-handle 作为 immediate-resume tail 最终结果表达式的最小路径。
  - 尝试推进 `T2003c0b2b1` 时又发现一个更底层的前置缺口：single-arm escape-continuation 的 multiple direct-site 路径虽然已能覆盖 `Unit` 结果 handle，但“non-Unit handle result + multiple direct sites”最小样例仍会在 LLVM codegen 报 `unknown local value` / `value coercion`。
  - 因此把原计划再插一层前置子任务 `T2003c0b2b0c`：先补 single-arm escape-continuation 多 direct site 的非 `Unit` 结果 lowering，再回到 mixed-arm post-immediate multiple direct sites。
  - T2003c0b2b0c 已完成：single-arm escape-continuation 的 multi-perform step trampoline 现已支持 pointer-like enum outer capture（按 `gc_ref` 通道存入/恢复 ContState），因此“outer immediate-resume tail + inner escape handle + multiple direct sites + non-`Unit` result”不再在第二次 direct perform 的 arm codegen 上报 `unknown local value`。
  - 已新增 run-pass 回归 `effect_resume_nested_escape_handle_tail_multi_perform_nonunit`，覆盖 inner escape handle 在 step trampoline 中再次进入 arm 时读取 pointer-like enum 外层局部、并继续推进后续 direct site 的最小路径。
  - T2003c0b2b1 已完成：mixed-arm direct sibling escape lowering 现已支持 immediate site 之后的多个 top-level direct escape sites；state 已新增 `pc` 字段，step trampoline 可在每次 `resume(...)` 后继续推进到后续 sibling escape site 或 tail 完成，并统一复用 `EscapeCaptureStorageKind` 的 `word / gc_ref` capture 协议。
  - 已新增 run-pass 回归 `effect_resume_mixed_escape_direct_multi`，覆盖 post-immediate multiple direct sites、第一次 escape 恢复值跨第二次 suspension 的 body-lift，以及 arm 内 pointer-like enum outer capture；旧的 build-fail `effect_resume_mixed_escape_is_error` 已移除。
  - T2003c0b2b2 已完成：mixed-arm escape sibling lowering 现已新增统一的 post-immediate site matrix 路径，把 top-level direct / indirect sites 接到同一条 continuation step trampoline 上；现已覆盖 multiple indirect sites 与 direct/indirect 两种混排顺序。
  - 已新增 run-pass 回归 `effect_resume_mixed_escape_indirect_multi`、`effect_resume_mixed_escape_direct_indirect`、`effect_resume_mixed_escape_indirect_direct`，并把旧的 post-immediate build 负例替换为 `effect_resume_mixed_escape_pre_immediate_direct_indirect_is_error`，继续锁住 pre-immediate 边界。
  - T2003c0b2b3 已完成：mixed-arm site-matrix lowering 现已支持 pre-immediate top-level direct / indirect escape sites；continuation step trampoline 可在恢复 pre-immediate escape site 之后重新命中 sibling immediate-resume site，并在 immediate arm `resume(...)` 后继续 replay 剩余 top-level tail 与 post-immediate escape sites。
  - 已新增 run-pass 回归 `effect_resume_mixed_escape_pre_immediate_direct`、`effect_resume_mixed_escape_pre_immediate_indirect`，并把旧的 top-level pre-immediate 负例替换为 nested-shape 负例 `effect_resume_mixed_escape_pre_immediate_nested_is_error`。
  - 已审计 `T2003c0b2c`：确认它同时跨越 nested direct-site replay、while 重入，以及 nested indirect call-site suspension 三类不同实现问题，单轮风险过高，因此继续拆成 `T2003c0b2c1` / `T2003c0b2c2` / `T2003c0b2c3`。
  - 继续审计 `T2003c0b2c1` 后确认，`nested block` 与 `if branch` 的 replay 复杂度也不对称：前者主要是顺序前缀/尾部 replay，后者还需处理双分支拦截与 CFG 合流。因此再拆成 `T2003c0b2c1a` / `T2003c0b2c1b`。
  - T2003c0b2c1a 已完成：mixed-arm site matrix 现已支持 statement-position nested block 中的 direct sibling escape site，覆盖 pre/post-immediate 两侧 replay；block-local body capture/lift 也已接入 continuation state。
  - T2003c0b2c1b 已完成：mixed-arm site matrix 现已支持 statement-position if then/else branch 中的 direct sibling escape site；step trampoline、pre-immediate state0 与 post-immediate state1 现已共享条件分派 helper，在命中分支 replay branch tail、未命中分支顺序执行后统一回到 after-if top-level tail。
  - 已新增 run-pass 回归 `effect_resume_mixed_escape_pre_immediate_if`、`effect_resume_mixed_escape_post_immediate_if`，并把旧的 nested-if 负例替换为 while 负例 `effect_resume_mixed_escape_while_is_error`，继续为 `T2003c0b2c2` 锁住 while body 边界。
  - 继续审计 `T2003c0b2c2` 后确认，“while body 的 direct site”还跨了两个不同难度的子问题：flat while-body direct site，以及 while 内再嵌 block / if 的 nested direct site。二者都需要 loop re-entry，但后者还额外需要 nested path replay，因此现再拆成 `T2003c0b2c2a` / `T2003c0b2c2b`。
  - T2003c0b2c2a 已完成：mixed-arm escape site matrix 现已支持 while body 中的 flat direct sibling escape site；state0、state1 与 continuation step trampoline 都会在 `resume(...)` 后先完成当前迭代尾部、重新检查 loop condition，并在后续迭代中再次命中同一个 sibling escape site。
  - 已新增 run-pass 回归 `effect_resume_mixed_escape_pre_immediate_while`、`effect_resume_mixed_escape_post_immediate_while`，并把 while 负例 `effect_resume_mixed_escape_while_is_error` 更新为 nested direct 诊断，继续为 `T2003c0b2c2b` 锁住 while nested path 边界。
  - T2003c0b2c2b 已完成：mixed-arm while site matrix 现已支持 while body 中的 nested block / nested if direct sibling escape site；首次命中 direct site 时可进入 nested path 前缀，`resume(...)` 后也会先 replay 命中的 nested path 尾部，再执行当前迭代余下语句、loop condition 与后续迭代。
  - 已新增 run-pass 回归 `effect_resume_mixed_escape_pre_immediate_while_nested_block`、`effect_resume_mixed_escape_post_immediate_while_nested_if`，并把 while 负例 `effect_resume_mixed_escape_while_is_error` 更新为更深层 nested 诊断，继续为 `T2003c0b2c3` / 更后续形状锁住边界。
  - 继续审计 `T2003c0b2c3` 后确认，nested indirect 仍同时跨越 statement-position nested block、if branch、while body 三类 CFG，以及 nested direct / indirect 共存矩阵。若继续整包推进，需要同时修改 site 扫描、resume-path 表示、callee suspend state replay 与 loop re-entry，单轮风险过高，因此继续拆成 `T2003c0b2c3a` / `T2003c0b2c3b` / `T2003c0b2c3c` / `T2003c0b2c3d`。
  - T2003c0b2c3a 已完成：mixed-arm escape `site matrix` 现已支持 statement-position nested block 中的 indirect call site；indirect site 现会携带 `resume_path`，并让 state0 / state1 / continuation step 共享 nested block prefix / replay / tail helper。
  - 已新增 run-pass 回归 `effect_resume_mixed_escape_pre_immediate_block_indirect`、`effect_resume_mixed_escape_post_immediate_block_indirect`，并新增 build 负例 `effect_resume_mixed_escape_if_indirect_is_error`，继续为 `T2003c0b2c3b` 锁住 if-branch nested indirect 边界。
  - 当前下一步调整为 `T2003c0b2c3b`：继续补 sibling escape-continuation 在 if then/else branch 中的 indirect call site；后续再扩到 while 与 nested direct+indirect matrix。
  - 另已确认一个不阻塞 `T2003c` 主链、但必须在其后统一收口的前端缺口：当前 parser 仍把 `;` 仅当可选分隔符，statement-position block、tail expr 与 trailing lambda / multiple trailing lambdas 的边界都不够清晰。
  - 原 `T2004` 的“只补裸 block 语法”方案已不再单独推进；后续改由新的 `T22` 统一承接：Rust 风格分号 / expression statement 语义、effect fixtures 去 `@Safe` workaround，以及规范 / 文档同步。
- 落地顺序：
  - T2001（已完成）：统一 arm 形态与 typecheck/HIR 不变量。
  - T2002a（已完成）：non-resuming 单 payload ABI 泛化（direct + indirect perform）。
  - T2002b（已完成）：escape continuation / CalleeSuspendState 恢复值 ABI 泛化。
  - T2003a（已完成）：补齐单-perform immediate-resume 的 `finally` cleanup 语义。
  - T2003b1（已完成）：扩展 immediate-resume 到 nested block 中的单个 direct perform。
  - T2003b2（已完成）：扩展 immediate-resume 到 if/branch 中的 direct perform。
  - T2003b3（已完成）：扩展 immediate-resume 到 while 中的 direct perform，并收口剩余稳定诊断。
  - T2003c0a（已完成）：补 mixed-arm immediate-resume + sibling non-resuming 所需的 LLVM 多 arm handle dispatch 最小能力。
  - T2003c0b1（已完成）：把 sibling escape-continuation arm 接入多 arm dispatch 的 direct single-site 子集，并补稳定诊断。
  - T2003c0b2a（已完成）：扩展 sibling escape-continuation 到 single indirect site。
  - T2003c0b2b0（已完成）：补 immediate-resume tail 中 nested handle result lowering。
  - T2003c0b2b0c（已完成）：补 single-arm escape-continuation 多 direct site 的非 `Unit` 结果 lowering。
  - T2003c0b2b1（已完成）：扩展 sibling escape-continuation 到 post-immediate multiple direct sites。
  - T2003c0b2b2（已完成）：扩展 sibling escape-continuation 到 post-immediate indirect/direct+indirect site matrix。
  - T2003c0b2b3（已完成）：扩展 sibling escape-continuation 到 pre-immediate top-level sites。
  - T2003c0b2c1a（已完成）：补 sibling escape-continuation 在 nested block 中的 direct site。
  - T2003c0b2c1b（已完成）：补 sibling escape-continuation 在 if branch 中的 direct site。
  - T2003c0b2c2a（已完成）：补 sibling escape-continuation 在 while body 中的 flat direct site。
  - T2003c0b2c2b（已完成）：补 sibling escape-continuation 在 while body 中的 nested direct site。
  - T2003c0b2c3a（已完成）：补 sibling escape-continuation 在 nested block 中的 indirect site。
  - T2003c0b2c3b：补 sibling escape-continuation 在 if branch 中的 indirect site。
  - T2003c0b2c3c：补 sibling escape-continuation 在 while body 中的 indirect site。
  - T2003c0b2c3d：收口 sibling escape-continuation 的 nested direct/indirect site matrix。
  - T2003c：补 mixed-arm / nested handle / GC stress 回归矩阵。
  - T22：补前端 Rust 风格分号 / expression statement 语义，收口 block / trailing lambda 边界，并同步 effect fixtures 与规范文档。

## 2. 语句语义 / Rust 风格分号规则（T22）

- 背景：Scoop 不保留换行语义，却已支持 Kotlin-like trailing lambda 与 multiple trailing lambdas；当前 `;` 仍只是可选分隔符，block tail 也尚未按 Rust 区分 terminated expr stmt 与 tail expr。这让 statement-position block、trailing lambda 与 effect nested-block fixtures 的边界不够稳定。
- 目标：
  - 引入 Rust 风格语句终止与 expression statement 语义：显式 `;` 切断前一条表达式，block 只有未 terminated 的 tail expr 才产生值。
  - 保持 trailing lambda / multiple trailing lambdas 作为 call 后缀语法不回归，并用显式 statement boundary 区分后续 block statement。
  - 把 effect fixtures 中仅用于 nested block 的 `@Safe` workaround 切回 plain block，并同步更新 `SCOOP_FULL_SPEC.md` / doctest fixtures / 相关说明文档。
- 落地顺序：
  - T2201：parser / AST 引入 Rust 风格语句终止规则与 `{}` / trailing lambda 消歧。
  - T2202：typecheck / HIR 收口 expression statement / block tail value 语义。
  - T2203：effect / regression fixtures 切到 plain block + semicolon fence。
  - T2204：spec / doc / doctest sync。

## 3. Structured Concurrency / `Task<T>`（T21）

- 背景：`spawn` / `join` 已有语法外壳，但执行链路仍按 `Int` 句柄与 `Int` 结果建模，不是真正的 `Task<T>`。
- 已确认的后端缺口：
  - HIR lowering / block rewrite 仍写死 `__scoop_task_spawn_int` / `__scoop_task_join_int`，并把 `spawn` body 包成 `Int -> UInt` 句柄路径。
  - sysroot task glue 仍以 `Task<Int>` / `Continuation<Int>` / `Executor.await(Task<Int>)` 为固定表面，而不是与 `Task<T>` 一起泛型化。
  - LLVM / runtime 仍走 `scoop_task_spawn_int` / `scoop_task_join_int`、`ScoopTaskU64`、`resume_u64` 这套 `u64` 单载荷模型。
- 目标：
  - typecheck / HIR 以 `Task<T>` 为真实模型，不再要求 `spawn { ... }` body 可赋给 `Int`。
  - `join` / 后续 `await` 相关链路统一返回 `T`，并能覆盖 scalar / ref / aggregate 结果类型。
  - runtime / LLVM 路径对齐 continuation payload 的通用传值方案，避免再引入新的 `Int` 特判。
- 落地顺序：
  - T2101：去掉 typecheck / HIR 中的 `Int` 硬编码。
  - T2102：去掉 `spawn/join` 语法糖与 sysroot glue 的 `_int` 专用路径。
  - T2103：补齐 LLVM 侧 `Task<T>` 结果 transport 与 `join` codegen。
  - T2104：补齐 runtime executor / `onComplete` / `await` 的泛型 payload 模型。
  - T2105：用结构化并发 fixtures 锁定语义边界与 GC 行为。

## 4. Lambda 推断与调用语义补齐（T23）

- 背景：lambda 已具备基础语法，但 expected-type 向下传播仍只覆盖 0/1/2 参数，receiver lambda 体内也还拿不到 `this`；调用规则在函数值 / funptr / ctor delegation 上仍有分叉。
- 目标：
  - expected function type 的传播与检查扩展到任意参数个数，并覆盖变量初始化、返回语境、调用实参等常见上下文。
  - receiver lambda 体内自动注入 `this` / 成员查找环境，消除“类型系统可表示、lambda 内却不可用”的断层。
  - 统一函数值调用、函数指针调用、`super(...)` / `this(...)` 构造器委托调用的实参匹配规则，消除命名实参与 receiver function type 的早期门禁。
- 落地顺序：
  - T2301：扩 expected-type propagation。
  - T2302：receiver lambda 的 `this` 语义。
  - T2303：调用语义统一与命名实参放行。

## 5. 泛型约束 / Pattern / 值类型能力补齐（T24）

- 背景：当前缺口集中在三个方向：`where` bound 只能写浅层 nominal type、pattern 在顶层与 `when` or-pattern 上仍不一致、值类型声明与 `with` 更新仍有硬限制。
- 目标：
  - 支持带类型实参的 nominal `where` bound，并让约束在实例化处、函数体内方法分发、诊断文本上保持一致。
  - 让顶层 `val` 可复用既有 pattern binding 语义；`when` or-pattern 可在 binder 集一致时共享绑定。
  - 补齐值类型声明与更新主路径：`struct` 字段默认值 / `var` 支持、`with` 的 base 类型扩展、嵌套路径更新 desugar。
- 落地顺序：
  - T2401：`where` nominal bound with type args。
  - T2402：顶层 pattern binding。
  - T2403：`struct` 字段模型（`var` / 默认值）。
  - T2404：`with` 更新扩展到更完整的值类型语义。
  - T2405：`when` or-pattern binder。

## 6. `const fun` 与 MIR 完整化（T25）

- 背景：`const fun` 目前仍停留在“纯函数签名”最小子集；MIR 路径里常见表达式与 effect 结构仍大量落到 `Todo`，还不能作为稳定的中端基础。
- 目标：
  - 放宽 `const fun` 在 effect row / `eff` 参数上的声明层门禁，改为“声明可表达、调用可验证、真正不支持的组合给出后置诊断”。
  - 消除 MIR 在 struct/tuple/interpolated string/member access/call/cast/type check 等常见路径上的 `Todo` 占位。
  - 为 `perform` / `handle` 提供最小但真实的 MIR 表示，使 MIR 至少可以承担结构化验证与后续优化入口，而不是纯占位视图。
- 落地顺序：
  - T2501：`const fun` 签名模型扩展。
  - T2502：常见表达式 MIR lowering 去 `Todo`。
  - T2503：effect/control-flow MIR lowering 去占位。

## 7. 低优先级：Annotation / FFI ABI（T26）

- 说明：`ISSUES.md` 第 9、10 点不阻塞当前主线，统一放到文件末尾；只有前述主线进入稳定回归后再推进。
- 目标：
  - annotation class 从 data-only 子集扩展到更完整的声明模型。
  - `@CallingConvention` 与 extern side table 不再只认 C ABI，为后续宿主互操作预留可验证的 ABI 表达能力。
- 落地顺序：
  - T2601：annotation class richer model。
  - T2602：FFI / calling convention 扩展。
