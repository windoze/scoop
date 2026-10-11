# M36 实施记录

开始日期：2026-10-10。基线：`d276dcf97`（M35 已验收），分支：`m36`。

## 实施计划

按完整生产／消费路径逐项提交；独立 parser 项提前，其余按设计批次推进。

| 项目 | 状态 | 实际变化与验证 |
| --- | --- | --- |
| R11 parser 修饰符 | 已完成 | 合并存在性与源码 span；parser 474 + doc 1，正式 CLI 4 项通过 |
| R12 literal pattern | 已完成 | 封闭字面量 payload；AST/parser 490、相关 HIR 135、正式 CLI 15 项通过 |
| R01 调用目标 | 已完成 | 完整调用分支、必填 receiver、完整去虚拟化改写；Rust 754、正式 CLI 8 项通过 |
| R04 固定元数 intrinsic | 已完成 | 完整一元／二元、固定形参方法引用、具名借用；Rust 1,246、正式 CLI 11 项通过 |
| R02 when | 已完成 | subject／纯条件分支与唯一覆盖类型来源；Rust 1,346、正式 CLI 11 项通过 |
| R03 MIR 临时态 | 已完成 | 最终 MIR 删除临时类别，私有 CFG 工作记录保留协程顺序；Rust 660、正式 CLI 8 项通过 |
| R07 return | 已完成 | HIR／MIR 与导出模板必填返回值；Rust 2,899、正式 CLI 8 项通过 |
| R05 窄身份 | 已完成 | FunctionTypeId 与 enum application 贯通生产／消费；Rust 710、正式 CLI 8 项通过 |
| R06 nominal 表示 | 已完成 | declared payload、intrinsic 类别与 C/Scoop 窄引用；Rust 1,399、正式 CLI 8 项通过 |
| R10 core protocol | 已完成 | 生产／导入端具名 typed 字段；Rust 387、正式 CLI 6 项、产物成员字节对比通过 |
| R08 native 结果发布 | 已完成 | 六类完整返回分支、单一间接结果存储；Rust 197、正式 CLI 5 项通过 |
| R09 codegen 调用 | 已完成 | 完整 callee 与调用准备分支；codegen 41、正式 CLI 5 项通过 |

## 验证与工作区约定

- 每批先 `cargo fmt --all`、`cargo clippy --workspace --all-targets`，再执行受影响 Rust 测试和正式 CLI fixture；已有用例覆盖不变语义，新用例锁定重构涉及的真实组合。
- 构建与测试的 `TMPDIR` 指向仓库 `target/m36-tmp`，不使用 `/tmp`。定期检查 `target` 大小，清理已验证的 fixture 工作区与过期增量文件，保留可复用依赖和验收报告。
- 本机针对性检查完成后，在 `ssh nuc12` 上从 `~/repos/scoop` 创建的独立工作树 `~/repos/scoop-m36-validation` 执行 Linux 完整 Rust／公共 runner／正式 CLI 验证；保留实际命令、计数与平台适用情况，不把快照更新或编译成功当作运行验收。
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

最终 Linux lint 发现 ELF metadata 测试的空 struct 构造器遗漏窄引用投影，已沿现有接口显式取得 definition。修正后两机 fmt／workspace clippy 通过，Linux codegen 378 项通过；该补充只迁移测试输入，不改变语言、阶段合同或产物。

## R01：完整调用目标与 receiver

structured MIR 与最终 MIR 的调用按 Direct、Virtual、Interface、Closure、FunctionBridge 保存完整目标；动态方法只接受本地、单态化或外部 Scoop 声明，函数签名身份不再重复。动态 receiver 必填并独立于普通实参，遍历与 liveness 始终先消费 receiver。CFG 在后续参数产生控制流时保存该值；去虚拟化一次构造完整 direct 调用，把已求值 receiver 放入首个逻辑实参。

方法引用保存尚未绑定 receiver 的具体目标，在加载闭包捕获后直接形成调用。接口投影、动态函数 bridge、callback 和协程协议适配沿用同一必填 receiver；实参数量与普通引用、slot、ABI 检查保留。协程临时调用当前合并为完整分支，其移出最终 MIR 的工作由 R03 单独完成。

