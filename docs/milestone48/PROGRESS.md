# M48 实施进度

本文保留各批次当时的实施结果；当前完整验收见[结果概要](validation/README.md)。原始日志、trace 与逐次报告保存在仓库外。

基线：`6571bb4a`，M46 已完成。按用户要求，本期完全实现、提交并验收后才推进 M38；此前 M38 TextWriter 批次已撤回。

## 1. 分批状态

| 批次 | 状态 | 当前证据与剩余工作 |
| --- | --- | --- |
| M48-1 合同、清理范围、release 基线 | 完成 | 三份规范、AGENTS 与 runner 说明已同步；1,628 份历史隔离候选全部迁移。同机原版本 55 次基线完成。 |
| M48-2 提前更新判定与普通构建记录 | 完成 | 源码、manifest、工具及产物摘要使用更新记录；所有暖构建只启动 scoop 自身，不读取受检输入／产物内容。 |
| M48-3 按需读取与删除消费重放 | 完成 | 保存记录 reader 直接消费；旧 Compile、HIR／MIR／LIR 重放及专用工厂退役。真实下游依赖一次打开、无重复读取，4,207 个机器 member 未读。 |
| M48-4 对象／native／runtime／最终链接复用 | 完成 | native／runtime、最终工具、稳定对象及 executable 直接复用；build／run／test 行为与真实更新均通过三平台 CLI 验收。 |
| M48-5 全量测试迁移与最终验收 | 完成 | dump 缓存与最小阶段补算、MORE-CLEANUP 全部完成；Darwin Rust 5,302、Linux Rust 5,338，CLI Darwin 4,089／GNU 4,078／musl 4,074 项通过；最终性能与调用观察完成。 |

## 2. 测量口径

[测量脚本](../../tests/benchmarks/m48/measure.py)使用 release 版 scoop/scoopc 和 debug Scoop profile。core、std、空 main、真实多 Cone 标准库／JSON 应用、C native／runtime 程序分别执行五组独立冷／暖命令；native 组再改变实际头文件执行增量构建。源码和工具全程保留，每组完成后清理该组缓存和 target。

冷缓存指新的 Scoop 缓存，不刷新 OS 文件缓存。每次保存真实命令、原始 stdout/stderr、构建结果及精确的 compiler build child 数；进程树 RSS 为 ps 抽样的同时存活后代总和。进程抽样可能漏掉短进程，不能把抽样工具列表当作精确 native／链接调用数。最终结构验收还须用实际调用观察证明这些工作已经消失。

## 3. 清理纪律

原始 [扫描清单](SOURCE-ISOLATION.tsv) 是迁移候选，不是删除语言测试的名单。后续每批保存旧路径、保留断言、新落点或删除原因；M48-5 重新扫描新增 case、内联描述、Rust helper 和活动设计。历史已完成记录保留当时事实，当前执行合同采用本期规范。

## 4. M48-2：配套编译器记录

首次使用按实际路径、mtime、大小及配套配置查询能力并保存普通 `tools-v1` 记录；连续构建直接复用记录，不再捕获整个 scoopc、读取程序内容计算摘要或在 child 执行前后重捕获。实际工具启动与协议错误仍传播；并发首次使用经普通文件锁共享一次探测，记录写后普通 rename，不读回或同步落盘。路径在记录中保存原始字节，语言身份与 runtime ABI 不变；compile key domain 更新为 v4。

本机 workspace fmt/clippy 通过，scoop 单元测试 106 项通过，fixture runner 公共规则 49 项通过，正式用例发现 4,093 项通过。新增[独立库用例](../../tests/fixtures/m48-compiler-cache/library/fixture.toml)与[native 组合](../../tests/fixtures/m48-compiler-cache/native/fixture.toml)均通过：冷编译 2 次、暖编译 0 次、仅更新实际工具 mtime 后 2 次、再次暖编译 0 次；native 每轮执行均输出 `native-c-ok`。工具与源码全程保留。Linux fmt/clippy 和 scoop 单元测试 109 项通过（含非 UTF-8 工具路径）；GNU、musl各 2 个正式 CLI 用例通过。非 UTF-8 文件名的实际工具运行用例只适用于 Linux 文件系统。

此子批只完成配套 scoopc 复用，当前仍存在源码镜像、缓存产物复核、native 预处理和最终重新链接，不能据此判定 M48-2 或 M48 已完成。已清理本机 `target/debug/incremental` 约 15 GiB，保留后续构建所需依赖与工具。

GNU 的空程序暖构建另经 `strace -f -e trace=execve,openat` 核对：没有启动 scoopc 能力查询或编译 child，也没有打开 scoopc 文件内容。对照原基线的 1 次能力查询，本批该调用已消失；native／最终链接调用仍存在，留在后续批次处理。

## 5. M48-2：Scoop 源码读取

manifest／single-file 的实际 Scoop 源码只读取一次供 UTF-8 解码与 parser 输入使用，不再在读取前后比较 inode、ctime、mtime、长度与重新解析后的路径。删除无剩余调用的 `StableFileObservation`。源码发现允许普通 symlink 指向 Cone 外的文件／目录；选中路径类型、缺失输入、重复选择、逻辑路径、目录遍历循环及 UTF-8 错误仍正常报告。此处不代表 Preparer 已改为提前命中，也不代表其他 native／产物路径审计已清除。

本机 workspace fmt/clippy、manifest 测试 56 项通过；2 个新 CLI 用例验证显式外部文件、默认目录内 symlink、外部测试目录、冷／暖构建与真实源码变化；M33 源码选择 33 个用例通过（34 variants、44 processes），保留缺失、循环、重复与语法／编码诊断。当前用例总数 4,094，较前一批减少 1 个退役的逃逸拒绝 case、增加 2 个实际编译用例。Linux fmt/clippy、manifest 测试 56 项通过；GNU 与 musl 各通过 2 个新用例和 33 个 M33 回归用例（musl 新用例、musl 回归）。

## 6. M48-2：实际输入路径

Preparer 将实际 manifest 根、single-file 源码及 prebuilt／cache 产物路径交给 scoopc。删除源码／manifest／依赖产物镜像、缓存命中后复制 `.slib`、输入树只读封存、写后读回、同步落盘和子输出目录审计；CompletedNode 只保存一份实际产物路径，删除 PrivateArtifactPath 及原路径映射。native 预处理结果目前仍作为普通临时输入写出，等 M48-4 的 depfile 更新判定与实际编译调整；本批没有宣称缓存判定已经先于源码读取、预处理和产物复核。

`staging.rs` 从 596 行缩至 117 行，`prepared.rs` 从 346 行缩至 261 行。本机 workspace clippy、scoop 单元测试 104 项通过（减少 2 项纯 staging 审计测试）。正式 CLI 8 个用例、9 variants、50 processes通过，覆盖单文件、实际 prebuilt 路径、cold／warm／输入变化、dump、native、外部源码与测试目录、菱形依赖及 moving GC。隔离 case 的迁移与删除见 [清理记录](CLEANUP.md)。另清理无构建进程使用的 `target/debug/incremental` 约 1.9 GiB。Linux workspace clippy、scoop 单元测试 107 项通过；GNU与 musl各通过同一组 8 个用例、9 variants、50 processes。

GNU 使用上述 single-file 用例的现有源码、工具及缓存追加一次实际暖构建跟踪：execve/openat 记录、构建记录。compiler child 为 0，记录中没有 scoopc 启动、源码 snapshot 镜像路径或 completed-artifacts 副本路径；源码内容读取、产物复核与最终链接仍须后续删除，未据此声称整个暖构建已经完成优化。

## 7. M48-2：普通文件内容读取

`ImmutableInputSnapshot` 只共享本次已读取的字节和摘要；删除稳定文件观察、读前／读后 metadata／inode／ctime 比较、重新解析路径对比、no-follow reader 及只为复核读取长度的分配／限长路径。实际路径定位与普通 `fs::read` 错误照常传播，symlink 按正常 I/O 跟随。native 预处理不再重查已读取输入并循环重试三次；源码、头文件和预处理结果的完整读取／持有仍待 M48-4 替换为更新判断和实际编译记录。

本机 workspace clippy、manifest 56 项与 toolchain 26 项测试通过；调整两项 native 测试的输入变更时序后针对性复验 6 项通过。5 个正式 CLI 用例、44 processes通过，覆盖 C／C++ 实际头文件、flags、源码和模式变化，以及 dump 和已有产物路径。删除 344 行稳定 reader，文件内容实现及错误定义集中为 153 行。Linux workspace clippy、manifest 56 项和 toolchain 29 项通过；GNU 5 个用例、44 processes通过，musl 3 个用例、27 processes通过，另 2 个 C++ 用例按既有目标声明不适用。

## 8. M48-2：源码元数据先于内容读取

