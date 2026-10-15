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

## 14. M48-4：最终 executable 记录与直接运行

新增 `executables-v1` 普通记录，使用已有 Code／runtime 摘要、target／profile、最终选项、library roots 与实际工具输入定位结果。命中发生在 Link 闭包读取、final 工具发现、startup／response file 与链接之前，只读取更新记录并查询实际输入 metadata 和所需输出是否存在。记录保存稳定 executable 路径与生产时的 link plan 摘要；输入变化后通过普通锁生成新结果，成功才发布记录。显式库与动态依赖记录实际文件、脚本及候选路径，包含缺失候选；symlink 改选与新增候选触发正常解析，不扫描无关目录内容。

库与程序共用结果发布记录；已有输出不再复制，首次另选输出或输出缺失时只发布已有结果。删除 OutputLocation 的 inode／路径来源检查、强制同步与 execution copy；run／test 直接执行本次构建结果的稳定缓存路径，argv[0] 保留用户输出位置；保留 cwd、stdio、exit／signal、测试事件及并发输出替换行为。用户输出的结果标识包含本次实际输入 metadata，与语义 link plan 指纹分开，避免真实链接输入更新后错误保留旧用户输出。final 工具身份采用 metadata，取消整个 compiler／linker 二进制读取。

本机 [workspace clippy](validation/executable-darwin-clippy.log.gz)、[toolchain 31 项](validation/executable-darwin-toolchain.log.gz)、[scoop 86 项](validation/executable-darwin-scoop.log.gz)、[链接器 1 项](validation/executable-darwin-linker.log.gz)及 [release 构建](validation/executable-darwin-release.log.gz)通过。正式 CLI [组合首轮](validation/executable-darwin-combined.log.gz)通过 8 项；两个新 fixture 的 Unsafe 语法及 run 不接受 `-o` 的请求写法修正后，[native 复验](validation/executable-darwin-native.log.gz)和 [executable 复验](validation/executable-darwin-record.log.gz)通过。最终成功覆盖共 10 个用例、12 variants、137 processes，包含生产／测试、过滤／列表、不同输出、debug／release、所需输出补产、依赖／native／runtime 更新与正在运行时替换输出。缺失真实 native 输入的[补充 CLI 诊断](validation/executable-darwin-missing-input.stderr.gz)同时确认缓存记录和已有 executable 的 metadata 均未改变。

本批尚未完成输入变化后的 final 工具发现结果复用、Scoop 对象的稳定缓存路径、artifact 消费 replay 清理、dump 最小阶段补产和全量历史测试迁移。M48 继续实施，未推进 M38。

Linux [workspace clippy](validation/executable-linux-clippy.log.gz)、[scoop 89 项／toolchain 34 项](validation/executable-linux-rust.log.gz)、[链接器 2 项](validation/executable-linux-linker.log.gz)和 [release 构建](validation/executable-linux-release.log.gz)通过。初次 CLI 预检发现传输归档附带的 macOS `._` 文件，清除这些传输副本后重新运行；保留[预检日志](validation/executable-linux-preflight.log.gz)，未将该次算作通过。GNU [10 个用例、12 variants、137 processes](validation/executable-gnu-fixtures.log.gz)与 musl [9 个适用用例、11 variants、125 processes](validation/executable-musl-fixtures.log.gz)通过。

按实际 GNU 工具输出补齐 GCC `-L` 目录之外的 ld `SEARCH_DIR`，以及 ld／collect2 的实际查找候选；Darwin 保留 xcrun 返回的工具 locator，以便正常 symlink 改选使记录更新。补齐后的本机 [clippy](validation/executable-darwin-clippy-followup.log.gz)、[toolchain 31 项](validation/executable-darwin-tools-followup.log.gz)和 [native CLI](validation/executable-darwin-native-final.log.gz)，以及 Linux [clippy](validation/executable-linux-clippy-followup.log.gz)、[toolchain 34 项](validation/executable-linux-tools-followup.log.gz)、[GNU native](validation/executable-gnu-native-final.log.gz)／[musl native](validation/executable-musl-native-final.log.gz)复验全部通过。直接运行稳定缓存路径后，本机[最后 3 个用例、4 variants、58 processes](validation/executable-darwin-run-final.log.gz)通过，覆盖完整 process 行为、测试过滤与 executable 更新；对应 [clippy](validation/executable-darwin-clippy-run.log.gz)通过。

