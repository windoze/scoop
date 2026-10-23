# M62 实施记录

范围与候选方案见 [设计](DESIGN.md)。每项独立实现、定向验证后提交；Linux 验证按用户最新要求停止未完成的完整 fixture，以失败项定向补测收尾，实际覆盖见 [验收记录](ACCEPTANCE.md)。原始日志与实验产物保存在仓库外，不使用 `/tmp`。

## 接口与类型判断的 TD 查询

- 规范明确复用登记时已经检查且运行期间不变的 TD 引用关系；接口查表和类型判断不再查询登记表。
- 保留接口缺失错误、完整子类型判断，以及分配／native root 等输入边界的检查；不改变接口表示或 runtime ABI。
- 本机通过 `cargo fmt --all`、release workspace clippy、TD 登记边界测试，以及 `interfaces-artifacts` 的两个跨 profile 变体（8 个正式 CLI 进程，包含独立链接）。开发 sysroot 已显式更新。
- 同一份 release Scoop 产物仅重链 runtime 后，三个现有负载输出一致。各运行三次，含启动的墙钟中位数：listIter 2.676 → 0.558 秒，forEach 2.701 → 0.564 秒，mapFilter 1.467 → 0.420 秒。该结果只代表本机这三个负载；未单独量化 `is_instance` 的收益。
- Linux 与最终组合验证在后续统一执行。

## 具体化后的 primitive intrinsic

- 约束调用选择出确定的整数、浮点、字符、Boolean 或 String primitive 实现后，直接生成对应 concrete HIR 操作；本地与导入实现共用该处理。
- 泛型 bound reference 保留具体 intrinsic 身份。选中的整数除法／余数实现同时准备并物化异常类型，保留除零异常。
- 新增真实 core 下的 provider／consumer／runner 编译器专测；debug 和 release 均检查对应 MIR 操作及无遗留 external primitive 调用，运行整数、浮点、字符、字符串、转换、普通／泛型方法引用及除零路径。
- 本机格式化和 release workspace clippy 通过；HIR lowering 的 1530 个单元测试通过；intrinsic 专测 3 项、6 个 profile 变体、16 个 CLI 进程通过。删除闲置 dev 构建产物，回收 target 中约 1.6 GiB，保留 release 与共享 fixture cache 供后续增量验证。
- 本项完成早期具体化修复。静态引用 invoke 的强制展开、去虚化后再下降及后续优化暴露的 intrinsic 仍在下一项处理；不把普通内联碰巧消除调用视为完成。
- Linux 验证使用独立的 `~/repos/scoop-m62`，现有 `~/repos/scoop` 留给其他工作。

## 已知 intrinsic invoke 的展开

- MIR 根据原 HIR 的 typed intrinsic／reference 身份识别本地入口及其 bridge／adjust，消费已知 closure 或 dispatch 目标。展开复用现有 CFG 克隆，保留调用点异常出口和实参求值；不受普通内联开关、成本或增长限制影响。
- 普通优化暴露新的已知 intrinsic 目标后继续归一化，直到该步骤没有实际改写。未知 closure 参数仍按原签名动态调用。生成入口仍保留，供真实动态引用使用。
- 调整构造函数声明顺序，修复定义 core 时除法方法引用先生成正文、异常构造函数位置尚未建立的问题。
- 本机格式化和 workspace clippy 通过；MIR crate 153 项测试通过，随后新增覆盖并通过优化专项 28 项。精确断言覆盖普通优化关闭、只启用去虚化、release、普通内联之后才暴露目标，以及除法异常出口。
- 正式 CLI intrinsic 专测 4 项、8 个 profile 变体、18 个进程通过，包含除法引用的 signed-min／-1 边界、除零，以及 suspend 恢复后在 try/catch 中调用 intrinsic 引用。
- 跨 Cone 去虚化指向外部 adjust／intrinsic 的路径尚需结合依赖正文消费继续处理；本项不宣称第 2.4 节全部完成。