Preparer 只保存实际选中的源码逻辑名、路径、mtime、大小和用途，删除父进程的源码字节／摘要副本。manifest 与 single-file 的内容读取留给实际执行的 scoopc；源码发现使用已有 metadata，不另外为更新判定读取内容。compile key domain 升为 v5，本机实际路径参与缓存定位但不改变语言身份。诊断显示仅按需读取涉及的源文件并在本轮共享；无效 UTF-8 由实际读取源码的 scoopc 报告，protocol 7 新增只有 host path 的文件级 origin，保留文件路径且不伪造字节区间。

本机 workspace clippy、manifest 57 项和 protocol 25 项、诊断显示通过。scoop 原有 104 项中 103 项通过，协议版本升级后固定缓存键向量按新编码更新，6 项 cache-key 复验通过，保留首次结果。37 个源码／CLI 回归中的 36 项通过；UTF-8 文件级诊断修复后，5 个相关 CLI、24 processes全部通过，覆盖 single-file、外部源码／测试目录、真实输入变化、缓存 warning 与 dump。新增只 touch 源码的断言：编译 child 为 1，但 artifact fingerprint 不变；内容变化仍得到新运行结果。

本批尚未删除缓存产物收据和整包读取，也未完成 manifest 解析复用、native 及最终链接提前命中。Linux workspace clippy、scoop 107 项、manifest 57 项和 protocol 25 项通过；GNU与 musl各 37 个正式 CLI 用例、38 variants、67 processes 通过。源码和工具全程存在，复用 GNU single-file 用例的实际缓存追加暖构建 trace：没有打开 `.scoop` 内容，没有 scoopc exec；构建记录中 child 数为 0。此处仍有 `.slib` 读取、native／最终链接工作，未将其算作整体命中优化完成。

## 9. M48-2：普通编译结果记录与库输出复用

protocol 8 直接返回生产时已有的 `ArtifactManifestSummaryV1`（含测试目录），取代七个独立指纹及 parent 的 child 成功产物交叉核对。摘要删除无人消费的归档长度、成员数和重复兼容性字段；生产代码直接共享测试目录，不重新解码自己的目录数据。cache 使用新 `compile-v2` namespace，以 `record.cbor` 保存摘要和 warning；命中只读记录与查询产物是否存在，不捕获 `.slib` 字节、不比较收据、不复核 warning 来源或生产闭包。缺失输出仍使该结果更新，普通读取失败照常报告。

成功 child 的临时输出经 rename 进入缓存，直接使用原摘要；保留共享／独占构建锁与取得独占锁后的重查，删除目录／权限／inode 审计、sync、写后读回、并发产物等价性比较和整份字节长期持有。`.slib` 生产发布也删除自己的 readback 和 sync。BuildArtifact 只在实际链接消费时才读取数据；库已有用户输出按旁置的结果记录直接返回，新输出／缺失输出只复制一次，其他构建覆盖同一位置时更新相应记录。

本机 workspace clippy、protocol 25 项、summary 3 项、driver 协议 4 项和 4 项诊断分类复验通过。scoop 原有 84 项中的 83 项通过；protocol 8 的固定缓存键向量更新后，9 项 cache-key／store 复验通过。CLI 的 6 个组合用例、新库输出用例与菱形依赖／moving GC通过，成功用例共 8 个、9 variants、54 processes；新 fixture 首轮遗漏默认 src 目录已修正，另保留含 1 次失败进程的首次日志。测试退役与替换详见 [清理记录](CLEANUP.md)。另在无 cargo/rustc 进程时清理 `target/debug/incremental` 728.5 MiB。

本批尚未完成 manifest 解析复用；显式 artifact 发现及 Compile／Link 消费仍有整包校验和 replay，native／runtime 暖构建及程序最终链接也仍在执行。Linux workspace clippy、scoop 87 项与 protocol 25 项、summary 3 项和 driver 协议 4 项通过；GNU与 musl各 8 个正式 CLI 用例、9 variants、54 processes 通过。源码、工具和原缓存产物始终存在，复用 GNU 库用例的已有输出追加暖构建系统调用记录：没有打开 `.scoop` 或 `.slib` 内容，没有 scoopc exec，也没有 rename 发布；构建记录中 child 数为 0。trace 仍显示 C 工具链版本、预处理和探测编译调用，留待后续提前更新判定；不能据此标记 M48 完成。

## 10. M48-2：manifest 解析记录

父进程的根、显式源码依赖、默认 core 和默认标准库入口共用 `manifests-v1`。按实际路径、mtime、大小及解析器 target catalog 定位解析结果；命中不读取 Cone.toml、不重新执行 TOML parser。记录直接保存解析后的生产／测试／native 配置、locator、诊断区间及对应文本，不维护另一份语言规则或校验链。普通构建锁协调首次解析，成功后 rename 发布；无效 manifest 不发布，缺失输入仍先报告定位错误。有效编译键继续使用用途投影，仅测试配置变化不使生产结果失效。源码保持存在，实际执行的 scoopc 仍正常读取自己的 manifest。

本机 workspace clippy、manifest 57 项、解析记录 3 项复验通过。scoop 首轮 87 项中的 82 项通过；5 项失败均源于旧测试 helper 将缓存配置在其断言不存在的 sysroot 下，改为独立缓存目录后 17 项 core 相关复验通过。解析记录测试覆盖完整配置／span、真实更新、无效与缺失输入、失败不发布，并保持负 Unix 时间戳可用。

CLI 首轮通过 24 项；manifest 源码 UTF-8 负例同步已落地的 child 文件级诊断后，2 项定向复验通过。成功用例合计 25 个、27 variants、85 processes，覆盖新用例的冷／暖、仅测试配置更新、实际源码选择更新与 48／49 退出值，以及测试依赖、target 条件、native、manifest 错误、菱形依赖和 moving GC。保留首轮失败日志；退役范围没有扩大，说明见 [清理记录](CLEANUP.md)。无 cargo/rustc 进程时清理本机 `target/debug/incremental` 1,544.3 MiB。

Linux workspace clippy、scoop 90 项与 manifest 57 项通过；GNU与 musl各 25 个正式 CLI 用例、27 variants、85 processes 通过。对 GNU 库的已有输出追加暖构建 trace：只打开根与 core 的两份解析记录，没有打开原 Cone.toml、`.scoop` 或 `.slib` 内容，没有 scoopc exec 或 rename 发布；构建记录中 child 数为 0。源文件、工具和原始产物全程保留。native 工具探测、预处理、消费 stage 的重复验证和最终链接复用仍待处理，M48 尚未完成。

## 11. M48-2：C 工具发现记录与 child 复用

父进程通过 `native-tools-v1` 保存普通 C 工具发现结果，按请求、实际生效的 PATH／REALGCC、工具和探测头文件／specs／搜索目录的 metadata 判断更新。Darwin 同时观察选中的 developer、SDK 和系统 deployment 输入；Linux 不再整读 GCC、cc1 或 assembler 来构造 compiler identity，产物使用版本与实际预处理宏配置，本机构建键单独携带工具输入 metadata。协议升为 9、compile key domain 升为 v6；parent 把完整调用记录交给 child，child 解码后直接用于编译，不重复探测。没有引入收据、内容复核、权限检查或工具移除验收。

本机 workspace clippy、protocol 26 项和 toolchain 28 项、driver 协议 4 项通过。scoop 首轮 87 项中的 86 项通过，新增键维度使旧固定向量失效；同步向量后 6 项缓存键复验通过。另保留协议首轮因帧长度增加一字节而失败的旧期望日志，更新后通过。成功 release 构建及 12 个正式 CLI 用例、13 variants、85 processes覆盖库与程序、源码／manifest／编译器更新、C 和 C++ native、独立 scoopc 和 moving GC。新增 Linux 用例 `m48-native-tool-record` 保留真实工具，用调用日志检查首次仅探测一次、暖构建无调用、工具 metadata 变化后重新探测和再次复用；GNU 与 musl 均通过。无 cargo/rustc 进程时清理本机 `target/debug/incremental` 3,753.1 MiB。

本批仅完成 C 发现与 child 传递；C++ 发现、native 预处理、runtime 对象和最终 executable 更新判定仍待处理，M48 尚未完成。

Linux workspace clippy、scoop 90 项／protocol 26 项／toolchain 31 项、driver 协议 4 项和 release 构建通过。GNU通过 17 个正式 CLI 用例、18 variants、94 processes；musl通过 13 个用例、15 variants、76 processes。新增用例的 GNU 调用记录与 musl 调用记录各只有冷构建和真实工具修改后的两组探测，两次暖构建没有新增调用，child 也没有重复发现。保持所有源码、工具和原产物存在，对 GNU 同一库与同一输出再执行一次暖构建 trace：只有 scoop 自身的 1 次 execve，没有 native 工具／scoopc exec，没有 Cone.toml／`.scoop`／`.slib` 内容打开，也没有 rename 发布；结果中的 child 数为 0。

## 12. M48-2／4：C++ 发现与用户 native 对象复用

