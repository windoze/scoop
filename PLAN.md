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
  - DONE（T1705）：新增 2 个 run-pass fixtures——`gc_continuation_cross_thread_resume_with_objects`（3-node class chain + struct Tag(String, Int)，2 次 suspend/resume 均在新线程中执行，resume 间主线程 GC collect）、`gc_continuation_multi_thread_concurrent_alloc_resume`（两独立 effect handler 各捕获 continuation + String locals，通过 threadSpawn 两个 worker 线程分别 resume，object Shared 共享状态 + 顺序 spawn/join 确保确定性输出，join 后主线程 GC collect 验证线程注册/注销生命周期正确性）。~~已知限制：SCOOP_GC_STRESS=1 + 跨线程 resume 导致 STW 死锁~~，使用显式 `__scoop_gc_collect()` 替代。
  - DONE（T0105）：修复 STW 死锁——所有 runtime/c 中的阻塞系统调用（`pthread_join`、`pthread_cond_wait`、`sleep`）前后添加 `scoop_enter_native`/`scoop_leave_native` 状态转换，使阻塞线程在 STW 期间被跳过。涉及 5 个函数（scoop_thread_spawn_join_resume_u64、scoop_thread_join、scoop_thread_sleep_millis、scoop_sync_condvar_wait、scoop_sync_once_run_blocking、scoop_channels_recv_u64）。SCOOP_GC_STRESS=1 下不再死锁；剩余 GC rooting 问题属 T0106 范围。
  - DONE（T0106）：GC rooting 审计——系统审计 runtime/c 全部 7 个源文件中所有调用 `scoop_alloc` 的函数，修复 4 个存在 GC-managed 指针跨分配点未 pin 的函数：`scoop_string_trim_indent`（pin value）、`scoop_process_args_array`（pin builder）、`scoop_array_builder_build_common`（pin b）、`scoop_thread_spawn`（pin env_ptr）。其余函数验证为已安全或无需修复。

## 3.1 编译器 + 核心库规范合规审计

> 以下任务来自对 `SCOOP_FULL_SPEC.md` 与编译器/核心库实现的系统审计。

- DONE（T0107）：String `==`/`!=` codegen——runtime/c 新增 `scoop_string_equals`（指针/长度/memcmp 三级比较），typecheck 扩展 `Eq`/`Ne` 接受 `String` 操作数，codegen 在 `CgTy::String` 时调用 runtime 函数并转为 Bool；`assertEqString` 改用 `==`；新增 `string_equality_basic` fixture（10 场景）。775 fixtures 通过。
- DONE（T0108）：Nullable 运算符 codegen（`?.`/`!!`）——HIR lowering 将 `?.`/`!!` 展开为 `when` + `Perform`（codegen 无修改，自动继承 `when`/`Raise` 路径）。`!!` 全链路可用（run-pass）；`?.` desugar 已实现（HIR golden 验证），端到端受阻于 typecheck（仅 struct receiver）+ codegen（`Option<Struct>` payload）组合缺口。777 fixtures 通过。
- DONE（T0109）：`with` 表达式 codegen（值类型更新）——HIR lowering 将 `base with { field: value }` 展开为 StructLit：typecheck 通过 OnceCell 写回各层 struct FQN 映射表，lowering 绑定 base 到合成 val（单次求值），递归构造 StructLit（直接覆盖的字段用 update 值，未更新字段用 MemberAccess 复制）。支持嵌套路径（`start.x: 1`）。新增 4 个 run-pass fixtures（简单更新/多字段/保持不变字段/嵌套路径）。781 fixtures 通过。
- DONE（T0114）：`Bool.toString()` + `print`/`println` Bool 重载——runtime/c 新增 `scoop_bool_to_string`（返回 GC-managed `"true"`/`"false"`），resolver/typecheck 新增 Bool.toString()，codegen 统一 `codegen_to_string_method` 分发（evaluate receiver first → dispatch by CgTy，解决 `(x==y).toString()` 路由问题），sysroot 新增 `print(Bool)`/`println(Bool)` 重载。新增 2 个 run-pass fixtures（minimal + comprehensive 17 场景）。788 fixtures 通过。
- DONE（T0115）：String 补齐 8 个方法——`trim`/`trimStart`/`trimEnd`/`isEmpty`/`replace`/`charAt`/`repeat`/`compareTo`。完整 pipeline：C runtime（8 个 GC-safe 函数）→ API 注册 → resolver 白名单 → typecheck → codegen symbols/ABI/dispatch。新增 `stdlib_string_methods_extended` fixture（27 场景）。789 fixtures 通过。
- DONE（T0116）：核心库 hardcoded 类型限制清单——审计 8 项限制并逐项标注归属：4 项有后续任务（T1818/T1822/T0131）、2 项后置（Task<T>/Float print）、1 项后置按需补齐（其他类型 hash）、1 项确认为设计决策（MutableArray COW）。
- DONE（T0117）：`@Extern(lib=...)` 参数传递到链接器——审计确认链接器传递管线已完整实现（`collect_extern_libs` → `LoweredHir.extern_libs` → `link_objs` → `clang -l<name>`）。新增 `ExternFun.lib: Option<String>` 字段用于诊断追溯，`extern_fun_of_decl` 填充该字段。新增 2 个 fixtures（run-pass + Cone）验证 `@Extern(lib = "c", name = "labs")` 端到端链接。791 fixtures 通过。

## 3.2 编译器限制审计（T0144）

- DONE（T0144）：新增 `COMPILER_LIMITS_AUDIT.md`，系统扫描 `llvm/`、`hir/`、`resolve/`、`typecheck/` 四个目录中的 `UnsupportedMainBody` / `UnsupportedExpr` / HIR `Todo(...)` / `todo!` / `unimplemented!` / `HACK` / `FIXME` / 典型 `Any` fallback。
- 原始信号统计：
  - `UnsupportedMainBody`：1325 个原始匹配
  - `UnsupportedExpr`：77 个原始匹配
  - HIR `Todo(...)`：13 个原始匹配
  - `todo!` / `unimplemented!` / `HACK` / `FIXME`：0
- 分类结论：
  - `RangeInclusive` / `Elvis` / `with_update` / `array_lit` 等残留信号已归类为“已有任务覆盖”或“dump-hir fallback / 防御性守卫”，不重复排短 TODO。
  - `class literal` 当前维持“仅注解 / comptime 可用”的阶段性边界；`FunPtr<F>` 维持 non-receiver 设计；function type named args 维持不支持。
  - runtime/C 剩余依赖当前主要集中在 host/OS 桥接、GC/unsafe 边界、以及格式化/哈希 helper；未发现像旧版 String API 那样值得整组迁移的新目标。
  - 修正 1 处过期注释：`typecheck/expr/call.rs` 不再错误宣称 interface dispatch 尚未实现。
