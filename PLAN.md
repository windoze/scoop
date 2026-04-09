# Scoop：近期计划（从 2026-04-07 起）

> 说明：本文件是新的短版计划，只记录“接下来要做的新任务”。历史计划与已完成事项请看 `PLAN-1.md` / `TODO-1.md`。

## 0. 工作原则（短版）

- 先语义正确、再做优化；每个优化都必须**可开关、可解释、可验证**。
- 以 fixtures 为主要验收方式；尽量用 `scoop test` 回归，不靠手工跑样例。
- 默认不新增编译器 intrinsic；如确实需要，必须先走 `RUNTIME_STDLIB_INTRINSIC_AUDIT.md` 的 gate 过程。
- 任务必须可拆成小步：单独实现、单独验收、单独回滚。

## 0.1 常用验收命令

```bash
cargo test --all
cargo run -p scoop_tools -- spec-fixtures check
cargo run -p scoop -- test
```

LLVM 端到端（本机需 `clang` + `llvm-config`；目标对齐 LLVM 21）：

```bash
cargo run -p scoop --features llvm -- test
```

## 0.2 工具链基线：LLVM 21（对齐 Rust stable）

- 状态：已将 LLVM 后端基线对齐到 LLVM 21.1（inkwell `llvm21-1` / llvm-sys 211），并在 `scoopc` build script 中校验 `llvm-config` 版本，避免不同机器版本导致行为漂移。
- 备注：后续涉及 LLVM codegen / run-pass / Cone build 的任务，默认都以 LLVM 21.1 为前置假设（见 TODO T0101）。

## 0.3 LLVM codegen 维护性重构（T0102）

> 背景：`crates/scoopc/src/llvm/codegen.rs` 已增长到 20K+ 行，定位与回归成本过高。该任务按“行为不变”的原则拆成可回归的小步（见 TODO：T0102a~T0102e）。

- DONE（T0102a）：模块骨架 + 抽出 `types.rs`（先搬 `CgTy/CgValue/enum layout` 等共享类型/常量）
- DONE（T0102b）：抽出 runtime ABI glue（runtime 符号声明/调用约定；新增 `runtime_abi.rs`/`runtime_symbols.rs`）
- DONE（T0102c）：抽出 type/layout lowering（niche/boxing/field GEP 等）
- DONE（T0102d）：抽出 expr/stmt/control-flow codegen
- DONE（T0102e）：抽出 effect/continuation/GC/statepoint 相关逻辑（新增 `effect.rs`/`gc.rs`）

## 0.4 HIR lowering 维护性重构（T0103）

> 背景：`crates/scoopc/src/hir/lower.rs` 已增长到 6K+ 行；该任务按“行为不变（dump-hir/fixtures 输出稳定）”的原则拆成可回归的小步（见 TODO：T0103a~T0103e）。

- DONE（T0103a）：`lower` 模块骨架 + 抽出 `types.rs`（共享类型与 side tables）
- DONE（T0103b）：抽出 `util.rs`（通用 helper / early-stage 特判收拢；新增 `crates/scoopc/src/hir/lower/util.rs`）
- DONE（T0103c）：抽出 `expr.rs`（表达式 lowering；新增 `crates/scoopc/src/hir/lower/expr.rs`）
- DONE（T0103d）：抽出 `stmt.rs`/`block.rs`（语句与块 lowering）
- DONE（T0103e）：抽出 `sugar.rs`/`patterns.rs`（语法糖与模式相关 lowering）

## 0.5 typecheck expr 维护性重构（T0104）

> 背景：`crates/scoopc/src/typecheck/expr.rs` 已增长到 10K+ 行，导航与回归成本过高。该任务按“行为不变”的原则拆分为职责清晰的子模块。

- DONE（T0104）：拆分为 `crates/scoopc/src/typecheck/expr/mod.rs` + `entry.rs`/`infer.rs`/`call.rs`/`ops.rs`/`member.rs`/`stmt.rs`/`collect.rs`/`util.rs`/`error.rs`（入口/推导/调用/语句/收集/工具分层）

## 1. Cone（改进项吸收）

