# 实际形状依赖与外来 helper 消费

`standalone.scoop` 覆盖外来 `Int` 装箱和类型测试；`combined.scoop` 组合覆盖
`Unit` 零尺寸装箱、`String` 类型测试、smart cast 拆箱、两次 default 展开和
未物化的 generic 声明。

正式 CLI 声明位于 `../m23-cli-layout-dependencies/shape-*`。保留原 core 的
`base.scoop`、实际 consumer 坐标、完整四阶段 golden 和产物 fingerprint；
移走源码后，下一层从 `.slib` 调用 consumer。产物 fingerprint 固定语义依赖图、
helper 导入、对象、登记及 Code 等内容。

driver 内部测试保留 source/provider 归属、完整选择集、helper 配对、描述符与
物理导入的对应关系，以及实际 LLVM/object 断言。Compile/Link 的格式、引用和
定点损坏检查继续执行；外来 helper 不在消费方重复发射本地定义。

原重复的文件快照和更新环境变量已删除，完整迁移对照见
`../../../docs/milestone23/stage11/CLI-LAYOUT-DEPENDENCIES.md`。
