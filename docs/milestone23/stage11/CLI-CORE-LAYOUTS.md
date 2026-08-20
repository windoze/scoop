# M23-11 core 布局 fixture 迁移

本文件逐项记录迁移结果；未列出完成记录的用例仍待验收，不代表 Stage 11 已完成。

## 现行语义与旧期望差异

`edde2400a` 已按实现规范为公开、参数自由且可比较的值类型生产实际派生相等正文。旧 core 快照缺少 `ForeignCallbackMode.equals`、`ForeignCallbackState.equals`、`SourceLocation.equals`，因此 base callable 数量由 179 变为 182；其余原布局摘要逐字一致。正式 CLI 与原生产逻辑当前生成的 base `.slib` 已逐字一致。旧整体指纹随实际函数和对象内容变化。

原 shared slot 负例 helper 只按 slot id 寻找待改记录，同一 slot 出现在不同 schema role 时会替换错误的记录并构造重复 key。现在按完整 `(role, slot)` key 修改，原 ancestor、modality、abstract obligation 和 signature 拒绝断言不变。包含该组断言的 `actual_core_sources_produce_closed_mir_and_lir_export_tables` 已运行通过；本次对照运行显式更新旧快照，最终验收仍须关闭更新。

`shared-equality-standalone` 中公开的 `SharedEqualityDeferred` 和 `SharedEqualityUnused` 同样必须提供派生正文。原断言改为检查两者均有 Strong 定义和导出 binding；私有 `SharedEqualityHidden` 仍有本地正文但不导出，不可比较类型仍无派生候选。该组内部测试重新运行通过。

41 个 core 产物中 40 个与原生产逻辑逐字一致。`shared-units-combined` 的唯一非摘要差异是私有 object 初始化显示名由 `layout_probe.scoop` 变为正式 CLI 的 canonical `scoop:scoop.core:0.1.0/src/layout_probe.scoop`。HIR、MIR、全部函数对象及其机器码相同；runtime image 对象仅该 C 字符串改变，LIR 对应显示名、对象偏移与内容摘要随之更新，沿用 Stage 11 已规定的 canonical 来源。

## 已迁移功能

| 原内部测试 | 正式 CLI 用例 | 进程／四阶段 golden | 产物核对 |
| --- | --- | --- | --- |
| `actual_core_sources_produce_closed_mir_and_lir_export_tables` | `base`、`standalone`、`combined` | 6／24 | 逐字相同 |
| `actual_layout_artifacts_retain_alias_chains_and_combined_member_signatures` | `shared-aliases-standalone`、`shared-aliases-combined` | 4／16 | 逐字相同 |
| `shared_mir_types_replay_standalone_and_combined_source_policies` | `shared-mir-standalone`、`shared-mir-combined` | 4／16 | 逐字相同 |
| `shared_accessor_forms_select_only_actual_source_machine_bodies` | `shared-accessors-standalone`、`shared-accessors-combined` | 4／16 | 逐字相同 |
| `shared_source_callable_inventory_replays_bodies_and_abstract_overrides` | `shared-callables-standalone`、`shared-callables-combined` | 4／16 | 逐字相同 |
| `shared_constructor_inventory_replays_primary_secondary_and_class_initializers` | `shared-constructors-standalone`、`shared-constructors-combined` | 4／16 | 逐字相同 |
| `heap_zst_fields_produce_valid_layout_objects_and_shared_metadata` | `heap-zst-storage` | 2／8 | 逐字相同 |
| `shared_object_inventory_replays_singletons_companions_and_initialization_entries` | `shared-objects-standalone`、`shared-objects-combined` | 4／16 | 逐字相同 |
| `shared_dispatch_replays_slot_order_targets_reabstraction_and_boxing` | `shared-dispatch-standalone`、`shared-dispatch-combined` | 4／16 | 逐字相同 |
| `shared_equality_replays_materialized_source_applications_and_default_only_keys` | `shared-equality-standalone`、`shared-equality-combined` | 4／16 | 逐字相同 |
| `shared_initialization_units_replay_source_keys_and_complete_strong_pairs` | `shared-units-standalone`、`shared-units-combined` | 4／16 | 仅 combined 的 canonical 初始化显示名变化，详见上文 |
| `shared_lir_layouts_replay_recursive_references_zst_and_base_prefixes` | `shared-layouts-standalone`、`shared-layouts-combined` | 4／16 | 逐字相同 |
| `shared_lir_callable_abis_replay_zst_indirect_results_and_boxing_adjustments` | `shared-abi-standalone`、`shared-abi-combined` | 4／16 | 逐字相同 |
| `shared_lir_dispatch_replays_complete_tables_and_actual_callable_abis` | `shared-lir-dispatch-standalone`、`shared-lir-dispatch-combined` | 4／16 | 逐字相同 |
| `shared_lir_descriptors_replay_ancestry_scans_and_registration_from_constituents` | `shared-td-standalone`、`shared-td-combined` | 4／16 | 逐字相同 |
| `shared_lir_shape_support_replays_finite_helpers_from_checked_mir_roots` | `shared-shapes-standalone`、`shared-shapes-combined` | 4／16 | 逐字相同 |
| `shared_ordinary_lir_bridges_replay_source_gc_and_layout_abis` | `shared-ordinary-standalone`、`shared-ordinary-combined` | 4／16 | 逐字相同 |
| `shared_initialization_abi_replays_the_protocol_role_and_complete_layout_contract` | `shared-init-abi-standalone`、`shared-init-abi-combined` | 4／16 | 逐字相同 |
| `shared_strong_digests_replay_complete_runtime_registration_roles` | `shared-digests-standalone`、`shared-digests-combined` | 4／16 | 逐字相同 |
| `shared_strong_reader_replays_complete_sections_from_actual_artifact_bytes` | `shared-production-standalone`、`shared-production-combined` | 4／16 | 逐字相同 |
| `property_initialization_uses_close_source_mir_lir_and_both_artifact_views` | `property-initialization-provider` | 2／8 | 逐字相同 |
| `ordinary_library_exports_members_with_available_machine_signatures` | `ordinary` | 2／8 | 原内部断言通过；完整阶段输出保留到 CLI |
| `ordinary_bridge_replays_real_dependency_function_and_getter_calls` | `ordinary-consumer` | 2／8 | 原内部断言通过；完整阶段输出保留到 CLI |

