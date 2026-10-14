# M48 实施进度

基线：`6571bb4a`，M46 已完成。按用户要求，本期完全实现、提交并验收后才推进 M38；此前 M38 TextWriter 批次已撤回。

## 1. 分批状态

| 批次 | 状态 | 当前证据与剩余工作 |
| --- | --- | --- |
| M48-1 合同、清理范围、release 基线 | 完成 | 已同步三份规范、AGENTS 与 runner 说明；复扫 1,628 份迁移候选并核对活动设计。Darwin 五组各五轮、native 另五轮头文件变化共 55 次测量完成，原始证据见 [基线](BASELINE.md)。精确 native／linker 调用计数作为最终结构验收补齐。 |
| M48-2 提前更新判定与普通构建记录 | 进行中 | 配套编译器、源码 metadata、manifest 解析记录、普通编译结果记录和库输出复用已落地（见下）；native 更新判定及其余路径审计仍待处理。 |
| M48-3 按需读取与删除消费重放 | 未开始 | parent 的编译结果／缓存产物复核已删除；各消费 stage 的验证资格、对象 replay 和重复闭包仍待移除。 |
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

## 9. M48-2：普通编译结果记录与库输出复用

protocol 8 直接返回生产时已有的 `ArtifactManifestSummaryV1`（含测试目录），取代七个独立指纹及 parent 的 child 成功产物交叉核对。摘要删除无人消费的归档长度、成员数和重复兼容性字段；生产代码直接共享测试目录，不重新解码自己的目录数据。cache 使用新 `compile-v2` namespace，以 `record.cbor` 保存摘要和 warning；命中只读记录与查询产物是否存在，不捕获 `.slib` 字节、不比较收据、不复核 warning 来源或生产闭包。缺失输出仍使该结果更新，普通读取失败照常报告。

成功 child 的临时输出经 rename 进入缓存，直接使用原摘要；保留共享／独占构建锁与取得独占锁后的重查，删除目录／权限／inode 审计、sync、写后读回、并发产物等价性比较和整份字节长期持有。`.slib` 生产发布也删除自己的 readback 和 sync。BuildArtifact 只在实际链接消费时才读取数据；库已有用户输出按旁置的结果记录直接返回，新输出／缺失输出只复制一次，其他构建覆盖同一位置时更新相应记录。

本机 [workspace clippy](validation/build-record-darwin-clippy.log.gz)、[protocol 25 项](validation/build-record-darwin-protocol.log.gz)、[summary 3 项](validation/build-record-darwin-summary.log.gz)、[driver 协议 4 项](validation/build-record-darwin-driver-protocol.log.gz)和 [4 项诊断分类复验](validation/build-record-darwin-diagnostics.log.gz)通过。scoop 原有 [84 项中的 83 项](validation/build-record-darwin-rust-first.log.gz)通过；protocol 8 的固定缓存键向量更新后，[9 项 cache-key／store 复验](validation/build-record-darwin-cache-tests.log.gz)通过。CLI 的 [6 个组合用例](validation/build-record-darwin-fixtures-first.log.gz)、[新库输出用例](validation/build-record-darwin-library-fixtures.log.gz)与[菱形依赖／moving GC](validation/build-record-darwin-diamond-fixtures.log.gz)通过，成功用例共 8 个、9 variants、54 processes；新 fixture 首轮遗漏默认 src 目录已修正，另保留含 1 次失败进程的首次日志。测试退役与替换详见 [清理记录](CLEANUP.md)。另在无 cargo/rustc 进程时清理 `target/debug/incremental` 728.5 MiB。

本批尚未完成 manifest 解析复用；显式 artifact 发现及 Compile／Link 消费仍有整包校验和 replay，native／runtime 暖构建及程序最终链接也仍在执行。Linux [workspace clippy](validation/build-record-linux-clippy.log.gz)、[scoop 87 项与 protocol 25 项](validation/build-record-linux-rust.log.gz)、[summary 3 项](validation/build-record-linux-summary.log.gz)和 [driver 协议 4 项](validation/build-record-linux-driver-protocol.log.gz)通过；[GNU](validation/build-record-gnu-fixtures.log.gz)与 [musl](validation/build-record-musl-fixtures.log.gz)各 8 个正式 CLI 用例、9 variants、54 processes 通过。源码、工具和原缓存产物始终存在，复用 GNU 库用例的已有输出追加[暖构建系统调用记录](validation/build-record-gnu-warm.trace.gz)：没有打开 `.scoop` 或 `.slib` 内容，没有 scoopc exec，也没有 rename 发布；[构建记录](validation/build-record-gnu-warm.stderr.gz)中 child 数为 0。trace 仍显示 C 工具链版本、预处理和探测编译调用，留待后续提前更新判定；不能据此标记 M48 完成。

