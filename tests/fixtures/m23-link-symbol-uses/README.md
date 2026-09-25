# 实际 Link 用途与 Code 重放

独立用例包含外来属性初始化、普通 callable、String 和 native 调用；组合用例继续加入
default、重复 getter/setter 及多种初始化路径。两个用例从真实源码产生完整 layout 产物，
各自通过共有 Compile 和 Link metadata 重放。

`*.symbols.snap` 锁定各 provider 的定义和完整 relocation 分区，`*.runtime.snap`
与 `*.coverage.snap` 锁定最终对象、登记指纹、owner 及 coverage 重放。
`*.code.snap` 保存 reader 重算的实际 Code 值及 native contract/library 数量；这些值
必须与 producer 的原值一致。反例在重建合法 archive hash 后分别篡改 distribution、
output 分支、两个 Code 字段和 native 表，包含两个 Code 值一致但都错误的情况。
依赖和当前产物执行相同反例；累计预算须在精确边界成功、少一个单位失败。

执行入口为 driver 测试 `property_initialization_uses_close_source_mir_lir_and_both_artifact_views`。
只在有意更新 Code 快照时设置 `SCOOP_UPDATE_LINK_CODE=1`。
