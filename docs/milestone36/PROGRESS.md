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
| R07 return | 已完成 | HIR／MIR 与导出模板必填返回值；Rust 2,899、正式 CLI 8 项通过 |
| R05 窄身份 | 已完成 | FunctionTypeId 与 enum application 贯通生产／消费；Rust 710、正式 CLI 8 项通过 |
| R06 nominal 表示 | 已完成 | declared payload、intrinsic 类别与 C/Scoop 窄引用；Rust 1,399、正式 CLI 8 项通过 |
| R10 core protocol | 已完成 | 生产／导入端具名 typed 字段；Rust 387、正式 CLI 6 项、产物成员字节对比通过 |
| R08 native 结果发布 | 已完成 | 六类完整返回分支、单一间接结果存储；Rust 197、正式 CLI 5 项通过 |
| R09 codegen 调用 | 已完成 | 完整 callee 与调用准备分支；codegen 41、正式 CLI 5 项通过 |

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

## R05：函数签名与 enum 的窄身份

函数签名驻留与替换直接返回 `FunctionTypeId`，普通类型边界从签名取 `canonical_type`。`EnumVariantApplication.owner` 使用 `EnumApplicationId`，查询、泛型替换、具体化、默认模板、派生编解码与相等、导入常量及模式消费均保留这一类别，variant／field 的 persistent identity 不变。导入格式、引用归属与替换完整性仍在原有边界检查。

本机 fmt／workspace clippy 成功；选择 enum、function、callback、default、M11、M12、M14、类型别名、M29、Option、variant 的相关测试，HIR 250、HIR lower 443、MIR lower 17，共 710 项通过。正式三个 CLI 已重建。新增函数值／函数指针 fixture 与泛型 enum／默认值／闭包组合 fixture，HIR/MIR golden 生成并审阅后以普通比较复验。

正式普通比较共 8 项、11 个配置、44 个进程、56 个 golden 通过，包含旧 FFI 指针、类型别名、派生相等、导入 enum 模式，以及 M35 trailing／组合产物的 debug/release 交叉构建、删除源码后的产物消费与重新链接、正常／移动／minor GC 运行。既有 golden 无变化，wire 和 runtime ABI 不变。报告保存在 `target/m36-validation/r05-regression-report.json` 与 `r05-verify-report.json`。验证完成后清理约 212 MiB 的 fixture cases，保留共用 sysroot 与缓存。

## R09：codegen 完整调用准备

`PreparedCallee` 的 Direct 分支持有 LLVM function，Dispatch 分支持有 table／slot；普通 call、native call 和 invoke 均消费这一完整目标，不再要求独立的 function Option。`CallPreparation` 区分 managed live set、已准备的 native transition 与无需准备的调用。native 分支从同一协议取得 safepoint 和 roots，在入口握手处消费 safepoint，随后一起保留 roots、transition、frame 与可能存在的结果存储；退出及 reload 不再通过多个相关 Option 恢复关系。同步明确 spec：已消费且后续不用的状态不为满足形式要求而冗余保存。

调用发射文件从 473 行降至 305 行；参数处理、callee 准备及 transition 准备分别放入 51、109、142 行的职责模块。LLVM builder／实际发射失败仍返回既有错误。

本机 fmt／workspace clippy 成功，codegen statepoint／invoke／GCLeaf／native／callback／C ABI 相关测试 40 项及 aggregate ABI 发射测试 1 项通过。新增 invoke fixture 覆盖 local／virtual／interface／closure 与 try/catch/finally；native 组合覆盖 native-safe／GCLeaf／native-borrowed、调用内 GC、返回后读取参数和结果，以及移动／minor GC。

正式普通比较共 5 项、7 个配置、32 个进程、22 个 golden 通过，包括已有 suspend 函数值、Scoop ABI 直接／间接含引用结果、并发 callback/native-safe transition 用例。新 HIR/MIR golden 经生成、审阅后普通复验，旧 golden 保持一致。报告为 `target/m36-validation/r09-verify-report.json`，wire 与 runtime ABI 未改变。

## R08：NativeBorrowed 结果发布

LIR 的 native borrowed 调用直接分为 Void、ElidedZst、DirectGcFree、DirectRooted、IndirectResultGcFree、IndirectResultRooted 六类，目标 ID 与返回类别一致。rooted 分支持有非空 scan；间接返回只保存一个 storage，调用与根发布从同一字段投影。删除独立结果描述、宽调用组合及其一致性断言。lowerer 在真实签名边界构造完整分支，共用的 call view 保持后端发射方式不变。

本机 fmt／workspace clippy 成功；codegen 112、LIR 61、LIR lower 24，共 197 项相关测试通过。新增两项 CLI fixture 覆盖六种结果、void／ZST 副作用、泛型闭包、Option、finally、结果内部引用及 native 调用中的 GC；新 HIR/MIR/LIR golden 经生成和审阅后以普通比较复验。

正式普通比较共 5 项、7 个配置、38 个进程、22 个 golden 通过，包含已有接口 native 调用和直接／间接移动根回归；正常、移动和 minor GC 均运行成功。报告为 `target/m36-validation/r08-new-report.json` 与 `r08-verify-report.json`。本轮清理已完成测试的五个旧可执行文件，释放约 262 MiB，保留当前 CLI、库及共享 sysroot。wire 与 runtime ABI 未改变。

## R10：core protocol 的具名字段

生产和导入端的八类 protocol 直接保存具名角色，使用对应的 nominal、callable、variant、field、slot、exact type 身份。移除语义数据中的通用 entry 数组和按位置解包 helper；整数类别直接映射固定字段。wire 仍使用原有 tag、字段编号和角色顺序，真实解码边界检查类别及引用后直接构造完整字段。owner、签名、dispatch 和成员关系检查接收相应字段，保留必要的普通引用合法性检查。

