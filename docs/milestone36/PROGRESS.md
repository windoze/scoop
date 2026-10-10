# M36 实施记录

开始日期：2026-10-10。基线：`d276dcf97`（M35 已验收），分支：`m36`。

## 实施计划

按完整生产／消费路径逐项提交；独立 parser 项提前，其余按设计批次推进。

| 项目 | 状态 | 实际变化与验证 |
| --- | --- | --- |
| R11 parser 修饰符 | 已完成 | 合并存在性与源码 span；parser 474 + doc 1，正式 CLI 4 项通过 |
| R12 literal pattern | 已完成 | 封闭字面量 payload；AST/parser 490、相关 HIR 135、正式 CLI 15 项通过 |
| R01 调用目标 | 待实施 | 完整目标与必填 receiver |
| R04 固定元数 intrinsic | 待实施 | 一元／二元、具名借用参数 |
| R02 when | 待实施 | 有／无 subject 的完整分支 |
| R03 MIR 临时态 | 待实施 | Cast、协程目标与 pending 留在 lowerer |
| R07 return | 待实施 | 完整语义结果，Unit 保留真实值 |
| R05 窄身份 | 待实施 | FunctionTypeId、enum application |
| R06 nominal 表示 | 待实施 | declared payload 与 C/Scoop 窄引用 |
| R10 core protocol | 待实施 | 具名 typed 字段，核对 wire 不变 |
| R08 native 结果发布 | 待实施 | 按返回约定构造调用与 root |
| R09 codegen 调用 | 待实施 | 完整 callee、native transition |

## 验证与工作区约定

- 每批先 `cargo fmt --all`、`cargo clippy --workspace --all-targets`，再执行受影响 Rust 测试和正式 CLI fixture；已有用例覆盖不变语义，新用例锁定重构涉及的真实组合。
- 构建与测试的 `TMPDIR` 指向仓库 `target/m36-tmp`，不使用 `/tmp`。定期检查 `target` 大小，清理已验证的 fixture 工作区与过期增量文件，保留可复用依赖和验收报告。
- 本机针对性检查完成后，在 `ssh nuc12` 的 `~/repos/scoop` 执行 Linux 完整 Rust／公共 runner／正式 CLI 验证；保留实际命令、计数与平台适用情况，不把快照更新或编译成功当作运行验收。
- 开始时 `target` 不存在；预先存在的 M37 设计稿保持原样，不纳入 M36 代码提交。

## R11：parser 修饰符

`suspend`、`override`、`const` 仅保存可选 span，method modality 保存取值与 span 的同一 payload。消费方直接匹配存在的修饰符，不再以 bool 判断后解包位置；原重复／互斥／声明位置检查仍由 parser 执行。顶层 `override` 属性及 nominal 错误现在指出完整修饰符，而非声明起点的空 span。

本机执行 `cargo fmt --all`、`cargo clippy --workspace --all-targets` 成功；`cargo test -p scoop-parser` 为 474 个单元测试 + 1 个文档测试通过。正式三个 CLI 已构建；`--suite tests/fixtures/m36-modifiers` 普通比较 4/4 通过，包括 const、abstract/final/override、接口与 suspend 组合运行和三项精确位置负例。新增 AST golden 先生成并审阅，再以未开启快照更新的普通流程复验。现有 AST/HIR/MIR/LIR 合同和 runtime ABI 未改变。

首轮 core 冷构建约 111 秒，复用同一缓存后约 26 秒。`target` 检查约 5.1 GiB，主要是当前 Rust 构建依赖，无过期 fixture 产物；后续批次复用 core/cache 并按需清理用例目录。

## R12：字面量模式

AST 使用 `LiteralPattern`，数值分别保存原始 literal 与负号，Char／String／Boolean／Unit 无任意表达式入口。dump 完整匹配该值域；HIR 沿原 literal typing 处理精确整数／浮点与相等规则。普通 when 的覆盖分析只提取已有合法 literal syntax；无符号负号仍按原 wrapping 规则归约，数值 token span 与外层括号／负号 span 均保留。

删除两个仅由手工非法 AST 触发的 HIR 测试，非法表达式和 unary plus 继续由 parser 源码负例及正式 negative fixture 验证。没有把原断言移动到生产包装构造器，也没有新增模式语法。

本机 fmt／workspace clippy 成功。AST 15、parser 474 + doc 1、相关 HIR 135 项通过；HIR 使用 `pattern m4:: m22_integer m35 m30` 选择。两个新 AST golden 已审阅，正式普通比较选择新 literal fixtures、全部 legacy M22 patterns、导入 enum 模式与 core 成员模式组合，共 15/15 通过，35 个进程、46 个 golden。HIR/MIR/LIR 既有快照保持一致，跨 Cone 构建／消费／链接／运行通过，wire 与 runtime ABI 不变。

实际报告分别保存在本地 `target/m36-validation/r12-new-report.json` 和 `r12-verify-report.json`；通过后清理共享 fixture 的 cases 工作目录，保留 sysroot、编译缓存与报告供后续批次复用。