LIR 调用主体为 214 行，动态调用 157 行、native 37 行、runtime 103 行；方法引用为 322 行、目标描述 159 行，MIR 调用 dump 独立为 92 行。本机 fmt／workspace clippy 成功；MIR 345、MIR lower 118、LIR lower 153、HIR lower 138，共 754 项相关 Rust 测试通过。两个沿用旧 receiver 索引的测试断言修正后，只复跑这两项。

新增两个正式 CLI 用例覆盖五种调用、receiver 与参数副作用、Nothing、泛型方法引用、动态 suspend 函数转换、finally，以及在实参挂起期间替换外部 receiver 后恢复。debug/release、正常／移动／minor GC 均已运行通过。已审阅的旧 MIR golden 仅同步 R07 完整 Unit 返回，旧 LIR 保持不变。

正式普通比较 8/8 通过，共 11 个配置、39 个进程、44 个 golden；还覆盖既有方法引用／方差／挂起函数值、callback、异常调用、精确诊断，以及跨库泛型方法引用的发布、删源消费与运行。报告为 target/m36-validation/r01-verify-report.json；旧 MIR 的全部差异已确认仅增加 Unit 返回值，旧 LIR 无变化。wire 与 runtime ABI 不变。本轮清理未使用的 incremental 缓存 2,689.5 MiB，保留当前工具和 fixture 缓存，明细见 r01-cleanup.log。


## R03：最终 MIR 与协程私有工作数据

最终 MIR 删除未被生产的 Cast、临时协程调用目标与 call 上的 pending 字段。语言 cast 已由现有私有正文转换展开；协程注册在 structured MIR 和 CFG 工作记录中保存完整协议方法、receiver、插入位置与结果 local，不借用假 callee 或空语句。liveness 将普通语句与私有挂起操作按语义顺序合并，保留已经分析的完整调用供状态机生成使用，最终输出仅含真实接口调用。

pending cleanup 只在 lowerer 保存稳定 local 与具体目标；生成 frame 时直接映射已保存的 slot。删除最终 MIR 的恒定 Root 验证和只构造该非法临时态的测试，保留真实 frame、signature、slot、引用、ABI 与 GC 检查。私有工作记录 30 行、cleanup 数据 27 行，liveness 主体 254 行，遍历 306 行、合并 69 行，没有复制完整 MIR 或新增跨阶段状态框架。

本机 fmt／workspace clippy 成功；MIR 344、MIR lower 118、LIR lower 153、HIR lower 的协程／suspend／Context／cast 45，共 660 项相关 Rust 测试通过。合并迭代器收尾后只复验 MIR lower 118 项。新连续挂起与嵌套 cleanup fixture 先由 R01 CLI 生成基线，再用 R03 普通比较；debug/release、正常／移动／minor GC 的运行及 HIR/MIR/LIR 均保持一致，覆盖同步／异步恢复、异常替换待定返回、泛型结果、Context 与再次挂起。

正式普通比较合计 8/8，共 14 个配置、49 个进程、53 个 golden 通过，还包含动态 receiver 在实参挂起期间被替换、旧 pending transfer chain、catch/finally、同步恢复、Context exit，以及 M35 跨库发布、删除源码后的产物消费和链接。旧快照仅同步 R05 的 enum application 身份、R06 的 declared payload 和 R07 的完整 Unit 返回；旧 LIR 无变化。wire 与 runtime ABI 不变。

合并验收报告为 target/m36-validation/r03-verify-report.json，基线、Rust、快照审阅与分批报告同目录保留。收尾清理已完成的 fixture cases，保留当前 CLI、库、sysroot 与编译缓存，明细见 r03-cleanup.log。

## R02：完整 when 与覆盖信息

Export／LocalConcrete HIR 及已绑定默认模板区分 Subject 与 Conditions。纯条件 arm 不允许 case，fallback 的覆盖类型使用 Infallible 排除；有 subject 的覆盖只记录类别，类型来自同一个 subject。共享 arm 保存 guard、正文与位置，遍历提供只读借用视图；必要的局部可变遍历只修改相应分支内部字段。前端保留 subject 单次快照、条件与 guard 短路、模式绑定、smart cast、诊断顺序、Nothing 与合法落空。

