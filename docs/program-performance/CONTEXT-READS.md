# Context 小访问器的生成路径

当前所有 profile 使用 typed 槽读写，restore 也生成字段写入和屏障；旧 current／enter／leave／restore 包装已退役。下面是最初 release 访问器阶段的历史对照，当前合同与消费者见 [Runtime 职责收敛](RUNTIME-RESPONSIBILITIES.md)。

在 `64c80b33b` 的数组初始化优化之后，继续复用已完成的 TaskContext fork／TLAB、接口 SSA 和根恢复能力。一次 ARM64 五秒采样中，排除启动与等待线程后，current／snapshot／enter／leave 合计占主线程 346／3,452 个样本；try-get／restore 未命中。这用于选择本批范围，不作为调用周期或预期收益。

## 实现与开启条件

- MIR snapshot 在 LIR 根计划前变成 typed current、判空与普通 HeapLoad，复用 generated TaskContext 的 root 布局。release 对真实 NoGC current／enter／leave 普通调用生成根槽读写；debug、invoke 和普通 extern 保持原调用协议。
- 新私有 TLS `scoop_rt_context_slot` 只保存原 `current_task_context` 根槽的 native 地址。attach 绑定、detach 清空，collector 与 C 后备仍访问原槽；不复制 managed 引用，不增加对象字段或 GC root。enter 的 previous guard 继续使用正常／异常路径上的精确根和实际重定位结果。
- ARM64 release 惰性取得并复用一次 invocation 内稳定的槽地址，和分配 context 共用原首次使用实现；x86_64 直接读取 TLS。只缓存 AS0 地址，槽内引用、binding root、native 往返和协程恢复后的值遵守原内存效果与 GC 合同。
- release 默认，无独立开关或新测量代码。snapshot 的基本降低与 TLS 合同在全部 profile 一致；runtime contract 为 **19**，metadata ABI 仍为 **9**，旧库／runtime／开发 sysroot 须重建。语言 ABI、接口表示和任务身份保持，设计见实现规范 2.16、运行时规范 9.2。

## 验证

先格式化与 workspace release Clippy，再在本机和 nuc12 各运行 32 项定向单测，覆盖 snapshot 判空／布局、guard 的实际 relocate 写回、普通同名 C extern 的 release 协议、原分配地址复用及 ABI。原 ABI 指纹的两项期望随 contract 更新后闭合。

ARM64 Darwin、x86_64 GNU／musl 各通过四组、五个变体：context-fork 的 debug／release × moving／minor，callback-concurrent、gc-lifetimes 和 coroutine-exits。没有全量验收。附加计数确认每目标 debug moving 为 101 次 full、复制 63,080 B；release minor 组合为 48 次 minor、15 次 full、复制 18,584 B。native 测试另覆盖主线程／foreign thread 槽隔离、C 与生成访问方式互读写、NativeSafe 期间搬迁、detach 清空。

三目标两份基准的 MIR 逐字节相同，LIR managed Alloc 静态位置仍为 25／17 处。最终 Scoop callable 中 current／snapshot／enter／leave 的直接 C 调用由 tail 的 3／1／7／14、synchronous 的 2／1／5／10 全部降为 0。ARM64 代表入口只取得一次槽地址，正常与 EH 出口在 guard 恢复后写回原槽；x86_64 使用本线程 TLS 与槽读写。原入口 poll、分配慢边、EH 根发布和必要 relocation 保留。

## 性能与成本

基线为保存的 `64c80b33b` release 工具，双方以同一份当前 C 源码生产各自 ABI 的 runtime index；每个目标的 **92 个 runtime 对象逐字节相同**。两边系统库各用对应工具重建，源码不变。复用 coroutine tail／synchronous 基准，每模式 200 万轮，一次预热、五对 AB／BA；普通 release，无 stress／统计或 CPU affinity。输出均核对，以下是整进程墙钟中位数的降低比例，不是单次访问成本。

| 模式 | ARM64 Darwin | x86_64 GNU | x86_64 musl |
| --- | ---: | ---: | ---: |
| tail immediate | 1.92% | 2.55% | 2.80% |
| tail deferred | 3.23% | 5.28% | 4.78% |
| synchronous leaf | 7.33% | 7.05% | 3.66% |
| synchronous wrapped | 6.22% | 6.69% | 5.07% |

所有模式另以相同 argv[0] 采集 GC 统计，双方分配字节、minor／full、copied bytes 与 root slots 相同。tail 两模式分别分配 672,000,360／672,000,352 B，均 644 次 minor、1 次 full、50,955 次根槽扫描；synchronous 两模式均为 416,000,304 B、398 次 minor、0 次 full、29,616 次根槽扫描。新访问路径自身不分配；没有把扫描次数当作静态根数量。

| 成本 | ARM64 Darwin | x86_64 GNU | x86_64 musl |
| --- | ---: | ---: | ---: |
| tail／synchronous text 增量 | +228／+164 B | +448／+320 B | +512／+320 B |
| 两程序完整文件增量 | 0 B | 0 B | 0 B |
| 四个系统库总大小变化 | −16,830 B | −10,108 B | −13,976 B |
| consumer 编译／链接墙钟变化 | −0.02% | +0.79% | −1.32% |
| consumer 编译／链接 CPU 变化 | −1.40% | +0.38% | −1.07% |
| consumer 编译峰值 RSS 变化 | −1.09% | +0.02% | −0.08% |

consumer 成本用三个 AB／BA 交替的新 root Cone cache 测量，复用预编译依赖；单次四库冷构建墙钟变化为 +1.95%／+0.04%／+0.21%，不作为稳定加速结论。ARM64 stackmap 节大小保持，ELF 未单列断言 stackmap 字节数。

上表固定同一运行库以隔离生成代码。相对原 runtime，槽地址另增加每线程 8 B；thread.c 对象文件的 text 增加 68／32／32 B，Darwin 另有 24 B TLS descriptor。地址复用也有寄存器成本：ARM64 代表 coroutine-start 栈帧从 352 增为 368 B。没有据删除的 C 调用数推算周期收益。

本机为 M3 Ultra／LLVM 22.1.8；nuc12 为 i7-12700H／LLVM 22.1.2，GNU 与 musl 均有实机结果。绑定树、try-get／push／restore、初始 root 和必要 C 后备继续共享；本批没有展开它们，也没有跨调用缓存 managed Context 值。原始日志、机器码与逐次数据在仓库外 `scoop-performance-results/2026-10-26/context-reads`，临时输入／构建在仓库 `tmp/program-performance/context-reads`。
