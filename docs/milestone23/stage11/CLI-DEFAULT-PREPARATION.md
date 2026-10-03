# 默认参数准备与局部正文迁移

`tests/fixtures/m23-cli-default-preparation` 通过唯一 Python schema 1 执行原
`m23-default-preparation` 与 `m23-type-source-defaults` 的 25 份源码。
本记录仅说明这一批迁移，不代表 M23-11 全仓验收完成。

## 覆盖对应

| 功能 | CLI case | 原测试保留的含义 |
| --- | --- | --- |
| 前向默认值准备 | `forward` | 普通函数、主／次构造器、enum、继承、泛型、vararg 与局部捕获 |
| 默认值循环 | 六个 `*-cycle` | 各 callable role 的定义位置、唯一循环主错误及后续展开错误 |
| 无效前向默认值 | `invalid-forward` | String→Int 类型错误和调用点的无效默认值展开错误 |
| 局部调用域 | `local-calls`、两个 `local-call-*` 错误 | 局部声明可调用；同名和外部 private callable 不获得额外可见性 |
| 局部捕获 | `local-captures`、`capture-combinations` | 定义方值、泛型局部默认值、重复 closure 与实际 ABI |
| 重复闭包 | `repeated-closures` | anonymous 正文复用，bound reference 的创建点独立 |
| 泛型引用 | `generic-reference`、`generic-closures` | 创建作用域、参数换序、多层展开、bound member／extension 和构造默认值 |
| 局部泛型参数 | `dependency-binders`、`local-binders`、`owner-arguments` | callee 签名、caller 捕获值、自有参数及未使用的 owner 参数 |

三个组合保持原三个文件的顺序和显式换行，以普通 `concat` 文件动作送入同一个
single-file 编译请求。其余输入原样复制；全部保持原 `scoop:single-file` 身份与
`main.scoop` 逻辑路径。core 先由真实源码构建并移走，再编译消费者；10 个正例均
保存 AST/HIR/MIR/LIR，移走程序源码后独立链接，以普通与移动 GC 运行原 main。

## 快照、诊断与清理

原 12 份选择性 HIR/MIR 快照的控制流、类型、值和语句顺序保持一致。
其中一份逐字相同；其余变化均为不再合并 core AST 后的局部函数、属性、字段、
capture binding、constructor、closure 或 coroutine arena 编号。新完整 golden
保留真实编号和 Export／LocalConcrete 边界，runner 不增加语义归一化。

9 个 negative case 按完整诊断数组验证原定义位置。原 Rust 测试只查找主错误，
实际 `mutual-cycle` 还报告无效默认值展开；`invalid-forward` 在准备和实际请求时
各保留类型错误，再报告调用点的展开失败。这些诊断逐项核对并完整保留，没有用
任意非零退出替代 HIR 错误。

删除原 12 份文件快照、三个仅承担文件快照／诊断编排的模块，以及剩余测试中的
快照选择与比较代码。保留 typed callable domain、参数类型、captured value identity、
闭包数量、创建 context、未选默认值不物化、定义文件恢复和 checked cast 类型检查。

最终只读验证：**19 用例／19 变体／68 进程／50 份阶段与计划 golden**，
29 个产物指纹匹配，13 项保留的 Rust 单元测试全部通过；格式化、Python lint 和
完整 workspace clippy 通过。原 fixture 源码未删除、未搬入 Rust 字符串。
