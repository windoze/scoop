# M27 实施与验收记录

状态：实现进行中；以下为分批证据，不代表整个 M27 已完成。

## M27-1 同步作用域

实现了 AST/HIR/MIR/LIR 的 context requirement 和 scope、实际 core MissingContextException、入口 lookup、结构化 push/restore cleanup、root gateway 的 task 初始化、可移动 GC 的 TaskContext/四叉持久树，以及沿现有 callable registration 和 associated atoms 发布 key cell 的对象与 runtime 路径。普通 callable ABI 保持原有参数。

新增实现文件按职责拆分，当前均不超过 170 行；较大的 metadata 发射模块拆出了既有 digest 处理。构建使用外部 LLVM 22.1，阶段性清理 Cargo target，并关闭本次构建的增量缓存和调试信息以控制临时产物体积。

正式 fixture：

- `tests/fixtures/m27-context/basic`：基本 binding、入口取得、退出恢复、缺失异常与 moving GC；
- `tests/fixtures/m27-context/scopes`：中间普通调用、延迟函数引用、入口 local 稳定、不同 key、anonymous/unused requirement、词法捕获、value 求值一次和异常消息；
- `tests/fixtures/m27-context/exits`：return/break/continue/throw/finally、绑定表达式抛出、GC 中保留结果，以及 caller 能捕获而正文 try 不能捕获入口失败。

三套 fixture 均保存 HIR/MIR/LIR golden，并各运行普通和 `SCOOP_GC_STRESS_MOVE=1` 两条路径。runtime 的 `task_context_test.c` 通过现有 codegen C 测试入口验证较高 slot、分支边界、嵌套恢复、fork snapshot、每次分配时 relocation，以及 native-safe 线程的 current task 扫描。

本批通过 `cargo fmt --all`、`cargo clippy --workspace --all-targets`，以及 parser 450、HIR 874、HIR lowering 1314、identity 337、MIR 345、MIR lowering 114、LIR 477、LIR lowering 146、codegen 312、slib 590 项库测试。driver 的 88 项库测试中，87 项在批量运行通过；剩余 core 产物闭合测试修正其对 compiler-generated Context 类型的选择条件后，单独复验通过。三个正式 fixture 共执行 9 个进程并核对 9 份 stage golden。

## M27-2 声明与导出契约

context 参数在 callable 声明与 portable callable body 中分别以独立有序字段保存，普通参数、函数类型及声明 identity 保持原有含义。导出类型引用、binder 解析与可见性沿既有签名路径处理；继承签名保存替换后的有序 key。接口继承、override、继承来的 class 实现与 property obligation 均检查 context 数量、顺序和类型。

补齐局部命名函数自己的入口 lookup，导入接口的继承参数随 owner arguments 一同替换。contextual stored/delegated/const property 在 parser 拒绝。源码契约测试覆盖 key 资格、名称/类型重复、默认值作用域、不可变入口 local、可见性、NoGC 和继承一致性。

新增 `declarations`、`types` 两个正式正例，每个均有 HIR/MIR/LIR golden 及普通/moving GC 运行；`negative/` 保存 51 个独立错误用例及完整诊断和 byte span。新增生产模块均低于 130 行。HIR wire 875、HIR lowering 1324 项测试与 profile 固定向量复验通过；完整 M27 CLI 批次以普通验收模式通过 56/56 个用例、66 个进程与 15 份 stage golden。

## M27-3 泛型与独立产物

具体化 callable 与 owner application 时检查替换后新合并的 key，并在同次具体化内复用已检查的声明与完整 key 列表。导入 class/struct/enum 的无泛型成员契约由原声明目录载入，接口契约随 owner 参数替换；没有 body 或未调用方法的类型应用也能在原使用位置诊断。补齐 enum 的 context member 与名为 context 的 variant 之间的 parser 消歧。

新增 `generics` 正例覆盖精确 Array/函数类型 key、class bound、generic owner、enum member、anonymous requirements 和泛型 local function；七个独立负例覆盖 callable、Array 以及 interface/abstract/class/struct/enum 应用的新合并。新增 `artifacts` 正式链路构建实际 core 和 A→B→C，每步移除上游源码；仅保留 slib、runtime objects 和独立 scoop 命令后完成链接并核对 link plan。跨 Cone 泛型 body、local function、default、property、interface dispatch 与同一 ODR body 的 context metadata 均完成普通和 moving GC 运行，导入 owner 的三个无调用负例保存完整诊断。

本批通过格式化与全 workspace lint、parser 451、HIR lowering 1327 项库测试及四项 callable registration 测试。正式 M27 CLI 集合以普通验收模式通过 65/65 个用例、84 个进程和 27 份 stage golden。新增生产模块为 19、47、79 行；codegen 使用既有 body 发射计划识别尚未生成基本块的 ODR 定义，保持 metadata 与 body 同对象发射。

## M27-4 协程任务传播

resumable frame 在发布 continuation 前保存完整 TaskContext，direct suspend 调用沿用当前 task。start helper 从当前 binding root fork；resume/failure adapter 仅在成功取得实际驱动权后进入 frame task，同步 latch 不切换任务。正常完成、再次挂起、body 失败、completion 抛出和 catch materialization 的 unwind 均恢复调用前的 task，completion 自身失败不会触发第二次通知。

