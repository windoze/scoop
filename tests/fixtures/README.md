# Scoop 文件测试

用例按职责和功能组织，不按实现里程碑排列。普通 API 和 consumer 使用 Cone 测试；fixture 只覆盖 scoop.test 无法完成的编译器、命令行或外部进程边界；具体执行条件遵守 [fixture schema 1](../fixture_runner/README.md)。

| 目录 | 当前用例数 | 职责 |
| --- | ---: | --- |
| `language/` | 3,415 | 语言规则、诊断、求值和跨 Cone 语义 |
| `libraries/` | 71 | 测试命令入口、独立外部协议、native 和进程边界 |
| `driver/` | 297 | CLI、输入、缓存、模板、脚本、测试命令和文档 |
| `compiler-correctness/` | 345 | IR、优化、产物、ODR、ABI 及默认 profile 合同 |
| `gc-correctness/` | 59 | 根、屏障、搬迁、回收、safepoint 和 stress 构建开关 |
| `regressions/` | 13 | 按具体故障形态保留的回归 |

普通 fixture 与内置 Cone 测试只运行 release；编译器和 GC 专测保留实际需要的模式组合。GC 专测先显式编入 `--enable-gc-stress`，再由环境变量选择运行行为；普通测试不继承调用者的 stress 环境。`scoop doc` 没有 profile 参数，按原文档/HIR 合同覆盖。详见 [测试模式](../../docs/minor-fixes/TEST-MODES.md)。

默认复用准备好的 sysroot 和共享 Cone cache，不逐项复制系统库或重复构建；`--cache-dir` 可独立于 `--work-dir` 指定，共享目录默认取 `SCOOP_CACHE_DIR` 或仓库 `target/fixture-cache`。只有被测事实要求冷构建、修改输入或隔离发布时才使用局部缓存。

每项用例用相邻 `name.fixture.toml` 或目录中的 `fixture.toml` 声明输入、步骤、完整诊断/输出和退出状态。共享源码跟随相应功能，跨组使用明确的相对引用；所有 `.scoop` 必须属于某个 inputs 或 support，支持文件不单独计作通过。程序中的历史 `EXPECT-*` 注释不参与判断。schema 仍支持内联载体，现有集合使用外置声明。

```sh
# 完整验收；未归属源码和无效引用会报配置错误。
python3 tests/run_fixtures.py --all

# 两个 Linux libc 目标分别验收。
python3 tests/run_fixtures.py --all --target x86_64-unknown-linux-gnu
python3 tests/run_fixtures.py --all --target x86_64-unknown-linux-musl

# 开发时按功能目录或名称选择。
python3 tests/run_fixtures.py --suite tests/fixtures/driver/new
python3 tests/run_fixtures.py --filter 'single-file-*'

# 显式更新选中的结构期望，审阅差异后再执行只读验证。
python3 tests/run_fixtures.py --suite tests/fixtures/compiler-correctness/optimization/calls/local-values \
  --update-snapshots
```

`--all` 禁止过滤和更新期望。`cargo test` 不能替代正式 CLI fixture；`--jobs` 控制并行度，单用例内部步骤和变体仍有序。平台不适用项单列，不计为通过；真实 ABI/链接差异按 target 保存，其余期望共用。

原始路径与名称可从迁移前 Git 版本追溯；[分类记录](../../docs/minor-fixes/FIXTURES.md)和[库测试迁移](../../docs/minor-fixes/LIBRARY-TESTS.md)记录保留的职责。里程碑文档保留当时路径作为历史记录。工具及 sysroot 准备见 [脚本说明](../../scripts/README.md)。