- 已产出设计：`CONE-IMPROVEMENTS.md`（目录结构 / build 产物 / profile / 增量构建路线）。
- 近期落地重点（见 TODO T1120~T1124）：
  - `scoop new`（DONE）：生成 `.gitignore`（忽略 `build/`）+ `src/main.scoop` 默认包含 `println`
  - `scoop build`（DONE：T1121/T1122）：支持 `--debug/--release`；默认写入 `build/debug/bin/<project-name>`，`--release` 写入 `build/release/bin/<project-name>`；中间产物进入 `build/<profile>/obj/`（并预留 `build/<target>/<profile>/…`）
  - `scoop run`（DONE：T1123）：在项目目录下“未构建则先构建再运行”，并支持 `--debug/--release`
  - incremental（DONE：T1124；增补：T1601）：写入 `build/<profile>/build.json` 记录 fingerprint（含 `opt_level`）；未变化且产物存在时打印 `skipping build (cache hit)` 并复用；可用 `--no-incremental` 或 `SCOOP_INCREMENTAL=0` 禁用；细粒度依赖图后置（v2）

## 2. Scoop 编译器优化（优化等级/去虚化/HIR-MIR）

### 2.1 优化等级（对外接口 + 默认策略）

- DONE（T1601）统一对外接口：
  - CLI：`scoop build/run/test` 支持 `-O/--opt-level <0|1|2|3|s|z>`。
  - manifest：`Cone.toml[native-build].opt-level`（支持字符串或整数），并定义优先级：CLI > toml > profile 默认值。
  - 默认策略：debug → `-O0`，release → `-O2`。
- DONE（T1601）LLVM 后端对齐：`TargetMachine` 的 `OptimizationLevel` 与 opt-level/profile 对齐（O0→None，O1→Less，O2/Os/Oz→Default，O3→Aggressive）。

### 2.2 LLVM pipeline（DCE/inlining/unroll 等）

- DONE（T1602）：LLVM 后端按 opt-level 运行 PassBuilder pipeline：
  - `-O0`：仅跑 statepoint rewrite 前置（`sroa,mem2reg`）+ `rewrite-statepoints-for-gc`，尽量保持 IR 可读。
  - `-O1/-O2/-O3/Os/Oz`：先跑 `default<…>`，再跑 statepoint rewrite；rewrite 之后仅做轻量清理（`instcombine,simplifycfg`）。
- 用 `--emit-llvm` + build fixtures（contains/not-contains）提供最小“优化确实发生”的回归证据；build fixtures 允许在 `// ARGS:` 里声明 `-O.../--opt-level` 固定单个 fixture 的优化等级。
- 低复杂度但高收益优先级（建议先接入再微调顺序）：
  - 必备清理：`instcombine`、`simplifycfg`
  - 早期冗余/内存优化：`early-cse`、`dse`、`dce`（必要时 `adce`）、`sccp`
  - release 才考虑更重的：`gvn/newgvn`、`jump-threading`/`correlated-propagation`、`memcpyopt`
- GC/statepoint 约束：
  - 绝大多数优化应放在 `rewrite-statepoints-for-gc` **之前**（避免在 `gc.statepoint/gc.relocate` 之后引入更多 pass 兼容性/排查成本）。
  - `rewrite-statepoints-for-gc` 之后仅做轻量清理（例如再跑一轮 `instcombine,simplifycfg`）。
  - `place-safepoints` 暂不纳入默认管线（在旧的 LLVM 18.1.8 上曾观察到 SIGSEGV；迁移到 LLVM 21 后需要单独验证其稳定性再决定是否接入）。

### 2.3 去虚化（receiver 类型已知时直调用）

- DONE（T1603）：LLVM 后端在 class vtable 调用点做最小去虚化：当 receiver 的静态 class 在编译单元内“无已知子类”时，直接调用该 slot 的 `impl_member_fqn`（不再走 `call_vtable` 间接调用）；并优先使用局部绑定的原始 `TypeId`（`env.local.hir_ty`）避免隐式 upcast 擦除类型导致漏优化。
- value type 默认静态分派（direct call）。
- final/sealed class 在可证明单一目标时生成直调用（或提供足够信息让 LLVM 去虚化）。

### 2.4 HIR/MIR 级优化（cheap wins）

- DONE（T1604）：无 `perform` 的作用域不生成 `handle` 结构/handler 链接，减少 runtime 开销。
- DONE（T1605）：建立“高级优化候选清单”，标注层级/收益/风险与依赖，并为每个候选项保留可拆分的任务入口（OPT-*）。

