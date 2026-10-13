# M41 实施记录

基线：`9e715a3c`，M39 的验收、提交及清理已完成。分支为 `codex/m41`。

## 当前状态

- 已读取设计、AGENTS.md 及三份权威规范，按功能细化合同。
- Add 已实现：八种整数、Float、Double、String 显式提供 `Add<Owner, Owner>`；具体化后的已知运算保持 intrinsic，实际接口调用使用既有装箱与 NoGC/Managed slot 适配。
- LocalConcrete intrinsic 保存完整物理参数类型；实际分派入口复用 callable materialization 和 primitive lowering，生产、导入两侧按声明的 slot/dispatch selection 发布与选择入口，没有新增 runtime ABI 或产物 wire tag。
- 跨库扩展正文的隐式 struct 字段读取复用字段身份，避免选择没有函数体的 Storage getter。
- Add 功能提交：`45339fb3`。
- 算术、一元、自增减、Compare、整数区间、读写、调用、component 与复合赋值能力已实现；Equals 和浮点默认比较迁移已完成，本机定向验证通过，正在执行 Linux GNU/musl 完整验收。
- IEEE 四态 API 定名为 IeeeComparison / ieeeCompare。
- 临时目录和日志使用 `target/m41/`，测试命令设置 TMPDIR；保留 M39 编译依赖以避免重复构建。

## 验证安排

每批先 fmt、workspace clippy，再针对改动执行 Rust 与正式 CLI fixture；每个完成的功能单独提交。
最后在 nuc12 的 `~/repos/scoop` 执行 Linux 完整适用范围验收，按成功完成状态清理 fixture 工作目录。
尚未执行的测试不计为通过，不使用批量快照更新代替诊断核对。

## Add 本机验证

- `cargo fmt --all`、`cargo clippy --workspace --all-targets` 通过；三个正式 CLI 的 release binary 构建通过。
- 新增 typed IR 测试 2 项通过，覆盖 11 个 intrinsic 的完整签名、GC effect、可发布入口及具体 Int bound 调用保留直接运算。
- HIR callable record 16 项、nominal callable classifier 10 项、原 intrinsic 测试 8 项、MIR lowering 118 项通过。
- `tests/fixtures/m41-add` 全部 11 项通过：15 个变体、43 次进程执行。包括 debug/release、异构结果、多 bound、alias、扩展边界、仅产物链接及正常/moving/minor GC 模式；7 个负例逐项核对位置与诊断。
- fixture runner 公共规则 47 项通过。
- core bootstrap 和跨库 shape 产物结构测试各 1 项通过；既有 M33 接口效果回归 3 项通过（5 个变体、21 次进程执行）。
- 本批定期删除早期失败 fixture 的 cases/cache/sysroot 与诊断采集工作目录，保留报告和日志；未使用 `/tmp`。

## 算术、一元、比较和区间能力

- 按语言规范 9.3.6 增加 Sub/Mul/Div/Rem、UnaryPlus/UnaryMinus/Not、Increment/Decrement、Compare、RangeTo/RangeUntil/Contains，共 13 项普通接口。core 明确实现已有对应操作；浮点 Compare 随下一批全序比较迁移。
- Increment/Decrement 使用普通递归上界。声明阶段先补齐名义继承关系、检查继承环，再检查实际出现的上界要求；参数约束组完成后再检验其中的递归引用。方法 operator 返回类型验证保留参数作用域，没有降低返回子类型要求。
- 完整 HIR lowering 测试 1,427 项通过，含新增的声明顺序、泛型方法/函数作用域、局部类型与错误上界回归；core 102 个实际 intrinsic 接口入口的签名、GC effect 和 MIR 完整性断言通过。
- 本机 `m41-*` 正式 CLI 共 23 项全部通过：34 个变体、110 次进程执行。新增 12 项覆盖十种数值类型、String/Char 比较、四种 range、实际接口分派、异常、写回副作用、三层库模板再次发布和 5 个准确负例；同时复验 Add。
- fmt、workspace clippy、release CLI 构建和 core bootstrap 结构检查通过。复验复用已完成的 core/cache，避免重复构建两种 profile 的 core。
- 跨库 shape 产物检查通过；本批本机验证无未闭合失败。完成后清理旧 Add、效果回归和首轮算术 fixture 的工作产物，保留报告与当前可复用缓存。