- 新增任务入口：
  - `T0151`：`for (x in iterable)` Custom iterator lowering + codegen
    - 2026-04-11：已完成。typecheck 会把 custom iterator 的 `iterator()/next()` 静态调用目标与类型写回 AST side table；HIR lowering 以 `val __for_iterable` + `val __for_iter` + `var __for_running` + `while/when` 展开，避免 `when` 分支内直接 `break` 触发 LLVM verifier 问题。新增 `for_in_custom_iterator_basic`、`for_in_custom_iterator_effects` 回归，覆盖基本迭代、required effects 传播与 `None()` 正常退出。
  - `T0152`：safe member access parity（ref receiver / extension property）
    - 2026-04-11：已完成。safe member access 的 typecheck 现在会在 unwrap `Option<T>` 后复用普通 member access 的解析结果，并把 ref receiver / extension property 的目标写回 AST side table；`check_expr_stmt` 也补上了 `SafeMemberAccess` 的推导入口，避免 `call(x?.prop)` 这类表达式语句实参跳过补写。HIR lowering 的 `Some(v)` 分支现已复用普通 member access lowering，并覆盖 extension property getter 脱糖。新增 `safe_member_access_ref_and_extension_ok`、`safe_member_access_ref_and_extension_basic` 与 HIR lowering 回归，覆盖 `Option<Class>` / `Option<Object>` 字段访问、safe extension property 与 `None -> None`。
  - `T0153`：receiver function value invocation
    - 2026-04-11：已完成。局部 receiver function value 调用不再在 typecheck 阶段被拒绝；调用按“receiver 作为第 0 个实参”检查 arity、receiver mismatch 与普通参数类型，并保持既有 effects / `@NoGC` / `const fun` 门禁。LLVM closure/function value 的间接调用 ABI 已扩展为 `env + receiver + params`，receiver lambda codegen 也会跳过 receiver 槽位再绑定显式参数。新增 `receiver_function_value_call_basic`、`receiver_function_value_call_arity_mismatch_is_error`、`receiver_function_value_call_receiver_mismatch_is_error` 回归。
  - `T0154`：higher-order aggregate returns（closure / function value / `FunPtr`）
    - 2026-04-11：已完成。higher-order 间接调用在返回 LLVM aggregate 时统一改用 hidden sret（`void + sret* + env/params`）；closure / function value / `FunPtr` 调用点与 lambda 定义同步挂 `sret` attribute，避免 `gc.result` 对 aggregate 返回值的限制。closure lambda 现已显式设置 `gc "statepoint-example"`，因此 `String.concat()` 等带分配的 closure 在 `SCOOP_GC_STRESS=1` 下也能稳定返回含 GC ref 字段的 struct。aggregate 默认值/早退路径一并补齐，新增 `higher_order_aggregate_return_closure_tuple`、`higher_order_aggregate_return_struct_mapper`、`unsafe_funptr_aggregate_return_tuple` 回归。

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
- DONE（T1817）：Hashing 落地——Int/String hash 实现：
  - **`Int.hash()`**（LLVM codegen inline）：SplitMix64-style bit-mixing（XOR/shift/mul 5 步），无 C runtime 调用。
  - **`String.hash()`**（runtime/c）：FNV-1a 哈希（offset basis 14695981039346656037，prime 1099511628211），逐字节处理。
  - **Codegen dispatch**：通过 receiver HIR 类型判断路由——`ValueTypeKind::Int` → inline，其它 → `scoop_string_hash` C 调用。
  - Fixture：`stdlib_hash_basic.scoop`（Int 6 + String 4 + 回归值 6 = 13 场景 + 完成标记）。
  - 下一步：T1818（Hash-based Set/Map）。
- DONE（T1818）：Hash-based Set/Map（Int key）：
  - **`MutableSet`**（`stdlib/collections_set.scoop`）：内部布局改为 `[entryCount, capacity, state, key, ...]` 的开放寻址哈希表；`add`/`contains`/`remove` 改走 linear probing，写操作仍返回新集合。
  - **`MutableMap`**（`stdlib/collections_map.scoop`）：内部布局改为 `[entryCount, capacity, state, key, value, ...]` 的开放寻址哈希表；`put`/`getOrDefault`/`containsKey`/`removeKey` 改走 linear probing，更新已有 key 时保持 entryCount 不变。
  - **兼容层**：`asSet()` / `asMapView()` 继续导出紧凑顺序视图；为兼容当前 `typealias` surface 下只读/可变扩展可能共享路由，mutable 侧查询 API 增加“哈希 backing / 顺序视图”自动识别，避免旧的只读视图调用被误按哈希布局解释。
  - **回归**：新增 `stdlib_hash_set_map_basic` fixture，覆盖同桶冲突（`0/3/8/24`）、重复插入、删除后重建、map 更新路径与只读视图导出；旧有 `stdlib_set_map_basic` / smoke fixtures 保持通过。
  - **验证**：`cargo test --all`、`cargo run -p scoop -- test`（`fixtures: ok (901)`）、`cargo clippy --workspace --all-targets --message-format short -- -D warnings` 通过。
  - 下一步：T1819（Ranges 增强：`..` / `until` / `for-in` 集成）。
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
- DONE（T0118）：`@CLayout(packed)` store alignment 修复 + run-pass 测试：
  - **Store alignment 修复**：`store_local_value`（`gc.rs`）在 `build_store` 后对 packed struct 显式 `set_alignment(1)`。
  - **审计结论**：当前 packed struct store 仅通过 `store_local_value`（整体 aggregate store）；field-level GEP + store 不存在于用户 struct 路径。
  - **Fixtures**（3 个 run-pass）：`clayout_packed_basic`（packed=1 字段读/写/函数传参/负值/var 重赋值）、`clayout_aligned_basic`（aligned=16/8 字段读/写）、`clayout_aligned_packed_combined`（aligned=8+packed=1 组合）。
  - 139 单元测试 + 794 fixtures 通过。
- DONE（T0119）：`@CLayout(packed = N)` 支持 N > 1（`#pragma pack(N)` 语义）：
  - **Typecheck**：`packed` 值验证接受 1/2/4/8/16（2 的幂）。
  - **Codegen**：`packed=1` 继续使用 LLVM 原生 packed struct；`packed>1` 使用手动 padding insertion + `pack_field_indices` 映射。
  - **关键 bug 修复**：`pack_field_indices` 缓存在 per-function `MainCodegen` 实例间丢失——early return 路径中重新推导。
  - 139 单元测试 + 798 fixtures 通过。
- DONE（T0120）：String 字节访问器——`byteLength()` + `getByte(index)`：
  - **两个方法均为编译器 intrinsic**（resolver 白名单 + codegen 内联 LLVM IR），无 runtime/c 函数调用。
  - `byteLength()`：GEP 到 `ScoopString.len`（字段 1）+ load i64。
  - `getByte(index)`：bounds check（index < 0 || index >= len → 返回 0）+ GEP 到 `data[index]` + load i8 + zero-extend to i64。
  - 返回类型为 `Int`（与现有 `length()`/`charAt()` 保持一致）。
  - Fixture：`string_byte_accessors`（20 个场景：byteLength 4 + getByte 12 + 联合验证 4）。
  - 139 单元测试 + 799 fixtures 通过。
- DONE（T0121）：`@Unsafe` String.unsafeSliceBytes intrinsic：
  - **runtime/c**：新增 `scoop_string_unsafe_slice_bytes(source, offset, len)`，defensive clamping + GC-safe（pin source）。
  - **完整 pipeline**：resolver 白名单 + typecheck（`in_unsafe_context()` 门禁 + 2 args → String）+ codegen 调用 runtime 函数。
  - **typecheck error fixture**：非 unsafe context 调用报 `UnsafeCallRequiresUnsafeContext`。
  - **run-pass fixture**：`string_unsafe_slice_bytes`（15 个输出行：基本切片 + 空切片 + 边界防御 + byteLength/getByte 联合验证）。
  - 139 单元测试 + 801 fixtures 通过。

- DONE（T0122）：String 操作迁移——将 runtime/c substring 类函数替换为纯 Scoop 实现：
  - **新增 `stdlib/string.scoop`**：9 个 extension functions（substring/indexOf/contains/startsWith/endsWith/split/trimStart/trimEnd/trim），基于 T0120 byteLength/getByte + T0121 unsafeSliceBytes 实现。
  - **codegen 新增**：`__scoop_array_builder_push_string` / `__scoop_array_builder_build_array_string` @Intrinsic 支持（split 返回 Array<String>）。
  - **移除 C runtime**：9 个 C 函数（scoop_string_substring/index_of/contains/starts_with/ends_with/split/trim/trim_start/trim_end）+ is_ascii_whitespace helper。
  - **移除编译器硬编码路径**：resolver 白名单（9 项）、typecheck 签名规则（9 组）、codegen string_method dispatch（9 分支）、runtime_abi（9 个 declare_runtime_*）、runtime_symbols（9 个常量）、runtime API allowlist（9 个 X-macro）。
  - **codegen 限制适配**：stdlib 函数避免 return/break/continue in block expressions（~~当前 LLVM codegen 不支持 function-level CFG~~ **T0141 已解除 return/break/continue 限制**）；使用 flag+越界赋值模拟 break；~~if 必须有 else 分支~~（**T0142 已解除 if-without-else 限制**）；常量通过 sizeOf 派生（~~无 Int/String 字面量~~ **T0140 已解除字面量限制**）。
  - 139 单元测试 + 801 fixtures 通过（行为完全不变）。