wire 沿用已有可选 subject、条件与覆盖标签；原始解码数据在绑定时一次形成完整结构。旧覆盖类型须与真实 subject 一致后才丢弃重复字段；新增三组解码测试覆盖无 subject 的 case／覆盖和覆盖类型冲突。默认模板引用仍按原站点与顺序遍历，具体化、MIR lower 和导入消费不再重新分类 subject 或解包不可能的 case。

按职责拆出 when 数据、主条件、具体化、模板投影／绑定与 MIR 条件模块；新增模块最长 276 行。本机 fmt／workspace clippy 无警告通过，HIR 888、MIR lower 118、前端 when／模式／默认参数／enum／Context 专项 340，共 1,346 项 Rust 测试通过。

新增独立与挂起／泛型／闭包／Context／finally 组合 fixture 先用 R03 CLI 生成基线。R02 的四份 MIR、四份 LIR 与基线逐字节一致；HIR 只改变完整分支表示。正式普通比较 11/11 通过，共 14 个配置、44 个进程、40 个 golden，包含 M35 when 的 effects、subject、smart cast、挂起、跨库发布与删除源码后消费，以及四项精确诊断负例。debug/release、正常／移动／minor GC 均通过；旧 MIR 差异仅同步 R07 的 Unit 返回值，旧 LIR 不变。

报告为 target/m36-validation/r02-verify-report.json，快照审阅记录为 r02-golden-review.log。wire 版本与 runtime ABI 不变。清理已完成的 fixture cases，保留当前 CLI、共享 sysroot、缓存及验收报告；明细见 r02-cleanup.log。

旧快照统一验收发现，穷尽 enum when 的默认参数遗漏了 when 定义位置的类型引用，导致三层跨库消费失败。引用收集现从完整 subject 取得类型，在原有 when 位置记录该引用；覆盖信息仍不重复保存类型。新增真实源码测试覆盖 enum、Boolean 和通配模式的默认参数，相关 Rust 29 项通过。现有 imported-enum-patterns、M35 when 产物与两个 M36 when 用例普通复验 4/4 通过，共 7 个配置、36 个进程、36 个 golden；修复前后的 consumer MIR／LIR 逐字节一致。报告和快照审阅为 target/m36-validation/final-r02-reference-verify-report.json 与 final-r02-reference-golden-review.log。

## R04：完整固定元数操作与数据借用

整数表达式的一元／二元分支分别使用窄操作类别和必填操作数，div/rem 只能携带 lhs/rhs。Export HIR、LocalConcrete HIR 与绑定默认模板共用这一完整 payload；遍历、效果分析、默认参数实例化、具体化与 MIR 消费不再重新组合独立的操作和参数形态。wire 保留原有标签与顺序，在实际解码绑定处检查元数后构造完整表达式；18 种操作的字节往返和三种非法元数组合均有测试。

方法引用根据已决议的一元／二元／MaybeUninit 操作直接建立所需形参，再调用固定参数的 lowering 接口。数据借用在具体化边界从实例逻辑形参取得对象类别及 FunctionTypeId，形成具名 object／block；无正文源码 intrinsic 复用已保存的 source parameter interface，导入实例使用模板签名。MIR 不再从可能为 Nothing 的实参类型猜测对象或回调类别，也不解包任意参数列表。整数、借用与回调的有序求值、终止传播、异常清理和 GC 保活保持原有规则。

新增整数模型 232 行，借用数据 21 行、具体化 94 行，方法引用分类 90 行、固定参数操作 94 行；整数 lowering 缩至 320 行。既有长文件随迁移缩减，没有复制整套 IR 或增加通用状态框架。本机 fmt／workspace clippy 无警告通过，HIR 890、MIR lower 118、前端整数／浮点／字符／引用／默认参数／借用专项 238，共 1,246 项不同的 Rust 测试通过。

两个新 CLI 用例覆盖运算副作用与 Nothing、除零、方法引用、泛型 when、Array／MutableArray／String 借用、回调异常清理和 GC。四份新 MIR、四份新 LIR 与 R02 基线逐字节一致。正式普通比较 11/11 通过，共 16 个配置、64 个进程、54 个 golden，包含源码 intrinsic 的 core 构建、跨库泛型与默认参数、删源消费和链接、debug/release、正常／移动／minor GC，以及参数元数、unsafe、回调类别与 GC 效果的精确诊断负例。