## 内联前副本清理与冷分支计价

- 内联成本分析与展开后清理共用局部副本传播；只转发同类型、未取地址且在循环外单次赋值的源，保留修改后快照、求值顺序和副作用。参数事实跨 callable 传递时清除 caller 的局部 id。
- 仅通向异常／trap 且存在正常返回路径的分支降低执行成本权重；实际代码增长仍完整计入 caller 的限制。成本实现拆成独立子模块。
- 本机格式化、release workspace clippy 与 MIR crate 全部 158 项测试通过。intrinsic、协程、内联的 13 项 CLI 专测通过；更新因副本消除变化的三个 release MIR 期望，debug 期望不变。
- 同一 sysroot/runtime 的三轮运行中位数：listIndex 0.244 → 0.177 秒，mapFilter 0.395 → 0.343 秒，struct 1.288 → 1.333 秒；输出一致，struct 未显示收益。可执行文件 25,859,800 → 25,935,656 bytes（约 +0.29%）。这是本机小样本对照，未据此调整全局内联阈值。

## 有界 core 标量 native helper

- Long／ULong hash、Boolean hash／equals 改用已有的 C ABI `@GCLeaf` 调用；普通方法在局部 unsafe block 内调用，公开签名与原计算结果保持。无新 runtime ABI 或注解，String 线性操作不参与此选择。
- core consumer Cone 新增两个测试，覆盖符号／无符号边界、整数扩展、Boolean 值、方法引用和 GC 后的接口调用。格式化、workspace clippy、两项 Cone 测试及 direct-c 跨产物专测（两种 profile、12 个 CLI 进程）通过。
- 同一编译器、分别消费变更前后 core 的三轮 hashLong（HashMap<Long, Long>）中位数为 0.877 → 0.840 秒，输出一致；这是包含其他集合成本的小样本结果，不能据此量化单个 native 调用的收益。
- 系统库更新暴露的 readline stackmap 缺项由后续独立修复闭合；不删除产物检查。再次清理闲置 dev 产物，回收约 1.8 GiB，保留 release 与共享 cache。

## 内联组合后的 safepoint 定稿

- 冷路径内联使 readline 的重复动态边界检查进入同一 callable；后段加载消除令比较对象相同，机器层随后删除不可达异常调用，而此前的根计划仍保留该 site。
- Managed LLVM 收尾复用现有 constraint-elimination，再进行 SCCP 与不可达块清理；在根计划定稿前消费新暴露的路径约束，不放宽 stackmap 校验，不增加根定稿后的普通优化。
- 小型 LLVM 回归在旧收尾下失败，修复后在 Darwin ARM64、Linux glibc／musl amd64 的机器产物中验证实际根数。格式化、workspace clippy、40 项 statepoint 专测通过；http 与 readline 两个实际库重新构建并通过原产物检查。

## 有界 Managed 叶函数入口

- LIR 对无调用、无循环、无条件分支的有限直线路径选择入口激活检查；限制操作数及单次复制大小，允许内联留下的纯跳转块。正常 Managed 状态只读取 mode，其他状态仍执行完整 safepoint 慢路径、根物化和 relocation，保留首次进入 Managed 的握手。
- 格式化、workspace clippy、LIR lowering 的 162 项测试通过；codegen 在 Darwin ARM64、Linux glibc／musl amd64 验证两种 poll 模式及根计划。gateway 首帧激活测试、4 项正式 CLI GC／多线程专测通过（9 个变体、35 个进程）。
- 生成的 V3 add／scale 机器码确认入口 acquire load 从四次减少到一次。现有组合负载各三轮中位数：struct 1.319 → 1.317 秒，listIndex 0.164 → 0.165 秒；结果一致，但未测出稳定的整体提速。最终文件仅减少 16 bytes，文件大小受链接布局影响，不能替代具体函数检查。

## 跨 Cone 普通小函数内联

