# Unit 派生相等跨 Cone 回归

公开 CLI 的默认值组合暴露了同名 Strong 定义冲突：提供方和消费方均请求内建
`Unit` 的派生相等正文。`m23-cli-unit-equality/cross-cone` 将触发条件缩小为普通
Unit 比较和导入默认值；修复前同样在 FinalLink 报告重复 Strong owner。
`m23-cli-source-defaults/derived-generic-defaults` 保留原泛型 struct、tuple 与
Unit 默认值源码，并通过普通工厂在下游实际调用多个 application。

实现复用既有 DerivedEquality generated key、StructuralType ODR group/member
和 Strong／ODR 共有产物路径，只调整 Unit 派生相等 helper 的归属。普通 Unit
类型、布局、装箱和其他参数自由名义类型的归属保持；不新增 wire、运行时入口
或身份表。MIR materialization 模块的单元测试移入子模块，以控制文件长度。

独立用例、泛型组合与既有 core shared-equality 用例共 **3 用例／15 进程／
30 份阶段与计划 golden**，8 个产物指纹通过只读验证。两个可执行用例在移走
源码后独立链接，普通和移动 GC 均通过。344 项 MIR 单元测试、1 项新增真实
源码类型化测试和原 core equality producer／reader 测试通过。

修复前后的 28 份阶段输出逐项审阅：差异仅为 Unit helper 的 callable body
符号及新增 ODR identity 导致的进程内 ImportedIdentityId 编号移动。控制流、
调用参数、ABI、类型和真实生成指令保持；新 golden 保留实际身份与编号，
Python runner 不增加语义归一化。此记录不代表 M23-11 全仓验收完成。
