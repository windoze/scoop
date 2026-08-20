# M40 验收记录

状态：已完成。三个 target 的全部适用 fixture 均有最终通过结果，未闭合失败为 0。

实施基线为 `b59294fbc`。交付范围与逐项取舍见 [设计](DESIGN.md)和[迁移说明](MIGRATION.md)。
本次不修改生产编译器、runtime、语言语义或产物格式；增加产物持有的 Rust 测试并明确完成边界。

## 已完成的改动

| 提交 | 内容 |
| --- | --- |
| `2b5aaa05f` | 普通功能、结构、产物/链接测试职责与默认写法 |
| `331c0a7c2` | 删除完整符号快照和仅为快照存在的 nm 步骤 |
| `c0580ff5c` | 收敛 IR golden、删除无消费者的 dump 请求 |
| `7bb591fd1` | 原生链接计划精确比较输入/绑定区段 |
| `916495122` | 删除 GC 用例无关计划、合并相同 SSA roots 平台期望 |
| `3687acb75` | 截断诊断按本次完整产物长度精确比较 |
| `4f69f1c2b` | 产物持有 CLI 改为同步流程，内部字节持有在 Rust 调用边界验证 |

普通语言用例继续执行编译、运行及精确诊断；已有跨 Cone、源码隔离、
debug/release、moving/minor GC 专测和 target 适用条件保持。
产物持有用例的重复 GC 压力运行移除；对应专测和链接/运行等价性均保留。
IR 专测复用 typed 字段断言及 9 项明确的 golden 专测；未批量刷新 IR 期望。
Native plan 期望从原有内容截取，仍锁定完整 native 输入选择及绑定关系。

## 验证环境与方法

- 本机：Apple M4 Pro，14 CPU、64 GiB RAM，Darwin/AArch64；Python 3.14、LLVM 22.1。
- Linux：`nuc12:~/repos/scoop`，Intel i7-12700H，20 逻辑 CPU、约 64 GiB RAM；
  Python 3.14、LLVM 22.1，现有 GNU/musl unwind 安装。
- Linux 的 compiler、runtime、sysroot、scripts 和 Cargo 文件 Git 内容与本机完全一致；
  同步本次 fixture/runner 后，用三个配套 release CLI 验收。
- 两机先 fmt/clippy 与公共规则验证；Python 变更先在本机完成 Ruff 格式化/lint，再运行测试。
  本机四个 stage crate 2,200 项单元测试及 1 项 doctest 通过；
  codegen/slib 的 15 项 ODR 选测及 driver 实际泛型 ODR 产物测试通过。
  Linux 的 3 项 codegen ODR 测试也通过，包含 GNU/musl 实际 ELF 合并。
  公共 runner 两机各 47 项通过。
  产物持有的两项 Rust 测试两机通过，真实构建图测试分别耗时 38.51/174.01 秒。
- 正式文件验收使用 `--all`，不更新期望。Darwin/GNU 并发度 8；
  GNU 的首批 core 重建结束后，musl 以并发度 4 交错验收。先前以 8 并发试跑的 musl
  在 37 项通过后主动中断，
  其余项不计为通过。随后 4 并发完成 247 项时保存报告并中断，待 GNU 结束后
  只续跑未完成项，避免并行争用及重复已通过项。完整范围以各来源逐 fixture 合并核对。
- 各 target 首轮使用独立的初始空 cache/sysroot；定向补验和续跑复用对应缓存。
  完成的 case 目录定期清理，所有临时文件位于仓库 `target/m40/tmp`。
  结束后清理本批 CLI case/cache、两机 release 构建产物及本机 dev profile，保留报告和日志。
- 未测量精简前的同环境耗时，因此不报告运行加速倍数。

## 文件规模与覆盖

同口径统计见 [JSON](validation/migration-statistics.json)，内容字节使用十进制。

| 项目 | 实施前 | 实施后 |
| --- | ---: | ---: |
| fixture 文件 | 32,890 | 15,711 |
| 内容字节 | 8,292,107,894 | 16,001,670 |
| snapshot 声明 | 12,257 | 57 |
| Scoop 源文件 | 4,939 | 4,939 |
| 诊断 JSON | 2,181 | 2,181 |
| fixture / 变体 | 3,771 / 4,242 | 3,771 / 4,242 |

内容减少 99.807%；没有压缩、外置或改存整份输出摘要。
57 条 snapshot 包含 45 条文件/值检查及 12 条 stdout/stderr 检查，未展开 target/variant。
保留阶段 golden 最大为 68,963 字节；73 份 native plan 合计 35,764 字节。
全部 Scoop 源码字节不变；诊断 JSON 仅 `artifact-corruption/truncated.json` 将
预计原始长度绑定到本次未损坏文件，其余诊断字段保持。

## 正式 CLI 结果

按实际执行来源合并，逐 fixture 核对当前声明的名称、target 适用性及全部变体数，
结果见[汇总 JSON](validation/summary.json)。每个 target 均覆盖发现的 3,771 项，未闭合失败为 0。

| target | 通过 | 不适用 | 变体 | 进程 | golden |
| --- | ---: | ---: | ---: | ---: | ---: |
| Darwin/AArch64 | 3,755 | 16 | 4,221 | 15,940 | 89 |
| Linux GNU/x86_64 | 3,741 | 30 | 4,203 | 15,731 | 71 |
| Linux musl/x86_64 | 3,739 | 32 | 4,201 | 15,727 | 71 |

下表保留各轮原始结果；补验结果替换同名旧结果，不能直接相加。