## 10. M48-2：manifest 解析记录

父进程的根、显式源码依赖、默认 core 和默认标准库入口共用 `manifests-v1`。按实际路径、mtime、大小及解析器 target catalog 定位解析结果；命中不读取 Cone.toml、不重新执行 TOML parser。记录直接保存解析后的生产／测试／native 配置、locator、诊断区间及对应文本，不维护另一份语言规则或校验链。普通构建锁协调首次解析，成功后 rename 发布；无效 manifest 不发布，缺失输入仍先报告定位错误。有效编译键继续使用用途投影，仅测试配置变化不使生产结果失效。源码保持存在，实际执行的 scoopc 仍正常读取自己的 manifest。

本机 [workspace clippy](validation/manifest-record-darwin-clippy.log.gz)、[manifest 57 项](validation/manifest-record-darwin-manifest-tests.log.gz)、[解析记录 3 项复验](validation/manifest-record-darwin-record-tests.log.gz)通过。scoop 首轮 [87 项中的 82 项](validation/manifest-record-darwin-rust-first.log.gz)通过；5 项失败均源于旧测试 helper 将缓存配置在其断言不存在的 sysroot 下，改为独立缓存目录后 [17 项 core 相关复验](validation/manifest-record-darwin-core-tests.log.gz)通过。解析记录测试覆盖完整配置／span、真实更新、无效与缺失输入、失败不发布，并保持负 Unix 时间戳可用。

[CLI 首轮](validation/manifest-record-darwin-fixtures-first.log.gz)通过 24 项；manifest 源码 UTF-8 负例同步已落地的 child 文件级诊断后，[2 项定向复验](validation/manifest-record-darwin-fixtures.log.gz)通过。成功用例合计 25 个、27 variants、85 processes，覆盖新用例的冷／暖、仅测试配置更新、实际源码选择更新与 48／49 退出值，以及测试依赖、target 条件、native、manifest 错误、菱形依赖和 moving GC。保留首轮失败日志；退役范围没有扩大，说明见 [清理记录](CLEANUP.md)。无 cargo/rustc 进程时清理本机 `target/debug/incremental` 1,544.3 MiB。

Linux [workspace clippy](validation/manifest-record-linux-clippy.log.gz)、[scoop 90 项与 manifest 57 项](validation/manifest-record-linux-rust.log.gz)通过；[GNU](validation/manifest-record-gnu-fixtures.log.gz)与 [musl](validation/manifest-record-musl-fixtures.log.gz)各 25 个正式 CLI 用例、27 variants、85 processes 通过。对 GNU 库的已有输出追加[暖构建 trace](validation/manifest-record-gnu-warm.trace.gz)：只打开根与 core 的两份解析记录，没有打开原 Cone.toml、`.scoop` 或 `.slib` 内容，没有 scoopc exec 或 rename 发布；[构建记录](validation/manifest-record-gnu-warm.stderr.gz)中 child 数为 0。源文件、工具和原始产物全程保留。native 工具探测、预处理、消费 stage 的重复验证和最终链接复用仍待处理，M48 尚未完成。

## 11. M48-2：C 工具发现记录与 child 复用

父进程通过 `native-tools-v1` 保存普通 C 工具发现结果，按请求、实际生效的 PATH／REALGCC、工具和探测头文件／specs／搜索目录的 metadata 判断更新。Darwin 同时观察选中的 developer、SDK 和系统 deployment 输入；Linux 不再整读 GCC、cc1 或 assembler 来构造 compiler identity，产物使用版本与实际预处理宏配置，本机构建键单独携带工具输入 metadata。协议升为 9、compile key domain 升为 v6；parent 把完整调用记录交给 child，child 解码后直接用于编译，不重复探测。没有引入收据、内容复核、权限检查或工具移除验收。