#### 2.4.1 高级优化候选清单（维护：OPT-*）

> 原则：每个候选项都必须（a）可单独开关，（b）可用 fixtures 提供回归证据，（c）明确与 GC/statepoint/effect 语义的关系与风险点。

**HIR/MIR（中端）**

- OPT-HIR-01：HIR 级常量折叠 + 简单 DCE（保守）
  - 层级：HIR（typecheck 后，lowering 前）
  - 预期收益：减少 IR 噪声；让后续 LLVM pipeline 更容易做 DCE/SCCP；对编译速度也有正向帮助
  - 风险/依赖：必须保守判定副作用（不得消除 `perform` / 可能分配/抛错/IO 的调用）；需要与 effect 系统的“可观察行为”定义对齐
  - 任务入口：OPT-HIR-01（后续可拆：pure 判定 → DCE → constfold）

- OPT-MIR-01：逃逸分析 + 栈上分配（把“明显不逃逸”的短命对象/聚合值从 GC heap 挪到 stack）
  - 层级：MIR 或 lowering→LLVM 边界（生成 `alloca` + mem2reg/SROA 友好形态）
  - 预期收益：显著降低 GC 分配率与压力；提升热点路径性能；减少 statepoint live set
  - 风险/依赖：必须严格排除“可能被 continuation 捕获/跨线程/写入堆容器/返回”的值；与 moving GC 的 root 更新协议要兼容（stack root 必须可枚举）；建议先从“局部 block 内、不含 perform 的范围”做最小可证明落地
  - 任务入口：OPT-MIR-01（后续可拆：逃逸判定 → 可安全栈化的类型集合 → fixtures/IR 断言）

- OPT-MIR-02：更精确的 live-range / root 缩减（降低 `gc.statepoint` 处的 live roots 数量）
  - 层级：MIR（变量生命期）+ LLVM（statepoint 前的 liveness/RA 友好布局）
  - 预期收益：减少 `gc.relocate` 数量与寄存器/栈槽压力；降低 safepoint 开销；对 `--gc-stress` 下的性能回归更敏感
  - 风险/依赖：需要与现有 lowering 的“局部变量绑定/临时值”策略配合；必须用 `--emit-llvm` + build fixtures 锁住关键 IR 形态（避免误优化导致 root 漏扫）
  - 任务入口：OPT-MIR-02（后续可拆：lowering 临时值策略 → liveness 缩减 → build fixtures）

- OPT-MIR-03：中端内联/去包装（把“必经的薄封装函数”内联掉，减少 call 边界）
  - 层级：MIR（自研内联）或 LLVM（通过属性/启发式增强 inlining）
  - 预期收益：减少 call 开销；放大后续 SROA/GVN 的收益；为进一步去虚化创造条件
  - 风险/依赖：会增加编译时间与 code size；与 statepoint rewrite 的顺序要谨慎（大多数内联应发生在 rewrite 前）；需要一个可控开关（例如仅 `-O2+`）
  - 任务入口：OPT-MIR-03（后续可拆：内联白名单/阈值 → 开关策略 → fixtures）

**LLVM（后端）**

- OPT-LLVM-01：更强去虚化（CHA / sealed/final metadata / vcall 优化信息贯穿）
  - 层级：HIR/MIR（类层级信息）→ LLVM（devirt/inlining 生效）
  - 预期收益：进一步减少间接调用；提升 inlining 机会；改善 hot method dispatch 的性能
  - 风险/依赖：需要“全程序可见”的 class hierarchy 视角（至少在单 crate/单编译单元内）；必须定义动态加载/反射（若有）的语义边界；建议先从 cone 单项目单编译单元场景落地
  - 任务入口：OPT-LLVM-01（后续可拆：CHA 数据结构 → codegen metadata → build fixture）

- OPT-LLVM-02：ThinLTO/LTO（release profile 的可选增强）
  - 层级：driver/build（链接阶段）+ LLVM
  - 预期收益：跨模块内联与 DCE；提升性能并可能减小体积
  - 风险/依赖：构建时间与工具链复杂度上升（lld/clang 配置）；需要在 Cone build 目录结构下落一个稳定可重现的 cache/产物策略；建议做成 opt-in（例如 `--lto=thin`）
  - 任务入口：OPT-LLVM-02（后续可拆：本机工具链探测 → CLI 开关 → fixtures/bench）