旧 HIR 同步完整表达式和先前表示变化；旧 MIR 除完整 Unit 返回外，移除三次无用的 Unit 借用结果读取，异常边不变。两份 release LIR 的差异仅为内联完整返回产生的七条空 Unit aggregate 及临时值重编号。wire 版本与 runtime ABI 不变。

一项复验遇到上轮只读产物残留，清理用例目录后单独复验通过；合并报告为 target/m36-validation/r04-verify-report.json，原始报告和 r04-golden-review.log 同目录保存。本轮清理约 410 MiB 的已完成 fixture cases 和 104 MiB 的过期测试可执行文件，保留共享缓存；最终批量验证使用优化构建的配套 CLI。本机公共 fixture runner 44 项亦已通过。

## 本机最终阶段回归（2026-10-11）

12 项功能完成后，使用最终 release 的三个正式 CLI 分批更新旧阶段快照，避免每项功能都重跑这一范围。共选择 1,237 项：1,236 项通过、1 项按 target 不适用；按每项最终结果去重，包含 1,450 个配置、9,646 个进程和 11,519 次阶段／计划快照检查。第二批暴露的默认参数 when 引用问题已在 `4453ce90b` 修复，并以四项普通比较闭合；保留初始失败和后续来源，不把它计作直接通过。实际结果见 [阶段回归](validation/darwin-snapshots.json)、[初始失败](validation/darwin-snapshots-initial-failure.json) 和 [引用回归](validation/default-reference-regression.json)。

迁移 2,605 份 HIR、2,157 份 MIR 和 229 份 LIR 快照。LIR 逐份审阅：200 份只有无用零大小值物化和临时编号变化；18 份进一步删除 Unit 字段提取；4 份中的六个函数在删除该提取后不再使用 aggregate 参数，入口 poll 的根集合相应为空；5 份删除无用的 MaybeUninit<Unit> 结果 local。另两份协程转储分别删除无用 Unit 保存槽，或删除多余保存／恢复及空跳转。一个私有 lambda frame 从 88 缩为 80 字节，字段与 scan 偏移同步调整；调用、异常及 finally 路径保持。逐份归类见 [LIR 审阅](validation/darwin-lir-review.json)。

随后关闭快照更新，普通比较 231/231 项通过：337 个配置、2,486 个进程、2,311 次阶段／计划快照检查。范围包括全部 26 个 M36 新用例、实际 LIR 变化与必要 Darwin 专项，见 [选择](validation/darwin-selection.json)、[结果](validation/darwin-final.json) 和 [实际命令／清理](validation/darwin-final-execution.json)。共享阶段的剩余范围交由 Linux 完整正式 CLI 再次比较，不在本机重复全部 1,237 项。公共 fixture runner 的 44 项测试通过。

新增 49 个 Rust 文件，最长 396 行；原有超过 500 行的文件没有增加超过 30 行，见 [长度检查](validation/file-lengths.json)。未新增依赖、TODO、todo! 或 unimplemented!。每批完成后清理用例工作目录，保留当前 CLI、共享 sysroot、缓存和报告；上述七批记录合计约 46.3 GB 的已清理逻辑文件大小，目录内可能存在硬链接，该数字不表示等量实际磁盘释放。

Linux 的完整 Rust 初跑已正常退出：43 个测试目标、5,474 项通过，0 failed、0 ignored，公共 runner 44 项通过。该初跑采用 R04 完成时的源码及 Linux 测试构造器修正；默认参数引用修复的相关 29 项已定向通过，其中新增一项真实源码测试，最终覆盖为 5,475 项。最终源码的 fmt／workspace clippy、三个 release CLI 构建均通过，详见 [Linux Rust 记录](validation/rust-linux.json)。GNU 完整正式 CLI 已按下节记录以普通全量和失败补验完成；musl 受影响范围继续验证。

## GNU fixture 可移植性修正

GNU 目标快照初跑 176 项，175 项通过；新 C 布局 fixture 刻意将 8 字节对齐的内部类型放入 pack(2) 的外层结构，GCC 的 packed-not-aligned 警告被 -Werror 拦下。沿用已有 M23 C ABI 用例的局部 GCC diagnostic push/pop，仅包围这一刻意低对齐声明，保留布局和其他警告检查。C 源码先经 clang-format，并在 Clang／GCC 以 -Wall -Wextra -Werror 执行语法检查。

