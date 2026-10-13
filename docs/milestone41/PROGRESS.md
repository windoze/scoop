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
- default callable 的 Linux 定向补验通过。另一项 shared equality 测试仍把 Unit 计入派生导出；现在明确断言 Unit 不属于派生集合，并保留六个实际结构类型及 deferred/private/unused 分类的精确数量检查。本机 fmt、workspace clippy 与完整 standalone/combined 产物测试通过，Linux 定向补验安排在后续验证中。

## 优化与 ABI 期望迁移：Linux GNU

- 在 GNU 目标实际生成并审阅 M34 native 与跨库产物的 6 份 LIR 期望。新增接口 TD/layout 与 Equals 身份符合 M41 声明；逐份归一化引用编号后，函数指令与签名没有差异。
- 两个正式 fixture 通过：4 个变体、26 次进程执行、6 次 golden 检查。完整 CLI 验收仍使用普通比较模式。
- 与大型 Rust 测试并行时，两轮冷构建触及 fixture 原有 300 秒时限。保留失败记录，单独构建 debug/release core 并复用正常构建缓存后完成两项验证；没有修改测试时限。已清理结束用例的私有产物，保留报告与日志。

## 优化与 ABI 期望迁移：Linux musl

- 实际构建 musl 的 debug/release core 后，两项 ABI fixture 全部通过：4 个变体、26 次进程、6 次 golden 检查。6 份 LIR 的函数指令和签名在引用归一化后没有差异；元数据与引用变化逐份核对，与已审阅的 GNU 同类变化一致。
- 三个目标的优化与 ABI 期望迁移完成。GNU 全量以普通比较模式执行中；musl 全量随后使用已完成的 core 缓存。为避免资源竞争，CLI 开始阶段短暂让 Rust 测试进程让出 CPU，随后继续原测试并调整 CPU 分配；没有丢弃已完成测试或放宽时限。

## 元数据对象投影的冷构建开销

- GNU 完整验证的私有缓存用例再次触及 300 秒时限。一次本机实际 core 构建采样定位到后端元数据投影：每个对象都 clone 全部 LLVM 元数据，再删除其他定义；新增接口放大了这部分重复复制。
- 先补充实现规范 2.5 的 codegen 边界，再按已规划对象序列逐层拆分候选模块。继续复用原有投影、别名、TLS 和可见性处理；最终对象集合、顺序、布局与 ABI 保持。中间定义组按原有 patch selection 约定排序；没有添加资源预算、平台开关或新依赖。新增模块 74 行，供对象发射和 LLVM 文本输出两个实际入口共用。
- 本机 fmt、workspace clippy 与完整 codegen 367 项通过。实际 debug core 的一次冷构建从约 62 秒降到 12.96 秒，新旧 `.slib` 的 SHA-256 完全一致；[测量记录](validation/metadata-projection.json)保存计时方式，不能视作多轮基准统计。
- 正式 CLI 定向验证 10 项全部通过：16 个变体、119 次进程、6 次 golden。包含四项私有缓存／冷构建、M34 native 与跨库产物、M41 Add/access/floating 三层库及浮点特殊位型矩阵；没有更新任何期望。
- 旧 GNU 全量在 102 项通过、15 项冷构建超时后主动结束，另 3,668 项中断与 30 项不适用；该报告保留为诊断记录，不计为完成验收。新后端将重新覆盖 codegen/driver 和完整 CLI，其他未变 crate 已通过的测试继续复用。

### 浮点求值顺序组合的期望迁移

- Linux 全量发现旧求值顺序组合仍将 `Double.NaN >= 0.0` 的结果写为 false；按已经实现的默认全序改为 true。Nothing、异常、短路与副作用顺序断言保持完整。
- 本机格式化、workspace clippy 后，两个浮点 Nothing 用例通过：4 个 debug/release 变体、12 次进程执行，包含 moving GC。Linux 以定向补验闭合这项旧期望，不重复已完成的全量用例。

### Core 形状依赖精确清单

- `Int` 的 RangeTo/RangeUntil 实现使形状消费实际到达 `IntRange`，其 Contains、Iterable、Iterator 与 Option<Int> 必须进入普通名义应用清单。补齐四项具体期望，继续精确比较完整集合，未放宽产物或布局检查。
- 本机格式化、workspace clippy 后，`actual_core_sources_produce_closed_mir_and_lir_export_tables` 完整通过，包含 standalone/combined 消费、机器产物和错误产物检查。Linux 对这项已定位的旧清单进行定向补验。

### 原生存储归档成员清单

- 三目标实际生成的 native-storage provider 均由 66 个物理成员变为 65 个，与 Unit 不再物化派生比较一致。审阅实际 MIR，Wide 和指针相等、六个 native callback bridge 均保留；审阅时重建的 Darwin `.slib` 与正式 fixture 字节完全一致。
- 仅更新三份归档成员清单的一行；Darwin、GNU、musl 均在普通比较模式通过正式用例，每目标 1 个变体、10 次进程、1 次 golden，包含无源码独立链接与 moving GC。

### 异常返回路径的局部结构检查

- GNU 全量发现 M8 返回路径的旧整模块 MIR 快照仍记录 Equality 和展开的 core 元数据。逐段核对后，六个用户函数的 CFG 没有变化；差异只在无关 core 声明与打印函数引用编号。
- 删除 397 行整模块快照，改为六个用户函数完整 CFG 的逐字节片段检查，以 main 的函数边界结束；保留返回、异常清理、finally 返回覆盖及运行输出。未加入筛选脚本、IR 归一化或新的 fixture 机制。
- 本机格式化、workspace clippy 后，正式用例通过（1 变体、2 次进程）。Linux 正在使用启动时加载的旧声明，待旧用例完成后同步并定向复验，保留全量原始报告。

### 优化专测 MIR 期望审阅

- M31 局部值和 M34 内联／去虚化属于优化专测，保留 debug/release 的精确输出。实际更新 10 份 MIR：8 份函数正文逐字节不变；M31 两份的外部引用由 0～2 对应到 2～4，其余函数指令与引用关系不变。头部变化为真实的运算符接口及其可达类型。
- 本机格式化、workspace clippy 后，以普通比较模式复验五项正式 fixture：10 个变体、44 次进程、14 次 golden 全部通过。跨库源码隔离、独立链接和 GC 模式继续检查。
- Linux Rust 完整覆盖已闭合：共 5,583 个不同测试全部通过，无忽略。修复前的全量日志与各项定向补验均保留，按测试目标和名称取最后结果；元数据投影修正后的全部 codegen 与 driver 检查均已覆盖。

### 旧相等合约用例的 NaN 迁移

- 两个 M33 合约程序仍对 generic same 与 Equals<Float> 视图断言 NaN 不相等。按 M41 已实现的默认相等规则更新四个断言，保留用户自定义非自反 equals、求值顺序、Hash、装箱及跨库调用检查。
- 本机格式化、workspace clippy 后，两个正式 fixture 通过：4 个 debug/release 变体、20 次进程，包含产物独立消费与 GC 模式。

### 结构相等的规范一致性

- 修正语言规范 11.11 遗留的 NaN 自反性表述，使其与 11.2.2 和已实现的字段比较一致：浮点 NaN 彼此相等，任意结构值仍可能因其他字段的用户自定义 equals 而不满足自反性。逐字段比较及禁止任意 identity／memcmp 快捷替换的约束保持。
- 核对三份规范的浮点／IEEE 描述与章节引用；这是既有语义的文档澄清，复用已完成的浮点结构组合、NaN 合约及非自反用户 equals 检查。
