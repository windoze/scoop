# M48 实施进度

基线：`6571bb4a`，M46 已完成。按用户要求，本期完全实现、提交并验收后才推进 M38；此前 M38 TextWriter 批次已撤回。

## 1. 分批状态

| 批次 | 状态 | 当前证据与剩余工作 |
| --- | --- | --- |
| M48-1 合同、清理范围、release 基线 | 完成 | 已同步三份规范、AGENTS 与 runner 说明；复扫 1,628 份迁移候选并核对活动设计。Darwin 五组各五轮、native 另五轮头文件变化共 55 次测量完成，原始证据见 [基线](BASELINE.md)。精确 native／linker 调用计数作为最终结构验收补齐。 |
| M48-2 提前更新判定与普通构建记录 | 进行中 | 配套编译器 metadata／能力记录、普通源码读取、实际输入路径已落地（见下）；更新判定仍须前移，产物收据与其余路径审计待删除。 |
| M48-3 按需读取与删除消费重放 | 未开始 | 尚未移除验证资格、对象 replay 和重复闭包。 |
| M48-4 对象／native／runtime／最终链接复用 | 未开始 | 现有暖程序构建仍重新链接、发布和制作运行副本。 |
| M48-5 全量测试迁移与最终验收 | 进行中 | 随实际输入路径迁移 1 个混合 fixture，删除或迁移相关 Rust 隔离断言；扫描清单余下 1,627 项，最终全量与性能验收未执行。 |

## 2. 测量口径

[测量脚本](../../tests/benchmarks/m48/measure.py)使用 release 版 scoop/scoopc 和 debug Scoop profile。core、std、空 main、真实多 Cone 标准库／JSON 应用、C native／runtime 程序分别执行五组独立冷／暖命令；native 组再改变实际头文件执行增量构建。源码和工具全程保留，每组完成后清理该组缓存和 target。

冷缓存指新的 Scoop 缓存，不刷新 OS 文件缓存。每次保存真实命令、原始 stdout/stderr、构建结果及精确的 compiler build child 数；进程树 RSS 为 ps 抽样的同时存活后代总和。进程抽样可能漏掉短进程，不能把抽样工具列表当作精确 native／链接调用数。最终结构验收还须用实际调用观察证明这些工作已经消失。

## 3. 清理纪律

原始 [扫描清单](SOURCE-ISOLATION.tsv) 是迁移候选，不是删除语言测试的名单。后续每批保存旧路径、保留断言、新落点或删除原因；M48-5 重新扫描新增 case、内联描述、Rust helper 和活动设计。历史已完成记录保留当时事实，当前执行合同采用本期规范。

## 4. M48-2：配套编译器记录

首次使用按实际路径、mtime、大小及配套配置查询能力并保存普通 `tools-v1` 记录；连续构建直接复用记录，不再捕获整个 scoopc、读取程序内容计算摘要或在 child 执行前后重捕获。实际工具启动与协议错误仍传播；并发首次使用经普通文件锁共享一次探测，记录写后普通 rename，不读回或同步落盘。路径在记录中保存原始字节，语言身份与 runtime ABI 不变；compile key domain 更新为 v4。

本机 workspace fmt/clippy 通过，[scoop 单元测试 106 项](validation/compiler-cache-darwin-rust.log.gz)通过，fixture runner 公共规则 49 项通过，正式用例发现 4,093 项通过。新增[独立库用例](../../tests/fixtures/m48-compiler-cache/library/fixture.toml)与[native 组合](../../tests/fixtures/m48-compiler-cache/native/fixture.toml)均通过：冷编译 2 次、暖编译 0 次、仅更新实际工具 mtime 后 2 次、再次暖编译 0 次；native 每轮执行均输出 `native-c-ok`。工具与源码全程保留。[CLI 原始结果](validation/compiler-cache-darwin-fixtures.log.gz)，[lint 结果](validation/compiler-cache-darwin-clippy.log.gz)。Linux fmt/clippy 和 [scoop 单元测试 109 项](validation/compiler-cache-linux-rust.log.gz)通过（含非 UTF-8 工具路径）；[GNU](validation/compiler-cache-gnu-fixtures.log.gz)、[musl](validation/compiler-cache-musl-fixtures.log.gz)各 2 个正式 CLI 用例通过。非 UTF-8 文件名的实际工具运行用例只适用于 Linux 文件系统。

