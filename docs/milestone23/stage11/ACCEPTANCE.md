# M23-11 验收记录：公开 CLI、单文件模式与总回归

状态：已完成并验收（2026-10-03）。最终代码与测试基线为 `5a0e9bd2d`，设计基线为 `0b180dde6`；本记录和阶段状态的后续提交只更新文档。前置为已验收的 [M23-10](../stage10/ACCEPTANCE.md)，完成条件见 [阶段设计](DESIGN.md)。

## 1. 实际交付

公开 `scoop build/run/link` 已接通同一套依赖图、快照、缓存、配套 compiler、runtime 构建与 program-link。library 无需 main；executable 先产生完整 `.slib`，再链接和原子发布。直接 `scoopc` 继续只编译一个 Cone，低层 `scoop-link` 继续消费显式产物闭包。

```sh
scoop build
scoop run -- first "two words"
scoop build --release
scoop build hello.scoop -o out/hello
scoop build libraries/math -o out/math.slib
scoop link --root-slib out/app.slib --dependency-slib out/core.slib \
  --runtime-objects runtime/index.cbor --target aarch64-apple-darwin -o out/program
```

省略 root 使用当前目录 `Cone.toml`；显式 single-file 使用唯一源码、固定 synthetic identity 和 core-only Cone 依赖。相邻 manifest、其他 Scoop/C/C++ 文件及 blob 不会被发现。已有 typed FFI requirement 仍可通过显式 `--library-path` 解析。

debug/release、默认 debug、`--release`、`--target-dir` 与 `target/<canonical-target-triple>/<profile>` 输出布局已实现。两种 profile 当前使用相同实际编译配置，优化设置仍按设计留待后续实现。`run` 完成稳定输出发布后执行同次结果，保留公开产物，正确传递原始 argv、stdio、cwd、environment、exit 和 signal，并清理私有执行文件与取消的 child。

诊断 primary／note 保留 canonical source 与独立显示位置；warning 可随缓存重放。协议 2 从一次编译返回实际 AST／HIR／MIR／LIR 文件；普通 cache hit 零 compiler child，显式观察只重新编译指定 source 节点一次。公开 link 只读取已有 `.slib`、runtime index 和 native 输入，不启动 Scoop compiler、不读取源码或探测 LLVM。

## 2. 完成门与真实覆盖

