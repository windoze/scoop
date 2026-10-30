# Scoop

Scoop 是一门静态类型、编译到原生代码的编程语言。它以 Kotlin 核心语法为基础，引入真正的值类型，使用单态化泛型和 LLVM 后端，并由 C runtime 提供分代 Immix GC。

项目仍处于未发布阶段，语言、库 API、ABI 和编译产物格式会随实现直接演进。当前进度和待完成能力见[实现路线图](docs/ROADMAP.md)。

## 主要特性

- Kotlin 风格的函数、类、接口、扩展、lambda、模式匹配与 `suspend` 协程。
- 原生 `struct`、`enum` 和 tuple 值类型，支持解构；使用 `Option<T>` 表达可选值。
- 单态化泛型、静态类型检查，以及使用 `f"...${...}..."` 的字符串插值。
- LLVM 原生代码生成、精确移动 GC、异常处理、线程与 C FFI。
- 以 Cone 为编译和包管理单位，通过 `Cone.toml` 描述项目，使用 `.slib` 保存编译产物。
- 配套构建、测试、格式化、文档和包管理命令，以及 LSP、Tree-sitter 和 VS Code 扩展。

Scoop 对 Kotlin 核心语法作了明确取舍；语言差异及完整规则见[语言规范](docs/specs/SCOOP-SPEC.md)。

## 快速体验

先按[工具包安装说明](scripts/distribution/README.md)准备工具链和系统库，或按下文从源码构建。将下面的程序保存为 `hello.scoop`：

```scoop
struct Point(val x: Int, val y: Int)

fun main() {
    val point = Point(3, 4)
    println(f"Hello, Scoop! (${point.x}, ${point.y})")
}
```

```sh
scoop run --release hello.scoop
```

输出：

```text
Hello, Scoop! (3, 4)
```

单文件程序不需要 `Cone.toml`。创建一个包含示例测试的项目：

```sh
scoop new --group examples.hello hello
cd hello
scoop run --release .
scoop test --release .
```

Windows 安装包的主命令为 `sco`，上述命令中的 `scoop` 替换为 `sco`；其他工具仍保留 `scoopc`、`scoop-link` 等名称。

更多完整程序见[示例目录](sysroot/share/scoop/examples)。安装包还提供 `scoop example --list` 和 `scoop example start/hello`，可离线查看示例、运行条件和预期结果。

## 从源码构建

编译器使用 Rust（edition 2024）和 inkwell 0.10。需要安装：

- 支持 edition 2024 的 Rust 工具链和 Cargo。
- 外部 LLVM 22.1；发行准备固定使用 LLVM 22.1.8。`llvm-config` 应在 `PATH` 中，或通过 `LLVM_SYS_221_PREFIX` 指向 LLVM 安装前缀。
- 所选目标的 C/C++ 工具链、系统 SDK 和链接器。
- Python 3.11+，用于开发准备脚本和 fixture runner；OpenSSL 准备需要 Python 3.12+。各 native 后端所需的其他工具见[准备脚本说明](scripts/README.md)。

以下为 macOS／Linux shell 示例，在仓库根目录执行：

```sh
cargo build --release -p scoop -p scoopc -p scoop-linker -p scoop-lsp -p scoop-pm --bins
export PATH="$PWD/target/release:$PATH"
```

如果设置了 `CARGO_TARGET_DIR`，相应调整命令路径。LLVM 是 Scoop 后端使用的外部依赖，不要求它与 Rust 工具链自带的 LLVM 版本相同。

运行 Scoop 程序前，还需要显式准备开发 sysroot。先按[准备脚本说明](scripts/README.md)构建目标所需的 native 后端，再准备 runtime 和内置库。例如，在 Apple Silicon macOS 上：

```sh
python3 scripts/prepare_sysroot.py \
  --target aarch64-apple-darwin \
  --scoop "$PWD/target/release/scoop" \
  --work-dir "$PWD/tmp/sysroot/aarch64-apple-darwin"
export SCOOP_SYSROOT="$PWD/sysroot"
```

