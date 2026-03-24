# Scoop

Scoop 是一个 Kotlin 风格的静态类型语言，目标是：

- 真正的值类型（struct/enum/tuple，copy 语义、不可变）
- 代数效果系统（统一 async、错误处理、控制流）
- 泛型单态化（monomorphization）
- 编译期执行与静态反射（`const fun` / `comptime`）
- LLVM 后端（Rust `inkwell`），自带运行时与 GC（早期用 C，后续迁移到 Scoop）

语言规范见 `SCOOP_FULL_SPEC.md`，实现路线图见 `PLAN.md`。

## 快速开始

前提：
- Rust（建议最新版 stable）
- clang（用于早期 C runtime 构建）
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

跑最小 fixtures（当前阶段只做 smoke：递归读取 `tests/fixtures/**/*.scoop`）：

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
- `crates/scoop_runtime/`：早期运行时构建（C runtime 的 build glue）
- `runtime/c/`：早期 C 运行时实现（GC/effect/线程等，逐步补齐）
- `tests/fixtures/`：编译期/运行期 fixtures（长期保证正确性）