按职责拆分生产、wire 和关系检查；新增模块均少于 400 行。测试构造器直接返回所构造的身份类别，删除重复的测试 owner 推导。增加 wire 角色类别不匹配的解码负例，不为已知字段新增运行期认证或重复分类。

本机 fmt／workspace clippy 成功，HIR 68、HIR lower 288、slib 31，共 387 项相关测试通过。新增基础 protocol 与协程／迭代／Option／finally 组合 fixture，先用改动前的 CLI 生成并审阅 HIR/MIR golden，再用新实现普通复验，正常、移动及 minor GC 均通过。新编译的 core 和两个用例共 3,265 个非 manifest archive 成员，与基线逐字节相同；包含 metadata 和机器码成员，section 版本与 runtime ABI 不变。

正式普通比较共 6 项、9 个配置、44 个进程、42 个 golden 通过，还包括导入 enum 模式、M35 组合产物的 debug/release 与删源后消费／链接、native-safe callback transition、缺省参数及协程中的源码位置。报告为 `r10-new-report.json`、`r10-regression-report.json`、`r10-locations-report.json`，字节比较记录为 `r10-byte-comparison.log`，均位于 `target/m36-validation`。本轮清理约 75 MiB 的已完成 fixture cases，保留共享缓存。

## R07：完整语义返回值

Export HIR、LocalConcrete HIR、structured MIR、最终 MIR 和已绑定导出模板的 Return 均必填结果表达式。源码 bare return 在语义入口形成具有定义位置的 Unit 常量；泛型替换为 Unit 后仍保留原调用作为返回表达式。CFG 先求值，再保存必要的非 Unit 结果并执行 cleanup；Nothing 求值终止当前路径，不制造正常返回值。遍历、内联、协程完成包装与默认模板消费均沿用完整表达式。

wire 保留现有 tagged-map 编号与版本，旧 Absent 返回在绑定时转换为 Unit 常量，新增兼容测试验证结果类型与定义／求值位置。新产物使用已有 Present 分支。LIR 按 ABI 选择物理返回，对纯 Unit 常量无需生成无用指令；其他表达式照常求值，现有 LIR golden 保持不变。

本机 fmt／workspace clippy 成功。受影响的 HIR 886、HIR lower 1,397、MIR 345、MIR lower 118、LIR lower 153，共 2,899 项 Rust 测试已覆盖；调整旧结构断言和审阅过的 golden 后，仅重跑此前失败的 58 项，全部通过。新增独立与泛型／闭包／finally 组合 fixture，覆盖 bare／显式／表达式函数 Unit 返回、Nothing、finally 替换结果、正常／移动／minor GC。

正式普通比较共 8 项、10 个配置、29 个进程、28 个 golden 通过，包含现有返回路径、协程 finally 挂起恢复、M35 组合的 debug/release、删除源码后的产物消费与链接，以及 bare／错误类型／lambda return 精确诊断。报告为 `target/m36-validation/r07-verify-report.json`。本轮清理约 192 MiB 的已验证 fixture cases，保留共享缓存。其余正式 HIR/MIR golden 的表示变化在 M36 全部项目完成后统一更新并进行 Linux 全量验收，避免每批重复全量编译。

## R06：declared nominal 与表示专用接口

LocalConcrete HIR 与 MIR 的 declared struct／class 使用独立 payload；源字段查询、继承字段填充、模式投影、布局与 C ABI 消费直接使用已选分支。MIR intrinsic struct／class 进一步按实际允许类别拆分，删除后续布局中另一族的不可能分支。intrinsic 不再参与 declared 空字段填充；装箱直接沿用已记录的 payload，模式字段访问保留已选类型。

LIR 的 Scoop／C shell 分别返回窄引用，第二遍字段填充保留同一引用和已经计算的布局。C bridge 与 LLVM helper 接收完整 C 视图，遍历持有 CStructRef。字段边界、继承前缀、按值循环、布局溢出、ABI 和扫描检查保留，不建立新的 arena 或身份体系。

按职责拆出 nominal、字段引用和 C payload 模块：HIR nominal 为 461 行、引用 172 行，MIR nominal 217 行，MIR lower nominal 372／struct 147 行，LIR struct 存储 296 行，LLVM types 486／C payload 94 行。本机 fmt／workspace clippy 成功；HIR 131、HIR lower 289、MIR 345、MIR lower 118、LIR lower 153、codegen 363，共 1,399 项相关测试通过；最终字段填充收尾另复验 MIR lower 118 和字段损坏负例 1 项。

新增独立与组合 CLI 用例覆盖泛型字段、继承与 ZST、装箱、Float／Char、MaybeUninit、嵌套 packed/aligned C struct、typed pointer、callback，以及正常／移动／minor GC。正式普通比较共 8 项、9 个配置、30 个进程、28 个 golden 通过，包含 C 布局三项负例、已有 MaybeUninit debug/release 与跨库 source nominal 消费。选中旧 HIR/MIR golden 同步 R07 完整返回值；release LIR 的两条无存储 Unit aggregate 来自内联返回值的表示更新，布局和 ABI 不变。

报告为 target/m36-validation/r06-verify-report.json，格式化、lint、Rust、快照生成及普通比较日志同目录保留。完成后清理过期测试可执行文件及用例工作目录，保留当前 CLI、库、sysroot 与编译缓存；清理明细见 r06-cleanup.log。Linux 对应 target 的新 C LIR golden 在最终完整验收时生成并复验。
