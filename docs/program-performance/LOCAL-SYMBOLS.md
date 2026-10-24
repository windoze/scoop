# Darwin 的 Scoop 符号在镜像内绑定

## 选择与边界

实际 Mach-O 将大量 Scoop 持久符号放入动态导出，包含已经在静态 Cone 闭包中完成选择的 weak 定义。直接 callable 调用因此仍可经过 dyld stub，注册记录、stackmap 和派发表中的引用也参与 weak binding。

最终链接将保留命名空间 `_scoop$1$*` 设为 private external，复用已有 `-unexported_symbol` 选项及 final-link 指纹。Scoop 的源码 public 可见性与 native 动态导出分别处理；普通 native／runtime C 符号和系统库导入保持。全部 profile 默认使用，无额外开关或产物布局。相容的已有 `.slib` 可直接重链接，库对象仍保留 Strong／ODR 定义，exact 类型身份、父表别名、函数地址和 GC／EH 引用不变。

本项不改内联器、poll、TLS 或 runtime 的镜像／stackmap 登记。间接接口调用仍按表中真实入口执行；系统 native 调用仍可能使用 stub。静态数据的装载期 rebase 仍然需要执行。

## 产物与运行结果

基线为 `ab026f5dd`。两版最终链接器消费完全相同的接口基准、provider、core `.slib` 和生产 runtime，仅重新链接。M3 Ultra、LLVM 22.1.8、普通 release、一个 GC worker；计时与 GC 诊断分开。

| 接口基准产物 | 原链接 | 镜像内绑定 |
| --- | ---: | ---: |
| Scoop 动态导出 | 21,643 | 0 |
| Scoop weak external 定义 | 12,040 | 0 |
| Scoop weak binding 条目 | 9,462 | 0 |
| 调用 Scoop 符号 stub 的直接调用点 | 310 | 0 |
| `__stubs` bytes | 2,364 | 804 |
| `__got` bytes | 3,352 | 48 |
| 完整 executable bytes | 13,271,352 | 11,035,848（−16.84%） |

`__text` 大小、只读数据、接口／类型表、GC／EH 和 stackmap 各 section 大小保持；native linker 将适用的直接调用绑定到真实入口，并可把本地元数据的 GOT 加载改成直接取址。没有复制或重新优化方法机器码。

每个稳态模式一对预热、七对交替 AB／BA，各执行 2000 万次；零次循环进程使用一对预热、31 对样本，包含正常初始化与退出。以下为独立样本中位数，内核与整进程分别记录，不把启动收益当作稳态吞吐收益。

| 接口模式 | 内核 ms，前→后 | 内核变化 | 整进程变化 |
| --- | --- | ---: | ---: |
| known | 34.810 → 34.846 | +0.10% | −14.23% |
| view | 51.626 → 51.705 | +0.15% | −10.26% |
| 每轮转换 | 85.691 → 85.513 | −0.21% | −6.69% |
| smart Child | 116.146 → 120.723 | +3.94% | −1.53% |
| Hash | 65.095 → 65.040 | −0.08% | −8.49% |
| Collection | 41.236 → 40.926 | −0.75% | −12.43% |
| mixed | 59.787 → 60.821 | +1.73% | −7.68% |
| 零次循环进程 | 内核约 2 µs，不据此估计吞吐 | — | 14.965 → 7.992 ms（−46.60%） |

保留 smart Child 和 mixed 的回退。前者 105 条、后者 96 条指令及函数地址保持，除三个全局数据页的 ADRP 地址外逐条相同；具体的地址布局与样本扰动贡献尚未拆开。Collection 实际少一次 GOT 间接加载，不能把它作为指令完全不变的对照。

另用当前编译器生成一次真实 `ArrayList<Long>` 遍历，再让两版链接器消费相同 `.slib`。50 万次遍历、每次 16 个元素，一对预热、七对交替样本：内核 51.633 → 50.447 ms（−2.30%），整进程 72.237 → 61.493 ms（−14.87%）。scalar 对照内核均为 0.989 ms；完整程序 18,132,744 → 15,167,592 bytes。全部十种测量模式的输出和 16 项 GC 计数一致。ArrayList getter 调用仍存在，不能将这项收益记为内联。

没有重编输入库，因此编译阶段和静态表生产成本不变。单独重复接口基准的最终链接，一对预热、五对 AB／BA：墙钟 1.500 → 1.478 s（−1.42%），CPU 19.756 → 14.827 s（−24.95%，native linker 使用多线程）；峰值 RSS 310,181,888 → 312,164,352 bytes（+0.64%）。

## 定向验证与下一步

Rust 格式化和 workspace Clippy 通过。一个实际 native 链接测试验证 callable、TD、body 边界全部本地化，普通 C 入口与 main 仍导出且运行正确。Darwin 两组正式 CLI 专测共 3 个 variant、11 次进程调用通过：跨 Cone 反向 debug／release 及独立 `.slib` 链接、单／多／菱形父表、泛型／装箱／空接口，以及 moving／minor GC。没有运行完整验收；本项仅改 Darwin 的最终链接配置，没有重复未受影响的 Linux 验收。

LLVM 的符号、动态导出／绑定表与实际反汇编确认上述数量，未把原始日志放入文档目录。原始程序、报告与命令保存在忽略的 `tmp/program-performance/local-symbols`。

同时核实后续优先项：当前 ArrayList 遍历在第三轮对 getter 的执行估价为 19、增长为 13，而 caller 只余 3，实际拒绝原因是累计增长；跨 Cone 普通成员仍被顶层正文选择限制挡住。完整 poll 的四次 acquire load、每个函数的 TLS poll state 获取，以及 runtime 的镜像／stackmap 登记仍待分别改进。