- OPT-LLVM-03：PGO（profile-guided optimization）工作流（instrument → run → use）
  - 层级：driver/build + LLVM
  - 预期收益：在真实 workload 上显著提升分支预测/内联决策质量；对复杂控制流收益明显
  - 风险/依赖：需要端到端工作流与样例/脚本；CI 运行成本较高；应先做“本地可用”的 v0 并提供文档与可选开关
  - 任务入口：OPT-LLVM-03（后续可拆：instrument 支持 → profile 合并 → use profile）

**Runtime/GC（运行期）**

- OPT-GC-01：分代/新生代（nursery）或 bump-pointer 分配策略（降低 minor GC 成本）
  - 层级：runtime/GC
  - 预期收益：显著降低短命对象的回收成本；降低停顿；提升吞吐
  - 风险/依赖：需要写屏障（card marking / remembered set）与更复杂的 GC 不变量；与多线程 STW/并发分配交互复杂；应以“可回归 fixtures + microbench”逐步推进
  - 任务入口：OPT-GC-01（后续可拆：nursery allocator → barrier → 验证 fixtures）

- OPT-GC-02：多线程下的 TLAB/线程本地分配缓存（减少全局分配锁竞争）
  - 层级：runtime/GC + 多线程 runtime
  - 预期收益：提升多线程分配吞吐；降低锁竞争；为 T1705 的多线程 fixtures 打基础
  - 风险/依赖：需要与 GC safepoint/STW 协议严格对齐；线程结束/抢占时的缓冲区回收要正确；需确定性测试防 flakiness
  - 任务入口：OPT-GC-02（后续可拆：TLAB 设计 → STW 协议 → 多线程 fixtures）

**Effect/Continuation（语义不变前提下的优化）**

- OPT-EFF-01：non-resuming / immediate resume fast-path（减少 continuation/state 分配）
  - 层级：lowering/codegen + runtime dispatch
  - 预期收益：常见 effect（例如错误传播、简单的同步 handler）可避免 heap state 机；降低 GC 压力与调度开销
  - 风险/依赖：必须以统一 dispatch 语义为前提（见 TODO：T1608），避免“特判路径”导致 active/inactive 规则失真；需要用嵌套 handler fixtures 做强回归
  - 任务入口：OPT-EFF-01（后续可拆：识别可 fast-path 的 handler → codegen → fixtures）

### 2.5 Effect/Continuation 完整语义（正确性优先：多次 suspend/resume）

> 背景：当前 escape continuation（`, k ->`）在 LLVM 后端仍是“最小可回归链路”，存在关键限制（例如单个 perform 点/位置约束/payload 受限），导致 stdlib 与 fixtures 需要手写 workaround（嵌套 handle/二段 handle），无法验证真实 async/await 的完整语义。

- 目标（见 TODO：T1606~T1612）：
  - `handle` body 支持 0..N perform 点：同一计算可经历多次 suspension/resume（每个 continuation one-shot）。
  - 统一 dispatch/unwind：以 runtime handler stack + perform slot 为单一语义基座（避免多套“特判”长期并存）。
  - resume payload 泛型化：`Continuation<T>` 的 `T` 覆盖 value/ref/复合类型，且与 moving GC 对齐（不允许 ptr<->int 作弊）。
  - 控制流表达式返回任意类型：`handle/if/when` 作为表达式支持 `tuple/struct/enum` 等复合值返回，避免后端“只能返回标量子集”的限制（见 TODO：T1610）。
  - 语句位置的 `handle` 不再需要 `val _: Unit = ...` workaround：后端在表达式语句位置默认以 `Unit` 期望类型生成并丢弃结果（见 TODO：T1611）。
  - `Nothing` 明确为 bottom type：运行时没有值；返回类型为 `Nothing` 的函数不会正常 return。后端若需要占位表示它，也只能用于不可达路径的 IR 连通，且该值永不可被观察（见 TODO：T1612）。**DONE**：引入 `CgTy::Never` + `CgValue::never()`；30+ match arms 全面覆盖；零大小布局；emit_return 发出 `unreachable`；coerce_value 处理 `(Never, _)` → `default_value(target)`；新增 3 个 run-pass fixtures。
  - `finally` 组合语义补齐：在 suspend/resume/传播路径上不漏执行、不重复执行。
