# Scoop

Scoop 是一个 Kotlin 风格的静态类型语言，目标是：

- 真正的值类型（struct/enum/tuple，copy 语义、不可变）
- 代数效果系统（统一 async、错误处理、控制流）
- 泛型单态化（monomorphization）
- 编译期执行与静态反射（`const fun` / `comptime`）
- LLVM 后端（Rust `inkwell`），自带运行时与 GC（长期 C runtime/GC；平台差异隔离在 `runtime/c`）

语言规范见 `SCOOP_FULL_SPEC.md`，实现路线图见 `PLAN.md`。
Kotlin runtime / Scoop core runtime gap 的能力矩阵审计见 `KOTLIN_RUNTIME_GAP_AUDIT.md`（T1314）。
标准库（std）分层与 capability matrix 设计见 `STDLIB_DESIGN.md`（T1316）。

## 快速开始

前提：
- Rust（建议最新版 stable）
- clang（用于 C runtime 构建）
- LLVM（可选：仅当启用 `scoopc` 的 `llvm` feature 时需要；需提供 `llvm-config`）

构建：

```bash
cargo build
```

如需启用 LLVM 后端（inkwell）：

```bash
cargo build -p scoopc --features llvm
```

> 注：当前 `llvm` feature 选择了 inkwell 的 `llvm18-1` 绑定，请安装对应版本的 LLVM 并确保 `llvm-config` 在 PATH 中。

生成最小 LLVM IR（当前阶段仅生成空 `main`，返回 0）：

```bash
# 需要确保 llvm-config 可被找到；例如 macOS + Homebrew（llvm@18）：
# export PATH="/opt/homebrew/opt/llvm@18/bin:$PATH"
PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" \
  cargo run -p scoopc --features llvm --bin scoopc -- \
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
# 需要确保 llvm-config 可被找到；例如 macOS + Homebrew（llvm@18）：
# export PATH="/opt/homebrew/opt/llvm@18/bin:$PATH"
PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" \
  cargo run -p scoop --features llvm -- \
  build tests/fixtures/spec_doctest/overview_minimal_main.scoop -o /tmp/overview_minimal_main
```

运行程序（同样需要启用 LLVM 后端；`run` 会把产物写到临时目录后执行，并透传退出码）：

```bash
# 需要确保 llvm-config 可被找到；例如 macOS + Homebrew（llvm@18）：
# export PATH="/opt/homebrew/opt/llvm@18/bin:$PATH"
PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" \
  cargo run -p scoop --features llvm -- \
  run tests/fixtures/spec_doctest/overview_minimal_main.scoop
```

## 目录结构（简述）

- `crates/scoop/`：命令行工具（driver）
- `crates/scoopc/`：编译器核心库（前端/中端/后端）
- `crates/scoop_runtime/`：运行时构建（C runtime 的 build glue）
- `runtime/c/`：C 运行时实现（GC/effect/线程等；平台差异收敛在 platform/backends）
- `tests/fixtures/`：编译期/运行期 fixtures（长期保证正确性）
