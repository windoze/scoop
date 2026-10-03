# M23-11 source-only nominal 的正式发布与消费

六份原始源码保持不变，由公开 CLI 发布 library，移走源码后再编译普通下游。provider 与 consumer 的 AST／HIR／MIR／LIR 均保存完整 golden。这些用例验证合法 library 的发布与消费，不要求不存在的程序入口。

迁移时在同批 CLI 生成的真实 `.slib` 上执行原 typed reader 断言；原初始化用例的三份阶段快照逐字相同。文件编排与 golden 比较由统一 Python runner 接管，HIR 局部类型不变量继续由原 crate 的普通单元测试负责。

| 功能 | 用例 | 只读进程／golden | 保留的断言与清理 |
| --- | --- | --- | --- |
| `formal_publication_retains_open_templates_beside_concrete_roots` | `standalone`、`combined`、`shape-demand` | 9／24 | 开放 nominal template 与非空 binder 保留；具体布局根均有对应声明；闭合父类型的 owner 均在根集合，前两例至少有一个闭合父类型 |
| `source_only_objects_preserve_required_initialization_through_all_emitted_stages` | `initialization-demand` | 3／8 | 原 HIR／MIR／LIR 三份快照逐字相同；迁移基线的 shape support closure 数为 2，suspend 成员物化修复后为 4（接口与单例各新增一项） |
| Concrete demand | `actual-demand`、`generic-records` | 6／16 | 正式产物消费触发普通需求、accessor 需求及泛型实例；保留 HIR crate 的局部 typed 不变量单元测试 |

原 HIR nominal 摘要快照也已在正式 `.slib` 上逐项重建核对：standalone 的 3 行与 combined 的 20 行全部一致，包括名字、kind、modality、binder、父类型、字段、构造器、成员数量和 machine 支持状态。combined 的 protected 声明与单个 default template 断言同样通过。删除旧文本行拼装和快照 helper，保留局部 typed facts、representation、inheritance 及与 arena 分配顺序无关的普通单元测试；正式 CLI 的完整 HIR golden 继续锁定这些字段。

闭合 suspend 成员发布修复后，DeferredSuspend 和 DeferredRegistry 沿同一物化闭包进入产物；DeferredRegistry 现在具有实际 singleton 初始化单位。六例重新通过只读 CLI 验证，原 typed 初始化身份关系仍保留。变化与独立运行回归见 [suspend 成员记录](CLI-SUSPEND-MEMBERS.md)。