- 落地顺序（T1606 已拆分为子任务 T1606a~T1606g；T1608/T1607 前置）：
  - T1606a（DONE）：0 perform 时退化执行 body（arm 不可达）
  - T1606b（DONE）：取消”perform 必须首语句”（补齐 capture/lift）
  - T1606c（DONE）：多 perform（pc + heap state machine）— 含 GC stackmap walker 修复：walk-through C frames 以覆盖 main→resume_u64(C)→step_fn 场景
  - T1608（DONE）：op_tag 稳定分配与统一 dispatch — T1606d 的前置依赖
  - T1607（DONE）：resume payload 泛型化（u64 → 任意 T）— 双通道 ABI（resume_word + resume_gc_ref），step 函数 3 参数签名，compound 类型 box/unbox
  - T1706（DONE）：多 perform 点回归 fixtures（async/await 真实写法）— 新增 2 个 run-pass fixtures：`effect_escape_continuation_async_executor_fifo`（单线程 executor 模式，3 次 perform，局部变量跨 suspension 累积）+ `effect_escape_continuation_multi_perform_cross_thread`（跨线程 resume + 2 次 perform）；单线程 fixture 在 `--gc-stress` 下稳定；跨线程 + GC stress 为既有限制（见 `effect_escape_continuation_resume_cross_thread` 同样挂起）
  - T1707（DONE）：控制流 + 多次 suspension 回归 fixtures — 新增 3 个 run-pass fixtures：if/else 分支合流（phi 变量跨 suspension 存活）、value/ref 混合 locals 跨多 perform 点（手动展开循环等价形态）、嵌套 handler re-perform + 控制流（active/inactive dispatch 规则）；typecheck 扩展 `infer_block_value_type` 支持 `while` 语句（handle body 块表达式中可使用 while 循环）
  - T1606d（DONE）：多 perform + 动态上下文/GC 回归加固 — 新增 4 个 run-pass fixtures：GC stress multi-string（SCOOP_GC_STRESS=1 下 3 次 String suspend/resume）、arm-performs-outer-effect（escape arm 内 perform 不同 effect 路由到外层 handler）、nested-escape-handlers（两独立 escape-continuation 交叉 resume）、reperform-from-escape-arm（escape arm re-perform 同一 effect 验证 self-capture prevention）
  - T1606e（DONE）：handle body 任意控制流（分支/循环）显式验证
  - T1606f-1（DONE）：间接 perform（non-resuming）— codegen 修复：perform 无本函数内 handler 时回退 flag-propagation；handler 新增 dispatch trampoline（raise_target_stack + op_tag 检查）确保间接 perform 正确路由且 Raise 等其它 effect 不被误捕获。新增 3 个 run-pass fixtures：basic/call-chain/closure，均在 `--gc-stress` 下稳定。
  - T1606f-2（DONE）：间接 perform（call-site suspension）— 两级状态保存设计：callee saves locals to TLS CalleeSuspendState（`{ GcObjectHeader, resume_word, locals... }`），handle body saves captures/lifts to ContState。`codegen_top_level_fun_suspendable` 添加 fresh/resume 双路径入口（TLS 检查）；`emit_callee_suspend_state_save` 在 perform 的 flag-propagation 前分配+PIN+保存 locals 到 TLS；`codegen_handle_expr_escape_continuation_indirect` (~700 行) 生成 ContState + step function + dispatch trampoline。Resume 时 step function 写 resume_word 到 CalleeSuspendState 并重新调用 callee。GC/statepoint 修正：TLS 指针保持 addrspace(0) 避免 statepoint 追踪问题。新增 1 个 run-pass fixture。
  - T1606f-3（DONE）：间接 perform（closure perform + locals linkage）— 闭包内 perform 的 callee-suspend 变换：`codegen_closure_fun_body_suspendable` 为含 perform 的闭包生成 TLS entry check / CalleeSuspendState save-restore 双路径；`collect_used_locals_in_expr_static` 新增 Closure 分支递归收集 captures；body-lift 计算扫描 call-site stmt 本身（step function 重建闭包需要 body locals）；body-lift save 提前到 body 执行流 call-site 前（body scope pop 后不可访问）；step function restore 支持 Ref/String/Bool 类型。新增 2 个 run-pass fixtures（basic closure + multi-type locals），均在 `--gc-stress` 下稳定。
  - T1606g（DONE）：嵌套 handle 显式验证 — 新增 3 个 run-pass fixtures：(1) body→outer（no-perform degenerate path + EffectB flag-propagation）、(2) arm indirect→outer（doFire() 间接 perform EffectB → outer non-resuming catch）、(3) outer resume inner multi-perform state machine（outer body resume k1/k2 推进 inner 2-perform state machine 后 perform EffectB）。修复 CalleeSuspendSaveCtx 未使用字段（perform_binding_id/perform_binding_cg_ty）。均在 `--gc-stress` 下稳定。
  - T1609（DONE）：`finally` + escape continuation — 在 escape continuation handle 的直接/间接 perform 两条 codegen 路径中新增 `finally_bb` + `finally_unwind_bb` 支持。arm body 正常完成 → finally → done；arm body Raise → finally_unwind → outer propagate。0-perform 退化路径已原生支持 `finally`。新增 4 个 run-pass fixtures（normal/multi-perform/no-perform/arm-raise），均在 `--gc-stress` 下稳定。