- DONE（T0141）：块级控制流——支持 block/loop 内 return/break/continue：
  - **`MainCodegen` 新增字段**：`loop_context_stack`（break/continue 目标 BB 栈）+ `return_context`（函数级 return BB + return alloca）。
  - **`codegen_while_stmt`** push/pop `LoopContext`；**`codegen_block_stmt`/`codegen_block_as_return_value`/`codegen_block_value_in_expected_context`/`codegen_block_as_exit_code`** 全部支持 `Break`/`Continue`/`Return`。
  - **`codegen_if_expr`**：then/else 分支在 break/continue/return 后检测 terminator，跳过 coercion/store/merge branch。
  - **`codegen_top_level_fun`**：创建 return_bb + return_alloca，正常/early return 两条路径都经由 return_bb emit LLVM ret。
  - **5 个 run-pass fixtures**：block_early_return、while_break_basic、while_continue_basic、nested_loop_break、return_from_loop。
  - 139 单元测试 + 807 fixtures 通过。

- DONE（T0142）：if 表达式无 else 分支支持：
  - **`codegen_if_expr`**：当 `else_branch` 为 `None` 时强制 `out_cg = CgTy::Unit`，移除原有非 Unit 报错。
  - **`stdlib/string.scoop`**：移除全部 9 处多余的 `else { }` 空分支。
  - **typecheck**：if-without-else 作为值表达式已正确报 `initializer_type_mismatch`。
  - **2 个 fixtures**：`if_without_else`（run-pass，12 行输出）+ `if_without_else_as_value_is_error`（typecheck fail）。
  - 139 单元测试 + 809 fixtures 通过。

- DONE（T0127）：泛型独立函数 LLVM 编译路径端到端支持：
  - **`build.rs`**：`run_frontend()` 通过 `check_file_exprs_with_monomorph_keys` 收集 monomorph keys，存入 `FrontendOutput`。
  - **`hir/lower/util.rs`**：新增 `collect_generic_fun_instantiations()` 从 monomorph keys 生成单态化 FunDecl。
  - **`hir/lower/mod.rs`**：`lower_for_compilation_unit_multi_files()` 接受 `monomorph_keys` 参数并调用上述函数。
  - **`llvm/codegen/mod.rs`**：新增 `try_resolve_monomorphized_standalone_fun_fqn()` + `resolve_expr_concrete_type()` 在 codegen 阶段将泛型函数调用重定向到单态化变体。
  - **5 个 run-pass fixtures**：`generic_fun_basic`、`generic_fun_multi_param`、`generic_fun_transitive`、`generic_fun_higher_order`、`generic_fun_recursion`。
  - 139 单元测试 + 819 fixtures 通过。

- DONE（T0143）：String 扩展方法从 stdlib 迁移到 sysroot core 库：
  - **`sysroot/string.scoop`**（新建）：9 个 extension methods 重写——字面量替换 `__string_zero`/`__string_one`、`break`/`return` 替换 flag+越界赋值、移除冗余 `else { }`。
  - **编译器 sysroot 加载分离**：`Sysroot` 新增 `compilable_source_paths`；`is_compilable_sysroot_file()` 按文件名分流；`collect_compilable_sysroot_files()` 供 build pipeline 将可编译 sysroot 文件加入 `input.sources`（走完整 resolve → typecheck → HIR → codegen 管线）。
  - **注释更新**：`resolve/scopes.rs`、`typecheck/expr/call.rs`、`llvm/codegen/mod.rs`、`llvm/codegen/runtime_abi.rs`、`runtime/c/scoop_runtime.c` 中的 `stdlib/string.scoop` → `sysroot/string.scoop`。
  - **删除 `stdlib/string.scoop`**。
  - 139 单元测试 + 809 fixtures 通过。

## 4.1 字面量 SourceMap 重实现（T0150）

> 背景：T0140 用 “HIR lowering 时直接解析 Int/String 值” 的轻量方案解决了多文件字面量可用性，但把源文本与 span 的关联切断了。随着后续 hex/binary、char、float 等任务推进，缺少精确 literal parse diagnostics 会成为硬障碍，因此需要先补回统一的 source lookup 基础设施，再逐步切换 literal 管线。