无本机 cargo／rustc 进程时清理 `target/debug/incremental` 810.7 MiB。文件保持职责划分：executable 编排、key、record 分别为 88／46／56 行，统一输出发布为 91 行。

最终键复用已解析的 C 调用记录，包含实际 driver、SDK／sysroot 和生效环境，避免仅有文件 metadata 时遗漏调用配置；不重新发现工具。本机最后一次 [clippy](validation/executable-darwin-clippy-key.log.gz)及 [executable CLI](validation/executable-darwin-key-final.log.gz)通过；Linux 对应 [clippy](validation/executable-linux-clippy-key.log.gz)通过。稳定缓存执行路径的 [GNU](validation/executable-gnu-run-final.log.gz)和 [musl](validation/executable-musl-run-final.log.gz)各 3 个用例、4 variants、58 processes 通过。

保留源码、工具及所有产物，以同一实际配置采集最终 [GNU 摘要](validation/executable-gnu-warm-summary.json)和 [musl 摘要](validation/executable-musl-warm-summary.json)：build 仅有 scoop 的 1 次 exec；run／test 各为 scoop 加缓存程序的 2 次 exec，程序 argv[0] 仍为用户输出位置。所有请求的 scoopc child 数为 0，生成物 metadata 不变，rename 数为 0；[内容打开检查](validation/executable-content-opens.json)确认没有源码、头文件、Cone.toml、`.slib`、对象、系统 stub 或 executable 的用户态内容打开。原始 trace 为 [GNU build](validation/executable-gnu-build-warm.trace.gz)、[run](validation/executable-gnu-run-warm.trace.gz)、[test](validation/executable-gnu-test-warm.trace.gz)，以及 [musl build](validation/executable-musl-build-warm.trace.gz)、[run](validation/executable-musl-run-warm.trace.gz)、[test](validation/executable-musl-test-warm.trace.gz)。采样前正常构建一次，记录工具重建后的更新；采样脚本及 argv 随证据保存，不把工具配置变化计为暖命中。

新增 GNU 正式 [工具改选 fixture](validation/executable-gnu-tools-final.log.gz)，1 个用例、18 processes 通过：通过真实 GCC 的 `-B` 选择真实 ld 的调用包装器，改选后只新增一个 executable、Scoop child 数为 0；两次暖构建的工具调用日志与生成物 metadata 均不变。[补充调用计数](validation/executable-gnu-ld-summary.json)为 4／4／8／8，实际运行均返回 44。补充脚本最初只改 PATH，但该 GCC 直接返回带 target 前缀的 ld 绝对路径，未选中包装器；[首次记录](validation/executable-gnu-trace-first.log.gz)保留，改用真实 `-B` 选择后通过，没有更改生产工具来制造命中。

本批 executable 结果复用与直接运行已完成。本节前述 final 工具发现持久化、Scoop 对象路径、artifact 消费重放、dump 和历史测试清理仍是 M48 未完成的交付项；本批的定向验证不替代最终全量回归与五轮性能复测。

## 15. M48-4：最终链接工具发现记录

新增 `final-tools-v1`，复用实际 linker、版本、运行库选择、系统输入摘要、探测头文件／搜索候选及 Darwin 已解析的 SDK 导出。普通构建与两种显式链接入口共用同一记录；源码或构建 profile 改变时只执行必要的程序编译和链接。实际工具、头文件、搜索候选、模式或有效 unwind 配置变化才重新发现，失败不发布。缓存键包含绝对选项路径和默认配置实际选中的 unwind prefix，避免默认 sysroot 变化后复用旧结果。记录直接保存原有 profile 所需数据，不添加另一套资格、内容复核或缓存损坏恢复路径。

Darwin 不再长期持有 SDK stub 原始字节或复制探测 stub 树；首次读取保存摘要和长度，NativeInputId 直接复用，身份编码保持。首次探测使用真实 SDK 文件，程序链接仍生成其所需导出的投影 stub。清理范围见 [记录](CLEANUP.md)。缓存编排为 82 行，Darwin／Linux profile 为 288／197 行；本机无 Rust 构建进程时清理 `target/debug/incremental` 963.4 MiB。

