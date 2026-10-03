# Core 运算用例迁移记录

本批使用统一的 `tests/run_fixtures.py` 和 schema 1，原 `.scoop` 均保留，
不把源码或断言转移到 Rust 内联注册。M23-11 的全仓验收尚未完成。

## 单文件组合

`m23-cli-core-operations` 通过公共 `concat(path,parts)` 将原文件与原显式换行
组成同一个 single-file 输入，保留 `scoop:single-file` 身份、源码位置及声明顺序。
core 包含原 driver 用例的六个扩展文件与两个 reference source-fields 文件，
由真实源码生成 `.slib`，随后删除 core 和消费者源码。独立复制的 `scoop` 在
没有 `scoopc` 与 LLVM 工具的 PATH 下从产物重新链接，比较 link plan 指纹，
并执行普通与强制移动 GC 两次运行。core 与根产物均固定实际 artifact 指纹。

| 功能 | 原输入 | 用例／进程／golden | 与原断言对照 |
| --- | --- | --- | --- |
| 成员调用组合 | `m23-imported-core-members/methods.scoop`、`combined.scoop` | 1／5／5 | 原 HIR export、完整 MIR/LIR 逐字相同；新 HIR 同时锁定 LocalConcrete |
| 整数相等与模式组合 | `m23-imported-core-members/equality.scoop`、`equality-patterns.scoop`、`equality-combined.scoop` | 1／5／5 | 原 HIR export、完整 MIR/LIR 逐字相同；新 HIR 同时锁定 LocalConcrete |

公共文件动作的 29 项单元测试通过，包含原始 UTF-8／二进制字节、显式分隔符、
目标作为输入、后续 patch、缺失输入不覆盖目标，以及 schema 与步骤引用校验。

## 单文件请求边界

组合迁移时核对了 DESIGN §3.1 的 single-file 规则：`scoop build/run <file>`
现在在请求配置边界拒绝 `--cone-path`，并明确区分 `scoop link` 的正常产物查找。
新增 `m23-cli-inputs/single-file-cone-path-{build,run}`，在缺失 compiler/sysroot 时
仍先报告完整的 `SCOOP_CLI_CONFIG`，不建立 cache/target。组合用例从普通 sysroot
artifact slot 取得已经编译的 core，不传额外 Cone locator。完整输入 suite
21 项／72 进程／8 份 golden 通过；两项 single-file 组合的产物和阶段期望均未变化。

## 独立成员与常量

普通输入保留原 `test:scoop-hir-lower:0.0.0`、`src/main.scoop` 身份。
先单独编译原 library 并锁定 AST/HIR/MIR/LIR，再通过实际程序验证值；
原 internal 成员由同 Cone 新增入口调用，不改可见性。public 常量与静态属性
由删除源码后的独立下游读取，静态可变属性还验证再次赋值。
所有正例均从根 `.slib` 独立链接并运行普通／移动 GC。
参数改名用例只修改原 core 的 `Int.shl(count: Long)` 参数名为 `distance`，
验证方法、infix、const 与旧具名参数诊断遵循真实源码声明。

| 功能 | 新 case 前缀与范围 | 用例／进程／golden | 原断言与实际验证 |
| --- | --- | --- | --- |
| 成员调用与参数 | `members-`：`methods`、`wrong-name`、`wrong-type`、`non-infix`、`conversion-argument` | 5／14／9 | typed core 成员选择、窄整数回绕、转换、Boolean/String 运算及 4 个完整错误 |
| 整数相等 | `members-`：`equality`、`equality-width`、`equality-signedness`、`equality-extension` | 4／12／9 | 8 种整数宽度／符号、操作数恰好一次及求值次序、3 个精确类型错误 |
| 整数字面量模式 | `members-`：`equality-patterns`、`equality-pattern-overflow` | 2／8／9 | 8 种极值模式的命中／未命中、alias、tuple 与 literal 越界 |
| 托管整数操作 | `members-`：`managed` | 1／6／9 | 实际有符号除法；内部 Option<String> 异常存储检查继续保留 |
| 成员源码参数改名 | `members-`：`edited-parameter`、`edited-old-name` | 2／8／9 | 真实 Int.shl 声明的具名参数与回绕，旧参数名拒绝 |
| 常量运算符 | `const-`：`operators`、`division-zero`、`type-mismatch` | 3／10／9 | 11 个 const 值、编码静态值、除零及声明类型不匹配 |
| 常量成员与 infix | `const-`：`methods`、`wrong-argument`、`non-infix`、`method-division-zero`、`conversion-argument` | 5／14／9 | 6 个 const 值、静态 Long 初值及赋值、4 个完整错误 |
| 常量源码参数改名 | `const-`：`edited-parameter`、`unknown-method`、`edited-parameter-old-name` | 3／10／9 | 具名与 infix 均折叠为 12，未声明方法与旧参数名拒绝 |

## 扩展 core 的原 driver 阶段用例

以下 8 个原 single-file 程序复用相同的实际 core 输入，均有完整四阶段输出、
固定产物、独立 link plan 与普通／移动 GC 运行。原 22 份阶段断言全部逐字保留：
HIR 对照原完整 Export，新 golden 同时保留 LocalConcrete/CrossCone；MIR/LIR
对照原完整输出。初始化调用的 shared external arena 正、反断言也由这些完整
MIR/LIR golden 保留。

| 功能 | 新 case（`library-` 前缀） | 用例／进程／golden |
| --- | --- | --- |
| core alias 类型与值名称 | `aliases` | 1／5／5 |
| 初始化与普通 core 调用 | `initialization-call`、`initialization-combined` | 2／10／10 |
| 常量与普通 core 调用组合 | `constant-combinations` | 1／5／5 |
| 整数默认参数组合 | `integer-defaults` | 1／5／5 |
| 托管整数默认值的异常布局 | `integer-exception` | 1／5／5 |
| 分支、局部值与默认参数组合 | `branch-default`、`branch-combinations` | 2／10／10 |

## 验证与退役

最终只读运行：**35 用例、35 变体、132 个真实进程、122 份阶段／plan golden**，
包含 18 个成功用例和 17 个完整诊断用例，61 个 artifact 指纹断言。
17 个诊断的原消息及 operand span 已逐项对照；其中 3 个混合整数相等错误保留
已有实现 `9950e8f48` 添加的显式 `toInt32()` 转换建议，原源码范围不变。

删除 driver 的 28 份旧阶段快照和 6 个专用 stage helper，删除 HIR 中已迁移的
诊断／dump 字符串编排，Rust 净减少 603 行。原 `.scoop` 全部保留。
保留 no-intrinsic／无外部调用、8 种 literal equality plan、typed const 和静态
编码值、异常 Option<String> 特化、改名后的 const 值等内部检查；driver 的
共享依赖闭包、metadata/ABI、source fields、直接输入、版本及重建指纹检查保留。
host target 不可用现在明确失败。清理后 25 项普通 core HIR 测试与 1 项 driver
综合测试通过，Rust/Python 格式化和完整 workspace lint 通过。

这只是 M23-11 的 core 运算批次；剩余 fixture 与最终全仓验收仍待完成。
