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
- TODO（T0103c）：抽出 `expr.rs`（表达式 lowering）
- TODO（T0103d）：抽出 `stmt.rs`/`block.rs`（语句与块 lowering）
- TODO（T0103e）：抽出 `sugar.rs`/`patterns.rs`（语法糖与模式相关 lowering）

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
  - `Nothing` 明确为 bottom type：运行时没有值；返回类型为 `Nothing` 的函数不会正常 return。后端若需要占位表示它，也只能用于不可达路径的 IR 连通，且该值永不可被观察（见 TODO：T1612）。
  - `finally` 组合语义补齐：在 suspend/resume/传播路径上不漏执行、不重复执行。
- 落地顺序（T1606 已拆分为子任务 T1606a~T1606d）：
  - T1606a（DONE）：0 perform 时退化执行 body（arm 不可达）
  - T1606b（DONE）：取消“perform 必须首语句”（补齐 capture/lift）
  - T1606c：多 perform（pc + heap state machine）
  - T1606d：多 perform + 动态上下文/GC 回归加固
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
- 关键补齐：单个 `handle` body 内多次 suspension/resume（多 perform 点），覆盖真实 async/await 写法（见 TODO：T1706/T1707）。
- `Continuation<T>` 完整性：覆盖 `T` 的全类型空间（struct/tuple/enum/ref/Continuation 自身）。
- GC correctness：跨函数复杂对象图、数组（value/ref 混合、value 内含 ref）、循环引用。
- GC + escaping continuation：确保 continuation 捕获环境的 roots 扫描/更新正确。
- 多线程扩展：把上述场景搬到多线程，固定调度避免 flakiness。

## 4. 标准库完整性（基于 `KOTLIN_RUNTIME_GAP_AUDIT.md`）

- 先做 investigation：把 gap 审计表转换为“可执行清单”（DONE/TODO/Blockers + 实现位置 + fixtures 链接）。
- 再拆任务：按领域/优先级拆成可单独回归的小步（collections/text/ranges/sequences/math/random/time/io 等）。
- 建立 stdlib 的 smoke + matrix fixtures，持续报告覆盖缺口（是否 gating 后置）。
