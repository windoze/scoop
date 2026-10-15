# M39 验收记录

> 原始日志、逐次 JSON 报告和压缩归档已移出版本库。本页保留验收范围、结果与限制；文中的历史报告名称仅标识当时的验证批次。

状态：已完成并验收，2026-10-13。最终代码提交为 `e7f2fb7d`；后续提交仅记录验证与验收。

实施基线：`e88f7d73`。范围见 [设计](DESIGN.md)，逐功能验证见 [实施记录](PROGRESS.md)。
本次只重构现有 Darwin/AArch64、Linux GNU/musl x86-64 的目标边界，不登记新架构。

## 验证环境

| 宿主 | 系统与架构 | Rust | 外部 LLVM | 其他工具 |
| --- | --- | --- | --- | --- |
| 本机 | Darwin 27.0.0，arm64 | 1.99.0 | 22.1.8 | Python 3.15.0 |
| nuc12 | Linux 7.0.0-38-generic，x86_64 | 1.99.0 | 22.1.2 | GCC 15.2.0、musl-gcc、Python 3.14.4 |

两个宿主均在仓库 `target/m39/` 保存临时文件、日志及 fixture 工作目录，没有使用 `/tmp`。
Linux 的 native unwind 安装保留并复用，正式 CLI 使用 release 构建。

## 功能提交

| 提交 | 完成内容 |
| --- | --- |
| `fe1f2b19` | 先同步语言、runtime、pipeline 规范及路线图 |
| `d61f67f8` | C toolchain invocation 与 generated-C 编译执行归属 |
| `c5d3e6e6` | runtime 的 OS/image/arch 组件及浮点环境 |
| `cbe686dc` | 导入 provider 物理 ABI 的本地存储绑定 |
| `725921d7` | Linux machine/libc 与实际 GCC 选择 |
| `594a6878` | ELF 容器和机器规则 |
| `406f24e4` | manifest 接收显式源码目标目录 |
| `ab266f9a` | 完整签名 ABI 规划与准备／完成阶段交接 |
| `0d31b793` | 静态目标布局及 backend 合同归属 |
| `d41900eb` | C producer 身份格式与工具执行支持范围 |
| `fe359f8b` | native 符号归一化显式传入目标数据 |
| `2fbbca3b` | target 身份格式与消费者支持范围 |
| `bf6ead33` | 对象测试使用 canonical target 文件名 |
| `e7f2fb7d` | GNU 不注入 REALGCC，错误 libc driver 由 probe 拒绝 |

## 边界审查

| 设计问题 | 当前实现与验证结论 |
| --- | --- |
| R01–R03：Scoop/C 分类 | [ABI planner](../../compiler/codegen/abi-plan/src/lib.rs)按完整签名规划，Scoop、DirectC、HFA、eightbyte、寄存器耗尽和 C 扩展共用后端规则；公共 IR 只保留存储、carrier 和计划数据。 |
| R04：布局与 LLVM 配置 | [静态 target data](../../compiler/codegen/target-data/src/lib.rs)保存三个真实目标的描述；[codegen target](../../compiler/codegen/src/target.rs)选择已实现 LLVM 配置。静态模块无 LLVM、分类器或工具进程依赖。 |
| R05：身份与支持 | [target](../../compiler/identity/src/capability/target.rs)、[backend](../../compiler/identity/src/capability/backend.rs)及 [C toolchain](../../compiler/identity/src/capability/c_toolchain.rs) identity 只表示 canonical 名称和版本。实际消费者明确拒绝未知身份／版本／不相容组合，manifest 读取显式 SourceTarget 目录。 |
| R06：工具执行 | [toolchain invocation](../../compiler/toolchain/src/c_bridge_invocation.rs)持有宿主工具、路径和环境；[driver](../../compiler/driver/src/generated_c.rs)执行 generated-C 编译，LIR 只保存合同。 |
| R07–R08：对象格式和机器 | [codegen ELF](../../compiler/codegen/src/artifact/elf/architecture.rs)和 [slib ELF](../../compiler/slib/src/link_object/elf/architecture.rs)分别承担输出和输入边界；machine、flags、重定位、EH 和动态指针规则按已实现机器解释。 |
| R09：Linux 工具和 runtime | [Linux target](../../compiler/toolchain/src/linux_target.rs)区分 machine/libc，GNU 使用所选 GCC，musl 使用显式 REALGCC 或 PATH 中的 gcc；[runtime sources](../../compiler/toolchain/src/runtime/sources.rs)按 OS/image/arch 组合。 |

[driver LIR 编排](../../compiler/driver/src/lir_pipeline.rs)依次完成准备、ABI 规划和正文构造。
准备状态独立于完整 LIR Module，正文只构造一次；所有真实调用形式及 runtime/gateway 都获得完整计划。
聚合布局以 typed 存储节点共享，不按大型 payload 的字节数展开。
[provider ABI 绑定](../../compiler/lir/src/abi/storage_signature.rs)复用导入物理计划，保留必要的本地存储、carrier 和 exact signature 检查。

parser、AST、HIR、MIR 及其 lowering 相对基线没有变更；lir-lower 不登记目标名称或执行机器 ABI 分类。
其 ABI planner 依赖仅用于测试。`scoop`、`scoop-linker`、`scoop-toolchain` 的正常依赖树不包含
`scoop-codegen`、`scoop-abi-plan`、inkwell 或 llvm-sys；原有依赖边界测试继续通过。

已有 canonical 名称、wire 编码、固定指纹和 runtime ABI 保持；ELF x86-64 RELA form 的 tag 11 不变。
没有刷新普通功能或 ABI golden，没有新增目标、授权、预算或插件框架。
新增代码文件最长 285 行；原有大文件中的迁移按职责拆出模块，没有机械拆分无关代码。