- 顺序调整说明（2026-04-08）：T1606d 依赖 T1706/T1707 的 fixtures 作为验收基础，但 T1706/T1707 原在 T17（验证套件）中排在 T1606d 之后。由于 T1706/T1707 的前置依赖（T1606a-c、T1608）均已完成，将其提前到 T16 中 T1606d 之前执行，确保依赖顺序正确。
- 设计要点（implementation-level 约束）：
  - handler 分发必须以稳定 `op_tag` 为核心（Appendix A：最近匹配 + active/inactive）；fqn 字符串仅作为诊断输出。
  - escaping continuation 的状态机应以 heap state 表示（pc + lifted locals），并在每次 perform 处生成 continuation；resume 进入 step trampoline 继续推进直到下一次 perform 或完成。
  - GC：state 对象必须写入正确的 type descriptor（trace bitmap / trace_fn），确保 moving/compaction 下 roots 可更新。
- 验收策略（见 TODO：T1706/T1707）：
  - 用“单个 handle 内多次 await”的 fixtures 作为最关键回归点（禁止嵌套 handle workaround）。
  - 控制流（if/when/循环）+ 多次 suspension 组合覆盖；在 `--gc-stress` 下稳定。

## 3. 端到端验证与回归（Continuation/GC/多线程）

> GC 是 Scoop 的生命线：所有高风险特性都必须有“复杂但可回归”的 fixtures，而不是只靠最小 demo 证明能跑。

- Escaping continuation：复杂 fixtures（模拟 async executor/scheduler），并在 `--gc-stress` 下稳定。
  - DONE（T1701）：新增 3 个多任务调度器 fixtures（FIFO/LIFO/round-robin），覆盖多独立任务×多 suspension×不同调度策略，均在 `--gc-stress` 下稳定。
- 关键补齐：单个 `handle` body 内多次 suspension/resume（多 perform 点），覆盖真实 async/await 写法（见 TODO：T1706/T1707）。
- `Continuation<T>` 完整性：覆盖 `T` 的全类型空间（struct/tuple/enum/ref/Continuation 自身）。
  - DONE（T1702）：新增 6 个 run-pass fixtures——struct、tuple(Int,String)、enum(Ok/Err)、ref class(multi-perform)、struct+ref field、Continuation<Continuation<Int>>。Parser 增强：修复嵌套泛型 `>>` 解析（split GtGt → Gt+Gt）。均在 `--gc-stress` 下稳定。
- GC correctness：跨函数复杂对象图、数组（value/ref 混合、value 内含 ref）、循环引用。
  - DONE（T1703）：新增 6 个 run-pass fixtures——struct-with-ref cross-function、class object graph（树结构）、Array<String> cross-function、short-lived/long-lived interleave、deep nested struct/class/ref、enum-with-ref-variant cross-function。4 个在 `--gc-stress` 下稳定；2 个使用显式 `__scoop_gc_collect()`（Array<String>/enum + GC stress 存在已知 double-free 问题）。