| 完成门 | 实际验收入口与记录 |
| --- | --- |
| 基本命令、library、默认输出、profile、single-file、run 进程行为 | `m23-cli` 的 basic／library／process；[输入迁移](MIGRATION.md#13-共享输入分类与-host-诊断)与 `m23-cli-inputs` |
| core 来源、cold／warm、依赖失效、只观察指定节点、缓存诊断 | [CLI-CACHE](CLI-CACHE.md)、[CLI-JSON](CLI-JSON.md)、`m23-cli-diagnostics` |
| 独立产物链接、原子发布、损坏、可重复性、native object／archive／dynamic | [迁移记录第 2～10 节](MIGRATION.md#2-program-link-值协程odr-和初始化组合)、`m23-cli-program-link`、`m23-cli-native-link` |
| 共同 HIR、声明位置、默认值、可见性与再次发布 | [源码调用](CLI-SOURCE-CALLS.md)、[默认参数](CLI-SOURCE-DEFAULTS.md)、[源码边界](CLI-SOURCE-BOUNDARIES.md)、[产物发布](CLI-HIR-PUBLICATION.md) |
| 双向泛型、兄弟实例、真实符号、ODR 与初始化 | [泛型成员](CLI-GENERIC-MEMBERS.md)、[兄弟实例](CLI-GENERIC-SIBLINGS.md)、[机器结果](CLI-GENERIC-MACHINE.md)、[冲突](CLI-GENERIC-CONFLICTS.md) |
| 值类型、接口、ZST、core 修改和真实 ABI | [core 布局](CLI-CORE-LAYOUTS.md)、[重建 core](CLI-REBUILT-CORE.md)、[MIR 生产](CLI-MIR-PRODUCTION.md)、[nominal runtime](CLI-NOMINAL-RUNTIME.md) |
| GC、异常、闭包、协程、FFI 与跨线程组合 | [suspend 成员](CLI-SUSPEND-MEMBERS.md)、[native 源码](CLI-NATIVE-SOURCE.md)、[runtime image](CLI-RUNTIME-IMAGES.md)、[生命周期](CLI-RUNTIME-LIFECYCLE.md) |
| 历史覆盖、统一声明、只加数据扩展、旧 infra 退役 | [历史对应表](LEGACY-COVERAGE.md)、[迁移记录](MIGRATION.md)、[清理记录](INFRA-CLEANUP.md)、[fixture schema 1](../../../tests/fixture_runner/README.md) |

全部文件 fixture 使用 `tests/run_fixtures.py` 和 `tests/fixture_runner` 的同一 schema 1；内联注释和附加 TOML 只提供同一种声明。新增功能用例、negative、native companion、普通／moving GC、cold／warm 与多 Cone 编排均由公共步骤表达，未新增按 case 名称分支的解释器。C companion 由明确的测试步骤构建，仅通过已有 library requirement 和搜索根参与产品链接。

最终发现 **2067 个 fixture、3040 份 Scoop 源码、0 份未归属源码**。原 M1–M22/M25 的 **534 份源码**与 M0 smoke 均保留；原 480 份合并式端到端用例的对应表覆盖成功、negative、原有 moving 条件，另外 54 份阶段源码也已明确归属。最终报告不含未选中、未运行、inapplicable 或失败项。

删除旧 Rust 文件 fixture runner、专用源码编译／快照维护／临时 link/startup helper 及其注册；普通 typed 内部单元测试保留。剩余 35 份 `.snap` 只服务直接构造 typed 数据的 MIR stage、shared type uses、native boundary 与 external boxing 单测，不再形成第二套语言源码 fixture 执行入口。详见 [清理记录](INFRA-CLEANUP.md)。

## 3. 全量发现的旧预期与严格复验

首轮无筛选 CLI 为 2004 passed／63 failed，不能作为完成证据。失败项按实际原因修正，每批保留原源码、运行结果和诊断，先格式化与 lint，再通过正式 runner 只读复验并独立提交：

| 原因 | fixture 数 | 变体／进程／golden | 审阅后的修正 |
| --- | --- | --- | --- |
| single-file 参数 | 4 | 4／32／52 | 四个 single 步骤移除多余 Cone locator，provider／consumer 输入不变 |
| 常量条件 | 2 | 2／18／22 | LIR 删除不可达分支，精确更新一个实际 executable 产物指纹 |
| 接收者调用 | 12 | 12／131／104 | HIR 使用 MethodCall／DirectSuperMethodCall，原 callee、参数及来源保留 |
| handle C 表示 | 2 | 4／12／26 | 复用唯一原字段身份的 UInt64Field 投影，Scoop aggregate ABI 保持 |
| suspend 发布 | 43 | 61／220／321 | 补齐已有成员和闭合物化，原函数正文及全部原符号保留 |

构造器修正见 [CLI-CONSTRUCTORS](CLI-CONSTRUCTORS.md)；常量分支见 [CLI-CONSTANT-BRANCHES](CLI-CONSTANT-BRANCHES.md)；其余见上表对应的源码调用、native 和 suspend 记录。受控快照观察只用于建立可审阅差异，不计作只读验收。

suspend 的 148 份阶段／计划差异按真实持久身份、符号和必要 arena 映射逐项审阅：原 MIR／LIR 函数均存在且正文不变，新增物化有实际调用与发布结果；十二份计划只把 provider 对象数从 74 改为 100。七组完整符号列表保留所有旧符号，每组增加 1005 个符号、426 个强符号，继续完整比较。core 的第三个初始化单元和 ensure 路径同时进入源码与物化集合。

临时观察曾因不同磁盘目录的 group id 产生 Apple `ar` 归档头差异；`ZERO_AR_DATE` 不清除 uid/gid。已在与正式验收一致的 `/tmp` 条件下重新生成并核对，native archive 指纹及原严格比较保持不变，没有把宿主目录差异当作编译器修复或放宽计划检查。

## 4. 最终无筛选验证

本机为 macOS 27.0.1（26A434）、Apple Silicon。Rust／Cargo 为 1.99.0；Scoop 使用外部 LLVM **22.1.8**，Rust 自带 LLVM 为 23.1.1；native 工具为 Apple clang 21.0.0、macOS 27.0 SDK。Python 3.14.7、Ruff 0.16.10。

开始验收前清除全部 `SCOOP_UPDATE_*`、含 `SNAPSHOT` 的 `SCOOP_*`、`INSTA_*`、`UPDATE_EXPECT`、`RUST_MIN_STACK` 和全局 `SCOOP_GC_STRESS_MOVE`。stress 仅由 fixture 声明设置到目标程序。Rust 使用 dev/test opt-level 1、debug 0、关闭 incremental；未关闭 debug assertions 或 overflow checks。三个配套工具由当前源码重新构建后复制到 `/tmp/scoop-m23-11-tools`，避免后续清理 target 影响实际测试进程。

```sh
export LLVM_SYS_221_PREFIX=/opt/homebrew/opt/llvm@22
export LLVM_CONFIG_PATH=/opt/homebrew/opt/llvm@22/bin/llvm-config
export CARGO_TARGET_DIR="$PWD/target"
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_PROFILE_TEST_OPT_LEVEL=1
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
export SCOOP_TEST_PAIRED_SCOOP=/tmp/scoop-m23-11-tools/scoop
export SCOOP_TEST_PAIRED_SCOOPC=/tmp/scoop-m23-11-tools/scoopc
export SCOOP_TEST_PAIRED_SCOOP_LINK=/tmp/scoop-m23-11-tools/scoop-link
cargo fmt --all
uv tool run --from ruff==0.16.10 ruff format --check tests/fixture_runner tests/run_fixtures.py
uv tool run --from ruff==0.16.10 ruff check tests/fixture_runner tests/run_fixtures.py
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p scoop -p scoopc -p scoop-linker --bins
python3 -m unittest discover -s tests/fixture_runner/tests
cargo test --workspace --no-fail-fast
python3 tests/run_fixtures.py --all \
  --work-dir /tmp/scoop-m23-11-acceptance/cli-final --jobs 8
```

完整 Rust 与完整 CLI 均无筛选，CLI 的 `--all` 禁止同时启用筛选和快照更新。两者独立并行执行，正常退出码均为 0；Rust 结果不替代 CLI 覆盖。

| 检查 | 最终结果 |
| --- | --- |
| Rust fmt、Clippy、Ruff format/check | 全部通过，Clippy 将 warning 视为错误 |
| 配套 scoop／scoopc／scoop-link 构建 | 通过，记录三个文件的完整 SHA-256 |
| Python 公共规则单元测试 | 29 passed |
| Cargo workspace | 43 个测试组，**5197 passed**，0 failed／ignored／filtered，291.75 秒 |
| 完整 CLI fixture suite | **2067／2067 passed**，0 unselected／inapplicable／failed，1839.11 秒 |
| CLI 实际执行 | **2157 个变体、11602 次进程、11012 次阶段与链接计划 golden 比较** |

golden 数是包含变体的实际比较次数，不是唯一文件数；进程数是 fixture runner 记录的启动数。

本机证据保留在 `/tmp/scoop-m23-11-acceptance`：`cli-final/report.json`、`cli-final.log`、`rust-final.log`、`rust-final-counts.json`、`final-python-unit.log`、`final-fmt.log`、`final-clippy.log`、`final-ruff-format.log`、`final-ruff-check.log`、`final-paired-tools.json`、`final-context.json` 和 `environment.json`。配套构建、Python 单元测试与两套全量运行的命令、退出码和耗时保存在对应 JSON；五组修正的审阅与只读报告位于 `/tmp/scoop-m23-11-final-repairs`。首轮报告保留于 `cli-full/report.json`。两轮大体积工作目录在各自结束后移至 `/Volumes/Data/home/chenxu/tmp/scoop-m23-11-completed`，原路径保留符号链接；归档内的 bytes 不变，未在迁移后的目录重跑或更新预期。

## 5. 提交、代码长度与清理

相对设计基线，已按功能独立提交 404 次（含设计、实现、迁移及预期修正；不含本验收文档提交）。完整顺序记录在 `/tmp/scoop-m23-11-acceptance/feature-commits.txt`。公开 CLI、build、run、artifact-link、诊断／dump、protocol 和 Python 发现／schema／执行／断言按职责拆分；新增 Rust／Python 文件最长 **433 行**，没有将整项功能堆入入口文件。

期间多次在 Cargo 停止后清理 target。最终全量前一次清理移除 2891 个文件、3.0 GiB；全部验收结束后再次执行 `cargo clean`，结果为 `Removed 28159 files, 25.4GiB total`。日志为 `target-clean-before.log` 和 `target-clean-after.log`，独立配套工具及验收记录保留。

本阶段交付范围保持 Darwin/AArch64、完整静态 Cone 图与一个主 executable；runtime metadata ABI 3 保持。final-link cache、发布安装器、watch／daemon、自动 native source 发现、release 优化及 M24/M26/M27 仍按原路线图安排。未增加来源授权、防伪、通用资源预算或另一套语言语义验证流程。