- DONE（T0150a）：`source.rs` 引入 `SourceMap` / `SourceId` / `SourceMapSpan` / `SourceLocation`；已覆盖多文件 slice、line/column、global-span 不重叠测试，并顺手清理 focused no-LLVM 验证路径上的 4 个既有 unused/dead-code warnings。
- DONE（T0150b）：LLVM 后端从单个 `&SourceFile` 切到 `&SourceMap + entry_source_id`；`scoop build` 按 `sysroot + 当前编译单元全部输入` 构建 source map 并传入 codegen。当前仍保留 T0140 的 parsed literal payload，非字面量旧 slicing 语义继续按入口文件回退；新增 `llvm::tests::lowered_hir_codegen_accepts_multi_file_source_map`。`cargo test --all` 与 `cargo run -p scoop -- test` 通过。补充检查：严格 `cargo clippy --workspace --all-targets -- -D warnings` 仍受既有仓库级 warning/clippy baseline 阻塞（大量 inkwell deprecated `ptr_type` 与长期 clippy lint），不是本任务引入。
- DONE（T0150c）：HIR Int/String literal / `WhenPat::IntLit` 已回退为 SourceMap-backed 解析；为避免将瞬时 `SourceId` 固化到 HIR，`FunDecl` / `TopLevelVar` / `ObjectInit` / `ClassInit` 改携带稳定 `source_path` provenance，LLVM codegen 通过 `SourceMap::source_id_of_path(...)` 在函数/对象初始化/类初始化/顶层变量初始化 emission 时切换 source context。`codegen_literal`、`when` 整数字面量匹配与 comptime 十进制整数字面量解析已统一走 `SourceMap + Span` 取原文再解析；HIR goldens 已更新。`cargo check -p scoopc`、`cargo test --all`、`cargo run -p scoop -- test` 通过。严格 `cargo clippy --workspace --all-targets -- -D warnings` 仍被既有仓库级 baseline 阻塞，并非本任务引入。
- DONE（T0150d）：LLVM 新增 `scoop::llvm::invalid_literal`，按当前 `SourceMap` source context 报出 file/line/column、literal 原文预览与附加 `source_code`；`build` / `run_pass_cone` fixture harness 现保留原始 diagnostic，`EXPECT-ERROR-AT` 可从 diagnostic-attached source 解析非入口文件定位。新增 `build/literal_parse_error_entry_file.scoop` 与 `run_pass_cone/multi_file_literal_parse_error_non_entry` 两个 failure regressions，并清理 T0140 遗留注释。`cargo fmt --check`、`cargo test --all`、`cargo run -p scoop -- test` 通过；直接 `scoop build` 现分别报 `literal_parse_error_entry_file.scoop:12:13` / `helpers.scoop:6:12`。严格 `cargo clippy --workspace --all-targets -- -D warnings` 仍被既有 workspace baseline 阻塞（`inkwell::ptr_type` deprecated 与长期 clippy lint），当前无 `InvalidLiteral` / `literal_text_preview` 新增 clippy 回归。
- DONE（T0145）：hex / binary integer literals——`syntax/lexer.rs` 现支持 `0x`/`0X` 与 `0b`/`0B` 前缀 lex 成单个 `IntLiteral` token，共享 `parse_int_literal(...)` 已接入 LLVM literal parsing、comptime、HIR lowering 的 `@CLayout(...)` 参数解析与 typecheck 注解参数解析。补齐 LLVM 顶层 `when (Int)` subject codegen 后，`when (x) { 0xFF -> ... }` 也可直接工作。新增 3 组回归：`run-pass/int_literal_hex_binary_basic`、`run-pass/when_int_literal_hex_binary_patterns`、`comptime/int_literal_hex_binary_basic`，并新增 lexer / parser 单元测试。验证：`cargo test --all` 通过，`cargo run -p scoop -- test` 通过（`fixtures: ok (841)`）；严格 `cargo clippy --workspace --all-targets -- -D warnings` 仍受既有 workspace baseline 阻塞（inkwell deprecated `ptr_type`、`too_many_arguments`、`result_large_err` 等），未观察到本任务改动路径上的新增 task-specific clippy 失败。
- DONE（T0146a）：Char 字面量前端语法与诊断——已落地 `TokenKind::CharLiteral`、`syntax/char_literal.rs` 共享 parser、lexer Char 诊断、AST `ExprKind::CharLit` / `WhenPat::CharLit`、parser expr/when pattern 接线，并补齐 resolver / HIR / typecheck / comptime / LLVM `when` pattern 的最小兼容分支。新增 1 个 parse pass fixture、4 个 parse failure fixtures、1 个 parser 单测与多组 syntax 单测。验证：`cargo test --all` 通过，`cargo run -p scoop -- test` 通过（`fixtures: ok (846)`）；严格 `cargo clippy --workspace --all-targets -- -D warnings` 仍被既有 workspace baseline 阻塞（`inkwell` deprecations 与长期 `too_many_arguments` / `result_large_err` lint），不是本任务引入。
- DONE（T0146b）：Char 类型静态语义——已补齐 `ValueTypeKind::Char` / `BuiltinTypes::char_`、implicit builtin `Char` 解析、HIR `LiteralKind::Char(char)` / `WhenPat::CharLit { span, value }` 与 `scoop.core.Char` type path lowering、typecheck Char 比较 / `when` pattern / `Char.toInt()`、comptime `ConstValue::Char` 与折叠逻辑。为保持 dump pipeline 回归稳定，同时刷新了受 builtin `TypeId` 漂移影响的 HIR / MIR goldens。验证：`cargo test --all`、`cargo run -p scoop -- test` 通过（`fixtures: ok (848)`）；严格 `cargo clippy --workspace --all-targets -- -D warnings` 仍受既有 workspace baseline 阻塞（`inkwell` deprecated API 与长期 `too_many_arguments` / `result_large_err` lint），不是本任务引入。
- DONE（T0146c1）：Char LLVM 标量落地——已把 `Char` 作为运行期 `i32` 标量值接到 LLVM/run-pass：`cg_ty_of` / `cg_ty_of_type_fqn` 映射到 `IntTy { bits: 32, signed: false }`，`codegen_literal` 支持 `LiteralKind::Char`，member access codegen 支持 `Char.toInt()` zero-extend 到目标 `Int`，`control_flow.rs` 补齐 top-level `when` Char pattern 与 tuple element Char pattern 的条件生成。新增 run-pass fixture `char_runtime_scalar_basic.*`，覆盖赋值、转义、Unicode escape、比较、`toInt()`、`when` pattern 与返回 `Char` 的函数。验证：`cargo test --all` 通过，`cargo run -p scoop -- test` 通过（`fixtures: ok (849)`）；严格 `cargo clippy --workspace --all-targets -- -D warnings` 仍受既有 repo baseline 阻塞（`inkwell` deprecated API、长期 `too_many_arguments` / `result_large_err`），本任务引入的 `cg_ty_of` 不可达分支 warning 已清理。
- DONE（T0146c2）：Char sysroot/runtime 文本化——已在 `sysroot/core.scoop` 增加 `struct Char : Hashable, ToString` 与 `toInt()/toString()/hash()` 声明；`typecheck/assignable.rs` 补齐 builtin `Char` → nominal interface 的约束判定；runtime 新增 `scoop_char_to_string(int32_t codepoint)` UTF-8 编码，并接到 `runtime_symbols.rs` / `runtime_abi.rs` / `scoop_runtime_api.h`；LLVM codegen 新增 `Char.toString()` / `Char.hash()` lowering、`print/println` 与 `ToString` builtin dispatch 的 Char 路径，以及 body-less 扩展函数顶层拦截 `scoop.core.toInt` / `scoop.core.toString` / `scoop.core.hash`。新增 `run-pass/char_runtime_textual_basic.*` 与 `run_pass_cone/char_multi_file_runtime_api/**` 两组回归。验证：`cargo test --all` 通过，`cargo run -p scoop -- test` 通过（`fixtures: ok (851)`）；严格 `cargo clippy --workspace --all-targets -- -D warnings` 仍受既有 baseline 阻塞（大量 `inkwell` deprecated `ptr_type` 与长期 `too_many_arguments` / `result_large_err`），非本任务引入。

## 4.2 Float builtin groundwork（T0147）

> 背景：原 `T0147` 横跨 builtin 类型、LLVM `CgTy`、sysroot API、resolver/typecheck builtin 路由与 runtime C ABI。与此前 `Char` 的落地类似，直接整包推进会让单轮改动和回归面过大，因此拆成可独立验收的小步。

