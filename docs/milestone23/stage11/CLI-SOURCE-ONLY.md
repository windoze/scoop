# M23-11 source-only nominal 的正式发布与消费

六份原始源码保持不变，由公开 CLI 发布 library，移走源码后再编译普通下游。provider 与 consumer 的 AST／HIR／MIR／LIR 均保存完整 golden。这些用例验证合法 library 的发布与消费，不要求不存在的程序入口。

迁移时在同批 CLI 生成的真实 `.slib` 上执行原 typed reader 断言；原初始化用例的三份阶段快照逐字相同。文件编排与 golden 比较由统一 Python runner 接管，HIR 局部类型不变量继续由原 crate 的普通单元测试负责。

| 功能 | 用例 | 只读进程／golden | 保留的断言与清理 |
| --- | --- | --- | --- |
| `formal_publication_retains_open_templates_beside_concrete_roots` | `standalone`、`combined`、`shape-demand` | 9／24 | 开放 nominal template 与非空 binder 保留；具体布局根均有对应声明；闭合父类型的 owner 均在根集合，前两例至少有一个闭合父类型 |