此子批只完成配套 scoopc 复用，当前仍存在源码镜像、缓存产物复核、native 预处理和最终重新链接，不能据此判定 M48-2 或 M48 已完成。已清理本机 `target/debug/incremental` 约 15 GiB，保留后续构建所需依赖与工具。

GNU 的空程序暖构建另经 `strace -f -e trace=execve,openat` 核对：没有启动 scoopc 能力查询或编译 child，也没有打开 scoopc 文件内容。对照原基线的 1 次能力查询，本批该调用已消失；native／最终链接调用仍存在，留在后续批次处理。[原始 trace](validation/compiler-cache-gnu-warm.trace.gz)。

## 5. M48-2：Scoop 源码读取

manifest／single-file 的实际 Scoop 源码只读取一次供 UTF-8 解码与 parser 输入使用，不再在读取前后比较 inode、ctime、mtime、长度与重新解析后的路径。删除无剩余调用的 `StableFileObservation`。源码发现允许普通 symlink 指向 Cone 外的文件／目录；选中路径类型、缺失输入、重复选择、逻辑路径、目录遍历循环及 UTF-8 错误仍正常报告。此处不代表 Preparer 已改为提前命中，也不代表其他 native／产物路径审计已清除。

本机 workspace fmt/clippy、[manifest 测试 56 项](validation/source-inputs-darwin-rust.log.gz)通过；[2 个新 CLI 用例](validation/source-inputs-darwin-fixtures.log.gz)验证显式外部文件、默认目录内 symlink、外部测试目录、冷／暖构建与真实源码变化；[M33 源码选择 33 个用例](validation/source-inputs-darwin-regression.log.gz)通过（34 variants、44 processes），保留缺失、循环、重复与语法／编码诊断。当前用例总数 4,094，较前一批减少 1 个退役的逃逸拒绝 case、增加 2 个实际编译用例。Linux fmt/clippy、[manifest 测试 56 项](validation/source-inputs-linux-rust.log.gz)通过；GNU 与 musl 各通过 [2 个新用例](validation/source-inputs-gnu-fixtures.log.gz)和 [33 个 M33 回归用例](validation/source-inputs-gnu-regression.log.gz)（[musl 新用例](validation/source-inputs-musl-fixtures.log.gz)、[musl 回归](validation/source-inputs-musl-regression.log.gz)）。

## 6. M48-2：实际输入路径

Preparer 将实际 manifest 根、single-file 源码及 prebuilt／cache 产物路径交给 scoopc。删除源码／manifest／依赖产物镜像、缓存命中后复制 `.slib`、输入树只读封存、写后读回、同步落盘和子输出目录审计；CompletedNode 只保存一份实际产物路径，删除 PrivateArtifactPath 及原路径映射。native 预处理结果目前仍作为普通临时输入写出，等 M48-4 的 depfile 更新判定与实际编译调整；本批没有宣称缓存判定已经先于源码读取、预处理和产物复核。

`staging.rs` 从 596 行缩至 117 行，`prepared.rs` 从 346 行缩至 261 行。本机 [workspace clippy](validation/input-paths-darwin-clippy.log.gz)、[scoop 单元测试 104 项](validation/input-paths-darwin-rust.log.gz)通过（减少 2 项纯 staging 审计测试）。[正式 CLI 8 个用例、9 variants、50 processes](validation/input-paths-darwin-fixtures.log.gz)通过，覆盖单文件、实际 prebuilt 路径、cold／warm／输入变化、dump、native、外部源码与测试目录、菱形依赖及 moving GC。隔离 case 的迁移与删除见 [清理记录](CLEANUP.md)。另清理无构建进程使用的 `target/debug/incremental` 约 1.9 GiB。Linux [workspace clippy](validation/input-paths-linux-clippy.log.gz)、[scoop 单元测试 107 项](validation/input-paths-linux-rust.log.gz)通过；[GNU](validation/input-paths-gnu-fixtures.log.gz)与 [musl](validation/input-paths-musl-fixtures.log.gz)各通过同一组 8 个用例、9 variants、50 processes。

GNU 使用上述 single-file 用例的现有源码、工具及缓存追加一次实际暖构建跟踪：[execve/openat 记录](validation/input-paths-gnu-warm.trace.gz)、[构建记录](validation/input-paths-gnu-warm.stderr.gz)。compiler child 为 0，记录中没有 scoopc 启动、源码 snapshot 镜像路径或 completed-artifacts 副本路径；源码内容读取、产物复核与最终链接仍须后续删除，未据此声称整个暖构建已经完成优化。

