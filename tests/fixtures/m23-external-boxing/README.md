这组 fixture 锁定 LIR 阶段消费外来 BoxedValue helper 的行为。

提供方来自 `m23-core-layout-exports/shared-shapes-standalone.scoop` 和
`m23-core-layout-exports/shared-shapes-combined.scoop` 的实际编译产物。
测试从已验证的 Strong V2 生产表及完整 layout/ABI 选择集中取得 helper，
再构造 LIR 往返装箱函数、验证 LLVM 并发射对象文件。

- `standalone.lir.snap`：空 struct 和包含空字段的普通值，无 GC 引用。
- `combined.lir.snap`：空 struct 和包含 String 引用、空字段及接口实现的值，
  校验装箱期间的递归根帧和 statepoint。

消费方没有本地 TypeDescriptor 定义；ZST 不分配 payload 存储。
相关反例验证 ShapeSupport、物理导入、provider、arena 绑定和资源预算。
这是一组 LIR 阶段测试，不代表源代码跨 Cone 装箱和最终 Compile/Link 闭包已完成。
