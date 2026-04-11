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
  - 当前下一步进入 `T2003c0b`：在 shared dispatch 的基础上，把 sibling escape-continuation arm 接入同一个 source-handle，并收口 mixed-arm immediate-resume 当前剩余的不支持组合。
- 落地顺序：
  - T2001（已完成）：统一 arm 形态与 typecheck/HIR 不变量。
  - T2002a（已完成）：non-resuming 单 payload ABI 泛化（direct + indirect perform）。
  - T2002b（已完成）：escape continuation / CalleeSuspendState 恢复值 ABI 泛化。
  - T2003a（已完成）：补齐单-perform immediate-resume 的 `finally` cleanup 语义。
  - T2003b1（已完成）：扩展 immediate-resume 到 nested block 中的单个 direct perform。
  - T2003b2（已完成）：扩展 immediate-resume 到 if/branch 中的 direct perform。
  - T2003b3（已完成）：扩展 immediate-resume 到 while 中的 direct perform，并收口剩余稳定诊断。
  - T2003c0a（已完成）：补 mixed-arm immediate-resume + sibling non-resuming 所需的 LLVM 多 arm handle dispatch 最小能力。
  - T2003c0b：把 sibling escape-continuation arm 接入多 arm dispatch，并收口剩余稳定诊断。
  - T2003c：补 mixed-arm / nested handle / GC stress 回归矩阵。

## 2. Structured Concurrency / `Task<T>`（T21）

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

## 3. Lambda 推断与调用语义补齐（T22）

- 背景：lambda 已具备基础语法，但 expected-type 向下传播仍只覆盖 0/1/2 参数，receiver lambda 体内也还拿不到 `this`；调用规则在函数值 / funptr / ctor delegation 上仍有分叉。
- 目标：
  - expected function type 的传播与检查扩展到任意参数个数，并覆盖变量初始化、返回语境、调用实参等常见上下文。
  - receiver lambda 体内自动注入 `this` / 成员查找环境，消除“类型系统可表示、lambda 内却不可用”的断层。
  - 统一函数值调用、函数指针调用、`super(...)` / `this(...)` 构造器委托调用的实参匹配规则，消除命名实参与 receiver function type 的早期门禁。
- 落地顺序：
  - T2201：扩 expected-type propagation。
  - T2202：receiver lambda 的 `this` 语义。
  - T2203：调用语义统一与命名实参放行。

## 4. 泛型约束 / Pattern / 值类型能力补齐（T23）

- 背景：当前缺口集中在三个方向：`where` bound 只能写浅层 nominal type、pattern 在顶层与 `when` or-pattern 上仍不一致、值类型声明与 `with` 更新仍有硬限制。
- 目标：
  - 支持带类型实参的 nominal `where` bound，并让约束在实例化处、函数体内方法分发、诊断文本上保持一致。
  - 让顶层 `val` 可复用既有 pattern binding 语义；`when` or-pattern 可在 binder 集一致时共享绑定。
  - 补齐值类型声明与更新主路径：`struct` 字段默认值 / `var` 支持、`with` 的 base 类型扩展、嵌套路径更新 desugar。
- 落地顺序：
  - T2301：`where` nominal bound with type args。
  - T2302：顶层 pattern binding。
  - T2303：`struct` 字段模型（`var` / 默认值）。
  - T2304：`with` 更新扩展到更完整的值类型语义。
  - T2305：`when` or-pattern binder。

## 5. `const fun` 与 MIR 完整化（T24）

- 背景：`const fun` 目前仍停留在“纯函数签名”最小子集；MIR 路径里常见表达式与 effect 结构仍大量落到 `Todo`，还不能作为稳定的中端基础。
- 目标：
  - 放宽 `const fun` 在 effect row / `eff` 参数上的声明层门禁，改为“声明可表达、调用可验证、真正不支持的组合给出后置诊断”。
  - 消除 MIR 在 struct/tuple/interpolated string/member access/call/cast/type check 等常见路径上的 `Todo` 占位。
  - 为 `perform` / `handle` 提供最小但真实的 MIR 表示，使 MIR 至少可以承担结构化验证与后续优化入口，而不是纯占位视图。
- 落地顺序：
  - T2401：`const fun` 签名模型扩展。
  - T2402：常见表达式 MIR lowering 去 `Todo`。
  - T2403：effect/control-flow MIR lowering 去占位。

## 6. 低优先级：Annotation / FFI ABI（T25）

- 说明：`ISSUES.md` 第 9、10 点不阻塞当前主线，统一放到文件末尾；只有前述主线进入稳定回归后再推进。
- 目标：
  - annotation class 从 data-only 子集扩展到更完整的声明模型。
  - `@CallingConvention` 与 extern side table 不再只认 C ABI，为后续宿主互操作预留可验证的 ABI 表达能力。
- 落地顺序：
  - T2501：annotation class richer model。
  - T2502：FFI / calling convention 扩展。