| 报告 | 通过 | 失败 | 不适用 | 变体 | 进程 | golden |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| [Darwin 全量](validation/full-darwin.json) | 3,752 | 3 | 16 | 4,216 | 15,930 | 89 |
| [Darwin 残余精简/诊断补验](validation/residual-darwin.json) | 2 | 2 | 0 | 4 | 29 | 3 |
| [Darwin GC 最终补验](validation/native-gc-darwin.json) | 2 | 0 | 0 | 4 | 16 | 0 |
| [Darwin 同步产物链接](validation/retained-darwin.json) | 1 | 0 | 0 | 1 | 4 | 0 |
| [GNU 全量](validation/full-gnu.json) | 3,739 | 2 | 30 | 4,199 | 15,727 | 71 |
| [GNU 最终补验](validation/retained-gnu.json) | 4 | 0 | 0 | 8 | 32 | 3 |

全量的两项 GC 计划比较和一项固定长度诊断失败已按迁移说明修复。
残余补验中的 SSA roots 三变体和完整产物损坏测试通过；两项 GC 用例因复用目录内的
只读旧产物而无法准备，清理对应 case 目录后最终补验通过。另一次缓存符号链接尝试
被编译器拒绝，未作为有效验证。补验未改编译器、没有更新快照，也未重复全量。
为避免把被删除的 core 全量计划重新放进仓库，全量报告仅缩短两项超长失败差异正文，
逐项状态与计数完整保留；原始报告与日志留在本机 `target/m40/logs/`。

GNU 的两项旧 GC 计划失败与 Darwin 原因相同，最终补验同时覆盖同步产物持有和 SSA roots。
截断产物诊断使用 Mach-O 固定位置，原声明仅适用于 Darwin；GNU/musl 保持不适用。

musl 的[部分结果](validation/partial-musl.json)记录 247 通过、1 超时、3,491 中断、32 不适用。
[定向补验](validation/retained-musl.json)的产物持有/GC/SSA roots 共 4 项、8 变体通过；
原超时的 `core-layouts-shared-dispatch-combined` 在低负载、单并发下
[106.15 秒通过](validation/core-retry-musl.json)，未调整 300 秒超时。
其余 [3,487 项续跑](validation/remaining-musl.json)以 8 并发全部通过
（3,944 变体、14,686 进程、68 次 golden 比较）；续跑未重复已通过项。

## 复跑命令

以下命令从仓库根目录执行；本机 Python 使用 `/opt/homebrew/bin/python3.14`，
Linux 使用 `python3` 并先将 `$HOME/.cargo/bin` 加入 PATH。
临时目录先用 `mkdir -p target/m40/tmp` 创建，再设置 `TMPDIR="$PWD/target/m40/tmp"`。
本机 `LLVM_SYS_221_PREFIX=/opt/homebrew/opt/llvm@22`，Linux 为 `/usr/lib/llvm-22`。

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
uv tool run --from ruff==0.16.10 ruff format tests/fixture_runner tests/run_fixtures.py
uv tool run --from ruff==0.16.10 ruff check tests/fixture_runner tests/run_fixtures.py
python3 -m unittest discover -s tests/fixture_runner/tests
cargo test -p scoop-parser -p scoop-hir-lower -p scoop-mir-lower -p scoop-lir-lower
cargo test -p scoop-codegen -p scoop-slib --lib odr
cargo test -p scoopc --lib actual_generic_library_emits_shared_odr_objects
cargo build --release -p scoop -p scoopc -p scoop-linker --bins
```

其中四个 stage crate 和 slib/driver 选测在本机执行；Linux 另执行
`cargo test --release -p scoop-codegen --lib odr -j 2`，验证实际 ELF 合并。
正式 CLI 命令如下，三个目标分别使用独立工作目录：

```sh
# Darwin 本机
/opt/homebrew/bin/python3.14 tests/run_fixtures.py --all --jobs 8 \
  --scoop target/release/scoop --scoopc target/release/scoopc \
  --scoop-link target/release/scoop-link --work-dir target/m40/full-darwin

# nuc12 GNU
python3 tests/run_fixtures.py --all --jobs 8 --target x86_64-unknown-linux-gnu \
  --scoop target/release/scoop --scoopc target/release/scoopc \
  --scoop-link target/release/scoop-link --work-dir target/m40/full-gnu

# nuc12 musl
python3 tests/run_fixtures.py --all --jobs 4 --target x86_64-unknown-linux-musl \
  --scoop target/release/scoop --scoopc target/release/scoopc \
  --scoop-link target/release/scoop-link --work-dir target/m40/final-musl
```

musl 续跑使用上述命令的 `--jobs 8`，将 `--all` 换成重复的 `--filter <name>`：
名字取 `partial-musl.json` 中 interrupted 且不在 `retained-musl.json` 成功集合的 3,487 项。
启动前清空 case 目录，复用 `final-musl` 的 cache/sysroot；core 超时项单独以 `--jobs 1` 补验。
产物持有定向 CLI 使用 `--filter program-link-retained-artifacts`；Linux 同批还选择
`native-link-runtime-gc`、`native-link-runtime-handles`、`m31-ssa-roots`，并发度 2。

产物持有边界的定向 Rust 命令（指定已构建的配套 release compiler）：

```sh
export SCOOP_TEST_PAIRED_SCOOPC="$PWD/target/release/scoopc"
cargo test --release -p scoop --lib completed_nodes_retain_shared_bytes_without_reopening_paths
cargo test --release -p scoop --test retained_artifacts
```
