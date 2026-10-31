# `.slib` 内容精简与逐成员压缩

本次改动减少大库产物的磁盘占用与读写量，同时控制压缩 CPU 成本。先缩减没有下游消费者的持久化数据，再在 ar 容器内压缩 Scoop 自用成员。格式合同以 [实现规范 2.6](../specs/SCOOP-IMPL-SPEC.md) 为准。

2026-11-01 的后续结构精简已统一三层 metadata 的成员内 ID 表，并将 ODR 目录移入 LIR。当前容器／manifest 为 4、profile 为 `cross-cone-generic/12`、三层 metadata outer schema 均为 4；成员目录只保存 ID、角色、存储长度和摘要。生产端仍用完整 key 生成稳定 ID；Native 源路径保存在对应角色中，对象目标与格式复用 manifest 的契约。逐轮固定输入的测量与验证见 [编译器优化报告](../compiler-performance/PROGRESS.md)。下文保留 2026-10-31 各轮的历史记录。

2026-10-31 的后续实施扩展到逐对象压缩、LIR 成员内 ID 表以及发布前结束边界符号清理。基线为 25 个当前格式 Darwin 库、772.16 MiB；其中对象 378.29 MiB、LIR 287.60 MiB。离线测量中逐对象 Zstd level 3 为 165.46 MiB，LIR ID 索引投影为 123.80 MiB，合计估算约 395.53 MiB。该估算包含 ID 表，不是正式 writer 的验收结果。

本次容器／manifest 升至 3，profile 升至 `cross-cone-generic/7`，LIR outer schema 升至 4。身份计算和 runtime ABI 保持；旧产物正常重建，不增加兼容分支。LIR 在已有 primitive wire reader 上直接解析 ID 索引，未知扩展保持 opaque。结束边界只在生产完成后从原生符号表和 Link 导出目录移除，保留实际被引用的起始边界；原生重定位和调试数据继续按原对象格式处理。具体 wire 字段与消费职责见实现规范 2.6。

## 2026-10-31 实测

Darwin/AArch64 的 27 个当前系统库和四种 runtime 配置已用配套工具重新准备。对原开发 sysroot 中同名的 25 个库进行比较：

| 内容 | 原产物 MiB | 重建后 MiB |
| --- | ---: | ---: |
| 完整 `.slib` | 772.16 | 386.08 |
| 原生对象成员 | 378.29 | 158.09 |
| LIR 成员 | 287.60 | 120.59 |
| `scoop.core` 完整产物 | 39.47 | 19.17 |

总尺寸减少约 50.0%。比较排除过期的 `scoop.llm.client` 和原基线没有的两个 window 库；原产物与当前源码并非完全固定输入的 A/B，对象数由 216,011 变为 217,359。重建的这 25 个库中，592,695 个起始边界仍存在，结束边界为零。生产时将已生成的原生 bytes 直接交给共享对象缓存，避免为缓存写入重新解压刚压缩的成员；读取产物时仍按需解压。

## 2026-10-31 验证

`cargo fmt --all`、`cargo clippy --workspace --all-targets` 和配套 release 命令构建通过。定向结果：

- `scoop-wire` 20 项、`scoop-slib` 257 项通过，包含 ID 表共享／非法索引、opaque 扩展、对象压缩、内存／文件 reader 及缓冲区复用。
- 两项原生符号清理测试通过：覆盖 AArch64／x86-64 Mach-O、ELF64 的 COMDAT 和地址索引表、COFF／BigObj，比较节内容、符号、重定位和组归属；结束边界被引用时拒绝清理。最终 Darwin executable 的符号可见性测试通过，并确认无结束边界。
- 5 个 generic-reader fixture 和 direct-c-artifacts 的 debug／release 组合全部通过，共 6 个 fixture、7 种组合、52 次进程调用。后者使用独立空对象缓存，实际覆盖压缩对象提取、C ABI、调试信息、重新链接与运行。

实际运行目标为 Darwin/AArch64；ELF／COFF 在本机发射对象并检查结构与引用，未声明 Linux／Windows 原生运行验收。全量 fixture 已按用户要求停止。此前扩大检查发现的 native target-include 路径归一化断言，在未改动的 `976e075e2` 独立工作区同样失败，未修改该无关模块。原始日志和逐项报告保存在仓库外。

以下保留首次实施的调研、格式及验证记录；当前格式以本文开头和实现规范 2.6 为准。

## 首次实施的调研依据

2026-10-22 在 Apple M3 Ultra 上检查 Darwin sysroot 的 19 个 `.slib`，总计 1205.36 MiB。其中 HIR 187.67 MiB、MIR 28.71 MiB、LIR 668.09 MiB、manifest 51.25 MiB，机器对象约 262.33 MiB。三层 metadata 均有真实消费者，主要冗余在 LIR 生产计划及链接记录的重复字段。

对 57 个实际 metadata 成员逐项进行单线程内存测量，输入合计 884.47 MiB。下表是整个样本的压缩大小和耗时，不包含磁盘 I/O；本机结果用于选型，不作为验收门禁。

| 编码 | 压缩后 MiB | 压缩秒 | 解压秒 |
| --- | ---: | ---: | ---: |
| lz4_flex 0.14 block | 519.85 | 0.713 | 0.132 |
| 原生 LZ4 1.10 block | 518.73 | 0.562 | 0.075 |
| Zstandard 1.5.7 fast 3 | 468.21 | 0.490 | 0.196 |
| Zstandard 1.5.7 level 1 | 431.15 | 0.929 | 0.305 |
| Zstandard 1.5.7 level 3 | 365.29 | 1.895 | 0.360 |