## 7. M48-2：普通文件内容读取

`ImmutableInputSnapshot` 只共享本次已读取的字节和摘要；删除稳定文件观察、读前／读后 metadata／inode／ctime 比较、重新解析路径对比、no-follow reader 及只为复核读取长度的分配／限长路径。实际路径定位与普通 `fs::read` 错误照常传播，symlink 按正常 I/O 跟随。native 预处理不再重查已读取输入并循环重试三次；源码、头文件和预处理结果的完整读取／持有仍待 M48-4 替换为更新判断和实际编译记录。

本机 [workspace clippy](validation/file-io-darwin-clippy.log.gz)、[manifest 56 项与 toolchain 26 项测试](validation/file-io-darwin-rust.log.gz)通过；调整两项 native 测试的输入变更时序后针对性复验 6 项通过。[5 个正式 CLI 用例、44 processes](validation/file-io-darwin-fixtures.log.gz)通过，覆盖 C／C++ 实际头文件、flags、源码和模式变化，以及 dump 和已有产物路径。删除 344 行稳定 reader，文件内容实现及错误定义集中为 153 行。Linux [workspace clippy](validation/file-io-linux-clippy.log.gz)、[manifest 56 项和 toolchain 29 项](validation/file-io-linux-rust.log.gz)通过；[GNU 5 个用例、44 processes](validation/file-io-gnu-fixtures.log.gz)通过，[musl 3 个用例、27 processes](validation/file-io-musl-fixtures.log.gz)通过，另 2 个 C++ 用例按既有目标声明不适用。

## 8. M48-2：源码元数据先于内容读取

Preparer 只保存实际选中的源码逻辑名、路径、mtime、大小和用途，删除父进程的源码字节／摘要副本。manifest 与 single-file 的内容读取留给实际执行的 scoopc；源码发现使用已有 metadata，不另外为更新判定读取内容。compile key domain 升为 v5，本机实际路径参与缓存定位但不改变语言身份。诊断显示仅按需读取涉及的源文件并在本轮共享；无效 UTF-8 由实际读取源码的 scoopc 报告，protocol 7 新增只有 host path 的文件级 origin，保留文件路径且不伪造字节区间。

本机 [workspace clippy](validation/source-metadata-darwin-clippy.log.gz)、[manifest 57 项和 protocol 25 项](validation/source-metadata-darwin-protocol-manifest.log.gz)、[诊断显示](validation/source-metadata-darwin-diagnostics.log.gz)通过。scoop 原有 104 项中 103 项通过，协议版本升级后固定缓存键向量按新编码更新，[6 项 cache-key 复验](validation/source-metadata-darwin-key.log.gz)通过，保留[首次结果](validation/source-metadata-darwin-rust-first.log.gz)。[37 个源码／CLI 回归](validation/source-metadata-darwin-regression-first.log.gz)中的 36 项通过；UTF-8 文件级诊断修复后，[5 个相关 CLI、24 processes](validation/source-metadata-darwin-fixtures.log.gz)全部通过，覆盖 single-file、外部源码／测试目录、真实输入变化、缓存 warning 与 dump。新增只 touch 源码的断言：编译 child 为 1，但 artifact fingerprint 不变；内容变化仍得到新运行结果。

本批尚未删除缓存产物收据和整包读取，也未完成 manifest 解析复用、native 及最终链接提前命中。Linux [workspace clippy](validation/source-metadata-linux-clippy.log.gz)、[scoop 107 项、manifest 57 项和 protocol 25 项](validation/source-metadata-linux-rust.log.gz)通过；[GNU](validation/source-metadata-gnu-fixtures.log.gz)与 [musl](validation/source-metadata-musl-fixtures.log.gz)各 37 个正式 CLI 用例、38 variants、67 processes 通过。源码和工具全程存在，复用 GNU single-file 用例的实际缓存追加[暖构建 trace](validation/source-metadata-gnu-warm.trace.gz)：没有打开 `.scoop` 内容，没有 scoopc exec；[构建记录](validation/source-metadata-gnu-warm.stderr.gz)中 child 数为 0。此处仍有 `.slib` 读取、native／最终链接工作，未将其算作整体命中优化完成。