跨挂起的 context 入口 local 和 scope mark 使用精确 CoroutineSlot；mark 保持 MIR 生成值类型身份，并沿既有 StructuralType group 合并 ODR helper。读取跨 Cone slot 的 GC 属性时借用可达依赖中已验证的类型记录。frame 的 task 字段使用 generated field tag 15，MIR identity-foundation /4、type-bridge /10 同步更新。

新增 `coroutines`、`coroutine-completion`、`coroutine-exits`、`coroutine-thread` 四套正式 fixture，覆盖立即完成、真实挂起、direct 链、同步 resume latch、绑定表达式挂起、泛型入口 local、return/break/continue、catch/rethrow、挂起的 finally、completion 抛出与外部线程恢复。四套均保存 HIR/MIR/LIR golden 并运行普通和 moving GC 路径。A→B→C 独立产物用例增加泛型 suspend context、跨 Cone direct 调用与重复 mark slot 的 ODR 消费。

本批通过格式化与无警告的全 workspace lint；identity 337、MIR 345、MIR lowering 114、LIR lowering 146 项库测试和 18 项 profile 测试通过。11 份既有 MIR stage snapshot 更新后关闭更新模式复验。完整 M27 CLI 集合以普通验收模式通过 69/69 个用例、98 个进程和 39 份 stage golden。新增生产模块为 125、131、191 行，协程主文件拆分后为 460 行。

## M27-5 Callback 快照与跨 Cone 消费

callback token 在注册时保存不可变 binding-root handle；空快照使用既有空 handle 表示，retain 不重新采样。invocation 的 active lease 保活 closure、snapshot 和 failure，C gateway 同时登记三个 native root。每次 managed adapter 从注册快照 fork 独立 TaskContext，正常、异常和 catch materialization 的退出均恢复调用前的 task。owner/active 归零后一起释放 handles，沿用 OneShot/Reusable 状态协议。

私有 adapter 采用 closure、nullable snapshot、result、argument storage、exception output 五个参数，源 callback 和 closure 签名不变。MIR storage ABI 使用 tag 2、identity-foundation /5，runtime ABI contract /6；旧 storage tag 和旧 section 版本在原格式边界拒绝。runtime、profile、复合 ABI 与相应固定向量同批更新。

共有 HIR 的既有 callback 注册/操作节点现在可在导入泛型正文中直接物化，封闭的 imported definition 保存原 registration identity、source origin 和完整词法实参。泛型替换后沿原 parent 生成 callback application，B/C 重复物化沿已有 ODR group 合并。共享字段与源码 ABI 的类型解析将待替换的 FunPtr<F> 归一化到同一 NativeFunctionPointer exact key，不新增平行名义实例或来源认证机制。

新增 callback-scopes 与 callback-concurrent 两套 fixture，覆盖词法捕获和动态快照并存、空快照缺失、重复/嵌套 invocation、共享 payload mutation、异常恢复、最后 owner 在 active invocation 中释放，以及多个 foreign thread 的独立 binding。coroutine-thread 增加 callback→resume→callback 的 task 恢复与 completion 抛出。A→B→C 用例增加泛型 callback 注册、retain/release/state/failure、重复物化和仅凭 slib、native archive、runtime objects 的独立链接。所有运行覆盖普通和 moving GC。

本批通过格式化及无警告的全 workspace lint；HIR 875、HIR lowering 1327、MIR 345、MIR lowering 114、LIR 477、LIR lowering 146、codegen 312、slib 591 项库测试及针对导入协议的复验通过。全部 M27 fixture 在全新工作目录、关闭快照更新的模式下通过 71/71 个用例、110 个进程和 45 份 stage golden。新增生产模块为 67、96、133、138 行；callback lowering 主文件为 249 行，runtime callback 为 429 行，拆分后的 ABI 主文件为 498 行。

## M27-6 GC 生命周期与总验收

新增 GC 生命周期 fixture，使用 M24 release hook 的实际释放计数检查嵌套 shadow、callback snapshot/active handle、两个独立 child task、正常与失败恢复。用例特意保留已完成 continuation，检查退出后的旧 binding root 不再被 frame 保活，并组合 direct suspend 立即完成与 moving GC。

该用例发现跨挂起 ContextMark 被消费后仍留在 frame slot。修复在 coroutine transform 完成挂起点改写后，为原有 ContextRestore cleanup 追加对应 slot 的 Empty 写入，正常与异常退出共用同一路径；未跨挂起的 mark 不增加 frame 存储，挂起不消费 mark。实现增加 31 行，协程主文件与 frame 模块分别为 461、221 行。

修复通过格式化、无警告的全 workspace lint 与 114 项 MIR lowering 测试；72/72 个 M27 fixture 在全新目录、关闭快照更新后复验通过，共 116 个进程、48 份 stage golden。公共 fixture runner 的 32 项测试通过。全仓总验收正在继续，全部完成前不标记路线图 M27 完成。