- GC + escaping continuation：确保 continuation 捕获环境的 roots 扫描/更新正确。
  - DONE（T1704）：新增 2 个 run-pass fixtures——deep object graph（class chain + struct-with-ref 嵌套，2 次 suspend/resume，graph 扩展后验证 child 链接存活）、alloc-heavy resume（3 次 suspend/resume + 累积 Record 对象 + caller 侧显式 GC collect）。均在 `--gc-stress` 下稳定。
- 多线程扩展：把上述场景搬到多线程，固定调度避免 flakiness。
  - DONE（T1705）：新增 2 个 run-pass fixtures——`gc_continuation_cross_thread_resume_with_objects`（3-node class chain + struct Tag(String, Int)，2 次 suspend/resume 均在新线程中执行，resume 间主线程 GC collect）、`gc_continuation_multi_thread_concurrent_alloc_resume`（两独立 effect handler 各捕获 continuation + String locals，通过 threadSpawn 两个 worker 线程分别 resume，object Shared 共享状态 + 顺序 spawn/join 确保确定性输出，join 后主线程 GC collect 验证线程注册/注销生命周期正确性）。已知限制：SCOOP_GC_STRESS=1 + 跨线程 resume 导致 STW 死锁（worker 阻塞在 native code 无法到达 safepoint），使用显式 `__scoop_gc_collect()` 替代。

## 4. 标准库完整性（基于 `KOTLIN_RUNTIME_GAP_AUDIT.md`）

- DONE（T1801）：产出 `STDLIB_COMPLETENESS.md`，覆盖 21 个能力领域，每个能力项标注状态/分类/实现位置/fixtures。P0/P1 缺口排序：
  1. **Text 基础（P0）**：`String.length`/`substring`/`startsWith`/`split`/`indexOf`/`contains`/`toInt`/`toString` — 当前最大缺口，需 runtime/c 新增 `scoop_string_*` API
  2. **泛型 collections（P0）**：`<T>` 版 forEach/map/filter/fold — 依赖编译器泛型单态化完善
  3. Text 格式化（P1）：StringBuilder/joinToString
  4. Math 基础（P1）：abs/min/max
  5. Hashing（P1）：真实 hash + hash-based Set/Map
  6. Collections 算法（P1）：sort/reduce/zip/flatten
  7. Ranges 增强（P1）：`..` syntax / `until` / for-in integration
  8. Duration/Random/Test utilities（P1）
  - intrinsic 结论不变：无需新增编译器 intrinsic
- DONE（T1802）：将 P0/P1 缺口拆分为 13 个可单独实现/验收的子任务（T1810~T1822）：
  - **P0 Text（最高优先级）**：
    - T1810：runtime/c 新增 `scoop_string_*` 底层 API（length/substring/startsWith/endsWith/indexOf/contains/split）
    - T1811：sysroot 声明 + codegen 路由 + run-pass fixtures
    - T1812：`Int.toString()` / `String.toInt()` 数值转换
  - **P0 Test utilities**：T1813（`assertEqString`/`assertEqBool`）
  - **P1 Math**：T1814（`abs`/`min`/`max` — 纯 Scoop）
  - **P1 Collections 算法**：T1815（`sort`/`reduce`/`zip`/`flatten` — Int 专用）
  - **P1 Text 格式化**：T1816（`StringBuilder`/`joinToString`）
  - **P1 Hashing**：T1817（Int/String hash 实现）→ T1818（Hash-based Set/Map）
  - **P1 Ranges**：T1819（`..` syntax / `until` / `for-in`）
  - **P1 Duration**：T1820
  - **P1 Random/PRNG**：T1821（xorshift64）
  - **P0 泛型 collections**：T1822（依赖编译器泛型 codegen 完善，当前 BLOCKED）
  - 建议实现顺序：T1810 → T1811 → T1812 → T1813 → T1814 → T1815 → …
- DONE（T1813）：Test utilities 扩展——`assertEqString` + `assertEqBool`：
  - `assertEqString`：使用 `length() + startsWith()` 实现等价比较（`String ==` 运算符尚未实现）；失败时打印 expected/actual 后 `Raise.raise`。
  - `assertEqBool`：使用 `require(expected == actual)`（`Bool ==` 已在 typecheck + codegen 中支持）。
  - Fixture：`stdlib_test_assertions_extended.scoop` + `.stdout`（7 个场景 + 组合测试 `Int.toString()` roundtrip）。
  - 下一步：T1814（Math 基础：`abs`/`min`/`max`）。