- DONE（T0147a）：Float builtin type plumbing——已在 `ty/mod.rs` / `typecheck/lower.rs` / `typecheck/layout.rs` / `rtti/*` / `sysroot/core.scoop` 中引入 `Float64` / `Float32` / `Double` 的类型身份与共享基础设施；implicit builtin type lookup、layout、RTTI、Cone/HIR 共享穷举路径已补齐，并新增 `typecheck/float_builtin_type_refs_ok.scoop`。LLVM 浮点标量映射刻意后置到 `T0147b`；验证：`cargo test --all` 通过，`cargo run -p scoop -- test` 通过（`fixtures: ok (852)`），严格 `cargo clippy --workspace --all-targets -- -D warnings` 仍受既有 workspace baseline 阻塞，非本任务新增。
- DONE（T0147b）：Float LLVM 标量映射——已在 `llvm/codegen/types.rs` 中新增 `CgTy::Float64/Float32` 与 `CgValue` 浮点辅助；`llvm/codegen/ty.rs` 已将 builtin Float 映射为 `double` / `float`；`mod.rs` / `control_flow.rs` / `effect.rs` / `gc.rs` / `layout.rs` 的共享标量路径已补齐 Float 分支，保证默认值、结果槽位、enum payload、continuation resume word 与局部存储不会因 Float builtin 触发 panic。新增 `float_builtin_types_lower_to_llvm_scalars` LLVM 单测，验证 IR/ABI 已落为真实浮点标量。验证：`cargo test --all` 通过，`cargo run -p scoop -- test` 通过（`fixtures: ok (852)`），直接 `target/debug/scoop test` 也通过；严格 `cargo clippy --workspace --all-targets -- -D warnings` 仍受既有 workspace baseline 阻塞（大量 `inkwell` deprecated API 与长期 `too_many_arguments` / `result_large_err`），未见本任务新增 clippy 回归。
- DONE（T0147c-1）：Clippy 基线清理（LLVM opaque pointer API 去弃用）——`crates/scoopc/src/llvm/codegen/gc.rs` 已新增 `llvm_ptr_type(...)` / `llvm_ptr_sized_int_type(...)` helper，并将 `mod.rs` / `effect.rs` / `gc.rs` / `layout.rs` / `control_flow.rs` / `runtime_abi.rs` / `llvm/mod.rs` 中所有旧的 `*.ptr_type(...)` / `ptr_sized_int_type_in_context(...)` 调用迁移到 LLVM 21 opaque pointer 新接口；同时清掉迁移引入的多余 typed pointer 临时变量。验证：`cargo check -p scoopc --features llvm`、`cargo test --all`、`cargo run -p scoop -- test` 通过；严格 `cargo clippy --workspace --all-targets -- -D warnings` 里已不再出现 deprecated pointer API 错误，剩余失败收敛到 `T0147c-2` / `T0147c-3` 范围。
- DONE（T0147c-2a）：Clippy 基线清理（lowering / resolve / LLVM helper 签名收口）——`hir/lower` 侧已把 delegated property FQN 收进 info，并把 monomorph lowering 公共输入打包为 `LoweringInputs` / `BoundMemberFunLoweringTarget`；`resolve/mod.rs` 已新增 `DeclOrigin` 收口 `insert_symbol` / `insert_synth_symbol` / `insert_constructor_overload` 的公共来源参数；LLVM 侧已新增 `MainCodegenInputs` / `TypeDescriptorSpec`，并将 effect handle 的递归 perform 扫描改为局部 `NestedPerformScanState`、间接 perform 路径改为 `IndirectEscapeContinuationPlan`。验证：`cargo fmt --all`、`cargo check -p scoopc --features llvm`、`cargo test --all`、`cargo run -p scoop -- test` 通过；严格 clippy 中 `too_many_arguments` 总数已从 78 降到 65，本子任务负责的 13 个告警全部清零。
- DONE（T0147c-2b）：Clippy 基线清理（typecheck 支撑模块签名收口）——`annotations.rs` 已新增 `AnnotationCheckContext` 收口注解检查的只读解析上下文；`override_effects.rs` 已新增 `TypeInterfaceImplTarget` 收口 interface impl effect 检查目标；`properties.rs` 已新增 `DelegatedPropertySignatureCheck` 收口 delegated property 签名检查请求；`val_pat.rs` 已新增 `ValPatChecker` 收口递归 pattern 检查的共享状态。验证：`cargo fmt --all`、`cargo test --all`、`cargo run -p scoop -- test` 通过；本轮负责的 12 个 `too_many_arguments` 告警全部清零，workspace 全局剩余 `too_many_arguments` 从 65 降到 53，后续收敛点继续落在 `typecheck/expr/**`。
- DONE（T0147c-2c）：Clippy 基线清理（typecheck expr 中等规模路径签名收口）——`typecheck/expr/mod.rs` 已新增 `ExprInferInputs` 统一承载表达式 type inference 的只读输入；`member.rs` 的 safe access / Elvis / not-null assert / splice field / member access helper 已改为消费共享输入；`ops.rs` 已新增 `NominalReceiverRef` 收口 operator lookup 的 receiver 三元组，并将 unary / operator overload / builtin scalar 路径切到 `ExprInferInputs`；`stmt.rs` 已新增 `StmtExprShared` / `StmtExprState` / `StmtExprFlow` / `FunBodyCheckInputs`，把语句递归中的只读环境、局部绑定表和控制流态拆开，`entry.rs` / `infer.rs` 调用点同步切换。验证：`cargo fmt --all`、`cargo check -p scoopc`、`cargo test --all`、`cargo run -p scoop -- test` 通过；定向 clippy 复核后，`member.rs` / `ops.rs` / `stmt.rs` 中 17 个 `too_many_arguments` 告警全部清零，剩余总数从 53 降到 36。
- DONE（T0147c-2d）：Clippy 基线清理（typecheck expr 主干签名收口）——`typecheck/expr` 主干路径已继续统一到共享上下文：`call.rs` / `infer.rs` 新增 `EnumTypeSubstContext` 收口 enum payload type substitution；`entry.rs` 新增 `CheckFileExprsRequest`、`FileExprShared`、`ClassExprShared`、`CtorCallCheckRequest`，并把返回类型推断与 class/member/ctor 检查链路切到 `StmtExprShared` / `StmtExprState` / `ExprInferInputs`。`stmt.rs`、`val_pat.rs`、`when_pat.rs`、`when_exhaustiveness.rs` 的复用调用点已同步切换。验证：`cargo fmt --all`、`cargo check -p scoopc --message-format short`、`cargo clippy -p scoopc --all-targets --message-format short -- -W clippy::too_many_arguments`、`cargo test --all`、`cargo run -p scoop -- test` 通过；严格 `cargo clippy --workspace --all-targets -- -D warnings` 输出中已不再出现任何 `too_many_arguments`，剩余失败已收敛到 `T0147c-3` 的 `result_large_err` / `private_interfaces` / `dead_code` / `large_enum_variant` 等基线告警。
- DONE（T0147c-3a）：Clippy 基线清理（先收缩非 Expr 主路径的大 `Err`）——`typecheck::properties`、`typecheck::override_effects`、`cone::scoopir::export`、`monomorph::lower` 已统一切到模块内 boxed result alias，并在需要的地方补齐 `From<...> for Box<...>`；`scoop` driver/fixtures 的诊断包装器与 `miette::Report` 转换点已同步兼容 boxed 错误。验证：`cargo fmt --all`、`cargo check --workspace --message-format short`、`cargo test --all`、`cargo run -p scoop -- test` 通过；定向 `cargo clippy --workspace --all-targets --message-format short -- -A warnings -D clippy::result_large_err` 复核后，本轮负责的 4 个模块已不再出现在输出中，剩余失败仅落在 `ExprTypeError` 主路径（`typecheck::expr/**`、`eff_row_subst`、`val_pat`、`when_*`），对应后续 `T0147c-3b`。
- DONE（T0147c-3b）：Clippy 基线清理（收缩 `ExprTypeError` 主路径）——最终没有继续引入 boxed result alias，而是直接缩小 `ExprTypeError` 枚举体：`typecheck/expr/error.rs` 中把超大 variant `GenericTypeArgInferenceConflict` 的 6 个 `String` 字段改为 `Box<String>`，并同步更新 `infer.rs` / `call.rs` 的两个构造点。定向 `cargo clippy --workspace --all-targets --message-format short -- -A warnings -D clippy::result_large_err` 通过后，`typecheck::expr/**`、`eff_row_subst`、`val_pat`、`when_pat`、`when_exhaustiveness` 的 `ExprTypeError` 相关 `result_large_err` 已全部清零；`cargo test --all` 与 `cargo run -p scoop -- test` 也已通过（`fixtures: ok (852)`）。当前严格全量 clippy 的剩余失败已完全收敛到后续 `T0147c-3c` 的结构性 lint。
- DONE（T0147c-3c）：Clippy 基线清理（清零结构性 warning）——最后一轮结构性 lint 已全部收口：删除 LLVM codegen 中未使用 helper、收紧 `CalleeSuspend*` 可见性、为复杂 tuple 返回值抽类型别名、统一 `val/var` parser helper、将 AST `Item` / `TypeMember` 的大 payload variant 改为 `Box<...>` 以消除 `large_enum_variant`，并顺手清理 `question_mark` / `while_let_loop` / `if_same_then_else` / `nonminimal_bool` / `cloned_ref_to_slice_refs` / `unnecessary_get_then_check` 等零散 warning。验证：`cargo fmt --all`、`cargo clippy --workspace --all-targets --message-format short -- -D warnings`、`cargo test --all`、`cargo run -p scoop -- test` 通过（`fixtures: ok (852)`）。
- DONE（T0147c-3）：Clippy 基线清理（`result_large_err` + 零散 warning）——`T0147c-3a ~ T0147c-3c` 已全部完成；workspace 已恢复严格 `cargo clippy --workspace --all-targets -- -D warnings` 基线。
- DONE（T0147c）：Float sysroot API 与 builtin 方法路由——`sysroot/core.scoop` 已将 `Float64/Float32` 声明为 `Hashable, ToString` 并补齐 6 个 builtin 方法；`resolve/scopes.rs` / `typecheck/expr/call.rs` 已接入最小 member 路由并避免与 `abs(Int)` 混淆；runtime 新增 `scoop_float{32,64}_to_string`、`scoop_float{32,64}_to_int`，并预留 `scoop_string_to_float64`；LLVM codegen 已补齐 runtime symbols/ABI、`toString()/toInt()` runtime lowering、`hash()` 位级 lowering，以及 `abs()/isNaN()/isInfinite()` 的直接 lowering。新增 `typecheck/float_builtin_methods_ok.scoop` 与 LLVM 单测 `float_builtin_methods_lower_to_runtime_calls_and_hash_bits`。验证：`cargo clippy --workspace --all-targets --message-format short -- -D warnings`、`cargo test --all`、`cargo run -p scoop -- test` 通过（`fixtures: ok (853)`）。

## 4.3 Float literals（T0148）

> 背景：在 `T0147` 完成 Float builtin type / LLVM scalar / sysroot API groundwork 之后，下一步是把 `3.14` / `1e-5` / `0.5f` 之类的源码字面量真正接入语言前端和执行链路。该任务横跨前端、静态语义、LLVM 与 comptime，已拆分为可独立验收的小步，避免一次性改动面过大。