新增普通 `cxx-tools-v1` 与 `native-objects-v1` 记录。C++ 发现只观察实际配套 driver、前端、标准库、宏和头文件输入，父进程的 native 编译及最终链接共享该结果；不整读工具／标准库二进制。native 记录保存实际源码／头文件 metadata、include 选择、有效 flags、工具配置和稳定对象路径，命中不调用预处理器、不读头文件或对象、不复制预处理单元。仅 Scoop 源码变化时复用原 native 对象；真实头文件变化、增加优先候选或所需对象缺失时更新。全部 native 编译成功后发布记录，旧对象仍供已有构建使用。协议升为 10、compile key domain 升为 v7；parent 将对象位置交给 child 打包，child 不再发现 C++ 工具或重新编译同一 native 源码。独立 scoopc 保留正常源码编译。

候选观察来自编译器 `-dI` 展开的实际 include 名称及所在文件，覆盖宏、嵌套引号查找、forced header 和 imacros；移除观察指令时保留行号和 C++ 原始字符串。首次 CLI 采样发现头文件父目录交叉展开产生约 20 万项／116 MB 记录，且 JSON 未缓冲写入；现按实际 include 查找生成候选并缓冲写入，同一 C++ 用例降为约 9700 项／5.6 MB。保留异常采样，不把被中断的运行计为验收通过。

本机 workspace clippy、scoop 87 项／protocol 26 项／toolchain 31 项、driver 协议 4 项和 release 构建通过。对象测试覆盖暖复用、Scoop 文件更新不重编 native、实际头文件更新、嵌套目录新增优先头文件、无关文件不失效、所需输出缺失和失败不发布。先前的首次 Rust 结果中旧键向量和外部路径比较期望已同步，最终相关 crate 全部通过。

CLI 定向复验通过 17 个用例，含 C++ debug／release、传递依赖、真实 native 冲突、外部实际输入、两个新对象复用用例；2 个 imacros 用例暴露 Clang 的额外观察指令，修复后最后 5 个用例、54 processes全部通过，覆盖 C／C++ flags、头文件、源码更新及新对象复用。新用例按冷／暖、仅 Scoop 更新、头文件更新、增加优先头文件运行，consumer 退出值为 43／44／102。另保留首次中断及修正诊断前的中断结果。历史隔离和路径审计迁移见 [清理记录](CLEANUP.md)。本机无 cargo/rustc 进程时清理 `target/debug/incremental` 1,287.2 MiB。

本批尚未完成 runtime 对象、最终 executable 复用、artifact 消费重放清理与全量历史测试迁移，M48 仍未完成。

Linux workspace clippy、scoop 90 项／protocol 26 项／toolchain 34 项、driver 协议 4 项、release 构建及 musl 两种链接模式的 C++ 需求诊断通过。首次 Linux clippy 暴露平台限定测试漏传新参数，原始诊断保留；修正调用并移除测试源码删除步骤后重新验证通过，本机后续 clippy也通过。

GNU 正式 CLI中 43 项通过；4 个自带工具选择的负例因 runner 额外传入 `--cc` 而重复参数，去掉 runner 的该参数后4 项独立复验全部通过，合计 47 个适用用例完成。musl通过 38 个用例、41 variants、90 processes，另 10 个按 target 声明不适用。覆盖 native 冷／暖、实际源码／头文件／flags／工具更新、C++ stdlib、重复定义、外部正常路径、传递产物及运行。

保留全部源码、工具与产物，对 GNU 新用例的 C／C++ 库输出分别执行同配置暖构建：C trace及 C++ trace均仅有 scoop 自身 1 次 execve，没有 native 工具或 scoopc exec，没有打开 Cone.toml／Scoop 或 native 源码／头文件／对象／`.slib` 内容，没有 rename；C 结果及 C++ 结果的 child 数均为 0。首次 trace 误用 `/usr/bin/gcc` 别名而非 runner 传入的规范路径，属于不同请求并触发更新，保留该次结果；上述暖构建证据使用与 fixture 完全相同的 compiler、PATH、target、输出及缓存配置。库验收不替代仍待实施的 runtime／最终 executable 复用。

## 13. M48-4：runtime 对象记录与直接路径链接

新增 `runtime-v2` 记录，按实际选中源码、depfile 头文件、include 候选、flags、C 工具 metadata 和 runtime 规则判断更新。命中不扫描／复制 runtime 树、不预处理、不读取源码／头文件／对象或 compiler 二进制；变化后直接编译原始源码，保留不同批次的稳定对象路径，全部成功才发布记录。复用已有 native include 观察和路径编码，不新增一套依赖解析器。runtime index schema 升为 2，保存生产时的对象摘要、合并符号与实际位置；读取不重算摘要／ABI 导出或复制对象，最终 native linker 直接使用这些路径。

删除最终 executable 的完整 readback、map／trace 来源证明与对象／stackmap／EH／动态绑定／code signature 重放，连同只服务它们的证明数据和专测。对象生产及实际 native 输入的格式／relocation／ABI 检查、真实链接失败传播、正常运行和 GC 行为保留；完整迁移范围见 [清理记录](CLEANUP.md)。本批代码净减少约四千行；runtime 编排与记录、依赖、编译模块分别保持在百行左右。新 `m48-runtime-record` 正式 CLI 用例覆盖冷／暖、无关头文件、只改 Scoop、改 runtime 头文件及源码后的真实返回值。

本机 workspace clippy、toolchain 31 项／protocol 26 项、runtime 3 项与 release 构建通过。链接器初轮 4 项及更新断言后的 1 项复验通过；共享 helper 取消删源码后，最终 4 项全部通过；runner 49 项通过。正式 CLI 15 个适用用例、23 variants、138 processes覆盖 runtime 更新、C/C++、独立链接、菱形依赖、动态库、callback 异常、TLS、ABI 与 moving GC；5 个 Linux 专属项不计为通过。无 cargo/rustc 进程时清理 `target/debug/incremental` 1,217.9 MiB。

本批不代表最终 executable 命中：最终工具发现与链接、Scoop 对象解包、artifact 消费 replay、run 私有副本、dump 最小阶段复用及 1,618 项历史源码隔离迁移仍待完成，M48 继续实施。

Linux workspace clippy、toolchain 34 项／protocol 26 项、runtime 4 项、release 构建和输入保留后的链接器 17 项通过。首次 clippy 的两个平台测试旧返回值断言已同步；不再保留空验证结果，未使用 helper 按实际平台限定。GNU 15 个适用用例、25 variants、139 processes通过，包含 ELF DSO 版本绑定、失败不发布及深调用栈。

对 GNU runtime 用例保留源码、工具与输出执行同配置暖构建 trace：child 数为 0，未打开 runtime 源码／头文件，没有 runtime 编译；父进程未打开 runtime 对象，实际 ld 直接读取已有 `runtime-v2/objects-*` 下的 94 份对象，详见采样摘要。最终工具发现、Scoop 对象消费及 native 链接仍有调用，本证据只证明本批 runtime 复用。首次采样的 PATH 与 runner 不同，触发编译输入更新，保留该次结果；暖构建证据采用 runner 实际生效的 PATH 和全部原配置。

musl 16 个适用用例、26 variants、133 processes全部通过，覆盖静态／PIE、动态库版本绑定、深调用栈、runtime 真实更新、独立链接和 moving GC；4 个不适用项不计为通过。本批在 Darwin、GNU、musl 的验收均保持实际输入存在，已完成 runtime 结果复用与最终输出复核退役；其余 M48 交付范围仍按上文继续。

## 14. M48-4：最终 executable 记录与直接运行

新增 `executables-v1` 普通记录，使用已有 Code／runtime 摘要、target／profile、最终选项、library roots 与实际工具输入定位结果。命中发生在 Link 闭包读取、final 工具发现、startup／response file 与链接之前，只读取更新记录并查询实际输入 metadata 和所需输出是否存在。记录保存稳定 executable 路径与生产时的 link plan 摘要；输入变化后通过普通锁生成新结果，成功才发布记录。显式库与动态依赖记录实际文件、脚本及候选路径，包含缺失候选；symlink 改选与新增候选触发正常解析，不扫描无关目录内容。

库与程序共用结果发布记录；已有输出不再复制，首次另选输出或输出缺失时只发布已有结果。删除 OutputLocation 的 inode／路径来源检查、强制同步与 execution copy；run／test 直接执行本次构建结果的稳定缓存路径，argv[0] 保留用户输出位置；保留 cwd、stdio、exit／signal、测试事件及并发输出替换行为。用户输出的结果标识包含本次实际输入 metadata，与语义 link plan 指纹分开，避免真实链接输入更新后错误保留旧用户输出。final 工具身份采用 metadata，取消整个 compiler／linker 二进制读取。

本机 workspace clippy、toolchain 31 项、scoop 86 项、链接器 1 项及 release 构建通过。正式 CLI 组合首轮通过 8 项；两个新 fixture 的 Unsafe 语法及 run 不接受 `-o` 的请求写法修正后，native 复验和 executable 复验通过。最终成功覆盖共 10 个用例、12 variants、137 processes，包含生产／测试、过滤／列表、不同输出、debug／release、所需输出补产、依赖／native／runtime 更新与正在运行时替换输出。缺失真实 native 输入的补充 CLI 诊断同时确认缓存记录和已有 executable 的 metadata 均未改变。

