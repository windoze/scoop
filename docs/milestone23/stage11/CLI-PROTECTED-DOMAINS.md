# 受保护访问域与默认值的正式产物覆盖

`m23-cli-protected-domains` 将 12 份原源码接入正式公开 CLI：9 个正例、3 个
负例，只读验收通过 67 次进程执行、109 份阶段与链接计划 golden、37 个
产物指纹。三个负例分别核对唯一诊断的原消息、canonical source 和完整 span。

| 原目录 | 新用例 | 覆盖 |
| --- | --- | --- |
| `m23-source-access-domains` | `access-standalone`、`access-combined`、`access-private-default` | 泛型宿主、继承默认值、private/internal 访问域与越域默认调用诊断 |
| `m23-setter-domains` | `setter-combined`、`setter-internal-on-protected`、`setter-protected-on-internal` | 十个属性、各宿主的 getter/setter 可见性及两个非法 modifier 位置 |
| `m23-type-protected-production` | `protected-members`、`protected-nested` | 下游继承调用、setter、private 辅助成员、受保护泛型嵌套类型与 enum |
| 同上 | `protected-default`、`protected-default-references` | 受保护构造默认值、隐式/显式 receiver、函数引用、tuple、enum、object 与动态全局默认值 |
| 同上 | `protected-default-varargs`、`protected-default-combinations` | 空/非空 vararg、泛型默认值与闭包捕获组合 |

正例先以原源码生产 provider，完整保存 AST、Export/LocalConcrete HIR、MIR
和 LIR。需要从合法访问域观察私有/受保护行为时，TOML 明确在工作副本的
原宿主中加入普通公开检查方法，再通过真实工厂与下游程序调用；原源码与
原声明可见性不变。这次物化也单独保存四阶段和指纹。消费程序完成构建后
移走源码，仅使用 `.slib` 和 runtime index 独立链接，分别在普通与移动 GC
下运行。

`setter-combined` 的原 36 行 HIR 与新 provider 的 Export 部分逐字相同。
对应 Rust 测试删除文件 dump、快照更新开关及负例循环，保留实际 artifact
reader 对十个属性、八个 support 声明、三个 internal/protected setter、
泛型宿主及 Unit 外部签名引用的结构断言；两项测试通过。七份旧摘要/快照
退役，原十二份源码全部保留。新的四阶段输出覆盖原摘要所涉及的声明与
默认值，额外锁定真正进入机器代码的 LocalConcrete 与下游物化结果。

闭包捕获组合在迁移中发现的引用闭包问题由
[默认捕获修复](CLI-DEFAULT-CAPTURES.md) 单独修复并提交，reader 检查保持。
本记录不代表 M23-11 全仓验收完成。