- DONE（T0148a）：Float 字面量前端打通
  - 已新增 `TokenKind::FloatLiteral`、`syntax/float_literal.rs`、AST `ExprKind::FloatLit`、HIR `LiteralKind::Float64/Float32`，并打通 lexer → parser → HIR lowering。
  - lexer 支持十进制 float、小数部分、科学计数法、`f` / `f32` 后缀，并明确保护 `1.toString()` / `1..2` 不被误判为 float。
  - parser / HIR 单测已覆盖普通 float、科学计数法、Float32 后缀和 typed HIR literal lowering。
  - 为保持全仓可编译，顺手补齐 resolver/property walker、MIR `ConstValue` 与 LLVM `codegen_literal` 的 Float literal 枚举分支。
  - 验证：`cargo test -p scoopc --lib float -- --nocapture`、`cargo fmt --check`、`cargo test --all`、`cargo run -p scoop -- test`、`cargo clippy --workspace --all-targets --message-format short -- -D warnings` 通过（fixtures `ok (853)`）。
- DONE（T0148b）：Float 字面量静态语义
  - `typecheck/expr/infer.rs` 已接入 `FloatLit` 默认推断：无后缀为 `Float64`，`f` / `f32` 为 `Float32`。
  - `typecheck/expr/ops.rs` 已新增 Float 静态规则：一元负号、基础算术、比较/相等性支持同类型 Float，并通过 `literal_absorbs_to_expected(...)` 支持“无后缀 Float 字面量 → Float32”的最小 absorption。
  - `entry.rs` / `stmt.rs` / `call.rs` / `infer.rs` 已统一复用 literal absorption helper，覆盖 initializer、默认参数、普通调用、构造调用、enum variant ctor、数组、`if` 分支、赋值、`return`、`with-update` 与 `Continuation.resume(...)`。
  - helper 会透传普通 block / `unsafe` block / `safe` block 的尾表达式，避免 `if (cond) { 1.5 } else { ... }` 在 `Float32` 期望类型下误报。
  - 新增 `tests/fixtures/typecheck/float_literal_static_semantics_ok.scoop`，集中回归默认 `Float64`、`Float32` 后缀、Float32 absorption、struct/class/array/if/default-param/return/assignment/with-update/比较/一元负号。
  - 验证：`cargo test -p scoopc float -- --nocapture`、`cargo run -p scoop -- test`、`cargo test --all`、`cargo clippy --workspace --all-targets --message-format short -- -D warnings` 通过（fixtures `ok (854)`）。
- DONE（T0148c）：Float 字面量 LLVM codegen
  - `crates/scoopc/src/llvm/codegen/mod.rs` 已补齐 Float LLVM 执行链路：一元负号、`+ - * / %`、`< <= > >=`、`== !=`、`Float64 <-> Float32` coercion，以及顶层 Float 常量初始化（字面量/一元负号）。
  - `==` 使用 ordered-equal，`!=` 使用 unordered-or-not-equal，确保 NaN 语义正确。
  - `scoop.core.abs/isNaN/isInfinite` 的 codegen 顶层扩展拦截已改为依赖真实 `CgTy`，避免局部 `VarRef` 的不稳定 `expr.ty` 导致误分发。
  - resolver 对 Float builtin API 的“保留为内建 member call”规则已同时接受 `Float64/Float32` 的短名与全名，避免误改写到扩展函数路径。
  - 新增 LLVM 单测 `float_literals_lower_to_arithmetic_comparisons_and_narrowing`，锁住 `fadd/frem/fcmp/fneg` 等关键 IR 形态；新增 run-pass fixture `float_literal_runtime_basic` 覆盖基础算术、比较、科学计数法、Float32 absorption 与 builtin 方法。
  - 验证：`cargo test -p scoopc float_literals_lower_to_arithmetic_comparisons_and_narrowing -- --nocapture`、`cargo test -p scoopc float_builtin_methods_lower_to_runtime_calls_and_hash_bits -- --nocapture`、`cargo clippy --workspace --all-targets --message-format short -- -D warnings`、`cargo test --all`、`cargo run -p scoop -- test` 通过（fixtures `ok (855)`）。
- DONE（T0148d-1）：Float comptime 值模型与基础常量折叠
  - `ConstValue` 已扩展 `Float(ConstFloat)`，并新增 `ConstFloatTy/ConstFloat` 保留 `Float64/Float32` raw bits。
  - `comptime/eval.rs` 已支持 Float literal 求值、一元负号、四则、比较、相等性，以及 `Float32 + 无后缀 Float literal` 的吸收规则；NaN 比较语义与运行期保持一致。
  - `comptime/interpreter.rs` 已对顶层/局部绑定、`const fun` 参数和返回值按声明类型归一化 Float，避免显式 `Float32` 绑定在后续求值里回退成 `Float64`。
  - `scoop` fixtures runner 已支持 Float comptime golden 文本格式；新增 `tests/fixtures/comptime/float_literal_basic.*`。
  - 验证：`cargo test -p scoopc comptime -- --nocapture`、`cargo test --all`、`cargo run -p scoop -- test`（fixtures `ok (856)`）、`cargo clippy --workspace --all-targets --message-format short -- -D warnings` 通过。
- DONE（T0148d-2）：多文件 / 非入口源文件回归
  - 新增 `run_pass_cone/float_multi_file_literal_basic`：在 helper 文件中覆盖 Float 顶层初始化、普通函数返回值、`Float32` absorption 与科学计数法比较；入口运行输出稳定为 `3.75 / 1.75 / 2.5 / 2.0 / true`。
  - 新增 `run_pass_cone/float_multi_file_literal_type_mismatch_non_entry`：helper 文件中的 `return 1.5` → `Int` 类型不匹配会稳定报 `scoop::typecheck::return_type_mismatch`，并把 `EXPECT-ERROR-AT` 锁到 helper 文件真实标签位置 `6:6`，避免错误漂移到入口文件。
  - 验证：`cargo fmt --check`、`cargo test --all`、`cargo run -p scoop -- test`（fixtures `ok (858)`）、`cargo clippy --workspace --all-targets --message-format short -- -D warnings` 通过。
- DONE（T0148d-3）：剩余转换、边角语义与审计
  - parser 对 `when (x) { 1.5 -> ... }` 现给出稳定的“Float 字面量暂不支持”错误，并通过占位 wildcard 继续消费当前 arm，避免额外级联 parse 噪声；新增 parse fixture `when_float_pattern_is_error` 与 Rust 单测锁住“单一错误”行为。
  - `comptime/eval.rs` 已补齐 Float builtin 方法折叠：`toInt()/toString()/hash()/abs()/isNaN()/isInfinite()`；新增 Rust 单测 `const_eval_float_builtin_methods` 与 fixture `comptime/float_builtin_methods_basic.*`。
  - LLVM f-string / 插值字符串现支持 `{Float}`，复用 Float `toString` 路由；新增 run-pass fixture `float_literal_other_contexts_basic`，覆盖 generic `println(Float)`、f-string 插值、`NaN` / `Infinity` 文本。
  - 审计顺带确认：顶层 `const val` 一般表达式仍有 `top-level value ref` 的老限制，但该问题并非 Float 特有，本轮仅记录，不在 T0148d-3 中扩 scope。
  - 验证：`cargo fmt --check`、`cargo test --all`、`cargo run -p scoop -- test`（fixtures `ok (861)`）、`cargo clippy --workspace --all-targets --message-format short -- -D warnings` 通过。
- DONE（T0148d-4）：顶层 `const val` 一般表达式 codegen
  - HIR lowering 新增 `TopLevelConst` / `TopLevelConstIndex` side table，收集顶层 `const val` 的 `fqn/source_path/span/ty/init`，供后端在表达式位置回放 initializer。
  - LLVM `ValueRef::TopLevel` 现在会识别顶层 `const val` 并按声明类型内联 initializer；同时加入最小递归检测，避免循环 `const val` 引用无限展开。
  - reachability collector 现在会在扫描顶层 `const val` 引用时递归进入 initializer，并继续扫描复合 callee（例如 `helper().concat(...)`），把仅经由顶层 const 可达的函数也带进 LLVM。
  - 新增 `run-pass/top_level_const_val_general_expr_basic` 与 `run_pass_cone/top_level_const_val_multi_file_basic` 两组回归，覆盖链式 const 引用、`const fun` 调用，以及多文件 helper 函数内引用 helper 文件顶层 const 的路径。
  - 验证：`cargo fmt --all`、`cargo test --all`、`cargo run -p scoop -- test`（fixtures `ok (863)`）、`cargo clippy --workspace --all-targets --message-format short -- -D warnings` 通过。