本机 [workspace clippy](validation/native-tools-darwin-clippy.log.gz)、[protocol 26 项和 toolchain 28 项](validation/native-tools-darwin-tool-protocol.log.gz)、[driver 协议 4 项](validation/native-tools-darwin-driver.log.gz)通过。scoop [首轮 87 项中的 86 项](validation/native-tools-darwin-rust-first.log.gz)通过，新增键维度使旧固定向量失效；同步向量后 [6 项缓存键复验](validation/native-tools-darwin-key-tests.log.gz)通过。另保留协议首轮因帧长度增加一字节而失败的[旧期望日志](validation/native-tools-darwin-tool-protocol-first.log.gz)，更新后通过。成功 [release 构建](validation/native-tools-darwin-release.log.gz)及 [12 个正式 CLI 用例、13 variants、85 processes](validation/native-tools-darwin-fixtures.log.gz)覆盖库与程序、源码／manifest／编译器更新、C 和 C++ native、独立 scoopc 和 moving GC。新增 Linux 用例 `m48-native-tool-record` 保留真实工具，用调用日志检查首次仅探测一次、暖构建无调用、工具 metadata 变化后重新探测和再次复用；GNU 与 musl 均通过。无 cargo/rustc 进程时清理本机 `target/debug/incremental` 3,753.1 MiB。

本批仅完成 C 发现与 child 传递；C++ 发现、native 预处理、runtime 对象和最终 executable 更新判定仍待处理，M48 尚未完成。

Linux [workspace clippy](validation/native-tools-linux-clippy.log.gz)、[scoop 90 项／protocol 26 项／toolchain 31 项](validation/native-tools-linux-rust.log.gz)、[driver 协议 4 项](validation/native-tools-linux-driver.log.gz)和 [release 构建](validation/native-tools-linux-release.log.gz)通过。[GNU](validation/native-tools-gnu-fixtures.log.gz)通过 17 个正式 CLI 用例、18 variants、94 processes；[musl](validation/native-tools-musl-fixtures.log.gz)通过 13 个用例、15 variants、76 processes。新增用例的 [GNU 调用记录](validation/native-tools-gnu-calls.log.gz)与 [musl 调用记录](validation/native-tools-musl-calls.log.gz)各只有冷构建和真实工具修改后的两组探测，两次暖构建没有新增调用，child 也没有重复发现。保持所有源码、工具和原产物存在，对 GNU 同一库与同一输出再执行一次[暖构建 trace](validation/native-tools-gnu-warm.trace.gz)：只有 scoop 自身的 1 次 execve，没有 native 工具／scoopc exec，没有 Cone.toml／`.scoop`／`.slib` 内容打开，也没有 rename 发布；[结果](validation/native-tools-gnu-warm.stderr.gz)中的 child 数为 0。

## 12. M48-2／4：C++ 发现与用户 native 对象复用

新增普通 `cxx-tools-v1` 与 `native-objects-v1` 记录。C++ 发现只观察实际配套 driver、前端、标准库、宏和头文件输入，父进程的 native 编译及最终链接共享该结果；不整读工具／标准库二进制。native 记录保存实际源码／头文件 metadata、include 选择、有效 flags、工具配置和稳定对象路径，命中不调用预处理器、不读头文件或对象、不复制预处理单元。仅 Scoop 源码变化时复用原 native 对象；真实头文件变化、增加优先候选或所需对象缺失时更新。全部 native 编译成功后发布记录，旧对象仍供已有构建使用。协议升为 10、compile key domain 升为 v7；parent 将对象位置交给 child 打包，child 不再发现 C++ 工具或重新编译同一 native 源码。独立 scoopc 保留正常源码编译。

候选观察来自编译器 `-dI` 展开的实际 include 名称及所在文件，覆盖宏、嵌套引号查找、forced header 和 imacros；移除观察指令时保留行号和 C++ 原始字符串。首次 CLI 采样发现头文件父目录交叉展开产生约 20 万项／116 MB 记录，且 JSON 未缓冲写入；现按实际 include 查找生成候选并缓冲写入，同一 C++ 用例降为约 9700 项／5.6 MB。保留[异常采样](validation/native-objects-darwin-slow-record.sample.gz)，不把被中断的运行计为验收通过。

