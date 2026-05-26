# Scoop

Scoop 是一个 Kotlin 风格的静态类型语言，目标是：

- 真正的值类型（struct/enum/tuple，copy 语义、不可变）
- 代数效果系统（统一可挂起操作、错误处理、控制流）
- 泛型单态化（monomorphization）
- 静态反射 intrinsic 与注解元数据
- LLVM 后端（Rust `inkwell`），自带运行时与 GC（长期 C runtime/GC；平台差异隔离在 `runtime/c`）

语言规范见 `SCOOP_FULL_SPEC.md`，实现路线图见 `PLAN.md`。
当前 pipeline refactor 的 P0 阶段已移除旧编译期执行 surface，P1 阶段已建立基础 crate 层与 cone-level compilation unit facade，P2 阶段已把 `AST -> HIR` 收口为发布 `HirStageOutput = { hir, hir_facts }` 的 semantic barrier，P3 阶段已把 MIR stage 收口为 MIR-owned handoff、`mir_facts` 数据产品和显式 MIR pass pipeline，P4 阶段已把 effect/control facts 收口为只读分析产物，P5 阶段已把 late-lowered handoff 收口为正式 `LirStageOutput = { lir, lir_facts }` 与 LIR opt pipeline，P6 阶段已闭合 global init/storage/entry order，P7-T05 已把 LLVM stage handoff、reachability/body emission 清场和 physical ABI/layout 收口到 `LIR + lir_facts + LlvmStageBaseContext`，P8-T01 已完成最终 residual 搜索与后端输入边界文档冻结，P9 已完成 stage/fact/cone crate split，P10 已落地 per-cone artifact、portable `TypeStore` wire format、per-cone fingerprint cache、`scoopld`/`link-cone` 与多进程 cone build driver。dependency gate 防止 emit/reachability/handoff、stage/cone 反向依赖、`scoop` facade in-process compiler/linker residual、旧 `comptime` keyword 和 LLVM `const_eval` helper 回退。top-level `val` 与 annotated top-level `var` 由 per-cone init routine 在用户 `main` 前 eager 初始化，object once 只服务 object singleton，`@Global` / `@ThreadLocal` storage policy 已贯通到 codegen/runtime。反射能力保留为 sysroot `@Intrinsic` 声明。
独立 `scoopc_hir_facts` 数据产品现在承载 HIR barrier 后发布给 MIR/effect/LIR/backend 的源码语义事实；`scoopc_mir_facts` 数据产品承载 MIR-owned root inventories、materialized snapshot binding、instance/callable family inventory、pass artifact metadata 与 MIR pass pipeline metadata；`scoopc_effect_facts` 数据产品承载 effect/control snapshot binding、callable/body/site facts、step schema 与 continuation schema 的 stage-independent 边界，P4 输出只发布 effect facts 而不嵌套 `MirStageOutput`；`scoopc_lir_facts` 数据产品承载 LIR stage summary、callable ABI、dynamic invoke、dispatch owner/slot、global init/storage/final-entry contracts、physical layout、callable symbols、type context bridge、resume packing、continuation object、surface-resume dispatch contracts 与 LIR opt metadata。LLVM backend 的 entry/global、reachability、body emission、stage handoff 和 physical ABI/layout 已按 P7-T05/P8-T01 收口到 `LIR + lir_facts + base context`；未来 C backend 必须复用同一套 backend-neutral 输入边界，而不能复制旧 HIR/raw MIR/effect facts 回看路径。跨进程 `TypeId` 通过 per-cone `type_store.bin` 的 portable `TypeStore` serialization 恢复，下游进程先重建本地同构 type universe 再消费 persisted facts/LIR。
Kotlin runtime / Scoop core runtime gap 的能力矩阵审计见 `KOTLIN_RUNTIME_GAP_AUDIT.md`（T1314）。
标准库（std）分层与 capability matrix 设计见 `STDLIB_DESIGN.md`（T1316）。
effect lowering 的统一状态机设计基线见 `docs/effect_unified_state_machine.md`（T2003u1）。

## 语法速览

当前前端关于局部 block、closure 和 annotated block 的规则已经固定为：

- 普通局部 block 必须写作 `do { ... }`。
- bare `{ ... }` 始终是 closure；放在调用后缀位置时就是 trailing lambda，也支持多个 trailing lambdas。
- 局部 unsafe block 必须写作 `@Unsafe do { ... }`。
- `@Safe do { ... }` 是局部 safe block；`@Safe { ... }` 是 annotated closure。

```kotlin
val blockValue = do {
  val base = 40
  base + 2
}

val closureValue = { x: Int -> x + 1 }

combine(do { 3 }) { it + 1 } { it + 2 } // 普通实参 + multiple trailing lambdas

@Unsafe do {
  submitToKernel(buffer)
}

val safeCallback = @Safe { text: String ->
  text.length
}
```

## 快速开始