## 普通库布局装配

`m23-cli-layout-assembly` 的 `standalone`、`combined`、`private-support` 三例分别构建原声明、加入原 `private-local.scoop` 和无关私有函数后的声明，再移走源码让真实下游消费产物。三例只读验证通过，共 12 次进程执行、36 份阶段 golden。原 `actual_source_mir_and_lir_assemble_complete_layout_exports` 的公开 section 字节不变、私有支持类型、ABI 与拒绝错误输入断言继续保留，已单独运行通过。

原三份布局摘要只分别增加 `Payload.equals`、`Value.equals`、`Token.equals` 及对应 callable 计数；其余布局、descriptor、dispatch、ABI 和形状内容逐字一致。`ordinary_bridge_replays_real_dependency_function_and_getter_calls` 的原 MIR/LIR selected 数量与完整旧摘要也保持一致。新增下游通过 `Exposed` 参数使用公开方法，遵守其原本未公开的构造函数可见性。

## 旧快照入口清理

对应 43 个 core／普通库用例及 3 个布局装配组合，已删除 191 份旧文本快照，以及只负责格式化和比较这些快照的代码。25 个原布局内部测试继续验证真实 producer／reader、完整导出表、public/private 字节不变、ABI、GC、dispatch、初始化、codegen 与损坏 metadata：关闭所有更新开关后，24 个在完整一轮通过，遗漏的 recursive layout 快照调用清理后，其独立测试也通过。最终全仓验收仍会再次运行完整集合。

公开类型的派生相等检查保留为 `mir_equality/applications.rs` 中的直接 typed 断言；`Unit`／`Any` 的种类和 GC 事实原先只在摘要中比较，现保留为直接断言。没有增加新的产物检查接口或 fixture 执行器。

全量 HIR 复验补清 `shared_callable_selection`、`shared_constructor_selection`
和 `shared_accessor_forms` 三处仍读取已删除摘要的调用及其更新环境开关。
保留原选择数量、外来来源为空、wire roundtrip 和实际 accessor 形式对照，
并直接核对 private/generic/abstract callable 的选择差异、槽与 GC effect、
泛型构造器排除、私有/受保护构造器保留，以及属性 public lookup。
1294 项 HIR 测试无筛选通过；对应六个 CLI 用例只读通过 12 进程、48 份
阶段 golden，已有指纹全部保持。