- 复用 shared-source 可移植 HIR 发布符合条件的普通顶层函数正文；consumer 将其放入独立 concrete 优化正文集合，MIR 复用原 CFG 下降、成本选择和克隆。原调用、provider 身份和外部入口保留，不生成第二份 Strong 定义；不支持的正文继续正常外部调用。
- 修正原先将“存在可移植正文”等同于“泛型”的两处调用选择。测试元数据保留每个真实声明的来源，跨库正文中的依赖沿既有调用选择传递。
- 格式化、workspace clippy、HIR 867 项、HIR lowering 原 1530 项及新增身份／运算回归通过；MIR 158 项已覆盖。内联 CLI 六项通过，新增独立 provider／consumer／runner 验证 debug／release、分支、常量除法、除零捕获、最小整数／−1、递归回退及实参只求值一次。动态除法仍可因正常成本策略保留调用。
- 独立冷缓存编译两个小 Cone（包含阶段 dump），provider 0.721 → 0.645 秒、runner 3.182 → 3.273 秒，单次结果只记录开销量级。同一 runtime 下千万次跨库整数 helper 五轮交替运行中位数 32.78 → 25.28 ms，输出一致；机器码确认循环中的 helper 调用消失，文件 16,554,440 → 16,553,944 bytes。含余数的另一负载未显示稳定收益，不据此扩大内联阈值。

## 去虚化后的外部 intrinsic

- 已知装箱值与 final String 消费 concrete HIR 保留的 exact 类型、接口 slot 和已选 primitive 实现；用原 intrinsic 下降构造完整优化正文，显式适配 receiver，在普通内联关闭时也展开。provider 物理 itable 和 Strong 定义仍由原产物提供。
- 补齐 String 特殊类型的接口导入，以及只有装箱接口调用时整数除法异常类型的物化；只为实际使用的约束保留实现，避免扩大依赖选择。
- 格式化、workspace clippy、HIR lowering 1531 项与 MIR lowering 158 项通过。intrinsic CLI 专测 4 项、8 个 profile 变体、18 个进程通过；新增装箱整数／浮点／字符、String、除零捕获、signed-min／−1，以及内联后才已知的目标和未知自定义实现。
- 最终 ARM64 机器码确认整数加法、浮点加法、整数除法与字符比较分别生成原生操作，String 调用规定的比较 helper；未知接口保留间接调用。内联后暴露的加法也已消除包装调用；异常分支保留构造与抛出。此项闭合跨 Cone 晚期 intrinsic 下降，不把 TLS getter 或必要 runtime helper 当作遗漏的 intrinsic 包装调用。

## TLAB 完成路径

- 成功 bump 后复用生成代码已确认的对齐大小、nursery 资格和当前 block，省去重复规格化与 block 回查；普通 runtime 分配继续原检查。两条路径共用清零、对象头、精确大小、line bitmap／release starts 发布及线程原子计数，不修改元数据并发协议。
- 格式化、workspace clippy、10 项 nursery C 回归通过；补充检查快路径清零、对象头、可查询起点及精确大小。正式 CLI accounting／threads 两项、四个 profile 变体、20 个进程通过，覆盖普通、minor 与 moving stress。
- 同一 release Scoop 产物仅更换 runtime，预热后五轮交替中位数：trees 0.4603 → 0.4506 秒，refStore 0.3570 → 0.3289 秒，输出一致。各自的分配、晋升和复制统计保留用于核对；未据此删除发布原子操作或更改 nursery 默认值。
- 全部内置库重建另外暴露 http.client 的 stackmap 根计数回归，作为后续编译器修复处理；不放宽产物检查。

## 共享后继中的物理 GC 根计数

- http.client 的 nullable 路径中，逻辑恢复标记已折叠为 null，共享后继／循环却仍使原动态 SSA 跨调用存活；按逻辑标记预算物理根数会少计。注册元数据改用 RS4GC 实际 gc.relocate 中不同 derived SSA 的数量，继续保留逻辑身份／base 合并处理及机器 stackmap 的精确检查，不增加第二套活跃性分析。
- 移除不再使用的优化前物理分组表示。新增小型共享后继回归，覆盖 Darwin ARM64 和 Linux GNU／musl 机器产物；格式化、workspace clippy、全部 256 项 codegen 测试通过。此前失败的实际 scoop.http.client release 库重新构建成功，系统库构建问题闭合。