本批尚未完成输入变化后的 final 工具发现结果复用、Scoop 对象的稳定缓存路径、artifact 消费 replay 清理、dump 最小阶段补产和全量历史测试迁移。M48 继续实施，未推进 M38。

Linux workspace clippy、scoop 89 项／toolchain 34 项、链接器 2 项和 release 构建通过。初次 CLI 预检发现传输归档附带的 macOS `._` 文件，清除这些传输副本后重新运行；保留预检日志，未将该次算作通过。GNU 10 个用例、12 variants、137 processes与 musl 9 个适用用例、11 variants、125 processes通过。

按实际 GNU 工具输出补齐 GCC `-L` 目录之外的 ld `SEARCH_DIR`，以及 ld／collect2 的实际查找候选；Darwin 保留 xcrun 返回的工具 locator，以便正常 symlink 改选使记录更新。补齐后的本机 clippy、toolchain 31 项和 native CLI，以及 Linux clippy、toolchain 34 项、GNU native／musl native复验全部通过。直接运行稳定缓存路径后，本机最后 3 个用例、4 variants、58 processes通过，覆盖完整 process 行为、测试过滤与 executable 更新；对应 clippy通过。

无本机 cargo／rustc 进程时清理 `target/debug/incremental` 810.7 MiB。文件保持职责划分：executable 编排、key、record 分别为 88／46／56 行，统一输出发布为 91 行。

最终键复用已解析的 C 调用记录，包含实际 driver、SDK／sysroot 和生效环境，避免仅有文件 metadata 时遗漏调用配置；不重新发现工具。本机最后一次 clippy及 executable CLI通过；Linux 对应 clippy通过。稳定缓存执行路径的 GNU和 musl各 3 个用例、4 variants、58 processes 通过。

保留源码、工具及所有产物，以同一实际配置采集最终 GNU 摘要和 musl 摘要：build 仅有 scoop 的 1 次 exec；run／test 各为 scoop 加缓存程序的 2 次 exec，程序 argv[0] 仍为用户输出位置。所有请求的 scoopc child 数为 0，生成物 metadata 不变，rename 数为 0；内容打开检查确认没有源码、头文件、Cone.toml、`.slib`、对象、系统 stub 或 executable 的用户态内容打开。原始 trace 为 GNU build、run、test，以及 musl build、run、test。采样前正常构建一次，记录工具重建后的更新；采样脚本及 argv 随证据保存，不把工具配置变化计为暖命中。

新增 GNU 正式 工具改选 fixture，1 个用例、18 processes 通过：通过真实 GCC 的 `-B` 选择真实 ld 的调用包装器，改选后只新增一个 executable、Scoop child 数为 0；两次暖构建的工具调用日志与生成物 metadata 均不变。补充调用计数为 4／4／8／8，实际运行均返回 44。补充脚本最初只改 PATH，但该 GCC 直接返回带 target 前缀的 ld 绝对路径，未选中包装器；首次记录保留，改用真实 `-B` 选择后通过，没有更改生产工具来制造命中。

本批 executable 结果复用与直接运行已完成。本节前述 final 工具发现持久化、Scoop 对象路径、artifact 消费重放、dump 和历史测试清理仍是 M48 未完成的交付项；本批的定向验证不替代最终全量回归与五轮性能复测。

## 15. M48-4：最终链接工具发现记录

新增 `final-tools-v1`，复用实际 linker、版本、运行库选择、系统输入摘要、探测头文件／搜索候选及 Darwin 已解析的 SDK 导出。普通构建与两种显式链接入口共用同一记录；源码或构建 profile 改变时只执行必要的程序编译和链接。实际工具、头文件、搜索候选、模式或有效 unwind 配置变化才重新发现，失败不发布。缓存键包含绝对选项路径和默认配置实际选中的 unwind prefix，避免默认 sysroot 变化后复用旧结果。记录直接保存原有 profile 所需数据，不添加另一套资格、内容复核或缓存损坏恢复路径。

Darwin 不再长期持有 SDK stub 原始字节或复制探测 stub 树；首次读取保存摘要和长度，NativeInputId 直接复用，身份编码保持。首次探测使用真实 SDK 文件，程序链接仍生成其所需导出的投影 stub。清理范围见 [记录](CLEANUP.md)。缓存编排为 82 行，Darwin／Linux profile 为 288／197 行；本机无 Rust 构建进程时清理 `target/debug/incremental` 963.4 MiB。

本机 workspace clippy、scoop 86 项与 toolchain 31 项、链接器 4 项通过。正式 CLI 新增独立用例、五项组合回归、native artifact 两种 profile共覆盖 7 个用例、9 variants、121 processes，验证源码更新、C／C++ native、native 库改选、测试过滤、debug／release、两种显式链接及真实运行。实际 unwind 键修正后的 release 构建和 新增用例复验通过。

Linux 首轮 Rust 测试通过 scoop 89 项、linker 17 项和 toolchain 34 项；新增测试首次因测试目录未提供 LLVM unwind 配套头文件失败。补齐实际头文件后，定向复验通过，覆盖实际 header 更新、失败不发布及恢复正常输入后的复用。最终 workspace clippy和 release 构建通过。GNU 组合通过 9 个用例、12 variants、147 processes；有效 unwind 键修正后的 独立用例复验通过。

源码、工具和产物始终保留。GNU trace 摘要记录实际源码从 48 改为 49：一次 Scoop child，加实际 startup C 编译与最终链接，共 8 次 exec；没有工具查询或探测编译／链接，发现记录 metadata 不变。随后暖构建仅 1 次 scoop exec，child 为 0，产物 metadata 不变，没有源码／header／manifest／slib／object 内容打开。保留源码更新 trace、暖构建 trace与采样脚本。另以独立 `scoop-link` 改变 `SCOOP_SYSROOT`，选中新配置中的实际错误 header 时失败且不发布，修正 header 后链接并运行返回 49；记录数为 1／1／2，脚本与错误输出一并保留。

musl 组合通过 8 个适用用例、11 variants、121 processes，包含 static 和 PIE、显式链接、GC／EH、test 选择和 native 输入更新；C++ 与 GNU 专用工具改选两项按原目标声明不适用。musl trace 摘要得到相同复用结果：源码更新后一次 child，共 10 次真实编译／链接 exec（包含 musl driver wrapper），发现记录不变且无工具探测；随后暖构建仅一次 scoop exec、无 child、无相关内容读取且产物 metadata 不变。原始更新 trace和暖 trace保留。

本批最终链接工具发现复用已实现并完成定向验收。M48 尚需稳定 Scoop 对象路径、普通 artifact 消费及 replay／双份数据／验证链清理、dump 最小阶段复用、1,618 项源码隔离测试迁移，以及最终全量回归与五轮性能复测；这些要求没有缩减，M38 保持未启动。


## 16. M48-4：首次生产稳定对象文件

归档 writer 在生成成员时保存已有内容摘要和 payload 范围，scoopc 在发布归档前借用同一缓冲区写出 `objects-v1/<digest>.o`；parent 将实际缓存目录传给 child。链接复用已有对象路径，首次显式导入或所需对象缺失时才补产；相对缓存目录转换为绝对对象路径，避免 native 工具切换工作目录改变路径含义。普通 native 独立 `.o` 使用原文件路径，选中的归档成员才需要稳定独立文件。发布使用临时文件与原子 noclobber，并发已有结果直接使用，没有内容读取、重复哈希、sync 或资格校验。链接计划及 embedded native 身份复用已有摘要。

本机 workspace clippy、归档 8 项、并发对象发布 1 项、scoop 86 项和链接器 4 项通过。9 个 CLI 组合用例共 13 variants、133 processes 通过，覆盖对象生产、多 Cone、C／C++ native、显式链接、源码与 native 输入更新、测试选择和 moving GC；普通 native 原路径调整后的 22 次进程定向复验通过。相对缓存路径修正后，clippy、release 构建与 m48-object-files 的 20 次进程复验通过。

Linux workspace clippy、release 构建、归档 8 项、并发发布 1 项、scoop 89 项和链接器 17 项通过。GNU通过 9 个用例、13 variants、133 processes；musl通过 8 个用例、12 variants、121 processes，C++ 用例按原目标声明不适用。

相对目录补充的 Linux clippy、release 构建，以及 GNU／musl各 20 次进程复验通过。保留全部源码、工具和原产物，先正常更新重建后的工具输入，再对未变化输入取构建结果并显式重新链接；GNU和 musl均确认 compiler child 为 0，native linker 实际打开 `objects-v1` 与原始 native `.o`，没有临时 `cone-*.o`／`native-*.o`，对象 metadata 全部不变，程序分别返回 43／42。保留 GNU 普通链接／native 链接、musl 普通链接／native 链接及采样脚本。这些是显式 relink 的对象复用证据，不表示 `.slib` 已停止重读或 replay 已移除。