首次实现选择 Zstandard fast 3，即 API level `-3`；2026-10-29 的补充测量后，统一改用标准 level `3`，结果见[后续体积调查](FOLLOW_UP.md)。使用维护中的 Rust [zstd crate 0.14.3](https://crates.io/crates/zstd/0.14.3)，底层为 [Zstandard 1.5.7](https://github.com/facebook/zstd/releases/tag/v1.5.7)，关闭默认可选功能；不引入第二套编码、字典训练或压缩线程池。

## 首次实施的内容精简

生产阶段仍保留生成对象、计算指纹所需的完整内存数据，只缩减跨产物传递的内容。

| 位置 | 保留的持久化内容 | 去掉的副本 |
| --- | --- | --- |
| cone production | canonical definitions、registrations、image、entry | object definition plans、digest finalization、shape/bridge production plans、重复 callable/shape ABI |
| registration production | identities、static storages、initialization units | safepoint/callable/type/immortal-object/runtime-scan 生产计划 |
| image plan | image symbol、链接使用的两个数组错误消息 atom | cone/dependencies/tables/definition plan 与其余生产 support atoms |
| undefined symbol uses | 排序去重的 `(member, symbol)` | relocation offset/type/target 与 requirement 副本 |
| defined boundary symbols | member、atom、Start/End | 可由目标符号规则恢复的边界名称 |

前四组的离线重编码估算可减少 183.14 MiB，边界名称另有约 64.99 MiB 重复数据。普通外部符号名称不能一概由 owner 推导，继续保存真实名称。对象中实际的 runtime、GC、重定位和边界符号都保留。

## 首次实施的容器与读取

- 容器 schema 和 manifest schema 升至 2；profile 升至 `cross-cone-generic/6`，production 升至 `cone-production/14`，链接记录升至 `link-identity-closure/18`，共用生产投影的 `strong-production` section 升至 24。未改变编码的 identity、mangling、HIR/MIR outer schema 与 runtime ABI 保持原版本。
- ar 第一项为 `manifest.zst`。Manifest、HIR、MIR、LIR 和 DiagnosticAttachment 分别编码为独立 Zstandard frame，包含原始长度；LinkObject 和 ExtensionBlob 仍保存原始 bytes。
- 成员目录与 ar size 使用实际存储长度，成员 hash 覆盖存储 bytes。三层语义指纹仍在压缩前由已有语义投影产生；Artifact 指纹自然反映新存储。
- 依赖发现仅读取、解压 manifest。目录按压缩后的长度定位；请求成员时才读取、解压，随后复用结果。内存与文件 reader 遵循同一规则，释放无需保留的压缩临时缓冲区。
- 解码保留单 frame、长度、截断等普通格式检查，不增加摘要复核、生产语义重放、通用预算或缓存内容审计。普通摘要缓存使用 `artifact-summaries-v2`，避免复用旧格式发现结果。
- 旧容器、旧 manifest 和旧 profile 直接拒绝，不提供探测回退或兼容解码；已有 `.slib` 需要重建。依赖是宿主编译器的 Cargo 库，不复用 sysroot 的目标平台 Zstandard 对象。

## 首次实施的验证范围

按本次任务要求，以最短时间确认正常编译、产物消费与链接：复用必要的现有序列化用例，抽查 Darwin、Linux GNU 和 Linux musl 的少量现有编译链接 fixture。完成格式化与相关 lint，不跑完整套件，不增加压缩开关、压缩率/大小门禁、性能验收矩阵或重复 roundtrip 测试。

实际复用以下现有用例：

- `archive::tests::reader_validates_manifest_directory_and_member_ranges`：内存归档读写与成员消费。
- `artifact_file::tests::complete_file_reads_only_requested_members_and_reuses_their_buffers`：文件与内存读取一致、按需读取、缓冲区复用。
- `link_object::link_identity_closure::wire::tests::closure_wire_round_trips_saved_fields`：精简链接记录的序列化。
- `m33-equality-collision-artifacts`：debug/release 库产物、泛型消费、独立链接与执行及既有诊断。
- `m48-final-tools`：源码更新、缓存复用、显式 `scoop link`、独立 `scoop-link` 和执行。

三个现有单元测试均通过，`cargo fmt --all` 和 `cargo clippy --workspace --all-targets` 均通过；最终测试输入调整后，相关 crate 的 lint 也通过。平台抽查如下：

| 目标 | 已通过 fixture | 变体 | 结果 |
| --- | ---: | ---: | --- |
| aarch64-apple-darwin | 2 | 3 | 通过 |
| x86_64-unknown-linux-gnu | 2 | 3 | 通过 |
| x86_64-unknown-linux-musl | 2 | 3 | 通过 |

每个平台通过的两个 fixture 共执行 25 次进程调用。Linux 首轮遇到预置 runtime 索引与当前工具不匹配；使用 `m48-final-tools` 已从当前源码生成的 runtime 后，仅重跑受影响的库消费 fixture，GNU 与 musl 均通过。未更改 runtime ABI 或兼容性检查。

此次未新增 test case 或 fixture；同步调整旧格式的固定 profile 编码与 manifest 错误输入，并删除失去被测 reader 的测试。

与调研时本机已有的 Darwin release core 相比，本次实际归档大小从 76,535,670 bytes（72.99 MiB）降至 42,385,522 bytes（40.42 MiB），减少 44.62%。这一观察只反映产物大小，不作为验收门禁；本次未进行全库或真实磁盘吞吐测量。

为保持验证最短，仅在仓库外准备各目标所需的新格式 core，复用现有 unwind；Darwin 使用现有 runtime，Linux 使用本次构建的 runtime。默认开发 sysroot 和其他旧 `.slib` 没有批量重建；使用新工具消费它们前，按 [sysroot 准备说明](../../scripts/README.md) 显式重建。原始日志和临时产物均保存在仓库外。