- DONE（T1814）：Math 基础——`abs`/`min`/`max`（Int）：
  - 新增 `stdlib/math.scoop`（`package scoop.core`）：纯 Scoop 实现，不使用 Int 字面量（零值通过 `sizeOf(x) - sizeOf(x)` 派生）。
  - `abs(x)`：与零比较，负数取 `-x`；`min(a, b)`：`a <= b ? a : b`；`max(a, b)`：`a >= b ? a : b`。
  - Fixture：`stdlib_math_basic.scoop` + `.stdout`（16 个场景：abs 5 + min 5 + max 5 + 组合嵌套 1）。
  - 下一步：T1815（Collections 算法：`sort`/`reduce`/`zip`/`flatten`）。
- DONE（T1815）：Collections 算法——`sort`/`reduce`/`zip`（Int 专用）：
  - `MutableArray<Int>.sort()`：原地选择排序（get/set 交换，O(n²)）。
  - `Array<Int>.reduce(op)` + `MutableArray<Int>.reduce(op)`：从首元素归约，effect-polymorphic。
  - `Array<Int>.zip(other)`：flat interleaved 布局 `[a0,b0,a1,b1,...]`（tuple array 尚不支持，降级方案）。
  - `flatten`：推迟到 T1822（需 `Array<Array<Int>>` codegen 支持）。
  - Fixture：`stdlib_collections_algorithms_basic.scoop`（sort 5 + reduce 4 + zip 3 + 组合 2）。
  - 下一步：T1816（Text 格式化：`StringBuilder` + `joinToString`）。
- DONE（T1816）：Text 格式化——`StringBuilder` + `joinToString`：
  - **`scoop_string_concat`**（runtime/c）：连接两个 ScoopString*，GC-safe（pin a/b before alloc）。注册到 `scoop_runtime_api.h`。
  - **`String.concat(other: String): String`**：完整管线——resolver 白名单 + typecheck（1 arg, returns String）+ codegen dispatch 到 C 函数。
  - **`Array<Int>.joinToString(separator: String): String`**（stdlib/array_iter.scoop）：使用 `concat` + `toString`，空串通过 `substring(zero, zero)` 派生。
  - **StringBuilder v0**：以 `var + String.concat()` 累积模式在 fixture 演示（stdlib 无法定义类/使用字面量）。
  - Fixture：`stdlib_string_builder_basic.scoop`（concat 5 + sb 模式 5 + joinToString 5 + 组合 1 = 16 场景）。
  - 下一步：T1817（Hashing 落地：Int/String hash 实现）。
- DONE（T1810）：Text runtime/c 底层 API：
  - 在 `runtime/c/scoop_runtime.c` 中实现 7 个 `scoop_string_*` 函数（length/substring/startsWith/endsWith/indexOf/contains/split）。
  - 所有函数遵循现有 runtime/c 风格：null check → 边界 clamp → 实际逻辑。
  - `split` 使用 `scoop_array_builder_*` 构建 GC-managed `Array<String>`。
  - 7 个符号按字典序注册到 `scoop_runtime_api.h` 的 X-macro 列表。
  - 下一步：T1811（sysroot 声明 + codegen 路由 + run-pass fixtures）。
- DONE（T1803）：stdlib smoke + matrix fixtures 回归基座：
  - **Smoke fixtures**（3 个）：`stdlib_smoke_collections_and_iteration`（Array/MutableArray/map/filter/fold/Set/Map）、`stdlib_smoke_ranges_and_io`（IntProgression rangeTo/downTo/forEach）、`stdlib_smoke_test_and_preconditions`（assertTrue/assertEqInt/require/check + try/catch）。
  - **Matrix tool**：`fixtures-matrix stdlib` 模式——21 个 stdlib 领域 × fixture 前缀映射；当前 15/21 域有覆盖，6 个缺口（Text formatting / Math / Hashing / Random / Net / Reflection）。
  - 运行：`cargo run -p scoop_tools -- fixtures-matrix stdlib`。
