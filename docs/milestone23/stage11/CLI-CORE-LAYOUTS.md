# M23-11 core 布局 fixture 迁移

本文件逐项记录迁移结果；未列出完成记录的用例仍待验收，不代表 Stage 11 已完成。

## 现行语义与旧期望差异

`edde2400a` 已按实现规范为公开、参数自由且可比较的值类型生产实际派生相等正文。旧 core 快照缺少 `ForeignCallbackMode.equals`、`ForeignCallbackState.equals`、`SourceLocation.equals`，因此 base callable 数量由 179 变为 182；其余原布局摘要逐字一致。正式 CLI 与原生产逻辑当前生成的 base `.slib` 已逐字一致。旧整体指纹随实际函数和对象内容变化。

原 shared slot 负例 helper 只按 slot id 寻找待改记录，同一 slot 出现在不同 schema role 时会替换错误的记录并构造重复 key。现在按完整 `(role, slot)` key 修改，原 ancestor、modality、abstract obligation 和 signature 拒绝断言不变。包含该组断言的 `actual_core_sources_produce_closed_mir_and_lir_export_tables` 已运行通过；本次对照运行显式更新旧快照，最终验收仍须关闭更新。

## 已迁移功能

| 原内部测试 | 正式 CLI 用例 | 进程／四阶段 golden | 产物核对 |
| --- | --- | --- | --- |
