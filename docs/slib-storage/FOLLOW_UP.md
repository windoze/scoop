# 调试实施期间的 `.slib` 体积调查

日期：2026-10-29。按本轮任务要求，直接实施收益明确、成本低的改动；较大格式／管线改动只记录依据，不阻塞基础调试、Throwable 栈快照或发行集成。

## 当前占用

只读分析本机已有的 25 个 Darwin sysroot `.slib`，没有为测量重建系统库。总大小 849.83 MiB；这是当前样本，不与 2026-10-22 的 19 库样本作直接增减比较。

| 内容 | 实际存储 MiB |
| --- | ---: |
| HIR metadata | 56.04 |
| MIR metadata | 20.33 |
| LIR metadata | 358.13 |
| manifest | 39.12 |
| 机器对象 | 363.89 |
| ar header 与 padding | 12.32 |

三层 metadata 保存各自的跨 Cone 接口、身份、布局和链接记录，不是完整 HIR／MIR／LIR 执行正文的三份快照。当前主要占用是 LIR 和机器对象，新增源码信息应避免复制，但仅移动源码信息不能解决全部体积问题。

## 本轮低成本改动：标准 Zstandard level 3

保持现有逐成员 Zstandard 格式，把统一压缩级别从 `-3` 改为 `3`。不增加压缩配置、字典、线程池或新的 reader；原有 `-3` frame 仍可正常读取，schema 与语义编码不变。新产物的存储 bytes、成员长度和 Artifact 指纹由现有 writer 正常生成；输入未变化的预编译库继续复用。

对上述 25 个库的 100 个已压缩成员做单次离线内存重编码，输入原始 metadata 合计 1010.58 MiB。先以 `-3` 重编码并确认与现有成员一致，再以 `3` 编码并确认解压结果一致。以下时间只含压缩／解压调用，不含磁盘 I/O、CBOR 解码或编译，不作为性能门禁。

| 级别 | 压缩后 MiB | 压缩秒 | 解压秒 |
| --- | ---: | ---: | ---: |
| `-3` | 473.62 | 0.534 | 0.255 |
| `3` | 366.80 | 1.728 | 0.375 |

现有成员离线合计减少 106.82 MiB，约占整个样本的 12.6%；这不是重新构建完整归档后的实测大小，更新后的 manifest 长度可能略有变化。一次性压缩成本增加约 1.2 秒，现有热缓存路径不执行压缩。

`cargo fmt --all` 与 `cargo clippy --workspace --all-targets` 已通过。现有 `archive::tests::reader_validates_manifest_directory_and_member_ranges` 和 `artifact_file::tests::complete_file_reads_only_requested_members_and_reuses_their_buffers` 均通过，覆盖实际 writer／reader、目录定位和成员缓冲复用。后续 Darwin、GNU、musl 的定向编译／运行继续通过新 writer 生产并消费产物，未新增压缩率断言或运行无关完整测试矩阵。体积测量仍限于上述 Darwin 现有成员的离线重编码；原始逐项测量和日志留在仓库外。

## 来源共享的可行性

文件身份、位置、源码作用域与变量声明可以集中保存，各阶段保留局部引用和实际变换映射；基础调试的新数据已通过公共 `scoop-source` 的共享记录落地。定义来源与求值来源继续保留各自语义，引用的记录相同时共享。源码文件身份已有 HIR `SourceRecord`，不把其完整内容再编码进 MIR 和 LIR。跨 Cone 必需的局部名称进入原 HIR 模板表，MIR／LIR 没有新增持久调试快照。

旧 HIR 模板有实际重复：`cross-cone-interface` 的 `definition_sources` 已保存来源集合，表达式等节点仍直接编码来源。其中 core 的 7,423 个来源出现 28,877 次。将集合外的相同来源离线替换为表索引、仍保留集合后：

| 样本 HIR 成员 | 当前 `-3` bytes | 索引投影后 `-3` bytes | 减少 |
| --- | ---: | ---: | ---: |
| scoop.core | 2,197,985 | 2,109,125 | 4.04% |
| scoop.http.client | 11,455,293 | 11,359,456 | 0.84% |

这项投影保留来源语义，但尚未实现对应 reader，也未覆盖所有来源表；它只用于评估已有压缩下的收益。完整迁移会涉及模板 writer／reader、跨 Cone remap 和相应 section 版本，收益明显小于直接调整压缩级别。本轮先让新增调试记录共享，旧来源 wire 的全面索引化留待后续。

## 后续候选

| 方法 | 当前依据与后续工作 |
| --- | --- |
| HIR 来源／声明集中编码 | 有上述重复及离线估算；需保留默认参数、泛型实例化、内联与诊断的真实来源，不能用源码重读恢复。优先按实际引用使用已有 typed identity，只为存储建立局部索引。 |
| LIR／manifest 的重复身份和符号记录使用局部索引 | 大型样本的 LIR 主要在 identity foundation、cone production、link identity closure；需要按真实消费者梳理重复字段，量出重编码收益后再改对应格式，不把引用校验扩展成新的身份体系。 |
| 减少对象与 DWARF 的重复 | 对象当前占 363.89 MiB，full DWARF 后还需按真实对象测量。对象粒度、类型合并或 split DWARF 涉及 ODR 选择、原生链接、Darwin dsymutil 及 Linux GNU／musl，应作为独立改动验证。 |
| `.slib` 外部调试 payload | 仅搬出数据不减少总体积，还会改变当前 self-contained 合同及依赖发布／定位流程。本轮保持 `.slib` 内对象的标准 DWARF，最终程序用 dSYM／`.debug`，不增加外部模板 payload。 |

运行期 Throwable／trap 栈诊断及 LLDB 最终消费原生展开信息和标准调试文件，不依赖编译器会话中的来源旁表。源码共享设计不会给程序增加来源表或 managed 分配。

## Throwable 快照存储的新增成本与后续共享

首次抛出准备使用真实 `Array<UInt8>` 描述符，当前在 HIR 收口前准备其类型，使导入泛型正文完整。2026-10-29 的最小跨库消费者因此多出 7 个 core 泛型支持函数，其 Mach-O 原生对象合计 24,930 bytes；这只是函数对象的增量，不含描述符与 metadata，也不是完整 `.slib` 增量。已有相同实例的库复用其身份，最终链接沿普通 ODR 选择合并。组合测试按声明的实际 Cone 归属分别核对 provider 与 core，并继续比较共享实例的对象、ABI 和注册内容。

可以由 core 统一物化字节数组描述符，再通过 typed runtime 数据别名引用；现有 String alias 只接受 Strong owner，而 `Array<UInt8>` 是 ODR owner。实施需要同时修改别名编码、ODR 选择、可达性与三目标最终链接，不能仅按名字导出一个指针或伪造数组 TD。本轮将其列为中等成本后续项，保留普通泛型与 GC 合同。另一候选是仅对实际抛出闭包准备该类型，但要覆盖编译器生成异常和协程转换，不应增加重复全量分析。
