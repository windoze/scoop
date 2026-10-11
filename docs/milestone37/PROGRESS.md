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
