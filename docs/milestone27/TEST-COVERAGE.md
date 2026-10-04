# M27 验收期间的重复测试清理

本次检查范围是 `scoopc` 的真实源码 producer／reader 集成测试，起因是完整
workspace 验收中的两个 core 测试耗时较长。以实际调用和断言为依据，不仅按
fixture 名称或历史迁移表判断覆盖。正式验收继续运行完整 CLI fixture。

## 已删除的重复入口

| 原 Rust 测试 | 现有 fixture | 覆盖依据 |
| --- | --- | --- |
| `ordinary_library_exports_members_with_available_machine_signatures` | `core-layouts-ordinary` | 使用原 `ordinary.scoop`；完整四阶段输出保留 `Published.ready`／`deferred`、空值参数／结果 ABI 和公开布局；真实 library 发布执行相同 producer。 |
| `heap_zst_fields_produce_valid_layout_objects_and_shared_metadata` | `core-layouts-heap-zst-storage`、`heap-zst-standalone`／`combined` | 前者使用原 core 扩展和八份阶段 golden，并在移走 core 源码后构建消费者；后两者额外覆盖堆对象空值字段的跨 Cone 调用、独立链接及普通／移动 GC 运行。旧入口没有独有的损坏输入分支，公共 reader 拒绝检查仍在 `base` 及其他专用用例执行。 |
| `shared_strong_reader_replays_complete_sections_from_actual_artifact_bytes` | `core-layouts-shared-production-standalone`／`combined` | 原两份源码的 core 产物和下游四阶段输出均保留；该入口仅重复公共 producer／reader 检查。相同 section、Strong、依赖和物理导入重放仍由 `base` 及专用拒绝用例验证。私有声明由完整阶段输出及产物指纹锁定。 |
| `formal_pipeline_calls_a_direct_dependency_through_the_cross_cone_artifact_closure` | `cli-process-source-observations`、`core-dependencies-ordinary` | 原入口仅构建 provider／consumer 并断言直接依赖集合；fixture 检查真实依赖身份、编译顺序和完整输出，移走源码后消费 `.slib`，独立链接并执行调用。 |

`actual_core_sources_produce_closed_mir_and_lir_export_tables` 只保留 `base`。
其中原 `standalone`／`combined` 没有独有的损坏输入分支，额外的零大小 ABI
成功断言已经包含在同名 CLI fixture 的 MIR／LIR golden。其他内部 ABI 拒绝
测试仍保留 `zero_sized_abi` 等公共检查。

以上移除四个 Rust 测试入口和两个重复参数用例，减少七次完整 core producer
执行，以及三次普通 library producer 执行；不删除源码 fixture 或拒绝断言。

## 必须保留的检查

- `base` 仍覆盖缺失泛型实化记录、HIR／MIR 不一致、布局／ABI／dispatch／GC
  元数据损坏、依赖形状和物理导入等拒绝路径。普通源码 fixture 无法构造这些
  通过前一层结构检查、但在后续引用或 ABI 检查中必须被拒绝的输入。
- `property_initialization_uses_close_source_mir_lir_and_both_artifact_views`
  不只是属性初始化正例。它还针对初始化使用、注册依赖、调用物化、对象内容、
  符号使用构造错误数据，并分别验证编译和链接 reader。现有 CLI 正例没有覆盖
  全部这些失败条件，因此本次不删除该入口。
- `edited_core_library_builds_from_a_manifest_and_is_consumed_from_any_output_path`
  仍含 protocol 与公共绑定共享身份、直接输入分类、错误 core 版本、非 core
  产物拒绝等内部断言。CLI 已覆盖其构建功能，但尚不能完整替代这些断言。
- 可重现性测试保留：它比较两个独立 checkout、不同文件创建顺序和三种输入
  形式的完整产物字节；冷／热缓存命中测试不能替代独立编译之间的比较。
- 其他专用测试中存在按源码分支选择的损坏 metadata 检查，未仅因已有同名
  fixture 而删除。

## 产物指纹与重复修改矩阵

`DecodedSlibEnvelope::open` 在解码内部语义前检查成员内容摘要及产物指纹。
`envelope_rejects_manifest_and_member_corruption` 已覆盖未重新计算摘要的损坏
产物被拒绝。指纹匹配不等于内部引用或 ABI 正确；必要的格式、类型、引用与
ABI 检查仍保留，不能将指纹用作额外的来源授权。

原 `link_symbol_uses` 集成测试会修改内容后重新计算成员摘要和产物指纹，
随后对每个候选重放完整 core 依赖闭包。移除其中三组重复矩阵：最终对象
清单／覆盖摘要、runtime 投影／摘要补丁，以及 Code 投影／摘要副本。
仅最终覆盖这一组就在两份输入上执行了 50 次完整重放；这些字段最终使用
整体比较，逐项枚举缺失、重复、顺序和摘要变化没有独立语言语义价值。

保留真实产物的完整 reader 成功检查、符号分类／引用及 ABI 拒绝检查。
容器损坏、最终对象重建、覆盖清单、注册摘要和 Code 指纹各自已有 `scoop-slib`
局部单元测试；正式 CLI fixture 继续锁定生成产物、阶段输出、独立消费、
链接和执行结果。这里删除的是重复全链路枚举，不改生产侧 reader 的行为。

## 验证

删除后先执行 `cargo fmt --all` 和 `cargo clippy --workspace --all-targets`。
两项均通过，lint 没有警告。对应八个核心 CLI fixture 在全新工作目录、关闭
更新模式后通过：8 个变体、27 个进程、65 份 stage／link-plan golden。
额外的堆空值字段运行组合及最终全仓验收见 [ACCEPTANCE.md](ACCEPTANCE.md)。

三组重复修改矩阵移除后，格式化与全 workspace lint 通过；`scoop-slib` 的
591 项单元测试全部通过。完整 workspace 初轮的慢测试本身通过，但耗时
2634.42 秒，不能作为合理速度基线。后续耗时复验见验收记录。
