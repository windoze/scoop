# `.slib` 内容精简与逐成员压缩

本次改动减少大库产物的磁盘占用与读写量，同时控制压缩 CPU 成本。先缩减没有下游消费者的持久化数据，再在 ar 容器内压缩 Scoop 自用成员。格式合同以 [实现规范 2.6](../specs/SCOOP-IMPL-SPEC.md) 为准。

## 调研依据

2026-10-22 在 Apple M3 Ultra 上检查 Darwin sysroot 的 19 个 `.slib`，总计 1205.36 MiB。其中 HIR 187.67 MiB、MIR 28.71 MiB、LIR 668.09 MiB、manifest 51.25 MiB，机器对象约 262.33 MiB。三层 metadata 均有真实消费者，主要冗余在 LIR 生产计划及链接记录的重复字段。

对 57 个实际 metadata 成员逐项进行单线程内存测量，输入合计 884.47 MiB。下表是整个样本的压缩大小和耗时，不包含磁盘 I/O；本机结果用于选型，不作为验收门禁。

| 编码 | 压缩后 MiB | 压缩秒 | 解压秒 |
| --- | ---: | ---: | ---: |
| lz4_flex 0.14 block | 519.85 | 0.713 | 0.132 |
| 原生 LZ4 1.10 block | 518.73 | 0.562 | 0.075 |
| Zstandard 1.5.7 fast 3 | 468.21 | 0.490 | 0.196 |
| Zstandard 1.5.7 level 1 | 431.15 | 0.929 | 0.305 |
| Zstandard 1.5.7 level 3 | 365.29 | 1.895 | 0.360 |

选择 Zstandard fast 3，即 API level `-3`。这一档在样本上兼顾体积和压缩成本；LZ4 解压更快，但文件更大。使用维护中的 Rust [zstd crate 0.14.3](https://crates.io/crates/zstd/0.14.3)，底层为 [Zstandard 1.5.7](https://github.com/facebook/zstd/releases/tag/v1.5.7)，关闭默认可选功能；不引入第二套编码、字典训练或压缩线程池。

## 内容精简

生产阶段仍保留生成对象、计算指纹所需的完整内存数据，只缩减跨产物传递的内容。

| 位置 | 保留的持久化内容 | 去掉的副本 |
| --- | --- | --- |
| cone production | canonical definitions、registrations、image、entry | object definition plans、digest finalization、shape/bridge production plans、重复 callable/shape ABI |
| registration production | identities、static storages、initialization units | safepoint/callable/type/immortal-object/runtime-scan 生产计划 |
| image plan | image symbol、链接使用的两个数组错误消息 atom | cone/dependencies/tables/definition plan 与其余生产 support atoms |
| undefined symbol uses | 排序去重的 `(member, symbol)` | relocation offset/type/target 与 requirement 副本 |
| defined boundary symbols | member、atom、Start/End | 可由目标符号规则恢复的边界名称 |

前四组的离线重编码估算可减少 183.14 MiB，边界名称另有约 64.99 MiB 重复数据。普通外部符号名称不能一概由 owner 推导，继续保存真实名称。对象中实际的 runtime、GC、重定位和边界符号都保留。

## 容器与读取

- 容器 schema 和 manifest schema 升至 2；profile 升至 `cross-cone-generic/6`，production 升至 `cone-production/14`，链接记录升至 `link-identity-closure/18`，共用生产投影的 `strong-production` section 升至 24。未改变编码的 identity、mangling、HIR/MIR outer schema 与 runtime ABI 保持原版本。
- ar 第一项为 `manifest.zst`。Manifest、HIR、MIR、LIR 和 DiagnosticAttachment 分别编码为独立 Zstandard frame，包含原始长度；LinkObject 和 ExtensionBlob 仍保存原始 bytes。
- 成员目录与 ar size 使用实际存储长度，成员 hash 覆盖存储 bytes。三层语义指纹仍在压缩前由已有语义投影产生；Artifact 指纹自然反映新存储。
- 依赖发现仅读取、解压 manifest。目录按压缩后的长度定位；请求成员时才读取、解压，随后复用结果。内存与文件 reader 遵循同一规则，释放无需保留的压缩临时缓冲区。
- 解码保留单 frame、长度、截断等普通格式检查，不增加摘要复核、生产语义重放、通用预算或缓存内容审计。普通摘要缓存使用 `artifact-summaries-v2`，避免复用旧格式发现结果。
- 旧容器、旧 manifest 和旧 profile 直接拒绝，不提供探测回退或兼容解码；已有 `.slib` 需要重建。依赖是宿主编译器的 Cargo 库，不复用 sysroot 的目标平台 Zstandard 对象。

## 验证范围

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