本机 [workspace clippy](validation/native-objects-darwin-clippy.log.gz)、[scoop 87 项／protocol 26 项／toolchain 31 项](validation/native-objects-darwin-rust.log.gz)、[driver 协议 4 项](validation/native-objects-darwin-driver.log.gz)和 [release 构建](validation/native-objects-darwin-release.log.gz)通过。对象测试覆盖暖复用、Scoop 文件更新不重编 native、实际头文件更新、嵌套目录新增优先头文件、无关文件不失效、所需输出缺失和失败不发布。先前的[首次 Rust 结果](validation/native-objects-darwin-rust-first.log.gz)中旧键向量和外部路径比较期望已同步，最终相关 crate 全部通过。

CLI [定向复验](validation/native-objects-darwin-retest.log.gz)通过 17 个用例，含 C++ debug／release、传递依赖、真实 native 冲突、外部实际输入、两个新对象复用用例；2 个 imacros 用例暴露 Clang 的额外观察指令，修复后[最后 5 个用例、54 processes](validation/native-objects-darwin-final.log.gz)全部通过，覆盖 C／C++ flags、头文件、源码更新及新对象复用。新用例按冷／暖、仅 Scoop 更新、头文件更新、增加优先头文件运行，consumer 退出值为 43／44／102。另保留[首次中断](validation/native-objects-darwin-fixtures-first.log.gz)及[修正诊断前的中断结果](validation/native-objects-darwin-fixtures.log.gz)。历史隔离和路径审计迁移见 [清理记录](CLEANUP.md)。本机无 cargo/rustc 进程时清理 `target/debug/incremental` 1,287.2 MiB。

本批尚未完成 runtime 对象、最终 executable 复用、artifact 消费重放清理与全量历史测试迁移，M48 仍未完成。

Linux [workspace clippy](validation/native-objects-linux-clippy.log.gz)、[scoop 90 项／protocol 26 项／toolchain 34 项](validation/native-objects-linux-rust.log.gz)、[driver 协议 4 项](validation/native-objects-linux-driver.log.gz)、[release 构建](validation/native-objects-linux-release.log.gz)及 [musl 两种链接模式的 C++ 需求诊断](validation/native-objects-linux-linker.log.gz)通过。首次 Linux clippy 暴露平台限定测试漏传新参数，[原始诊断](validation/native-objects-linux-clippy-first.log.gz)保留；修正调用并移除测试源码删除步骤后重新验证通过，本机[后续 clippy](validation/native-objects-darwin-clippy-followup.log.gz)也通过。

[GNU 正式 CLI](validation/native-objects-gnu-fixtures.log.gz)中 43 项通过；4 个自带工具选择的负例因 runner 额外传入 `--cc` 而重复参数，去掉 runner 的该参数后[4 项独立复验](validation/native-objects-gnu-tool-negatives.log.gz)全部通过，合计 47 个适用用例完成。[musl](validation/native-objects-musl-fixtures.log.gz)通过 38 个用例、41 variants、90 processes，另 10 个按 target 声明不适用。覆盖 native 冷／暖、实际源码／头文件／flags／工具更新、C++ stdlib、重复定义、外部正常路径、传递产物及运行。

保留全部源码、工具与产物，对 GNU 新用例的 C／C++ 库输出分别执行同配置暖构建：[C trace](validation/native-objects-gnu-c-warm.trace.gz)及 [C++ trace](validation/native-objects-gnu-cxx-warm.trace.gz)均仅有 scoop 自身 1 次 execve，没有 native 工具或 scoopc exec，没有打开 Cone.toml／Scoop 或 native 源码／头文件／对象／`.slib` 内容，没有 rename；[C 结果](validation/native-objects-gnu-c-warm.stderr.gz)及 [C++ 结果](validation/native-objects-gnu-cxx-warm.stderr.gz)的 child 数均为 0。首次 trace 误用 `/usr/bin/gcc` 别名而非 runner 传入的规范路径，属于不同请求并触发更新，保留[该次结果](validation/native-objects-gnu-c-request-change.stderr.gz)；上述暖构建证据使用与 fixture 完全相同的 compiler、PATH、target、输出及缓存配置。库验收不替代仍待实施的 runtime／最终 executable 复用。

## 13. M48-4：runtime 对象记录与直接路径链接

新增 `runtime-v2` 记录，按实际选中源码、depfile 头文件、include 候选、flags、C 工具 metadata 和 runtime 规则判断更新。命中不扫描／复制 runtime 树、不预处理、不读取源码／头文件／对象或 compiler 二进制；变化后直接编译原始源码，保留不同批次的稳定对象路径，全部成功才发布记录。复用已有 native include 观察和路径编码，不新增一套依赖解析器。runtime index schema 升为 2，保存生产时的对象摘要、合并符号与实际位置；读取不重算摘要／ABI 导出或复制对象，最终 native linker 直接使用这些路径。