## Darwin 最终导出精简

- 仅在最终 executable 隐藏 Scoop body 起止定位别名，保留 `.slib`／对象的 ODR 定义、callable 与 native 导出和 GC／EH 引用。固定 linker 参数进入原配置指纹，普通缓存按既有更新规则处理。
- 格式化、workspace clippy 与真实 Darwin 链接回归通过；检查边界符号仍存在于本地符号表、退出动态导出，main／native 入口继续导出，程序输出 42。
- 同一组 Scoop／runtime 产物仅改变 final-link：文件 25,934,904 → 19,694,696 bytes，`__LINKEDIT` 20,413,496 → 14,189,672 bytes，81,330 个边界导出全部移除，本地符号总数不变。30 轮交替热启动中位数 22.88 → 21.29 ms，单次峰值 RSS 16.53 → 13.44 MiB；首次进程运行 507 → 484 ms，未控制系统页缓存，不当作严格冷启动结论。

## Linux 验收输入修复

- 全量 Rust 测试发现旧的 C++／musl 拒绝测试使用无 package 的 Cone 源码，先触发了既有包名规则。补齐 `package dev.m33`，使测试到达原本要检查的链接配置；本机 fmt／clippy 与 Linux 该用例重测通过。其余已通过测试不因这一测试输入修正重复全量执行。

## 常量路径上的对象起点合并

- 补充 nullable 共享后继与 immortal／dynamic select 的组合：逻辑恢复已为 null 时，原 select 仍可能成为真实物理根。根准备独立保留无派生地址的 managed phi／select 的对象起点事实，继续拒绝实际逻辑根中的派生地址；已标明的合并直接复用，不新增活跃性分析。
- 三目标的 27 项 statepoint 专测通过，包括常量／动态合并和派生地址负例；本机 fmt／workspace clippy 及四项最终 CLI 组合通过（8 个变体、36 个进程），覆盖跨库 intrinsic／普通内联和 moving／minor 多线程根。

## intrinsic 标量的内联复制成本

- V3 对照复现 `add`／`scale` 保留循环内调用。候选化简后成本分别为 66／48：MIR 将 Float、Double、Char 保留为 intrinsic struct，旧模型误给每个标量临时值附加 aggregate copy 成本。按已有 typed representation 区分后，V3 两个候选分别为 39／27，进入原有循环阈值 40；字段权重、普通 struct 参数成本及增长限制均未调整。
- 新增真实 core 的编译器专测，精确检查 debug 保留、release 消除 V3 add／scale 和 Float 小函数调用，并运行核对计算结果与参数值独立性。本机 fmt／workspace clippy、158 项 MIR lowering 测试、内联增长及语义专测通过。完整内联组另更新四份旧期望中的 ArithmeticException 声明与对应 external 编号，七项均通过。
- 同一 sysroot／runtime，预热后五轮交替运行一亿次 V3 计算：原版本 1.309 秒、修复后 0.170 秒、手工展开 0.170 秒、关闭浮点融合的 C 0.134 秒；输出一致。约 7.7 倍收益来自该负载，不能概括为所有 struct 函数或整个编译器的最大收益；严格浮点与 Managed 循环 poll 责任保持。

## 导入正文的源码位置验收

- Linux 全量 Rust 验证发现 type-site 测试仍假定只有 core 物化正文保留外部求值来源。跨 Cone 普通内联新增的 provider 正文也应保留自身来源；测试按已发布正文的 typed callable 身份单独断言其 definition／evaluation 来源，继续要求默认实参在 consumer 求值并保留 provider 定义位置。
- 本机 fmt／workspace clippy 和实际 provider／consumer 产物重测通过；未修改来源传播实现或放宽未知来源的断言。

