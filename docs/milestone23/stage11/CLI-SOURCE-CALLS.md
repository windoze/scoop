# 实际调用位置、接收者与函数引用迁移

本批把十一份尚未归属的原源码接到正式 CLI：四份调用位置、三份接收者消费方
及其 provider、一份挂起函数引用、两份 enum 身份。共用 schema 1；每个成功
程序移走源码后独立链接，并在普通与移动 GC 下运行。

| 功能 | 用例 | 实际观察 |
| --- | --- | --- |
| 调用位置 | 4 | 真实 provider 计数，直接／facade 路由均调用三次；局部重载胜出时零次；默认值、构造器、循环、guard、本地函数和 lambda 组合分别调用 14／15 次 |
| 接收者 | 3 | String 调用、到 Any 的本地适配、泛型默认值、getter/setter 和导入默认值均实际执行 |
| 挂起函数引用 | 1 | 返回的 suspend 函数值分别应用到 Int 和 String，通过 startCoroutine 验证结果 |
| enum 身份 | 2 | 单文件执行普通／泛型 enum、payload、ZST、嵌套值和默认字段的原 main |

调用位置与接收者消费方保留 `test:scoop-hir-lower:0.0.0` 和 `src/main.scoop`。
调用 provider 使用原 typed coordinate `test:occurrence-provider:1.0.0`，facade
使用真实 public import；新增普通计数函数只观察运行结果。

退役九份摘要快照及 Rust 文件比较／更新开关。内部测试继续直接检查十八个
源码调用的定义／求值来源、重复默认值、路由及无关 arena 插入后的稳定性。
完整物化还包含派生相等函数对 core Boolean.equals 的调用：这些按真实
SourceDeclaration、生成函数根及 Boolean 签名断言，源码调用仍检查原
SourceBinding 路由。没有把新记录过滤成未验证的数据。

接收者测试保留静态类型与适配后参数的区别，增加完整参数 exact identity 和
跨 provider 默认值数量检查。enum 测试直接比较 typed variant/field/exact
identity，并保留 Export→LocalConcrete→MIR 一致性及声明插入稳定性；移除
其字符串投影。挂起引用的发布测试继续保留。十三个已迁移引用负例的原消息和
源码 token 均逐项核对后，删除重复 Rust 文件循环。

只读验证：新增十个 CLI 用例通过 **67 进程／118 份阶段与计划 golden**，
37 个产物指纹匹配；另十三个已有引用负例通过 39 进程。十三项保留的 Rust
结构测试、格式化、workspace clippy 和 Python lint 全部通过。
这份记录不代表 M23-11 全仓验收完成。
