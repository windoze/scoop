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