前提：
- Rust（建议最新版 stable）
- clang（用于 C runtime 构建）
- LLVM 21.1（默认需要；需提供 `llvm-config`。如需在未安装 LLVM 的环境构建，可使用 `--no-default-features` 关闭 LLVM 后端）

构建：

```bash
cargo build
```

> 注：当前 LLVM 后端使用 inkwell 的 `llvm21-1` 绑定，请安装对应版本的 LLVM 并确保 `llvm-config` 可被找到。
>
> 例如 macOS + Homebrew（llvm@21）：
>
> ```bash
> export PATH="/opt/homebrew/opt/llvm@21/bin:$PATH"
> ```
>
> 也可以显式指定：
> - `LLVM_CONFIG_PATH=/path/to/llvm-config`
> - `LLVM_SYS_211_PREFIX=/path/to/llvm@21/prefix`（会使用 `$LLVM_SYS_211_PREFIX/bin/llvm-config`）

如需禁用 LLVM 后端（只构建前端/中端与 fixtures runner 的“无后端模式”）：

```bash
cargo build --no-default-features
```

生成 LLVM IR：

```bash
# 需要确保 llvm-config 可被找到；例如 macOS + Homebrew（llvm@21）：
# export PATH="/opt/homebrew/opt/llvm@21/bin:$PATH"
PATH="/opt/homebrew/opt/llvm@21/bin:$PATH" \
  cargo run -p scoop -- \
  build --emit-llvm tests/fixtures/spec_doctest/overview_minimal_main.scoop \
  -o /tmp/overview_minimal_main.ll
```

运行 CLI：

```bash
cargo run -p scoop -- --help
```

跑 fixtures（递归执行 `tests/fixtures/**`；按 phase 目录路由执行，未实现 phase 会给出清晰诊断）：

```bash
cargo run -p scoop -- test
```

生成最小可执行文件（需要启用 LLVM 后端，并安装 `llvm-config`；链接阶段使用 clang）：

```bash
# 需要确保 llvm-config 可被找到；例如 macOS + Homebrew（llvm@21）：
# export PATH="/opt/homebrew/opt/llvm@21/bin:$PATH"
PATH="/opt/homebrew/opt/llvm@21/bin:$PATH" \
  cargo run -p scoop -- \
  build tests/fixtures/spec_doctest/overview_minimal_main.scoop -o /tmp/overview_minimal_main
```

运行程序（同样需要启用 LLVM 后端；`run` 会把产物写到临时目录后执行，并透传退出码）：

```bash
# 需要确保 llvm-config 可被找到；例如 macOS + Homebrew（llvm@21）：
# export PATH="/opt/homebrew/opt/llvm@21/bin:$PATH"
PATH="/opt/homebrew/opt/llvm@21/bin:$PATH" \
  cargo run -p scoop -- \
  run tests/fixtures/spec_doctest/overview_minimal_main.scoop
```

`scoop build` / `scoop run` 支持 `-j, --jobs <N>`（或 `SCOOP_BUILD_JOBS`，默认 4）控制互不依赖 cone 的并发子进程编译；也支持 `--sysroot-dep <name>`（可重复）来显式启用额外 sysroot source cone，CLI 值优先于 `SCOOP_SYSROOT_DEPS` 环境变量。单文件输入会先由 `scoop` materialize 为 `build/<profile>/virtual/<name>@0.0.0/` 下的标准 cone，再走 `scoopc build-single-cone` / `scoopc link-cone` 子进程 build/link 路径；`scoop` facade 不再 in-process 跑 frontend、codegen、runtime 编译或 final link。

当前 executable `main` 只接受四种形状：`fun main(): Unit / Pure!`、`fun main(): Int / Pure!`、`fun main(args: Array<String>): Unit / Pure!`、`fun main(args: Array<String>): Int / Pure!`。若使用 `main(args)`，可在 `scoop run` 后用 `--` 继续传参；运行时会把完整 native argv 传入 `args`（包含 `argv[0]`）。正常返回 `Unit` 会映射为退出码 `0`，正常返回 `Int` 会把该值作为进程退出码。

```bash
PATH="/opt/homebrew/opt/llvm@21/bin:$PATH" \
  cargo run -p scoop -- \
  run tests/fixtures/run-pass/std_process_args_exit_basic.scoop -- foo bar
```

## GC microbench（baseline vs Immix）

> 用途：本地对比 GC 分配吞吐与碎片化趋势；不做跨机器阈值 gating。

一键对比（推荐）：

```bash
# 吞吐
tools/gc_microbench.sh throughput --object-size 256 --rounds 50 --batch 50000

# 碎片化：稀疏存活（pin）导致 reserved bytes 升高（non-moving Immix 的典型现象）
tools/gc_microbench.sh fragmentation --object-size 256 --initial 200000 --pin-stride 100
```

直接跑单个 backend（更容易做参数扫描）：

```bash
cargo run -p scoop_runtime --release --bin gc_microbench -- \
  fragmentation --object-size 256 --initial 200000 --pin-stride 100
```