## 实际验证结果

| 范围 | 通过 | 不适用／忽略 | 其他覆盖 |
| --- | ---: | ---: | --- |
| Linux Rust workspace 完整覆盖 | 5,576 | 0 ignored | 48 个测试目标，含 doctest；汇总及原始日志来源、逐项覆盖 |
| Linux GNU 正式 CLI 全量 | 3,741 | 30 不适用 | 4,203 变体、15,731 进程、71 次 golden；报告、日志 |
| Linux musl 正式 CLI 全量 | 3,739 | 32 不适用 | 4,201 变体、15,727 进程、71 次 golden；报告、日志 |
| Darwin 各功能定向 CLI，按最终同名结果去重 | 83 | 1 不适用 | 105 变体、376 进程、16 次 golden；覆盖报告 |
| 公共 fixture runner unittest | 每台宿主 47 | 0 | 本机与 Linux 各一次 |

两个 Linux CLI 运行均选择全部 3,771 项，各执行一轮，最终报告 `complete=true`、`unselected=0`，
没有失败或中断。musl 覆盖默认静态与显式动态 PIE；两个 Linux 目标的全部五项小值／native／跨库产物组合均通过。
本机是定向验证，不计为 Darwin 全量。两台宿主的最终 fmt、workspace clippy 与三个 release CLI 构建均通过。

Rust 验证中发现并修复两项问题：target Debug 名称不能用作对象文件路径；GNU driver 不应设置 musl 的 REALGCC。
前者定向补验，后者重跑完整 toolchain 包（Linux 29 项）并由最终两个 CLI 全量覆盖，其他已通过包按覆盖续跑。
最终不同测试按目标与名称去重后全部通过，没有未闭合失败；详见[实施记录](PROGRESS.md)。
未修改测试超时，没有执行批量快照更新。

## 清理与保留

- 实施过程中多次清理本机 debug incremental，Linux 开始前清理旧 target（约 21 GiB）；native unwind 安装保留复用。
- 两个 Linux 全量运行中按成功完成状态分批清理 case 目录，不删除运行中用例或失败证据；清理日志保留每次记录。
- GNU 完成后清理约 6 GiB 私有 cache；最终清理本机 14 个 cache/sysroot 目录（约 699 MiB）及 Linux musl cache、cases、sysroot、临时目录与空 incremental（约 6,116 MiB），见记录。
- 保留报告、原始日志、release 命令及可供 M41 复用的编译依赖；M39 的 fixture 私有缓存和临时目录均已清理。测试全程使用仓库内的临时目录。

## 后续架构移植的位置

| 目标 | codegen | driver/toolchain、slib/linker | runtime |
| --- | --- | --- | --- |
| Linux ARM64 | Linux AAPCS64、LLVM 配置、ELF EH/stackmap 及帧规则 | 目标／源码目录、Linux machine、ELF AArch64 relocation、loader/unwind 和 cache 目标选择 | ELF 汇编符号/CFI、anchor、返回 shim、浮点环境及 GC/EH 运行验证；不能直接使用 Darwin 汇编 |
| RISC-V 64 | 过程 ABI、LLVM 配置、指令／帧／stackmap/EH 规则 | 目标目录、工具链及 ELF machine/flags/relocation 和最终镜像规则 | 新 arch frame/anchor/shim、浮点环境、libunwind 与实际运行验证 |
| LoongArch64 | 过程 ABI、LLVM 配置、指令／帧／stackmap/EH 规则 | 目标目录、工具链及 ELF machine/flags/relocation 和最终镜像规则 | 新 arch frame/anchor/shim、浮点环境、libunwind 与实际运行验证 |

这些是后续移植工作，M39 未宣称其可编译或运行。若真实 ABI 出现现有数据模型无法表达的物理事实，
须先修订相应数据合同；不在 parser/HIR/MIR 或通用 LIR lowering 登记架构或追加分类分支。

## 复跑方式

从仓库根目录运行，先建立 `target/m39/tmp`，设置 `TMPDIR="$PWD/target/m39/tmp"`。
本机 LLVM 前缀为 `/opt/homebrew/opt/llvm@22`，Linux 为 `/usr/lib/llvm-22`；
Linux 先将 `$HOME/.cargo/bin` 加入 PATH。本轮 Linux 使用 `CARGO_BUILD_JOBS=6`、`CARGO_INCREMENTAL=0`。

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
cargo test --workspace
cargo build --release -p scoop -p scoopc -p scoop-linker --bins
python3 -m unittest discover -s tests/fixture_runner/tests

python3 tests/run_fixtures.py --all --target x86_64-unknown-linux-gnu --jobs 8 \
  --scoop target/release/scoop --scoopc target/release/scoopc \
  --scoop-link target/release/scoop-link --work-dir target/m39/linux-gnu
python3 tests/run_fixtures.py --all --target x86_64-unknown-linux-musl --jobs 8 \
  --scoop target/release/scoop --scoopc target/release/scoopc \
  --scoop-link target/release/scoop-link --work-dir target/m39/linux-musl
```

本次 Rust 全量遇到失败后按已完成包续跑：对象文件名修复只补验 terminal poll，
其余 workspace 使用 `--exclude scoop --exclude scoop-abi-plan --exclude scoop-ast --exclude scoop-codegen --no-fail-fast`。
GNU driver 环境修复后补验整个 toolchain 包，并补齐先前排除的四个包的 doctest；
真实构建图与 driver 大型组合没有重复全量运行。所有命令均未使用更新快照选项。