## 索引、调用、解构与复合赋值能力

- 新增 Get/Set/Get2/Set2、Invoke0/Invoke/Invoke2、Component1/Component2 及五个复合赋值接口；使用普通固定参数签名。List、MutableList、MutableArray 和 String 显式提供相应实现，数组写入沿用普通方法包装存储 intrinsic 的既有分层。
- binding 的 component 路径接受 interface 与类型参数，复用成员和 bound 调用决议；struct/tuple 的直接解构仍投影字段。规范同步明确 `_` 位置也调用 componentN，不改变求值顺序。
- 新增 12 个正式 CLI fixture：七个正例覆盖单／二维下标、副作用、三种调用元数、接口与泛型解构、for、五类 op=、组合以及三层库再次发布和无源码链接；五个负例逐项核对歧义、结果类型和显式 conformance 诊断。
- 本机 fmt、workspace clippy、release CLI 构建、M41 typed IR 4 项、解构 Rust 回归 12 项、core bootstrap 结构检查通过。
- 全部 M41 共 35 项完成验证。首轮 34 项通过；唯一失败为新产物用例未公开构造函数，修正后与新增 struct/for 解构组合一同补验，两项全部通过（4 个变体、22 次进程执行）。正例均覆盖 debug/release 及正常、moving、minor GC。
- 清理旧 fixture cases/cache/sysroot、诊断采集产物及约 1.9 GiB 增量编译目录；保留报告、日志和最新 core 缓存。

## Equals 统一接口

- 三份 spec 与 core 将 Equality/equalTo 迁移为 Equals/operator equals，不保留兼容 alias 或转发方法。成员限定、名义 conformance、接口参数不变性和 NoGC/Managed slot 规则保留；结构比较不补齐接口实现。
- Unit 显式成员参与普通调用，不再同时生成同签名派生比较；默认参数和跨 Cone 消费保存真正的 core callable。原 23 份代表性 HIR 期望只变更接口名称与 operator 声明两行，逐一与实际差异核对；Unit 的相关结构断言改为普通依赖入口。
- 迁移真实程序与负例，包括两种旧 API 删除诊断、派生／手写成员不产生 conformance、继承接口不变性、NoGC、指针、原子引用、整型枚举和同名用户接口。新 M41 用例覆盖 Equals 与 Add/Hash、接口视图、方法引用、装箱和泛型运算符的组合。
- 重建 core 的夹具同步实际声明片段与 import。226 份共享 Long 声明替换规则仅调整匹配片段，保留原用例行为；重定位 Int、String 和继承能力的代表用例已验证。
- fmt、workspace clippy、release CLI 构建通过。完整 HIR lowering 1,427 项与 core bootstrap 结构检查通过，包含 core 113 个 intrinsic 接口入口的精确签名和 MIR 验证。
- 正式 CLI 选择 79 项，全部完成验证：首轮 69 项通过；9 项预载旧诊断，以及重定位 String 夹具漏保留测试成员，在核对和修正后补验 10 项全部通过（10 个变体、18 次进程执行）。新增／原相等正例覆盖 debug/release、正常/moving/minor GC 与仅产物链接；Unit 跨库默认参数及重建 core 后的链接均通过。
- 浮点默认比较仍待下一批迁移。受公共接口增加影响的优化／ABI 专用 LIR 期望与跨平台完整验收，在浮点批次完成后统一处理；不以这批定向验证替代最终验收。
- 清理前两轮缓存、case、sysroot 与诊断采集产物，保留报告、日志和最新可复用缓存。

## 浮点默认比较与显式 IEEE API

