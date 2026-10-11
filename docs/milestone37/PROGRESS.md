# M37 实施记录

基线：`ec9d8849d`（M36，2026-10-11）。工作分支：`codex/m37`。既有未跟踪设计稿为本次实现范围，随首项功能纳入版本控制；本文件只记录实际完成和验证的工作。

## 实施与验证约定

每项功能先同步权威规范，完成 parser/HIR、必要后续阶段与产物路径，再运行独立、negative 和组合 fixture，完成后单独提交。遵守范围纪律，不引入授权、防伪、通用预算或假想扩展框架；按实际职责控制文件长度。

Rust 每批先 `cargo fmt --all`、`cargo clippy --workspace --all-targets`，再运行相关 Rust 测试并构建配套 CLI。本机使用 `--release` 构建配置，复用既有产物并避免与编辑器的 debug 测试枚举争用构建锁。本机定向验证通过后，在 `nuc12:~/repos/scoop` 完成 Linux 全量 Rust 与正式 CLI 验收，并验证 musl 受影响路径；用户已明确允许更新该目录。不重复执行无关全量。

构建及测试使用仓库 `target/m37-tmp` 作为 TMPDIR；日志、报告与临时脚本保存在 `target/m37-validation`，不使用 `/tmp`。定期清理已验证的 fixture 工作区和过期编译缓存，保留可复用依赖、工具及报告。开始时本机 target 约 20 GiB。

## 功能进度

### 1. 兼容修饰符与语言边界（完成）

规范固定五个修饰符完全忽略，明确禁止 interface 方法自身泛型、泛型虚方法及外层局部 var 捕获；runtime 契约和 wire 编码无需变化。

parser 在函数、property/accessor、值参数及类型参数的合法位置消费修饰符，保留语法诊断所需位置后丢弃，不增加 HIR/LLVM hint。支持混合/重复标记、非递归 tailrec、无 inline 的 reified、constructor/enum/annotation 参数及局部函数；同名普通参数、函数和 variant 继续合法。共享现有 vararg 解析，语法辅助模块 88 行，减少函数与 nominal 文件中的重复代码。

本机 fmt、workspace clippy（release）通过；parser 479 项与 doc-test 1 项通过。12 个正式 CLI fixture 普通比较全部通过，15 个配置、23 个进程、18 个 HIR/MIR/LIR golden；包括 debug/release、接口宿主泛型、final 泛型方法、显式引用对象状态、异常/finally 与 moving GC。9 个 negative fixture 固定真实诊断位置和完整信息。快照已检查普通分派、初始化和捕获结构；release 的内联仍来自既有优化器，与兼容标记无关。

普通报告见 [01-modifiers-darwin.json](validation/01-modifiers-darwin.json)。生成快照不计入普通验证次数。完成后清理本批 fixture cases，保留共用缓存、sysroot 和报告供后续增量验证。

### 2. 单语句控制体与 else-if（完成）

if/else/while/for 使用同一个控制体入口，单语句形成保留源码位置的普通 block，沿用现有 HIR 作用域和 MIR 控制流。直接 else-if、最近 if 结合、else 前分号、无值 return 的 else 边界和 body 内尾随 lambda 均闭合；没有新增后端分支或 wire 形态。控制结构 parser 170 行，新测试文件 59 行。

本机 fmt、workspace clippy（release）通过；parser 482 项及 doc-test 1 项通过。8 个新增 CLI fixture 的普通比较全部通过，11 个配置、17 个进程、18 个阶段 golden，涵盖 debug/release、循环跳转、嵌套 if 值、Elvis return、异常/finally、lambda 与作用域错误；另有两个 M35 尾随 lambda 正式回归通过，2 个配置、6 个进程、8 个 golden。两条旧 parser 负例已按新规范替换为条件括号检查及已有正向覆盖。

报告见 [02-control-bodies-darwin.json](validation/02-control-bodies-darwin.json) 和 [02-regression-darwin.json](validation/02-regression-darwin.json)。已审阅嵌套 if 的正常值合流及原循环条件入口，原 M35 快照无需迁移。

### 3. Unicode 与反引号标识符（完成）

规范固定 Unicode XID 与非空反引号名称，以实际码点序列比较，不做规范化；标识符、package、注解路径、成员与跨 Cone 名称均保留原有分段。使用持续维护的 unicode-ident 1.0.26（核对 crates.io 更新日期为 2026-09-17），没有自建 Unicode 字符表。lexer 区分关键字与带引号 token，AST 只保存实际文本；持久化名称继续用原有 UTF-8 text 编码，已有合法实体身份不变。

注解改用已有非空 QualifiedNameSyntax，删除按点号恢复路径的逻辑；成员查找与 override 使用真实声明名称，限定 enum 构造沿用原有 MethodCall AST，移除重复的拼接名称构造入口。访问器使用已有类别区分，允许源码方法与访问器显示名称相同而不互相代替。修复 `$名称` 错误插值诊断扫描中的 UTF-8 字节边界，并在非法反引号名称中只报告首个非法字符。新 lexer 子模块 43 行、parser 名称测试 85 行；没有新增 runtime 契约或 wire 标签。

本机 workspace fmt/clippy（release）通过；parser 486 项、identity 341 项、HIR 890 项、HIR lower 1,398 项及 2 个 doc-test 通过；提交前另补验限定 enum variant 与同名顶层 Array 构造器保持独立推导的回归。新增名称用例含 4 个正向 fixture（其中跨 Cone 产物链 1 个）和 9 个 negative fixture；普通 CLI 比较连同 2 个 M29 属性与静态产物回归共 15/15 通过，18 个配置、38 个进程、26 个 HIR/MIR/LIR golden。覆盖 debug/release、UTF-8 位置、码点不规范化、关键字/空格/点号/反斜杠名称、泛型与别名、接口/继承/引用、注解路径、重导出、仅产物消费与 moving GC。

报告见 [03-names-darwin.json](validation/03-names-darwin.json)。已清理之前完成的 fixture cases（52.1 MiB），并在确认没有 Rust 构建运行后清理闲置的 debug incremental（14.7 GiB）；保留 release 依赖与增量 fixture 缓存。Linux LLVM 22.1.2 的实际前缀已确认为 `/usr/lib/llvm-22`，最终验证将显式配置，避免系统默认 LLVM 21.1。