## 4.4 Array literals（T0149）

- DONE（T0149）：Array 字面量类型推断移除无上下文限制
  - AST `File` 已新增表达式类型 side table；`typecheck/lower.rs` 与 `typecheck/expr/{mod,entry}.rs` 现把每个表达式的最终 `TypeId` 写回 AST，供 HIR lowering 在不改变 Debug/golden 形态的前提下复用完整 typecheck 结果。
  - `typecheck/expr/infer.rs` 已支持无 expected type 的非空 `[...]` 推断为同构 `Array<T>`；空数组 `[]` 无上下文时报 `scoop::typecheck::array_lit_type_annotation_required`，混合元素类型报 `scoop::typecheck::array_lit_element_type_mismatch`。`typecheck/expr/error.rs` 已补齐稳定诊断码。
  - `hir/lower/expr.rs` 已优先读取 typecheck side table 的数组结果类型，并继续把元素期望类型向下传给 nested array / struct literal；`val` 标注、函数参数、`return` 与赋值右侧都不再把 array literal 降成 `Todo("array_lit")`。`dump-hir` 回退路径仍保留保守 heuristic，并额外补上本地函数签名的完整 `Array<T>` / `MutableArray<T>` hint，更新后 `tests/fixtures/hir/array_lit_lowering.hir` 反映出更精确的结果类型。
  - 新增 run-pass fixtures：`array_lit_infer_unannotated_and_nested_basic`、`array_lit_infer_fun_arg_return_assign_basic`、`array_lit_infer_string_char_float_basic`；新增 typecheck failure fixtures：`array_lit_empty_without_annotation_is_error`、`array_lit_mixed_element_types_is_error`。其中 Char / nested-array 用例通过绑定中间局部值绕开既有 rvalue 链式 codegen 限制，避免把无关缺口混入 `T0149`。
  - 为满足当前仓库“严格 clippy 过零 warning”基线，顺手把 `hir/lower/mod.rs` 的 `HirLowering::new(...)` 收口为 `HirLoweringSetup`，消除一个现存 `too_many_arguments` lint；该改动不改变 lowering 语义。
  - 验证：`cargo fmt --all`、`cargo test --all`、`cargo run -p scoop -- test`（fixtures `ok (868)`）、`cargo clippy --workspace --all-targets --message-format short -- -D warnings` 通过。

## 4.5 字面量完整性收尾审计（T0150，当前伞型任务）

> 背景：T0145~T0149 已分别打通 hex/binary、Char、Float、Array 等字面量主路径，但剩余工作已从“单一功能缺失”转为“跨语境完整性审计”。一次性完成所有语境的补齐会把 parser/typecheck/lowering/codegen/comptime/fixtures 全部耦合在同一提交里，因此拆成按语境分层的小任务，逐个落地并回归。

- DONE（T0150e）：`when`/模式匹配字面量完整性
  - typecheck 现为 `Int/String/Bool` literal pattern 补上 subject 类型约束，新增稳定诊断：`when_int_pat_not_int`、`when_string_pat_not_string`、`when_bool_pat_not_bool`；原有 `when_char_pat_not_char` 保持不变。
  - LLVM `when` codegen 现支持 `CgTy::String` subject：无 guard fast-path 与 guard/or-pattern 链式路径都会调用 `scoop_string_equals` 做字面量比较；tuple 元素中的 `String` literal pattern 也同步补齐。
  - 新增 run-pass fixture `when_literal_string_bool_char_basic`，覆盖 String / Bool / Char 顶层 pattern，以及 tuple 内部的 `(String, Char)` literal pattern；新增 typecheck failure fixture `when_string_pattern_not_string_is_error` 锁定错类型诊断。既有 parse fixture `when_float_pattern_is_error` 继续锁定 Float pattern 的单一错误行为。
  - 验证：`cargo fmt --all`、`cargo test --all`、`cargo run -p scoop -- test`（`fixtures: ok (870)`）、`cargo clippy --workspace --all-targets --message-format short -- -D warnings` 通过。

- DONE（T0150f）：comptime / `const` 语境字面量完整性
  - 新增 `tests/fixtures/comptime/literal_const_comptime_matrix.*`，统一覆盖 Int（含 `0x` / `0b`）、Bool、Char、Float 在 `const fun + comptime block/if` 中的求值，并锁定 `Unit`、`Tuple`、`Array` 的 comptime 表现。
  - `crates/scoopc/src/comptime/tests.rs` 新增 `const_eval_literal_matrix_across_const_fun_const_val_and_comptime_paths`，把相同语义落到 Rust 单测。
  - `crates/scoopc/src/comptime/value.rs` 注释明确：v0 阶段 `ConstValue::Tuple` 也承载 array literal / 常量序列，因此 fixture 输出中的 Array 继续按 tuple 形态呈现。
  - 验证：`cargo fmt --all`、`cargo test --all`、`cargo run -p scoop -- test`（`fixtures: ok (871)`）、`cargo clippy --workspace --all-targets --message-format short -- -D warnings` 通过。

- DONE（T0150g）：多文件 + 插值字符串 + 直接方法调用语境
  - LLVM `codegen_interpolated_string` 已补齐 `Bool` / `Char` 插值：`Bool` 走 `scoop_bool_to_string`，`Char` 复用 `codegen_char_method_to_string`，随后统一读取 `ScoopString.len/data` 参与拼接。
  - 同时修复多文件 f-string 文本片段取源错误：原先错误使用 `entry_source()`，导致 helper 文件的静态文本从 `main.scoop` 切片；现改为 `current_source_slice()`，与当前 codegen source context 对齐。
  - 新增 `run_pass_cone/literal_multi_file_interpolation_direct_basic`：在同一多文件 fixture 中锁定 helper 文件的 Char / Float / Array 字面量、`f-string` 插值（Bool/Char/Float/array-literal method result）以及 `42.toString()` / `'A'.toInt()` / `[1,2,3].size()`。
  - 验证：`cargo fmt --all`、`cargo test --all`、`cargo run -p scoop -- test`（`fixtures: ok (872)`）、`cargo clippy --workspace --all-targets --message-format short -- -D warnings` 通过。

- TODO（T0150h，已拆分）：类型上下文吸收与字面量运算语义
  - 该项已拆为 3 个顺序子任务，避免把数值吸收、数组上下文传播和字面量运算矩阵混在同一提交中回归。
  - DONE（T0150h-1）：数值字面量运算表达式的 expected-type absorption
    - typecheck 现会把 expected numeric type 下传到数值一元/二元运算表达式；直接 `Int` / 无后缀 `Float` 字面量在可吸收时会记录为目标类型。
    - HIR lowering 现会优先复用 typecheck side table 中的数值类型：窄整型 `Int` literal 保留目标整数类型，吸收到 `Float32` 的无后缀浮点字面量 lowering 为 `LiteralKind::Float32`，一元/二元表达式也优先采用 typecheck 推导结果。
    - 新增 `typecheck/literal_numeric_expected_type_absorption_ok` 与 `run-pass/literal_numeric_expected_type_absorption_basic`，覆盖局部/顶层初始化、`return`、call、array element、嵌套表达式，以及 `Array<Float32> = [1.5, 2.5f]` 运行期编码回归。`cargo test --all`、`cargo run -p scoop -- test`（fixtures `ok (874)`）与 `cargo clippy --workspace --all-targets --message-format short -- -D warnings` 通过。
  - DONE（T0150h-2）：数组字面量目标类型向更深嵌套表达式传播
    - typecheck `infer_expr_type_in_expected_context` 现覆盖 `Block` / `UnsafeBlock` / `SafeBlock` / `When`，block tail value 与 when arm value 会继续接收数组元素的目标类型，避免 expected type 在组合表达式边界丢失。
    - HIR lowering 新增 `lower_block_with_expected` 与 when-arm expected 透传；`if` / `when` / block 的结果类型优先复用 typecheck side table，数组字面量中的复杂元素会自动包一层内部绑定，把元素 expected type 继续传给更深层表达式，而不要求用户显式声明中间变量。
    - 同步修复数组 lowering 中 builtin 标量别名规整：`Array<UInt8>` / `Array<Float32>` 等场景会把 nominal alias 规整为真正 builtin scalar，避免后端把元素误降成 struct 并在 coercion 阶段失败。
    - 新增 `typecheck/literal_array_expected_type_nested_ok` 与 `run-pass/literal_array_expected_type_nested_basic`，覆盖嵌套 `if` / `when` / block / nested array / `Array<UInt8>`；`cargo fmt --all`、`cargo test --all`、`cargo run -p scoop -- test`（fixtures `ok (876)`）与 `cargo clippy --workspace --all-targets --message-format short -- -D warnings` 通过。
  - DONE（T0150h-3）：字面量运算/比较/直接方法调用矩阵与诊断锁定
    - HIR call lowering 现优先保留 typecheck side table 写回的结果类型；extension/member/default-arg/general call 在降糖成顶层调用后不再把结果退化为 `Any`，修复 `val x = (-2.5).abs()` / `val x = id(1)` 这类无显式类型标注的局部绑定在 LLVM 后端触发 `value coercion` / `call callee type` 的回归。
    - 新增 LLVM 单测 `lowered_call_results_keep_concrete_types_for_local_bindings`，用完整 parse→resolve→typecheck→lowering→codegen 链路锁定 unannotated local call result 的 concrete type 保真。
    - 新增 `run-pass/literal_ops_compare_direct_matrix_basic` 覆盖 Int/Char/Float/String/Array 字面量与算术、比较、直接方法调用的组合；新增 `typecheck/literal_compare_bool_is_error` 与 `typecheck/literal_direct_call_float_only_is_error` 锁定仍不支持的失败路径。`cargo fmt --all`、`cargo test --all`、`cargo run -p scoop -- test`（fixtures `ok (879)`）与 `cargo clippy --workspace --all-targets --message-format short -- -D warnings` 通过。