## 缓存摘要 fixture 修正

- 完整 Linux fixture 发现旧观察脚本仍查询 `artifact-summaries-v1`，而规范与实现已使用 v2。仅同步测试路径，保留记录数量／mtime、真实源码变更、实际产物存在和冷／热调用断言。
- 固定版本 Ruff 格式化与 lint、本机完整用例（16 个进程）通过；未改变缓存更新或读取行为。

## GC 扫描中的 immortal 重复查找

- mark 与 relocation 对堆外引用查询 immortal 地址表，查询结果原先只决定是否报错。普通 runtime 改为直接保留堆外引用；stress runtime 与显式 `SCOOP_GC_VERIFY_HEAP` 构建保留查询，不依赖应用 debug／release profile。共用已有对象起点解析，堆内非法地址继续报错，不增加第二次 block 查找；映像登记、pin 与 handle 边界检查保持。
- 本机 fmt／workspace clippy、C 警告检查、四种 runtime 配置及其余 36 项 collector 回归通过；重建普通／stress runtime 后，正式 GC fixture 四项、八个变体、40 个进程通过。优化配置测试保留测试 fixture 的初始化断言和明确的合成帧边界。独立 Linux 副本的 GNU／musl 各四种配置通过；对象文件确认普通 runtime 的两个扫描入口没有 immortal 查询引用，stress／显式验证仍保留。未据此声称整体 GC 加速比例。
- GNU `-O2` 机器码复核发现抽取的对象起点解析 helper 增加了一次调用；加上普通 `inline` 提示后消除该调用，不扩大或省略检查。本机四种配置及 GNU／musl 各四种配置重新验证通过，普通／stress runtime 随源码更新。
- 同一 release Scoop 产物只更换调整前／后的 runtime，单 worker、65536 个引用槽、601 次 full、零搬迁，预热后五轮交替进程墙钟中位数：全 immortal 0.629 → 0.219 秒，全堆对象 0.311 → 0.307 秒，半数 immortal 0.480 → 0.276 秒。每组输出、分配量、标记对象数和 39,388,939 次引用槽访问一致。此对照隔离非移动 full 的标记成本，不外推到其他图或整体应用。

## 优化组合后的 MIR 期望

- Linux 全量揭示去虚化快照仍保留清理前副本、旧的冷路径内联决策与异常依赖编号。核对原程序和新的 IR 后，同步去虚化与局部值专测期望；开放参数／未知返回仍保留动态分派，原有循环、异常合并、构造及可变字段行为保持。return-paths 的结构断言仅更新 UnwrapException 构造入口的 external 编号。
- 本机及 GNU 正常重测四项、七个变体、18 个进程与八份 MIR golden 全部通过；未调整优化策略或放宽运行断言。

## 归档成员名称期望

- Linux 全量发现泛型产物快照仍使用旧 `manifest.cbor` 名称；按现行 `.slib` 规范与 writer 的固定 `manifest.zst` 同步三目标期望，其余成员名称与数量保持。
- Darwin、GNU 与 musl 各十二项正常复测通过，每目标十二个变体、95 个进程与十二份成员列表 golden；覆盖泛型 reader、机器码消费及 structural box，未修改产物格式或读取规则。

## 收尾

- GNU 运行中出现的十五个失败项均已定向复测通过；按用户要求停止剩余完整 fixture，未完成项保留在报告中。全部 26 个测试 Cone、Rust workspace 覆盖及两目标必要组合结果见验收记录。
- 最终 GC 增量通过 Linux fmt／workspace clippy、37 项 collector 回归和 GNU／musl 各四项正式 GC fixture；没有遗留的已知失败。
- 收尾清理本机约 3.7 GiB、独立 Linux 目录约 23.1 GiB 的已结束测试可执行文件缓存，保留 release 工具与可复用编译缓存；原始记录留在仓库外。
