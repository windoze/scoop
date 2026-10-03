# 委托 registration 损坏向量

这些字节片段从当前 fixture 正式生产的 consumer `.slib` 及其五份变体一次生成。生成时复用原 Rust 测试的 typed registration、wire encoder 与普通 Slib writer，并逐项确认原 reader 错误；生成器未保留在仓库。

每次测试先从原 Scoop 源码重新生产实际产物，并检查完整 SHA-256，再以公共 `replace` 动作唯一替换二进制片段。修改后的 SHA-256 也必须完全一致。片段同时包含对应 container/member/semantic 摘要变化，保证实际 compiler 到达原 LIR 语义边界。Python runner 不解析 IR、CBOR 或 Slib，也不执行用例脚本。

- `swapped-initializer-and-ensure`：交换 unit 的字段 6/7 和完整 callable 引用字段 21/22，拒绝 initializer_role 不匹配。
- `missing-unit`：删除一个初始化单元 registration，拒绝 InitializationUnit 表长度。
- `missing-delegate-storage`、`missing-failure-root`：分别删除实际 delegate storage/failure root registration，拒绝 StaticStorage 表长度。
- `missing-initializer-callable`：删除实际 initializer callable registration，拒绝 SurfaceMismatch。

原有效产物先完成正常及 moving GC 运行。每个损坏变体由相同原 downstream 源码经正式 `scoop build` 消费，检查完整诊断且不得发布请求的 `.slib`。格式或生产内容改变时需要重新审阅向量，不放宽摘要或错误条件。

M24 迁移对照原有效产物与当前正式产物，确认上述 registration 表未改变后，保留相同的删除与交换；当前 TypeDescriptor 的显式 release policy 和实际定义摘要完整保留。按共有编码重新生成 member、LIR semantic 和 artifact 摘要，并核对二进制替换得到的完整产物与迁移结果一致。