- DONE（T0150i）：边界值与词法/诊断审计
  - `syntax/int_literal.rs` / `syntax/float_literal.rs` 新增 checked parser 与稳定错误枚举；`lexer.rs` 先吞完整候选文本再统一校验，新增 `InvalidIntLiteral` / `InvalidFloatLiteral`，锁定 `0x` / `0b102` / `1__2` / `1e+` / `1e9999` 等文本的单一诊断路径。
  - LLVM 后端新增整数字面量范围检查：默认 `Int` 溢出、窄整型初始化、负整数字面量、`when` 整数字面量 pattern 均不再静默 wrap/truncate；源码字面量回查遇到合成 span 时安全降级为“非源码字面量”，避免误报。
  - 为保证 `cargo run -p scoop -- test` 全量回归稳定，同时修复 3 个既有 codegen 稳健性问题：
    - expected-context 最终 coercion 绑定到 `expr.span`；`expected == Unit` 时仅保留副作用、不要求显式 `T -> Unit` coercion；
    - `block.ty == Any` 且无显式 expected 时，不再把 block 尾值强制按 `Ref` 收窄；
    - 普通 statement block 中的表达式语句统一走 `expected = Unit` 的 codegen 路径，避免循环内 `if/break/continue` 生成脆弱 CFG。
  - 新增 parse/build failure fixtures 与 Rust 单测，验证完成后 `cargo fmt --all`、`cargo run -p scoop -- test`（fixtures `ok (890)`）、`cargo test --all`、`cargo clippy --workspace --all-targets --message-format short -- -D warnings` 全部通过。

## 5. 泛型 where 约束完善

- DONE（T0131）：`interface ToString` 引入 + 现有 `toString` 硬编码迁移 + `print`/`println` 泛型化：
  - **`sysroot/core.scoop`**：新增 `interface ToString { fun toString(): String { return "" } }`；`Int/Bool/String : ToString`；body-less extension functions `fun Int.toString(): String` / `fun Bool.toString(): String` / `fun String.toString(): String`；泛型 `print<T>`/`println<T>` 签名（body-less，resolver 用）；内部 `__scoop_print_string`/`__scoop_println_string` runtime 映射。
  - **`sysroot/print.scoop`**（新建，compilable sysroot）：泛型 `print<T>`/`println<T>` 函数体，调用 `value.toString()` → `__scoop_print_string`。
  - **`sysroot/mod.rs`**：`is_compilable_sysroot_file` 新增 `print.scoop`。
  - **`llvm/codegen/mod.rs`**：4 个新增 codegen 拦截——`scoop.core.toString`（extension function dispatch by CgTy）、`scoop.core.ToString.toString`（where-bound builtin dispatch）、`scoop.core.__scoop_print_string`/`__scoop_println_string`（runtime 映射）。旧 `scoop.core.print`/`scoop.core.println` 拦截保留（向后兼容）。
  - **新增 fixture**：`tostring_interface_basic`（16 场景：built-in print/println、toString extension、变量、concat 组合）。
  - 139 单元测试 + 829 fixtures 通过。

- DONE（T0129）：泛型函数调用处 where 约束检查：
  - **`resolve/mod.rs`**：`FunSig` 新增 `where_clause` 字段保留 AST where 子句。
  - **`typecheck/expr/mod.rs`**：`FunSigOwned` 新增 `where_constraints: Vec<FunWhereConstraintInfo>`。
  - **`typecheck/expr/collect.rs`**：新增辅助函数从 AST where_clause + type_params 构建约束列表。
  - **`typecheck/expr/call.rs`**：新增 `check_fun_where_constraints_after_instantiation`，在 6 个 `instantiate_fun_sig_for_call*` 调用点后验证约束。
  - **4 个新增 fixtures**：单/多约束不满足、满足、泛型传递调用。
  - 139 单元测试 + 823 fixtures 通过。

- DONE（T0128）：泛型验证与修复——泛型与 GC / 特殊化类型交互：
  - **5 个 run-pass fixtures**（全部在 SCOOP_GC_STRESS=1 下稳定）：
    - `generic_class_gc_ref_field`：`Holder<String>` — GC trace 扫描引用字段，GC collect 后存活
    - `generic_class_gc_value_field`：`Holder<Int>` — 值类型 payload 无需 GC trace + 与 `Holder<String>` 混合共存
    - `generic_class_gc_specialized_type`：`Wrapper<Array<Int>>` — Array 特殊化路径在泛型 class 内正确工作
    - `generic_class_gc_multi_alloc`：多实例 `Holder<String>` + `Pair<String/Int>` — GC 分配点安全
    - `generic_class_gc_nullable_ref`：`Holder<String?>` — niche 优化 + nullable GC trace + Some/None 混合
  - 139 单元测试 + 834 fixtures 通过。

## 6. Comptime String 操作完善

- DONE（T0123）：`const fun` 支持 String `+` 和 substring 类操作：
  - **`comptime/eval.rs`**：新增 String `+`（concatenation）二元运算——`eval_binary_eager` 在 `Add` 操作前优先匹配 `(String, String)` 分支。
  - **`comptime/eval.rs`**：新增 `try_eval_string_method_intrinsic` 统一 dispatch 函数——替代原有的 `trimIndent` 硬编码，支持 21 个 String 方法 intrinsics：`trimIndent`/`byteLength`/`getByte`/`length`/`substring`/`indexOf`/`contains`/`startsWith`/`endsWith`/`split`/`isEmpty`/`trim`/`trimStart`/`trimEnd`/`replace`/`charAt`/`repeat`/`compareTo`/`concat`/`toString`/`hash`。所有方法语义与 runtime/c + sysroot/string.scoop 一致。
  - **辅助函数**：`string_index_of`/`string_split`/`string_compare_to`/`string_trim_ascii_ws`/`string_trim_start_ascii_ws`/`string_trim_end_ascii_ws`/`is_ascii_ws`——独立的字节级 String 操作实现。
  - **新增 2 个 comptime fixtures**：`const_fun_string_ops_basic`（String `+`/`==`/`!=`/`byteLength`/`getByte`/`length`，16 个场景）+ `const_fun_string_methods`（substring/indexOf/contains/startsWith/endsWith/split/trim/replace/charAt/repeat/compareTo/concat/toString/isEmpty/hash + 组合 `extractDomain` const fun，40+ 个场景）。
  - 139 单元测试 + 836 fixtures 通过。
