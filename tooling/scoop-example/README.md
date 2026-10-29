# Scoop 示例阅读器

`scoop example` 是安装在 `<SYSROOT>/commands/scoop-example` 的独立 Rust 程序，Windows 文件名为 `scoop-example.exe`。它通过已有外部命令入口运行，不依赖编译器 crate、LLVM 或系统库产物；查询时只读取配套的索引、说明和源码。

Windows 主驱动名为 `sco.exe`，通过 `sco example` 调用该阅读器；以下示例中的 `scoop` 在 Windows 上替换为 `sco`。macOS／Linux 继续使用 `scoop example`。

## 使用

```sh
scoop example
scoop example --list
scoop example language
scoop example language/option
scoop example language/compiler-inputs
scoop example language/imports
scoop example --search "JSON 序列化"
scoop example json/typed --format source
scoop example http/get --format source --file client.scoop
scoop example test/assertions --format json
```

默认输出中文 Markdown，含完整程序、运行方式、预期结果、规则和误用说明。source 输出所选文件原文；多文件主题必须指定显示路径。JSON 使用 `schema = 1`，kind 区分 index、list、group、search、topic 和 error。未知主题、无结果及 I/O 错误返回 1，参数错误返回 2；错误只写 stderr，正文完整准备后才写 stdout。

搜索以空白分词，要求各词均命中同一主题的 ID、标题、摘要、别名或关键词；英文大小写不敏感。检索只显示摘要与匹配字段，之后用主题 ID 阅读正文。推荐、分类和并列搜索结果遵循索引的编辑顺序。

显式选择安装根时使用 `scoop --sysroot /path/to/scoop example ...`。程序始终从自身可执行文件位置读取内容，环境变量不会把它导向另一份安装。把输出的文件保存到可写目录后，按正文运行；后续 run/check/test 也应使用同一安装根。命令不会运行示例或写出项目。

AI 编程工作流见配套 [scoop-programming skill](../scoop-programming/SKILL.md)：从主题检索和 JSON 文件还原开始，结合库 API、项目依赖及正式 CLI 完成验证。该目录随 dist 工具包提供，不自动安装到 AI 工具中。

## 开发安装

在仓库根目录执行；只需 Rust/Cargo，不构建编译器：

```sh
python3 scripts/build_scoop_example.py
SCOOP_SYSROOT="$PWD/sysroot" scoop example --list
```

构建缓存默认放在仓库旁的 `scoop-work/scoop-example/cargo`。`--target-dir` 可指定其他目录；`--sysroot` 可将命令和材料装入另一份已有的配套工具链。复用示例的源文件仍由该工具链的 `src/` 提供。完整发行脚本 `scripts/package_toolchain.py` 自动构建并安装本命令、打包内容。

脚本遵循 `CARGO_BUILD_TARGET` 选择的 Rust 宿主产物目录，并保留 `.exe` 后缀。完整 Windows 发行按实际 PE imports 将本命令所需的非系统 DLL 一并放在 `commands/`；独立开发安装使用对应 Rust 目标的正常运行库环境。

## 内容维护

编辑 [catalog.toml](../../sysroot/share/scoop/examples/catalog.toml) 注册已完成的主题。`document` 相对索引目录，`source` 相对安装根，`path` 是用户保存文件时的相对路径。已有库例子引用原源码；新增教学程序放在 `programs/<主题 ID>/`。正文使用“运行、预期结果、关键说明、常见误用”四节，不复制源码代码块。相关主题必须已经发布。

当前有 26 个完整主题，包括编译期资源／环境和块级 import；后续内容按[主题清单](../../docs/scoop-example/TOPICS.md)补充。新增主题时同步增加下述独立行为验证；不要将待补内容先放进 catalog。主题目录和三个输出形态详见[设计](../../docs/scoop-example/DESIGN.md)。

## 独立验证

在仓库根目录执行，只针对本程序：

```sh
cargo fmt --manifest-path tooling/scoop-example/Cargo.toml --check
cargo clippy --manifest-path tooling/scoop-example/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path tooling/scoop-example/Cargo.toml --locked
uv tool run --from ruff==0.16.10 ruff check tooling/scoop-example/tests/run_examples.py scripts/build_scoop_example.py
python3 -m unittest discover -s scripts/tests -p test_package_toolchain.py
python3 -m unittest discover -s scripts/tests -p test_package_licenses.py
```

使用已经准备好的本机工具链验证发布示例。下面的脚本从真实 `scoop example --format json` 还原显示文件，编译并运行全部已发布主题，检查固定输出、两个测试用例、命令行帮助/错误、临时资源清理以及本机 HTTP 会话。编译期输入示例还检查环境变量有值、缺失、空值三种构建，以及运行期环境不影响嵌入值：

```sh
python3 tooling/scoop-example/tests/run_examples.py \
  --scoop /path/to/installation/bin/scoop \
  --sysroot /path/to/installation \
  --work-dir ../scoop-work/scoop-example/validation
```

使用 release 和共享 Cone cache。HTTP 例子需要空闲的 127.0.0.1:8080；脚本只终止自己启动的服务进程。`--topics language/option json/typed` 可选择复验范围。日志、还原文件与编译输出保留在 work-dir，不提交。该脚本不运行编译器测试套件，也不准备或重编系统库。

仅验证本次编译期输入和导入示例时，追加 `--topics language/compiler-inputs language/imports`。
