# Scoop

Scoop 是一个 Kotlin 风格的静态类型语言，目标是：

- 真正的值类型（struct/enum/tuple，copy 语义、不可变）
- 代数效果系统（统一 async、错误处理、控制流）
- 泛型单态化（monomorphization）
- 编译期执行与静态反射（`const fun` / `comptime`）
- LLVM 后端（Rust `inkwell`），自带运行时与 GC（长期 C runtime/GC；平台差异隔离在 `runtime/c`）

语言规范见 `SCOOP_FULL_SPEC.md`，实现路线图见 `PLAN.md`。
当前 `const fun` 的声明级合同保持保守纯计算：只能省略 effect row，或显式写 `/ Pure` / `/ Pure!`，不支持 `<eff ...>`。
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

生成最小 LLVM IR（当前阶段仅生成空 `main`，返回 0）：

```bash
# 需要确保 llvm-config 可被找到；例如 macOS + Homebrew（llvm@21）：
# export PATH="/opt/homebrew/opt/llvm@21/bin:$PATH"
PATH="/opt/homebrew/opt/llvm@21/bin:$PATH" \
  cargo run -p scoopc --bin scoopc -- \
  --emit-llvm tests/fixtures/spec_doctest/overview_minimal_main.scoop \
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

## 目录结构（简述）

- `crates/scoop/`：命令行工具（driver）
- `crates/scoopc/`：编译器核心库（前端/中端/后端）
- `crates/scoop_runtime/`：运行时构建（C runtime 的 build glue）
- `runtime/c/`：C 运行时实现（GC/effect/线程等；平台差异收敛在 platform/backends）
- `tests/fixtures/`：编译期/运行期 fixtures（长期保证正确性）