本机 [workspace clippy](validation/final-tools-darwin-clippy.log.gz)、[scoop 86 项与 toolchain 31 项](validation/final-tools-darwin-rust.log.gz)、[链接器 4 项](validation/final-tools-darwin-linker.log.gz)通过。正式 CLI [新增独立用例](validation/final-tools-darwin-fixtures.log.gz)、[五项组合回归](validation/final-tools-darwin-regression.log.gz)、[native artifact 两种 profile](validation/final-tools-darwin-artifacts.log.gz)共覆盖 7 个用例、9 variants、121 processes，验证源码更新、C／C++ native、native 库改选、测试过滤、debug／release、两种显式链接及真实运行。实际 unwind 键修正后的 [release 构建](validation/final-tools-darwin-release-final.log.gz)和 [新增用例复验](validation/final-tools-darwin-fixtures-final.log.gz)通过。

Linux [首轮 Rust 测试](validation/final-tools-linux-rust-first.log.gz)通过 scoop 89 项、linker 17 项和 toolchain 34 项；新增测试首次因测试目录未提供 LLVM unwind 配套头文件失败。补齐实际头文件后，[定向复验](validation/final-tools-linux-record-tests-final.log.gz)通过，覆盖实际 header 更新、失败不发布及恢复正常输入后的复用。最终 [workspace clippy](validation/final-tools-linux-clippy-final.log.gz)和 [release 构建](validation/final-tools-linux-release-final.log.gz)通过。[GNU 组合](validation/final-tools-gnu-fixtures.log.gz)通过 9 个用例、12 variants、147 processes；有效 unwind 键修正后的 [独立用例复验](validation/final-tools-gnu-fixtures-final.log.gz)通过。

源码、工具和产物始终保留。[GNU trace 摘要](validation/final-tools-gnu-trace-summary.json)记录实际源码从 48 改为 49：一次 Scoop child，加实际 startup C 编译与最终链接，共 8 次 exec；没有工具查询或探测编译／链接，发现记录 metadata 不变。随后暖构建仅 1 次 scoop exec，child 为 0，产物 metadata 不变，没有源码／header／manifest／slib／object 内容打开。保留[源码更新 trace](validation/final-tools-gnu-changed.trace.gz)、[暖构建 trace](validation/final-tools-gnu-warm.trace.gz)与[采样脚本](validation/final-tools-trace.py.gz)。另以独立 `scoop-link` 改变 `SCOOP_SYSROOT`，选中新配置中的实际错误 header 时失败且不发布，修正 header 后链接并运行返回 49；[记录数为 1／1／2](validation/final-tools-config-summary.json)，[脚本](validation/final-tools-config.py.gz)与错误输出一并保留。

[musl 组合](validation/final-tools-musl-fixtures.log.gz)通过 8 个适用用例、11 variants、121 processes，包含 static 和 PIE、显式链接、GC／EH、test 选择和 native 输入更新；C++ 与 GNU 专用工具改选两项按原目标声明不适用。[musl trace 摘要](validation/final-tools-musl-trace-summary.json)得到相同复用结果：源码更新后一次 child，共 10 次真实编译／链接 exec（包含 musl driver wrapper），发现记录不变且无工具探测；随后暖构建仅一次 scoop exec、无 child、无相关内容读取且产物 metadata 不变。原始[更新 trace](validation/final-tools-musl-changed.trace.gz)和[暖 trace](validation/final-tools-musl-warm.trace.gz)保留。

本批最终链接工具发现复用已实现并完成定向验收。M48 尚需稳定 Scoop 对象路径、普通 artifact 消费及 replay／双份数据／验证链清理、dump 最小阶段复用、1,618 项源码隔离测试迁移，以及最终全量回归与五轮性能复测；这些要求没有缩减，M38 保持未启动。


## 16. M48-4：首次生产稳定对象文件

