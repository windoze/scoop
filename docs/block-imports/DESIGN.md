# 块级 import

状态：已实现（2026-10-28）。权威语义见语言规范12.4.3、12.4.6与实现规范2.2.1。

## 范围

在现有语句块中支持 exact、alias、star import。声明之后生效，嵌套块继承，离块恢复；支持普通函数、lambda、局部函数／类型正文、初始化与控制流块。禁止局部 public import，不改变文件头导入、package selector 或普通表达式语法。

## 实现

- AST 保留 ImportSyntax，parser 复用 selector 并把块内 public import 诊断在原位置；formatter 保持位置与幂等。
- import selector 复用当前 Cone 静态 namespace 及直接依赖的已加载 binding。定义处解析时保留可见性与来源过滤，不增加依赖加载或凭证机制。
- 词法 scope 保存已生效的导入，嵌套 callable／局部类型沿现有 scope 快照继承。每个块提供 exact 和 star 两层，内层优先；文件级层次随后。调用、值、类型、extension、operator 与 callable reference 共用这些层。
- HIR 输出只保存实际引用，不包含 import 运行期节点。LSP 记录 import／alias 定义、使用和原 span，并按当前位置收集补全与签名候选；折叠只合并同一作用域连续的 import。

## 验收

普通行为在独立测试 Cone 中验证：局部导入、不泄漏、声明前后、嵌套恢复、alias／star、overload、type／variant、扩展与函数引用、lambda／局部函数捕获定义处绑定、泛型与跨 Cone 消费。必要的 compiler fixture 验证 public import、无效 selector、不可访问与同层歧义的诊断及位置。少量 parser／typed HIR／LSP 测试锁定语法、实际身份、格式化幂等和别名导航。按仓库要求先格式化／lint，再运行相关测试。

Darwin 验证通过：workspace 格式化与 lint；块级 import 的 7 项 HIR／编辑器测试、formatter 幂等与 analysis 折叠测试；既有 import 回归 94 项、named call 回归 23 项；`tests/cones/block-imports/consumer` 的 5 项 release 测试；`block-import-diagnostics` 正式 fixture 的 2 个进程、7 项错误位置与消息断言。跨 Cone 测试涵盖泛型特化、默认 lambda、局部类型、重载回退与扩展 invoke。Linux 随编译期 intrinsic 整组能力做平台组合验收。