归档 reader／writer 分为 354／158 行，对象文件 helper 连同并发测试为 71 行；未作机械切割。无本机 Rust 构建进程时清理 `target/debug/incremental` 3,411.5 MiB。测试及退役范围见[清理记录](CLEANUP.md)。

本批解决稳定对象路径和重复摘要计算；当前 Link 消费仍读入完整 `.slib` 并执行旧 replay，不能声称已实现按需读取或移除验证链。普通 artifact reader、双份数据与 replay 清理、dump 最小阶段复用、1,618 项源码隔离迁移、最终全量回归和五轮性能复测仍待完成；M38 未启动。

## 17. M48-5：历史源码隔离迁移（fa2d9faf）

剩余 1,618 行扫描清单已逐项记录当前落点，覆盖 core、provider、consumer、native 输入和公共 Rust helper；清单之外的空环境包装一并处理，合计修改 1,646 份 fixture 描述。删除 304 个普通语言用例中的 306 次重复链接，原运行与诊断断言保留；真正的 native／ABI／ODR／runtime／链接专测继续使用真实工具和存在的源码。损坏产物的防御分支及其独占期望退役，缺失输入、失败不覆盖已有程序、正常输入更新和所需输出补产继续覆盖。详细映射、保留文件操作及例外见[清理记录](CLEANUP.md)。本批没有修改生产代码，CLI 使用 `cffba262` 的 release 工具。

本机 workspace clippy、Ruff 格式与lint、49 项 runner 测试和 4,105 项完整预检通过。Rust 17 项 core 发现、类型位置、ODR／优化、泛型对象与独立 core 输入通过。

本机最新通过结果覆盖 75 个不同用例、90 variants、557 processes，包括 17 个 Darwin 专属用例。保留首次代表组、追加组、定向修正和最终发布诊断复验的原始结果：修正了正常源码 core 的 warning 期望、独立 scoopc 所需默认产物、metadata 变化后的重新编译断言，以及旧路径审计退役后实际 I/O 错误的消息。没有通过放宽 stdout／诊断比较来忽略差异。

Linux workspace clippy、17 项 core 发现、类型位置、ODR／优化、泛型对象、独立 core 输入及 49 项 runner 测试通过。GNU 首轮通过 98 项后遇到 `/tmp` 配额／空间错误，musl 首轮未能开始有效验收；两次退出码均为 120，不能计为通过。确认进程已结束后保留日志并清理临时用例目录；续跑工作、日志及 `TMPDIR` 移到仓库所在 NVMe 磁盘，每批 200 项，完成后保留报告并清理临时用例目录。续跑使用第 18 节已验证的 reader 工具，跳过 GNU 已通过的 98 项及同目标 reader 组合中已通过的迁移项；续跑脚本保存选择与顺序。不适用 target 单列，不计为通过。

无本机 cargo／rustc 进程时清理 incremental 193.7 MiB。首次记录时 Linux CLI 结果尚未全部取得；core-default 与 program-link 输入错误两组已随第 18 节提交，续跑与完成结果见下文。M48-5 的 dump 与最终回归仍待完成。reader／replay 清理、dump 最小阶段复用及最终全量和性能复测仍待完成，M38 未启动。

GNU 续跑第 1 批通过 200 项、202 variants、944 processes；第 2 批通过 199 项、199 variants、1,656 processes，另 1 项按目标不适用。前三批报告已保留，其中第 3 批通过 200 项、200 variants、1,588 processes。临时用例目录由续跑脚本在全部子进程结束后清理；后续批次继续执行，未重复运行先前成功项。

第 4 批出现 `imported-class-value-interface-core` 依赖定位失败：续跑脚本重复使用数字用例路径，复制的两个不同 manifest 恰好大小与保留 mtime 相同，命中上一批同一路径的解析记录。这属于测试目录复用问题；不为此添加内容 hash 或缓存恢复。独立工作路径下 Darwin 与 GNU 均通过 1 项、11 processes。后续续跑脚本为每批保留独立源路径，共享已有 cache／sysroot，按报告跳过已通过及不适用项。原第 4 批已结束：199 项通过、上述 1 项失败，199 variants、1,291 processes；失败项以独立路径复验通过。报告已保存，独立批次续跑已启动，未重跑已通过项。

独立路径续跑第 1 批通过 198 项、248 variants、1,428 processes、15 goldens，另 2 项按 target 不适用；报告保存后已清理用例临时目录，第 2 批继续执行。

独立路径第 2 批通过 183 项、217 variants、1,329 processes、31 goldens，另 17 项按 target 不适用；报告保存后清理本批临时目录，第 3 批继续执行。

独立路径第 3 批通过 200 项、200 variants、1,149 processes；报告保存后继续剩余 GNU 用例及 musl 验收。

独立路径 GNU 第 4 批通过 141 项、142 variants、739 processes、1 golden；GNU 剩余项执行结束，续跑进程转入 musl。 按原选择清单与全部成功／不适用报告做集合核对，1,646 个名称均有结果，没有遗漏；这不替代最终当前编译器的全量回归。

musl 独立路径第 1 批通过 200 项、200 variants、864 processes；报告保存后继续第 2 批。

musl 独立路径第 2 批通过 199 项、201 variants、1,387 processes、6 goldens，另 1 项按 target 不适用；报告保存后继续第 3 批。

musl 独立路径第 3 批通过 200 项、200 variants、1,602 processes、5 goldens；报告已保存，续跑进程继续后续批次。

musl 独立目录第 4 批已完成：200 项通过、200 variants、1,524 processes，无失败；日志与报告已归档。其余批次继续使用原冻结工具运行，不能算作最终实现的全量结果。

musl 独立目录第 5 批已完成：200 项通过、230 variants、1,289 processes、3 goldens，无失败；日志与报告已归档。原冻结工具继续第 6 批，当前实现的最终全量验收仍待执行。

musl 独立目录第 6 批已完成：183 项通过、17 项按目标不适用，236 variants、1,433 processes、43 goldens，无失败；日志与报告已归档。冻结工具继续后续批次。

musl 独立目录第 7 批已完成：200 项通过，202 variants、1,200 processes、0 goldens，无失败；日志与报告已归档。冻结工具继续后续批次。


musl 独立目录第 8 批完成：200 项通过、201 variants、1,150 processes；第 9 批完成：36 项通过、36 variants、123 processes、1 golden。后台进程已结束，GNU与 musl分别覆盖原选择清单的全部 1,646 个名称，无遗漏。现有 1,644 份待提交描述、4 份期望和 6 个 helper／说明文件与 Linux 被测文件一致；另两份描述已在前批提交。最终复扫仍为 25 份已审阅的正常 remove／move 用例，Ruff 与 49 项 runner 测试通过。[迁移验收记录](validation/source-isolation.md)区分冻结工具证据与尚未执行的最终当前代码全量回归。本批迁移完成，M48 整体仍未完成。

## 18. M48-3：消费已有内容摘要

提交：`c8ec11f9`。

`ArtifactSnapshot` 删除无人使用的整包摘要；归档 reader 不再逐成员计算 SHA，manifest 解码直接采用保存的 Artifact 指纹，Compile／Link 解码不重算 HIR／MIR／LIR 摘要。生产端仍按原公式生成摘要，wire 字段和版本不变；删除消费端摘要比较的错误类型、测试分支及失去用途的语义哈希适配层。真实格式、区间、typed 引用和目标配置检查保留；没有另加严格 reader。

本机 workspace clippy、567 项 slib 测试、release 构建及 12 个 CLI 用例、16 variants、137 processes通过。Linux fmt、workspace clippy、568 项 slib 测试、release 构建通过；GNU通过同一组 12 项、16 variants、137 processes，musl通过 11 个适用项、14 variants、123 processes，C++ 项按原声明不适用。覆盖普通 core／prebuilt、跨 Cone 泛型／接口、C／C++、测试依赖、M46 序列化、稳定对象及 executable 缓存，以及真实输入错误。

本批仅完成内容摘要重算退役。显式输入的旧 `ImmutableInputSnapshot` 捕获仍可能计算整包 hash，消费 stage 仍保留对象重放、语义／布局证明闭包和重复对象字节；manifest 最小读取与这些路径正在后续批次处理。M48-3 尚未完成。

## 19. M48-3：manifest 最小读取与 prebuilt 准备

提交：`aebc0fee`。

字节输入与文件输入共用 manifest 摘要读取逻辑，只消费归档 magic、首个成员头与 manifest payload；后续对象及 section 不参与依赖发现。归档头字段解析与原成员 reader 共享。prebuilt 准备直接传递路径及已取得的摘要，删除第二次整包捕获、hash、摘要比较和发现前后状态检查；其 `BuildArtifact` 与普通编译结果一样只在实际需要时读取内容。删除只服务改写已发现产物的测试及相关错误变体，目录／缺失输入仍报告实际 I/O 错误。