归档 writer 在生成成员时保存已有内容摘要和 payload 范围，scoopc 在发布归档前借用同一缓冲区写出 `objects-v1/<digest>.o`；parent 将实际缓存目录传给 child。链接复用已有对象路径，首次显式导入或所需对象缺失时才补产；相对缓存目录转换为绝对对象路径，避免 native 工具切换工作目录改变路径含义。普通 native 独立 `.o` 使用原文件路径，选中的归档成员才需要稳定独立文件。发布使用临时文件与原子 noclobber，并发已有结果直接使用，没有内容读取、重复哈希、sync 或资格校验。链接计划及 embedded native 身份复用已有摘要。

本机 [workspace clippy](validation/object-files-darwin-clippy.log.gz)、[归档 8 项](validation/object-files-darwin-archive-tests.log.gz)、[并发对象发布 1 项](validation/object-files-darwin-store-tests.log.gz)、[scoop 86 项](validation/object-files-darwin-scoop.log.gz)和[链接器 4 项](validation/object-files-darwin-linker-latest.log.gz)通过。[9 个 CLI 组合用例](validation/object-files-darwin-combined.log.gz)共 13 variants、133 processes 通过，覆盖对象生产、多 Cone、C／C++ native、显式链接、源码与 native 输入更新、测试选择和 moving GC；普通 native 原路径调整后的 [22 次进程定向复验](validation/object-files-darwin-native-latest.log.gz)通过。相对缓存路径修正后，[clippy](validation/object-files-darwin-clippy-relative.log.gz)、[release 构建](validation/object-files-darwin-release-relative.log.gz)与 [m48-object-files 的 20 次进程复验](validation/object-files-darwin-relative.log.gz)通过。

Linux [workspace clippy](validation/object-files-linux-clippy.log.gz)、[release 构建](validation/object-files-linux-release.log.gz)、[归档 8 项](validation/object-files-linux-archive-tests.log.gz)、[并发发布 1 项](validation/object-files-linux-store-tests.log.gz)、[scoop 89 项](validation/object-files-linux-scoop-tests.log.gz)和[链接器 17 项](validation/object-files-linux-linker-tests.log.gz)通过。[GNU](validation/object-files-gnu-fixtures.log.gz)通过 9 个用例、13 variants、133 processes；[musl](validation/object-files-musl-fixtures.log.gz)通过 8 个用例、12 variants、121 processes，C++ 用例按原目标声明不适用。

相对目录补充的 Linux [clippy](validation/object-files-linux-clippy-relative.log.gz)、[release 构建](validation/object-files-linux-release-relative.log.gz)，以及 [GNU](validation/object-files-gnu-relative.log.gz)／[musl](validation/object-files-musl-relative.log.gz)各 20 次进程复验通过。保留全部源码、工具和原产物，先正常更新重建后的工具输入，再对未变化输入取构建结果并显式重新链接；[GNU](validation/object-files-gnu-trace-summary.json)和 [musl](validation/object-files-musl-trace-summary.json)均确认 compiler child 为 0，native linker 实际打开 `objects-v1` 与原始 native `.o`，没有临时 `cone-*.o`／`native-*.o`，对象 metadata 全部不变，程序分别返回 43／42。保留 [GNU 普通链接](validation/object-files-gnu-relink.trace.gz)／[native 链接](validation/object-files-gnu-native.trace.gz)、[musl 普通链接](validation/object-files-musl-relink.trace.gz)／[native 链接](validation/object-files-musl-native.trace.gz)及[采样脚本](validation/object-files-trace.py.gz)。这些是显式 relink 的对象复用证据，不表示 `.slib` 已停止重读或 replay 已移除。

归档 reader／writer 分为 354／158 行，对象文件 helper 连同并发测试为 71 行；未作机械切割。无本机 Rust 构建进程时[清理 `target/debug/incremental` 3,411.5 MiB](validation/object-files-darwin-cleanup.log.gz)。测试及退役范围见[清理记录](CLEANUP.md)。

本批解决稳定对象路径和重复摘要计算；当前 Link 消费仍读入完整 `.slib` 并执行旧 replay，不能声称已实现按需读取或移除验证链。普通 artifact reader、双份数据与 replay 清理、dump 最小阶段复用、1,618 项源码隔离迁移、最终全量回归和五轮性能复测仍待完成；M38 未启动。