普通构建和测试消费已准备的系统库与 runtime；首次运行或内置库变更后需重新执行对应准备步骤。开发构建通过 `SCOOP_SYSROOT` 选择仓库资源，安装包则默认使用安装目录中的资源。

目标包括 macOS AArch64／x86_64、Linux x86_64（glibc／musl），以及 Windows x86_64 的 MSVC UCRT、LLVM-MinGW MSVCRT／UCRT。目标与 ABI 规则见[编译器与产物规范](docs/specs/SCOOP-IMPL-SPEC.md)，构建环境基线见[发行配置](scripts/release.toml)；各目标需要匹配的 SDK 和 native 资源。

## 开发与验证

Rust 变更先格式化和 lint，再运行测试：

```sh
cargo fmt --all
cargo clippy --workspace --all-targets
cargo test --workspace
```

正式 CLI fixture 使用配套命令和已准备的开发 sysroot。runner 默认从 `target/debug` 读取工具，因此另行构建这些命令：

```sh
cargo build -p scoop -p scoopc -p scoop-linker -p scoop-lsp -p scoop-pm --bins
export SCOOP_SYSROOT="$PWD/sysroot"
python3 -m unittest discover -s tests/fixture_runner/tests
python3 tests/run_fixtures.py --all
```

`cargo test` 不能替代正式 CLI 验收。普通库行为优先放在所属 Cone 的 `scoop.test` 测试中；编译诊断、产物、链接、ABI 和命令行行为使用 fixture。工具路径覆盖、定向运行和声明格式见[fixture runner 说明](tests/fixture_runner/README.md)，贡献时请遵循[仓库开发约定](AGENTS.md)。

## 仓库结构

| 路径 | 内容 |
| --- | --- |
| [compiler/](compiler/) | Rust 编译器、各阶段 IR、构建驱动、链接器、包管理器和语言服务 |
| [runtime/](runtime/) | C runtime、GC、异常与 FFI 支持 |
| [sysroot/src/](sysroot/src/) | core、标准基础库及其他内置 Cone 的 Scoop 源码 |
| [sysroot/vendor/](sysroot/vendor/) | native 后端及其他第三方组件的固定输入和许可说明 |
| [tests/](tests/) | Cone 测试、编译器 fixture 及公共 runner |
| [tooling/](tooling/) | Tree-sitter、VS Code 扩展、示例阅读器及配套编程资料 |
| [scripts/](scripts/) | 开发 sysroot 准备、native 构建和工具链打包 |
| [docs/](docs/) | 规范、设计、路线图和验收概要 |
| [benchmark/](benchmark/) | 性能基准与运行说明 |

编译流水线为 `source → AST → HIR → MIR → LIR → LLVM → native code`。各 stage 通过独立的 IR/meta crate 交换数据，由 driver 编排。

## 文档

- [语言规范](docs/specs/SCOOP-SPEC.md)：语法、类型系统、核心库与 FFI。
- [运行时规范](docs/specs/SCOOP-RUNTIME-SPEC.md)：对象模型、GC、异常与运行时契约。
- [编译器与产物规范](docs/specs/SCOOP-IMPL-SPEC.md)：编译阶段、实体身份、目标 ABI、构建、链接与 `.slib`。
- [实现路线图](docs/ROADMAP.md)：里程碑、实现进度和待完成事项。
- [库 API 风格](docs/specs/SCOOP-API-STYLE.md)与[标准基础库规范](docs/specs/SCOOP-STDLIB-SPEC.md)。
- [编辑器工具说明](tooling/README.md)与[VS Code 扩展说明](tooling/vscode-scoop/README.md)。

## 许可证

除另有说明外，Scoop 采用双许可证，使用者可以任选其一：

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

SPDX 许可证表达式：`MIT OR Apache-2.0`。

第三方组件遵循各自的许可证，相关文本与声明保留在对应组件目录中。