目标审阅同时确认 call-targets/suspend、intrinsics/operands、intrinsics/borrows、when/flow 的八份 debug/release LIR 含平台 ABI 差异，改用 program.${profile}.${target}.lir.txt。既有 Darwin 输出逐字节保留；新增八份对应 GNU 输出及 C 布局的 GNU LIR。16 字节 enum 的物理 carrier、C byval 和含非自然对齐字段的 aggregate 传参／返回按实际 target 记录，HIR／MIR 保持共享。

两平台分别以普通比较复验这五项：均为 5 项通过、9 个配置、38 个进程、27 次阶段快照检查；debug／release、正常／moving／minor GC 断言均保留。修正不涉及 Rust 源码或 ABI 规则，沿用已有 Rust 验证和配套 release CLI。实际初跑与补验见 [GNU 初跑](validation/gnu-snapshots-initial.json) 和 [可移植性记录](validation/target-fixture-portability.json)。其余 GNU 输出继续进入正式 --all 普通验收。

## GNU 正式全量与失败补验

一次正式普通 `--all` 选择全部 3,091 项，初次为 2,855 项通过、206 项失败、30 项不适用。失败分别为 192 项阶段快照迁移、12 项 core 构建超过 300 秒、1 项 native 构建超过 300 秒，以及 1 项修饰符诊断位置预期。保留 [初次摘要](validation/gnu-all-initial.json) 和 [完整原始报告](validation/gnu-all-initial-report.json.gz)，没有将初次失败改写为通过。

审阅现有编译转储后更新预期，以 6 并发对这 206 项执行普通比较，205 项通过；嵌套 enum 默认参数继续执行到第三层消费时又遇到一份旧 LIR 预期，核对后仅复验该项，通过全部 12 个进程和 14 次阶段／计划快照检查。13 项原超时均在原有时限内通过，没有修改超时或断言。实际来源见 [首轮补验](validation/gnu-recheck-initial.json)、[嵌套 enum 补验](validation/gnu-nested-enum-recheck.json) 和 [执行记录](validation/gnu-rechecks-execution.json)。

按每项最后的普通结果合并，GNU 为 3,061 项通过、30 项不适用，覆盖 3,305 个配置、14,478 个进程和 12,256 次阶段／计划快照检查；全部 26 个 M36 新用例通过。此结果来自正式全量加定向补验，不是一次全绿运行，见 [最终覆盖及逐项来源](validation/gnu-final.json)。

本轮迁移 449 份阶段快照：173 份 HIR、102 份 MIR、174 份 LIR，补齐此前命名选集未包含的旧 CLI 转储和 GNU 专用输出。HIR 对应完整整数操作、declared、when 与 Return；bare return 新增真实 Unit 和来源记录，生成包装函数直接返回已有 Unit 调用。MIR 只增加完整 Unit 返回值。169 份先前生成的 GNU LIR 已逐份分类，见 [LIR 审阅](validation/gnu-lir-review.json)；补充的五份 LIR 中三份仅有未使用的零大小值，两份是嵌套 enum 的局部变量顺序变化。对后两份逐一映射定义、使用、GC root 与生成的调试名称后，整份文件与新输出完全一致，见 [阶段审阅与局部编号映射](validation/gnu-stage-review.json)。

另将 `override release` 的错误区间从 `18..18` 更新为完整关键字 `18..26`，符合 R11 已实现的准确修饰符位置；错误代码、消息和严重程度保持原样。本机额外普通复验三项旧 CLI 输入／缓存用例、一项修饰符负例和一项嵌套 enum 产物用例，均通过，见 [Darwin 补充结果](validation/darwin-supplement.json)。以上修改仅涉及 fixture 预期和记录，沿用已通过 fmt／clippy 的 Rust 源码与 release CLI。

失败目录在相应普通补验通过后清理，未完成项的现场一直保留到通过。期间清理 122 个已完成补验的旧目录，远端 target 实际占用由约 26.3 GB 降至 21.1 GB；逻辑文件大小与实际占用分别记录。Linux 原工作目录的 21 个既有修改文件仍与初始记录一致。
