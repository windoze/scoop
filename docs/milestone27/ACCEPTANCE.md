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

## 后续批次

M27-4 协程任务传播、M27-5 callback snapshot 和 M27-6 总验收仍在实施计划内。全部完成前不标记路线图 M27 完成。