删除最终 executable 的完整 readback、map／trace 来源证明与对象／stackmap／EH／动态绑定／code signature 重放，连同只服务它们的证明数据和专测。对象生产及实际 native 输入的格式／relocation／ABI 检查、真实链接失败传播、正常运行和 GC 行为保留；完整迁移范围见 [清理记录](CLEANUP.md)。本批代码净减少约四千行；runtime 编排与记录、依赖、编译模块分别保持在百行左右。新 `m48-runtime-record` 正式 CLI 用例覆盖冷／暖、无关头文件、只改 Scoop、改 runtime 头文件及源码后的真实返回值。

本机 [workspace clippy](validation/runtime-record-darwin-clippy.log.gz)、[toolchain 31 项／protocol 26 项](validation/runtime-record-darwin-tool-protocol.log.gz)、[runtime 3 项](validation/runtime-record-darwin-runtime-tests.log.gz)与 [release 构建](validation/runtime-record-darwin-release.log.gz)通过。链接器初轮 [4 项](validation/runtime-record-darwin-linker.log.gz)及更新断言后的 [1 项复验](validation/runtime-record-darwin-linker-followup.log.gz)通过；共享 helper 取消删源码后，最终 [4 项](validation/runtime-record-darwin-linker-final.log.gz)全部通过；[runner 49 项](validation/runtime-record-runner-tests.log.gz)通过。正式 CLI [15 个适用用例、23 variants、138 processes](validation/runtime-record-darwin-combined.log.gz)覆盖 runtime 更新、C/C++、独立链接、菱形依赖、动态库、callback 异常、TLS、ABI 与 moving GC；5 个 Linux 专属项不计为通过。无 cargo/rustc 进程时清理 `target/debug/incremental` 1,217.9 MiB。

本批不代表最终 executable 命中：最终工具发现与链接、Scoop 对象解包、artifact 消费 replay、run 私有副本、dump 最小阶段复用及 1,618 项历史源码隔离迁移仍待完成，M48 继续实施。

Linux [workspace clippy](validation/runtime-record-linux-clippy.log.gz)、[toolchain 34 项／protocol 26 项](validation/runtime-record-linux-tool-protocol.log.gz)、[runtime 4 项](validation/runtime-record-linux-runtime-tests.log.gz)、[release 构建](validation/runtime-record-linux-release.log.gz)和输入保留后的[链接器 17 项](validation/runtime-record-linux-linker-final.log.gz)通过。首次 clippy 的[两个平台测试旧返回值断言](validation/runtime-record-linux-clippy-first.log.gz)已同步；不再保留空验证结果，未使用 helper 按实际平台限定。GNU [15 个适用用例、25 variants、139 processes](validation/runtime-record-gnu-fixtures.log.gz)通过，包含 ELF DSO 版本绑定、失败不发布及深调用栈。

对 GNU runtime 用例保留源码、工具与输出执行同配置[暖构建 trace](validation/runtime-record-gnu-warm.trace.gz)：[child 数为 0](validation/runtime-record-gnu-warm.stderr.gz)，未打开 runtime 源码／头文件，没有 runtime 编译；父进程未打开 runtime 对象，实际 ld 直接读取已有 `runtime-v2/objects-*` 下的 94 份对象，详见[采样摘要](validation/runtime-record-gnu-warm-summary.json)。最终工具发现、Scoop 对象消费及 native 链接仍有调用，本证据只证明本批 runtime 复用。首次采样的 PATH 与 runner 不同，触发编译输入更新，保留[该次结果](validation/runtime-record-gnu-request-change.stderr.gz)；暖构建证据采用 runner 实际生效的 PATH 和全部原配置。

musl [16 个适用用例、26 variants、133 processes](validation/runtime-record-musl-fixtures.log.gz)全部通过，覆盖静态／PIE、动态库版本绑定、深调用栈、runtime 真实更新、独立链接和 moving GC；4 个不适用项不计为通过。本批在 Darwin、GNU、musl 的验收均保持实际输入存在，已完成 runtime 结果复用与最终输出复核退役；其余 M48 交付范围仍按上文继续。