- 三份 spec 同步默认等价类全序：正负零相等，全部 NaN 相等且排在正无穷之后。Float/Double 显式实现 Compare，compareTo 返回 -1/0/1；const、普通运算、when 值匹配与结构派生共用同一语义。
- 新增普通 IeeeComparison enum 与 NoGC ieeeCompare 方法，以分类和默认比较组合四态结果。移除 isTotallyOrdered、totalOrder intrinsic 及位型排序辅助实现；不改变算术、舍入、转换、存储或 FFI。
- FloatBinaryOperator 的旧 IEEE 比较与 totalOrder tags 6～12 退役，新默认比较与 compareTo 使用 13～19。解码明确拒绝旧语义，raw bits 保存规则不变；不增加 runtime ABI 或兼容入口。
- 普通 core 浮点成员参与 literal receiver 的精度探测，按实际 nominal owner 判断；源 core 与依赖 core 各有验证，扩展仍不能借此改变字面量推导。同步 M30/M33 历史说明与旧 NaN 检测、派生比较期望。
- M41 浮点共 10 个正式 fixture（其中两项迁移自 M30）：默认六种关系与 compareTo 的两种精度、18 种特殊位型交叉矩阵；独立 IEEE 四态、常量、generic/alias/boxing/when/结构组合、三层仅产物消费，以及五个准确负例。正例覆盖 debug/release 与 GC 组合；矩阵另检查 signaling NaN 比较后位型不变。
- 本机 fmt、workspace clippy、release CLI 构建通过。HIR lowering 原 1,427 项与新增源 core 字面量用例通过；3 项浮点 wire 测试及 core bootstrap 结构检查通过。实际 intrinsic 接口入口增加至 115，参数、GC effect 与 MIR 完整性检查通过。
- 正式 CLI 首轮选择 99 项：95 通过、1 项 musl 专属不适用；旧 compareTo 缺失负例及两个新夹具源码错误在迁移后闭合。补验 9 项全部通过（14 个变体、56 次进程），包含 7 项结构派生回归；精度混用的新 compareTo 负例也通过正式 runner。最终适用用例均已闭合，不把新语义下成功编译的旧负例继续保留为错误规则。
- 本批仅完成本机功能验证；优化/ABI 专用 LIR 期望审阅及 Linux GNU/musl 完整验收仍待下一批。清理旧 cases/cache/sysroot 和诊断采集产物，保留报告与当前可复用缓存。

## 优化与 ABI 期望迁移：本机

- 审阅并更新 M31 SSA roots 与 M34 small-values 的 8 份本机 LIR 期望，保留专用 ABI、GC 与优化合同断言。变化来自新增的名义运算符接口、Equals 身份与其可达类型支持；逐份比较函数体，除 external-fn/external-td/td 引用编号变化外，指令和签名没有差异。
- 更新轮 4 项通过（含新 compareTo 精度混用负例），8 个变体、39 次进程、9 个 golden 检查；随后普通模式复验 3 项全部通过，7 个变体、38 次进程、9 个 golden 检查。Linux 专属的 12 份 LIR 期望留待对应目标实际生成和审阅，不从 Darwin 推测。

## Linux 验证中的既有测试修正

- Linux 全量入口发现 child runner 的旧测试仍要求 GNU 和 musl 都设置 REALGCC，与 M39 已交付的 musl wrapper 专用规则矛盾。修正测试：GNU 验证变量未设置，musl 验证实际 GCC 可执行；继续验证 PATH、私有 scratch 位置及结束后的清理。生产实现和语言行为未改。
- 两台宿主的 fmt / workspace clippy 通过；Linux scoop 单元测试 100 项、依赖边界集成测试 3 项和实际产物持有集成测试 1 项全部通过。其余 workspace 使用 `--exclude scoop --no-fail-fast` 续跑，避免重跑该包已完成的测试。
- default callable 产物测试原先以 Unit 比较覆盖 DerivedEquality。Unit 已改为普通 Equals 成员，原输入不再产生这一引用；保留 Unit 用例，并增加具体 tuple 的默认参数比较，继续检查全部七类 callable 引用。本机 fmt、workspace clippy 和该完整产物读写用例通过；Linux 仅补验受影响用例。
