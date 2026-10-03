# M23-11 初始化、调用与布局依赖的 CLI 迁移

`m23-cli-layout-dependencies` 通过正式 `scoop build` 生产 core、普通 provider 和 consumer，再移走它们的源码，由独立下游消费 `.slib`。20 项只读验证全部通过，共 65 次进程执行、176 份阶段 golden。19 项成功，`ordinary-any-wrong-result` 保留 Any 不能隐式收窄为 String 的完整 canonical 诊断、源码区间及无产物断言。

原源码及坐标保持：core 的 `src/layout_probe.scoop` 使用原初始化 provider；shape／LIR 依赖用例仍包含原本只有注释的 `base.scoop`，因为它也是实际 source inventory 的一部分。普通初始化和 Any provider 分别为 `test:property-provider:1.0.0`、`test:any-calls:1.0.0`；普通 consumer 为 `test:scoop-hir-lower:0.0.0`。

迁移核对采用实际原 producer 产物与 CLI 产物：两个 core、十个初始化／调用／对象 consumer、四个 shape／LIR consumer，共 16 份归档逐字节一致。TOML 固定检查 45 个实际 artifact fingerprint，保留原对象、Code、runtime image、符号与依赖内容的整体约束。

34 份旧阶段输出中 24 份逐字一致；其余十份 core consumer LIR 只增加最终产物已有的初始化依赖边（独立例一条、组合例三条）。旧 Rust dump 发生在连接这些边之前，原 typed 断言与最终产物已包含它们。shape／LIR 的四份下游 HIR 在补回原 `base.scoop` 后，局部 `ImportedIdentityId` 索引随 source inventory 增加一项而移动，其余全文相同。

五个普通 consumer 的实际 CLI HIR 产物另经原摘要对照：初始化 use 数量分别为 1／4／0，Any 调用数量 1／9、receiver 数量 0／4、展开默认值数量 0／1，原摘要逐行一致。Any 参数与结果仍保留完整 exact type，String receiver 仍是原 core 名义类型。

旧对象／调用统计中 core 多出的三个派生 equals 及相应指纹变化沿用 [core 布局迁移记录](CLI-CORE-LAYOUTS.md) 中已核实的 `edde2400a` 修正。本次没有提交旧快照更新，也没有改变对应语言规则。

| 功能 | 正式 CLI 用例 | 进程／阶段 golden |
| --- | --- | --- |
| 属性初始化、展开默认值与不活跃正文 | `core-initialization-combined`、`core-initialization-standalone`、`ordinary-initialization-combined`、`ordinary-initialization-inactive`、`ordinary-initialization-standalone` | 18／52 |
| 外部 extension receiver | `core-extension-combined`、`core-extension-standalone` | 6／16 |
| Any 调用签名、getter、默认值与错误结果类型 | `core-any-combined`、`core-any-standalone`、`ordinary-any-combined`、`ordinary-any-standalone`、`ordinary-any-wrong-result` | 17／44 |
| 实际对象内容与物理引用 | `core-objects-combined`、`core-objects-standalone` | 6／16 |
| 定义符号、未定义引用与 runtime 内容 | `core-symbols-combined`、`core-symbols-standalone` | 6／16 |
| MIR shape 依赖与真实机器物化 | `shape-combined`、`shape-standalone` | 6／16 |
| LIR 依赖与 ABI 选择 | `lir-combined`、`lir-standalone` | 6／16 |

原 78 份文件快照及专用格式化／比较 helper 已删除，相关 Rust 代码净减少 347 行。source call、receiver、初始化 use 与 registration edge、Compile／Link section 相等、ABI／物理选择、实际 final object 字节、完整定义／未定义符号集合、Code／runtime 内容、11 类对象 mutation 和 reader 精确拒绝继续用原 typed 断言检查。`RuntimeMutation::Patch` 不再携带仅供旧 dump 打印的角色字段，生成 mutation 时仍按每个实际 semantic field role 选取代表。

清理后格式化和 lint 通过；两个 HIR 初始化／Any 专项，以及 `actual_core_sources_produce_closed_mir_and_lir_export_tables`、`property_initialization_uses_close_source_mir_lir_and_both_artifact_views` 两个完整 producer／reader 测试均在关闭更新开关时通过。CLI 的 20 项也已在清理后只读运行通过。此记录不替代 M23-11 最终全仓验收。