本机 workspace clippy、85 项 scoop 测试、8 项归档测试、3 项摘要测试通过；摘要专测在完整真实归档上断言实际读取位置停在 manifest 末尾。I/O 分类调整后另做 6 项 locator 复验。release 构建及 7 个 CLI 用例、47 processes通过，覆盖真实 core artifact、源码与 prebuilt 选择、依赖错误、发布复用、独立 scoopc、moving GC 和 M46 序列化。无 cargo／rustc 进程时清理 incremental 2,221.3 MiB。

Linux 使用独立 `target/m48-summary-reader/release`，保留正在运行第 17 节回归的原 release 工具。fmt、workspace clippy、88 项 scoop、8 项归档、3 项摘要及 release 构建通过；GNU和 musl各通过 7 个 CLI 用例、47 processes。输入边界追加 Darwin、GNU和 musl各 13 个 sysroot 负例、15 processes，全部通过。

保留源码、工具及所有产物，对已有 prebuilt core 做 GNU 系统调用采样：41,595,590 字节的产物只打开一次，实际读取 1,964,277 字节，恰好停在 manifest 末尾；compiler child 为 0。保留摘要与命令、构建结果及采样脚本。

本批尚未引入显式 artifact 的摘要更新记录，连续发现仍读取必要 manifest；完整 Compile／Link 的按需 section 共享与 replay 清理也未完成。其余隔离迁移继续使用第 18 节原 release 工具验收；本批的定向结果不替代最终全量回归和性能复测。

## 20. M48-3：依赖摘要更新记录

提交：`0240929b`。

显式 artifact、搜索目录与默认 core 产物共用 `artifact-summaries-v1`，按实际路径、mtime、大小和 target／backend 选择摘要记录。命中不打开归档；输入变化后只读取 manifest，并通过普通锁与原子发布保存结果。真实输入缺失和目标错误仍失败，记录解码不增加恢复分支。实现与单测分别为 112／83 行。

本机 clippy、87 项 scoop 测试、49 项 runner、4,106 项预检通过；Darwin、GNU、musl 最终各通过 21 个 CLI 用例、21 variants、78 processes。新增用例保持所有实际输入存在，覆盖冷／暖、touch、依赖公开常量从 48 改为 49 和运行结果。首轮 fixture 文本替换误匹配包名，修正后只重跑该项，未把失败结果计为通过。Linux 摘要／locator／core 测试及 clippy、release 通过，详见[完整证据](validation/artifact-record.md)。

GNU 暖构建采样确认 41,595,590 字节的 core 产物打开 0 次、读取 0 字节，compiler child 为 0；摘要记录 metadata 不变。生产源码、工具、原产物始终存在。完整 Compile／Link 按需读取、重放与证明链退役、最小 dump 阶段及最终全量／性能验收仍在 M48 范围内，M38 未启动。

## 21. M48-3：codegen 复用扫描计划与 atom 索引

提交：`5f27c057`。

函数对象发射借用 production 中的 runtime scan 计划，按 callable body 查询，不再为每个 LLVM module 重扫全部函数；Context cell 和 runtime scan atom 使用已有边界索引，删除逐 atom 的整 Cone 查找。初始化映射按职责拆为 100 行模块，主发射文件降为 544 行；未增加 wire 或 ABI。

Darwin 374 项、Linux 389 项 codegen 测试通过；Darwin、GNU、musl 各 7 项 CLI 组合、10 variants、58 processes 通过，覆盖 Context、协程、根、借用、M46 序列化及 moving／minor GC。初始化 helper 拆分后，两平台 clippy 和 8 项初始化复验通过；本机清理 incremental 756.4 MiB。证据见[验收记录](validation/scan-reuse.md)。完整 reader／replay、dump、历史清理与最终全量／性能验收继续，M38 未启动。

## 22. M48-3：Compile 直接读取已保存的定义数据

提交：`176b50e5`。

Compile 从共享 identity 数据解析已有 defined owners 与 ODR member ABI，实际跨产物符号归属及 key／ABI 合并继续保留。移除 Compile 的机器对象解析、materialization 重放、final／provisional 对象副本和 Code／RuntimeImage 重建；读取 API 不再要求 C bridge profile。删除无生产消费者的 Compile／Link 重复摘要及对象数量，统一采用 manifest 摘要。相关对象破坏测试及专用 helper 同步退役，类型位置和 ODR 两个 Rust 用例保留源码的迁移随本批提交。

本机 568 项、Linux 569 项 slib 测试及两端 clippy、release、正式发布、ODR 验证通过；本机属性初始化、机器 callable／默认值、re-export 组合通过。Darwin、GNU、musl 各覆盖 9 个不同 CLI 用例、11 variants、106 processes；最后的摘要简化另做本机 5 项定向复验。Linux 首次同步遗漏重叠测试文件，补齐后通过，失败记录保留。本机清理 incremental 4,233.4 MiB，详情见[验收记录](validation/compile-definitions.md)。

本批不代表按需文件读取已经完成：加载层仍读完整归档，Compile 的语义／布局重放和 ProgramLink 的对象证明仍待退役；dump 最小阶段、历史迁移、最终全量和五轮性能复测继续，M38 未启动。历史迁移独立目录续跑进程仍使用原 `target/release`；新 Compile 验证使用 `target/m48-summary-reader/release`，两套工具没有覆盖。

## 23. M48-3：Compile 依赖文件按需读取

提交：`862262a1`。

依赖加载层保留文件句柄、一次解析的 manifest 和按需复用的成员缓冲区。文件与内存输入共用偏移及成员头规则；删除整包读取、读取前后 metadata 比较和未使用成员扫描。Darwin 569 项、Linux 570 项 slib 以及各端 preflight、clippy、release 通过；Darwin／GNU／musl 各通过 20 个 CLI 用例、22 variants、91 processes。本机清理 incremental 2,842.3 MiB。GNU `scoopc build` 采样中，41,595,590 字节 core 只打开一次，读取 29,476,197 字节，恰好覆盖 manifest 和三个 metadata；4,208 个 ELF 对象未读。详细结果见[验收记录](validation/file-reader.md)。

本批不包含 ProgramLink／parent／dump 的旧整包路径与剩余重放；这些入口正继续迁移。历史源码隔离迁移与最终全量／性能验收尚未完成，M38 未启动。

## 24. M48-3：ProgramLink 共用文件输入

提交：`e393aad7`。

普通 build 后链接、显式 `scoop link` 和独立链接器均使用 `ArtifactInput` 文件入口；显式链接发现与消费共用 manifest 和成员缓冲区。构建结果仍先保存路径／摘要，需要链接时才打开文件。移除最后的 `ArtifactSnapshot` 调用及其模块，重复 Cone 候选采用已有 Artifact 指纹，不比较整包字节。修复公共 manifest 读取的 I/O 错误分类，显式链接输入错误保持 artifact origin 并报告实际 I/O；测试入口夹具统一 production／test 的实际源目录，保留零编译与产物路径断言。

本机 87 项 scoop、4 项 linker 和正式 ProgramLink 通过；Linux 90 项 scoop、17 项 linker（13 项首轮通过，缺配套 compiler 的 4 项指定当前 release 后补跑通过）及正式 ProgramLink 通过。Darwin／GNU／musl 各覆盖 9 个成功 CLI 用例、12 variants、125 processes。GNU 显式链接的三个 `.slib` 各打开一次，manifest 一次读取，成员范围无重叠；程序输出 `42`。本机另清理 incremental 4,020.7 MiB。失败环境记录、夹具修正和完整证据见[验收记录](validation/link-file.md)。

实际采样也表明，旧 Link replay 仍请求所有机器对象；本批不代表对象零读取或证明链已删除。后续仍需从已生成数据取得最终选中镜像的六类注册记录及摘要，退役对象／注册重放和 final／provisional 副本，并完成 Compile 语义／布局清理、dump 复用、历史迁移与最终全量／性能验收。M38 未启动。


## 25. 最终 image 使用已保存的运行时记录（8e31caf9）

`single-cone-production/7` 保存六组 canonical runtime records 与 typed definition 引用；最终 image 指纹直接按实际所选 definition 筛选这些记录，不再从完整对象证明结构取得输入。运行时 ABI 和指纹公式保持不变。新数据已接入实际链接消费，但其他 ProgramLink 消费方仍触发对象／注册／Code 重放，整条旧路径尚未退役。

本机 570 项、Linux 571 项 slib 测试、两端 clippy／release／实际 ProgramLink 读取通过。Darwin、GNU、musl 各通过 10 个 CLI 用例、11 variants、142 processes，包含 ODR 合并与冲突、初始化、GC、argv、对象复用和 M46 序列化。本机清理 inactive incremental 2,992.0 MiB。实现范围、固定格式期望更新及完整证据见[验收记录](validation/image-records.md)。

## 26. 正常 ProgramLink 直接消费成员与符号数据（61feb20c）

删除正常链接的对象、注册表、Code 与 coverage 重放链，直接读取保存的成员／typed definition、符号、ODR ABI 和 native requirements。`ArtifactFile` 的克隆共享同一文件入口；稳定对象存在时传路径，缺少输出时才读取对应归档成员。删除无生产用途的 terminal proof 分支，未改变格式与 runtime ABI。

