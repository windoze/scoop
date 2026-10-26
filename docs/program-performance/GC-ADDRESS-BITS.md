# GC 地址转整数的重定位时序修复

`0b97b04d7` 的 `gc-correctness/maybe-uninit/gc` 在 ARM64 release、`SCOOP_GC_STRESS_MOVE=1` 下可复现崩溃。`writeYoung` 对同一 large 数组连续分配、写入：数组引用在慢分配返回后已更新，但普通 LLVM CSE 复用了搬迁前的四级页表索引。屏障由旧索引找到 region，再以新地址计算 card，最终越界写坏脏卡组。LLDB 已确认错误发生在第二次写入；仅重做 acquire 加载或增加 runtime 越界检查不能修正地址来源。

编译器原本已有 `ni:1`；修复继续保留该合同，并在普通优化前把 Managed 函数的 AS1 `ptrtoint` 暂存为自有 NoGC 操作。它以 `memory(inaccessiblemem: readwrite)` 保持调用之间的顺序，不声明对普通对象或栈内存的读写。操作始终持有实际指针，根定稿和 RS4GC 因而能更新后续真实使用；RS4GC 后原位恢复 `ptrtoint`，删除临时调用及声明。

操作按编译器属性身份识别，同名普通 extern 仍为普通调用。既有有界卡表区域认识该操作，继续在已证明无 safepoint、查询必经的区域提升查询和合并卡标记。NoGc 函数保持原流程；没有新增 runtime 入口、诊断代码、对象字段、根种类或 ABI。

这是全部 profile 默认生效的 GC 正确性修复，无独立开关。应用和库中受影响的机器码须用新编译器重新生成；runtime 可以原样复用，单独重链接不能修复旧机器码。

## 正确性与机器产物

- 两机格式化、相关 crate 的 release all-targets Clippy，以及各 55 项 statepoint 回归通过。新增最小 IR 覆盖 ARM64／x86_64、debug／release、直接调用／条件 poll；沿真实返回值检查 `lshr → ptrtoint → relocate/phi`，并实际发射目标对象。同名 extern 和无关对象读取的消除另有断言。
- 三目标各通过 MaybeUninit GC 与引用数组 GC 两组，合计 3 个 variants、22 个进程，包含正常、moving、minor 及已有 survivor 组合。覆盖 large 数组、字段／静态存储、接口与可空值、父接口投影及有界屏障路径；没有执行全量 fixture 或 workspace 验收。
- ARM64 最终 `writeYoung` 在慢分配后重载数组到 x19，再以 x19 重新执行四层位提取；x86_64 从恢复后的 r14 重新计算各层索引。最终 fixture 和三个性能程序均无临时地址操作符号。ARM64 该函数静态指令由 647 增至 665，栈帧由 112 降至 80 B；不把这些静态数量当成每次调用的动态成本。

## 运行与空间成本

M3 Ultra／LLVM 22.1.8 与 NUC i7-12700H／LLVM 22.1.2；正常 release、固定源码、同一套预编译 core／std／runtime，仅替换消费者编译器。Linux 运行固定 CPU 4。1 对预热、5 对 AB／BA 交替正式运行，墙钟中位数如下，输出均一致。

| 程序 | ARM64 / Darwin | x86_64 / GNU | x86_64 / musl |
| --- | --- | --- | --- |
| dispatch | 0.441460 → 0.442449 s（+0.22%） | 0.363735 → 0.370296 s（+1.80%） | 0.434070 → 0.432549 s（−0.35%） |
| lambdas | 0.227365 → 0.226108 s（−0.55%） | 0.257323 → 0.278428 s（+8.20%） | 0.287084 → 0.303816 s（+5.83%） |
| 接口数组复制／父接口投影 | 0.126369 → 0.126352 s（−0.01%） | 0.078676 → 0.079411 s（+0.93%） | 0.084494 → 0.084434 s（−0.07%） |

Linux lambdas 存在实际代价：main 增加 22 条静态指令，text 增加 128 B。Darwin lambdas 的 main 少 11 条，text 少 44 B。三个目标的完整文件大小均不变，dispatch 和接口数组的代码段大小也保持；Darwin dispatch 热循环指令与根恢复保持。Linux 回退不作为噪声抹去，也不据此允许恢复错误的跨 GC 复用。

独立 GC 统计中，三目标各程序的分配字节、minor／full 次数和 root_slots 均保持：lambdas 为 738,736,680 B、781／1 次、55,366 个根槽访问。没有把保留正确地址的代价解释成新增分配或根扫描。

消费编译使用独立冷 cache／target、复用预编译系统库，1 对预热、3 对交替正式构建。三程序编译墙钟变化为 Darwin −1.51%～−0.28%、GNU −0.35%～+0.77%、musl +0.21%～+0.81%；峰值 RSS 最大增加为 Darwin 的 2.85%。一次误用已命中 baseline cache 的编译测量已作废并保留记录，上述数字来自重新隔离的冷构建。

初测与最终版的三个程序在每个目标均为相同机器产物；收紧临时操作的内存副作用有独立 IR 证明，不把两次计时差异归为这项收紧的收益。Darwin dispatch 初测 +5.91%、单独复核 +1.94%，Linux lambdas 初测 +7.06%／+5.37% 均保留在原始记录中。

日志、逐次 JSON、IR 和机器码位于仓库外 `scoop-performance-results/2026-10-26/gc-address-bits/`，最终数据在 `refined/` 和 `linux-results/refined/`；临时编译器与复现脚本位于仓库忽略的 `tmp/program-performance/gc-address-bits/`。