## Safepoint 基线

> 用途：可重复观察当前优化主线对 LLVM statepoint 数量与 `gc-live` roots 压力的影响，不做阈值 gating。

```bash
cargo run -p scoop_tools -- safepoint-baseline
```

该命令会自动编译一组内置 workload，并输出 `-O0` / `-O2` 下的 safepoint / roots 统计 Markdown 表。当前方法、workload 与最新快照记录在 `docs/safepoint_baseline.md`。

## 目录结构（简述）

- `crates/scoop/`：命令行 facade；负责 virtual cone materialization、cone DAG 调度与子进程派发
- `crates/scoopc/`：编译器 umbrella crate；只保留 facade re-exports、`frontend.rs`、`pipeline/`、`session/` 与 driver 编排 helper
- `crates/scoopc_span/`：基础 span / 诊断坐标 crate；后续 stage/fact crate 可共享的 span owner
- `crates/scoopc_source/`：基础 source identity / source map crate；不承载 cone/project membership
- `crates/scoopc_types/`：基础 type universe / effect row crate；负责后续跨阶段共享类型基础设施
- `crates/scoopc_ids/`：基础 stable identity crate；负责后续跨阶段 ID 与 stable key primitives
- `crates/scoop_project_model/`：基础 project / source-cone / cone compilation unit 模型与 cone artifact metadata crate
- `crates/scoopc_ast/`：AST stage crate；承载 lexer / parser / syntax / AST 数据结构
- `crates/scoopc_hir/`：HIR stage crate；承载 resolve / infer / typecheck / typed-HIR lowering
- `crates/scoopc_mir/`：MIR stage crate；承载 direct-style MIR、monomorph / RTTI / devirtualization 与 MIR materialization
- `crates/scoopc_effect_facts_stage/`：effect facts stage crate；从 MIR handoff 发布 effect/control facts
- `crates/scoopc_lir/`：LIR stage crate；承载 effect lowering、late LIR、LIR opt 与 backend-neutral source-body handoff
- `crates/scoopc_codegen_llvm/`：LLVM codegen stage crate；拥有 LLVM/inkwell/llvm-sys 依赖与 stackmap parser
- `crates/scoopc_cone/`：cone stage-payload 磁盘层；依赖 stage/fact/base crate 执行 archive、visibility、pre-specialize、consume 等跨 cone 操作，stage crate 不反向依赖它
- `crates/scoopc_hir_facts/`：独立 HIR semantic facts 数据产品；只依赖基础 crate，不依赖 `scoopc` facade 或 stage/backend crate
- `crates/scoopc_mir_facts/`：独立 MIR facts 数据产品；承载 root inventories、snapshot/pass-artifact binding 与 MIR pass pipeline metadata；只依赖基础 crate，不依赖 `scoopc` facade、HIR/MIR stage 或 backend crate
- `crates/scoopc_effect_facts/`：独立 effect/control facts 数据产品；承载 snapshot binding、callable/body/site facts、step schema 与 continuation schema；只依赖基础 crate，不依赖 `scoopc` facade、MIR/LIR stage 或 backend crate；由 P4 只读分析阶段发布，不回写 MIR snapshot，也不通过 P4 output 嵌套上游 stage output
- `crates/scoopc_lir_facts/`：独立 LIR facts 数据产品；承载 LIR stage summary、callable ABI、dynamic invoke、dispatch owner/slot、global init/storage/final-entry contracts、physical layout、callable symbols、type context bridge、continuation/resume publication contracts 与 LIR opt pipeline metadata；只依赖基础 crate，不依赖 `scoopc` facade、effect/MIR/LIR stage 或 backend crate；是 LLVM 与未来 backend 的 backend-neutral 输入基础
- `crates/scoop_runtime/`：运行时构建（C runtime 的 build glue）
- `runtime/c/`：C 运行时实现（GC/effect/线程等；平台差异收敛在 platform/backends）
- `tests/fixtures/`：编译期/运行期 fixtures（长期保证正确性）

`scoopc` 通过 `scoopc::base::{span, source, types, ids, project_model}`、stage facade（`ast` / `hir` / `mir` / `effect_lowered` / `llvm`）以及 fact facade（`hir_facts` / `mir_facts` / `effect_facts_product` / `lir_facts_product`）提供兼容入口；stage/fact/cone crate 应直接依赖对应基础 crate、fact crate 或前序 stage crate，而不是反向依赖 `scoopc`。P9 之后，任何 stage 行为改动都必须在 owning stage crate 内完成；不得在 `scoopc` umbrella、其它 stage crate、cone crate 或 backend crate 中新增跨 crate 兜底实现来绕过边界。`ProjectInput::build_closure_sources()` 是 source-cone DAG 的 build-closure source view，不是单一 compilation unit；需要 cone 级语义时使用 `compilation_units()` / `consumer_compilation_unit()`。