本机 564 项、Linux 565 项 slib，两端对象存储、scoop、linker、真实 ProgramLink／泛型机器对象及 clippy／release 通过。Darwin／GNU 各 17 项 CLI、22 variants、245 processes；musl 16 项通过、1 项 C++ 声明不适用、20 variants、231 processes。GNU strace 证明三个产物各打开一次，manifest／metadata 无重复读取，4,458 个 ELF 成员读取量为零，真实程序输出 42；此结论不代表语义／布局重放已退役。本机清理 incremental 3,178.9 MiB。证据见[验收记录](validation/link-data.md)。

旧 single-cone API、Compile／ProgramLink 语义／布局链、dump、历史迁移和最终全量／五轮性能验收继续，M38 未启动。

## 27. 旧 single-cone Link 重放 API 退役（5e81e275）

删除无生产调用的 single-cone Link 逐级状态、对象／注册／Code 重建，以及 Link identity wire 的 materialization、digest、object、symbol 和 final object 重放比较。final 对象清零生成 provisional 副本的代码、错误类型及私有 helper 同步删除。19 项专用测试退役，1 项迁移为普通 wire 往返，归档和对象生产测试继续保留；manifest summary 使用正常产物，不再改写最终 image digest。

本机 545 项、Linux 546 项 slib 及两端 fmt／workspace clippy／release 通过；Darwin、GNU、musl 各 6 项 CLI、8 variants、93 processes 全部通过。本机另清理 incremental 1,316.7 MiB；最后的注释修正又通过 fmt／workspace clippy。证据及逐项退役清单见[验收记录](validation/old-link.md)。

其他遗留生产投影比对 API、只服务验证的持久字段、Compile／ProgramLink 的语义与布局重放、dump 复用、历史迁移和最终全量／五轮性能验收仍未完成。M48 保持实施中，M38 未启动。

## 28. production 与 Link identity 持久字段精简（43ee11a4）

`single-cone-production/8` 只保存 ODR 目录和 runtime image records；`link-identity-closure/16` 保存实际物理 member、Scoop typed definition 分配、defined／undefined symbols，删除 atom／patch／对象副本／image 与 entry owner 投影，以及未消费的 generated-C unit 列表。重复优化、native、Code／RuntimeImage、C bridge 投影和其比对 API 同步退役；生产 Code 输入与 runtime ABI 不变。

本机 542 项 slib、26 项 protocol、实际泛型产物和 17 项 CLI（22 variants、245 processes）通过；Linux fmt／clippy／release、543 项 slib、26 项 protocol 与实际泛型产物通过。GNU 17 项 CLI、22 variants、245 processes；musl 16 项通过、1 项 C++ 不适用、20 variants、231 processes。GNU I/O 采样确认三个产物各打开一次、无重叠读取，4,458 个 ELF 成员读取量为零；最大归档与读取量各减少 2,399,204 字节，实际程序输出 42。本机两次清理 incremental 共 5,959.4 MiB。完整记录见[验收文件](validation/link-fields.md)。其他两个重复 Link closure section、语义／布局重放、dump、历史迁移与最终全量／五轮性能验收仍待完成，M38 未启动。

## 29. 重复跨 Cone Link closure section 退役（d9ae49dc）

两个重复 section、coverage／对象副本、decoder 与发布重放接口已删除，Code 直接使用既有 ordinary／physical imports；47 个代码／规范文件净删 1,962 行。本机 fmt／clippy、528 项 slib、26 项 protocol、实际泛型／core 产物和 17 项 CLI（22 variants、245 processes）通过；Linux fmt／clippy／release、529 项 slib、26 项 protocol 与实际泛型／core 产物通过。GNU 17 项 CLI 通过（22 variants、245 processes）；musl 16 项通过、1 项 C++ 不适用（20 variants、231 processes）。GNU I/O 采样中三个产物各打开一次，manifest／metadata 无重叠读取，4,458 个 ELF 成员未读，实际程序输出 42；最大归档与读取量各减少 286,292 字节。清理本机旧 fixture 与 incremental 共 5,873,430,432 字节。[记录与证据](validation/closures.md)。其他语义／布局重放、writer 复核、冷 locator、dump 和最终全量／五轮性能验收继续，M38 未启动。

## 30. 冷显式产物入口复用（da7c2dec）

摘要缓存未命中后保留已打开的 ArtifactFile，与 locator、prepared 候选和 completed node 共用 BuildArtifact；命中已有摘要时继续延迟打开。修复首次 discovery 与后续真实链接重复打开同一文件、重读 manifest 的问题。本机 87 项、Linux 90 项 scoop 测试和两端 fmt／clippy／release 通过；三平台各 3 项 CLI（3 variants、33 processes）通过。GNU 父进程冷加载只打开 provider 一次，读取 26,444 字节，其中 manifest 1,814 字节仅一次，无重叠；后续全命中零打开、零读取、零 compiler child，实际程序退出码均为 50。[记录](validation/cold-entry.md)。本机清理 incremental 790,932,155 字节；Linux 已结束的历史工作目录清理约 116.37 GiB。语义／布局重放、writer 复核、dump 和最终全量／五轮性能验收继续，M38 未启动。


## 31. ProgramLink 直接读取 production 与 native ABI 数据（74c1b42d）

ProgramLink 从已保存字段取得成员选择、image／入口／trap 符号、六类注册与 native 标量／指针表示，删除整份 strong production、digest、注册、布局、descriptor／dispatch、shape support 与 physical import 重放。普通 param-free bridge 不再解码，完整 LIR foundation 不再长期保留；无持久格式或 runtime ABI 变化。46 个代码／spec 文件净删 1,114 行。

本机 fmt／workspace clippy、528 项 slib、6 项入口、4 项 linker、真实产物 ProgramLink 及 release 通过；Linux 对应 529 项 slib、6 项入口、17 项 linker、真实产物和构建通过。Darwin／GNU 各 24 项 CLI 通过（29 variants、270 processes）；musl 23 项通过、1 项 C++ 不适用（27 variants、256 processes）。GNU 采样证明三个产物各打开一次、无重复读取，4,458 个机器对象成员未读、compiler child 为零，实际输出 `42`。本机／Linux 分别清理 incremental 4,950,100,741／13,139,123,342 字节。[范围、退役测试和证据](validation/link-inputs.md)。

ProgramLink identity／foundation 边界、Compile 重放、writer 其余复核、dump 复用和最小阶段、最终全量与五轮性能验收仍待完成。此次定向结果不替代完整 M48 验收，M38 未启动。

## 32. 外部物理定义选择复用已有导出（aa5c2cd0）

ShapeLinkProvider 直接使用导出记录、已有 definition symbol plan 和初始化／静态存储数据，不再要求整份 strong production，也不逐次重建符号、扫描 atom 集合或反查 type／callable 注册表重做布局、scan、继承及 dispatch slot 比对。20 个代码／spec 文件净删 203 行，移除 177 行重复类型检查模块；未修改 wire、profile 或 runtime ABI。

本机与 Linux 的 fmt／workspace clippy、17 项 shape-link、7 项 Link closure、真实泛型产物和 release 构建均通过。Darwin、GNU、musl 各通过 16 项 CLI、18 variants、199 processes，覆盖泛型／ODR、继承／接口／装箱、属性／初始化、native、runtime image、M46 序列化及对象文件复用。清理本机上一批已完成的 CLI 工作目录，释放 2,152,275,968 字节；本批测试目录和增量结果保留。[范围与证据](validation/provider-data.md)。

Compile reader 当前仍先重放 strong production，再传入三个必要字段；该重放与其他语义／布局重建尚待退役。ProgramLink identity／foundation 边界、writer 复核、dump 复用和最小阶段、最终全量及五轮性能验收继续，M38 未启动。


## 33. Compile production 直接读取与集中清理

Compile 只读取 definition symbol plan、静态存储／初始化语义及必要定义引用；完整 V2 production／registration、digest 与 LIR 依赖闭包重放及其独占 API 一并退役。23 个显式测试函数删除或替换，普通功能和 negative fixtures 保留。122 个代码／规范文件净删 3,222 行。

本机 fmt／workspace clippy、454 项 LIR、527 项 slib、release 构建及真实泛型产物通过。属性初始化长测试在调度调整时中断，未计通过；本批 CLI 与 Linux 验收未执行。清理已结束 fixture 目录约 7.09 GB，保留增量缓存。[范围、退役清单与已有验证](validation/compile-data.md)。

按用户最新要求调整后续工作：先在本机完成 M48 所有剩余修改和清理，迭代期间仅做格式化、lint 和受影响的针对性测试；整体完成后先做本机完整复验，通过后统一进行 Linux GNU／musl 验证，不再每轮到 Linux 测试。其余语义／布局重放、writer、dump 及最终全量／五轮性能验收仍未完成，M38 未推进。


## 34. Compile 来源对照与依赖闭包集中退役

