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

## 1. Cone（改进项吸收）

- 已产出设计：`CONE-IMPROVEMENTS.md`（目录结构 / build 产物 / profile / 增量构建路线）。
- 近期落地重点（见 TODO T1120~T1124）：
  - `scoop new`：生成 `.gitignore`（忽略 `build/`）+ `src/main.scoop` 默认包含 `println`
  - `scoop build`：按 profile 写入 `build/<profile>/…`（默认 debug，`--release` 写入 release）
  - `scoop run`：在项目目录下“未构建则先构建再运行”，并支持 `--debug/--release`
  - incremental：先确保输出目录与行为稳定（v0 always rebuild），再做粗粒度 fingerprint 跳过构建（v1），细粒度依赖图后置（v2）

## 2. Scoop 编译器优化（优化等级/去虚化/HIR-MIR）

### 2.1 优化等级（对外接口 + 默认策略）

- 统一 CLI 与 `Cone.toml[native-build]` 的 `opt-level`（或等价字段）与优先级规则。
- 明确 debug/release 默认映射，避免“同名不同义”。
- 同步把 LLVM 后端的 `TargetMachine` 优化等级与 profile/opt-level 对齐（当前 `host_target_machine()` 仍是 `OptimizationLevel::None`）。

### 2.2 LLVM pipeline（DCE/inlining/unroll 等）

- 以 LLVM 默认 pipeline 为主，按 opt-level 启用常见 passes（DCE、inline、loop unroll、SROA 等）。
- 用 `--emit-llvm` + build fixtures（contains/not-contains）提供最小“优化确实发生”的回归证据。
- 低复杂度但高收益优先级（建议先接入再微调顺序）：
  - 必备清理：`instcombine`、`simplifycfg`
  - 早期冗余/内存优化：`early-cse`、`dse`、`dce`（必要时 `adce`）、`sccp`
  - release 才考虑更重的：`gvn/newgvn`、`jump-threading`/`correlated-propagation`、`memcpyopt`
- GC/statepoint 约束：
  - 绝大多数优化应放在 `rewrite-statepoints-for-gc` **之前**（避免在 `gc.statepoint/gc.relocate` 之后引入更多 pass 兼容性/排查成本）。
  - `rewrite-statepoints-for-gc` 之后仅做轻量清理（例如再跑一轮 `instcombine,simplifycfg`）。
  - `place-safepoints` 暂不纳入默认管线（在旧的 LLVM 18.1.8 上曾观察到 SIGSEGV；迁移到 LLVM 21 后需要单独验证其稳定性再决定是否接入）。

### 2.3 去虚化（receiver 类型已知时直调用）

- value type 默认静态分派（direct call）。
- final/sealed class 在可证明单一目标时生成直调用（或提供足够信息让 LLVM 去虚化）。

### 2.4 HIR/MIR 级优化（cheap wins）

- 无 `perform` 的作用域不生成 `handle` 结构/handler 链接，减少 runtime 开销。
- 建立“高级优化候选清单”，后续按依赖逐项拆分立项（避免想到哪做哪）。

## 3. 端到端验证与回归（Continuation/GC/多线程）

> GC 是 Scoop 的生命线：所有高风险特性都必须有“复杂但可回归”的 fixtures，而不是只靠最小 demo 证明能跑。

- Escaping continuation：复杂 fixtures（模拟 async executor/scheduler），并在 `--gc-stress` 下稳定。
- `Continuation<T>` 完整性：覆盖 `T` 的全类型空间（struct/tuple/enum/ref/Continuation 自身）。
- GC correctness：跨函数复杂对象图、数组（value/ref 混合、value 内含 ref）、循环引用。
- GC + escaping continuation：确保 continuation 捕获环境的 roots 扫描/更新正确。
- 多线程扩展：把上述场景搬到多线程，固定调度避免 flakiness。

## 4. 标准库完整性（基于 `KOTLIN_RUNTIME_GAP_AUDIT.md`）

- 先做 investigation：把 gap 审计表转换为“可执行清单”（DONE/TODO/Blockers + 实现位置 + fixtures 链接）。
- 再拆任务：按领域/优先级拆成可单独回归的小步（collections/text/ranges/sequences/math/random/time/io 等）。
- 建立 stdlib 的 smoke + matrix fixtures，持续报告覆盖缺口（是否 gating 后置）。
