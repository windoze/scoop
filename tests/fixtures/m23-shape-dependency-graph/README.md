# 实际形状依赖与外来 helper 消费

`standalone.scoop` 单独覆盖外来 `Int` 装箱和类型测试；`combined.scoop` 组合覆盖
`Unit` 零尺寸装箱、`String` 类型测试、smart cast 拆箱、两次 default 展开和未物化的
generic 声明。测试从真实 core 源码产生 MIR/LIR layout 表与 Strong V2 对象定义，
消费方的函数则全部从上述源码降低。

- `*.mir.snap` 和 `*.lir.snap` 锁定共有 metadata reader 重放的语义依赖图。
- `*.machine.mir.snap` 和 `*.machine.lir.snap` 锁定实际函数、装箱、拆箱和类型测试。
- driver harness 验证 MIR helper 的 source/provider 归属，消费方不产生这些 helper
  的本地 layout、描述符、dispatch adjust 或 registration，并检查 LLVM 和 Strong V2 对象发射。
- 完整 layout 来源投影直接复用 MIR 来源接口；实际 Type/ShapeSupport 用途经完整 section
  和字节解码后的共有 reader 重放，必须与 `*.lir.snap` 的独立语义依赖图一致。
- layout 的五组本地 exports、物理导入和 Strong V2 registration 完成同一 source/export
  校验，来源记录漂移和累计预算反例由同一入口拒绝。
- `*.artifact.snap` 记录实际组装产物及 Compile、Link 独立重放后的 provider/物理导入。
  Link 的四组专用载荷和原累计预算贯穿共有路径；修改 Link-only provider 并重建合法
  archive hash 的反例，必须在物理导入关联处拒绝，不能仅靠外层摘要或成功解码放行。
- `*.code.snap` 锁定两个 provider 的实际 Code 指纹与完整重放结果。反例覆盖 distribution、
  library/executable 分支、两个位置的 Code 值及 native contract/library 表；同时改错两个
  Code 值并重建合法 archive hash 也必须拒绝。工作量和内存预算验证精确边界及少一个单位。
- 反例覆盖缺少完整选择集、错误 consumer、缺少 ShapeSupport、裸描述符不能替代
  ShapeSupport、缺少 helper 物理导入、错误 helper 配对、预算耗尽，以及 runtime
  String 描述符不能单独授予源码类型测试能力。

机器层测试使用完整 provider 表建立消费选择，其测试用途只覆盖这一 lowering 入口。
最终 CLI 的 layout profile、逐次访问和初始化用途、Compile/Link 双 view 发布仍由
M23-6 的相应完成门独立验收。

执行入口为 driver 测试 `actual_core_sources_produce_closed_mir_and_lir_export_tables`。
只有有意更新快照时设置 `SCOOP_UPDATE_SHAPE_DEPENDENCY_GRAPH=1`；常规验证不设置。