删除普通 callable bridge 的 HIR／MIR 重建、MIR→LIR ABI 对照、MIR/LIR dependency closure 重算，并集中退役 HIR→MIR 类型、callable、constructor、object、dispatch、coroutine、equality 对照及专用修改工厂。初始化单元改为读取保存的 strong callable 调用对；typed 引用、格式／顺序及实际签名关系检查继续保留。真实源码和普通 negative fixtures 保留。109 个代码／规范文件净删 7,341 行。

本机 fmt／workspace clippy、452 项 LIR、19 项 MIR section、527 项 slib、真实初始化组合和 core 产物通过；普通跨库 function／getter 在前半批通过。清理 10 个已结束 fixture 目录，释放 9.43 GB，保留近期编译缓存。[范围、原始验证与清理记录](validation/compile-relations.md)。

本批未执行 Linux、整仓全量或性能验收。先在本机完成剩余布局／物理、底层 reader、writer、dump 等全部修改，再本机完整复验，随后统一 Linux 验收；M38 未推进。


## 35. callable ABI 与 dispatch 保存记录读取

callable ABI reader 不再调用生产构造器派生 body／definition 或重编码比较；dispatch 直接读取 table、slot、implementation 与 ABI 引用，接收者适配借用已有 layout。集中删除 MIR dispatch 重建、ABI 查找对照、候选修改工厂及无消费者的 API。44 个代码／规范文件净删 396 行，格式与 runtime ABI 不变。

本机最终 fmt／workspace clippy、452 项 LIR、527 项 slib、真实分派 standalone／combined 和泛型 ODR 产物通过；前半批另有 ZST／间接返回／装箱 ABI 组合通过。[范围、退役清单与验证日志](validation/callable-dispatch.md)。target 没有新增已结束 fixture 目录，保留近期增量缓存。

继续先完成本机全部修改及清理，再本机完整复验，通过后统一 Linux 验收。本批未运行 Linux 或全量，M48 仍在实施，M38 未推进。


## 36. param-free shape-support 导出直接读取

读取保存的八种 role 与 typed 引用，退役 MIR／源码 root 闭包重建、单条／整表重编码对照、测试专用重复表构造器和候选修改工厂。保留实际 source、box、coroutine helper 生产规则与组合用例；25 个代码／规范文件净删 223 行。

本机 fmt／workspace clippy、452 项 LIR、527 项 slib 和真实 shape-support standalone／combined 产物通过。最后清理无调用适配函数后仅补跑 6 项直接受影响测试，未重复整套组合。[范围、退役清单与日志](validation/shape-reader.md)。target 无新增已结束 fixture 目录，增量缓存保留。

本轮未到 Linux。剩余 layout／descriptor／physical、HIR／strong-profile、writer、ProgramLink 边界和 dump 工作仍先在本机完成，再本机完整复验及统一 Linux 验收；M38 未推进。


## 37. layout、descriptor、physical 与闲置复核集中清理（实施中）

合并完成保存布局／descriptor reader、MIR 布局与 descriptor 重建退役、物理合同副本与整套深比较退役，并顺带删除闲置 C bridge／Link 比较 API、writer 对象 fingerprint 复核和三层 foundation 全量重编码比较。代码／规范 147 文件净删 4,228 行。physical import section 升为 16，Code 合同输入及 runtime ABI 保持。改动仍在工作区，尚未提交或宣称功能完成。

整仓 fmt／clippy 通过。遵照用户进一步调整，本轮未运行任何功能测试或 Linux 验证，全部功能验收留到剩余本机清理完成后统一执行。`target/tmp` 为空，增量缓存保留。[范围、退役清单及 lint 日志](validation/layout-descriptor-cleanup.md)。

继续 HIR／strong-profile、ProgramLink 边界、dump 等剩余清理，再做本机完整复验、统一 Linux GNU／musl 和五轮性能验收；M38 未推进。


## 38. 旧 Compile、HIR 消费重放与 dump 集中闭合

在第 37 节基础上继续集中清理旧 single／cross-cone Compile 流水线、native ABI replay、LIR 完整 production reader、HIR 声明与类型继承重放，以及仅供这些路径使用的测试工厂和错误分支。实际源码生产、typed 引用与必要格式／ABI／GC 职责保留。dump 采用按输入键缓存和 observation-only child，只补所需阶段；协议升至 11，机器产物与最终 executable 可继续复用。

相对于 `9edf9060`，代码／测试／规范已跟踪部分累计净删 40,660 行，移除 253 个文件；新建文件另列于工作区。整仓 fmt／clippy 无警告，Ruff 与 49 项 runner 规则测试通过。本机完整 Rust 验收 5,314 项通过；正式 CLI 4,089 项适用用例通过、18 项平台不适用。本机五轮性能复测已完成，正在统一 Linux；尚未提交本批或宣称 M48 完成。范围、名称级退役清单及后续统一验收见[集中清理记录](validation/consolidated-cleanup.md)。

同机性能对照补充：历史基线文档标注 M3 Ultra，本次主机实际为 M4 Pro，因此另在独立的 `6571bb4a` checkout 上构建原版本并复用同一测量脚本执行 55 次命令。当前实现与旧版本各 55 次测量全部完成；未改动当前工作区的已验收二进制。同机空程序暖构建中位数 3.175→0.059 秒、多 Cone 应用冷构建 65.476→45.001 秒、暖构建 10.778→0.059 秒。全部原始结果及统计见[性能记录](PERFORMANCE.md)，临时 worktree 随证据保存后归档。


## 39. 按 MORE-CLEANUP 集中收缩重复验收

集中移除 488 次普通语言显式重复链接、34 次无针对性的 core consumer、2 次 program-read 无用首次构建；普通输出、诊断、多 Cone 依赖和 moving-GC 运行保留。Rust core-layout 全部 typed 断言保留，36 次对象／打包缩为 3 个代表，完整双视图读取集中到 base。6 个 core 消费者改为实际 ABI、descriptor 和初始化调用。842 个相同辅助 core 输入及 172 个相同修改版 core 输入在本轮测试中共享实际源码路径，执行正常 build／缓存复用。

同步移除对象验证链的重复整包 SHA-256、回填后清零比较和对象结构重解析，保留首次格式／引用／GC 与实际 digest patch 检查。退役旧 single／cross-cone strong profile 的正向 descriptor／fingerprint 矩阵及专属工厂，当前 profile 的版本与 inventory 仍覆盖；旧 profile 明确拒绝。旧 M12 诊断文字更新为当前规则描述。

本机 fmt、workspace Clippy、Ruff 0.16.10、50 项 runner 规则及全部 4,107 份声明发现通过；统一 Rust 验收 5,302 项通过：完整首轮后，仅补验两条同步旧期望的测试；正式 CLI 4,089 项通过、18 项不适用，只补验三个修正源码的 consumer。完整 CLI 首轮墙钟 28.1 分钟；有效用例累计耗时比上一批减少 48.1%，进程调用减少 684，保留 4,613 variants 与 88 项 golden。最终代码五轮性能测量已完成，共 55 次构建；多 Cone 应用冷构建中位数 43.096 秒，暖构建 0.059 秒，native 头文件更新 0.680 秒。已清理本轮结束的 38.3 GiB fixture 工作目录，保留近期编译缓存；本机全部稳定后统一 Linux。Linux 旧 CLI 在 825 项通过、0 失败时停止，另有 29 不适用、3,253 中断，未启动 musl。原完整 Rust／Darwin CLI 与性能结果保留为上批证据。本轮遵从最新验证规则：删除用例不触发重跑，修改用例只复验受影响项；包含编译器生产变更的本批完成后统一验收。详见[进一步清理记录](validation/more-cleanup.md)。


## 40. 最终验收与原始材料移出仓库

MORE-CLEANUP 全部修改在本机完成并完整复验后，再统一执行 Linux：fmt、Clippy、release、Rust 5,338 项及 runner 50 项一次通过；GNU 正式 CLI 4,078 项通过、29 项不适用，musl 4,074 项通过、33 项不适用，两套均零失败、无需补验。GNU／musl 完整墙钟分别为 35.1／35.1 分钟。本机最终结果仍为 Rust 5,302、CLI 4,089 通过及 18 不适用，补验只涉及两条旧期望和三个新增 consumer。

最终 GNU 55 次、musl 5 次实际调用观察通过。全部 27 次暖构建只启动 scoop 自身，无编译、预处理、链接、受检内容读取或 rename。真实下游依赖一次打开，4,207 个机器对象成员未读；详细计数见[最终 I/O 概要](validation/final-io.md)。同机性能结论见[最终性能](PERFORMANCE.md)。

原始日志、trace、逐次报告和一次性脚本共 890 份、约 33 MiB 移到仓库外；validation 仅保留 Markdown 结果概要。fixture／Rust 迁移与保留文件操作整理为普通 TSV 清单。此处文档与日志整理不修改编译器、runtime、sysroot 或用例，不触发功能测试重跑。已结束的两机 fixture 工作目录均清理，保留仍复用的增量编译缓存。本批完成 M48 最终闭合；M38 实现尚未推进。
