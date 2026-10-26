# 小正文导出与跨 Cone 主值构造器内联

当前的收益、热度、累计增长和正文尺寸选择由[调用点内联评分](CALL-SITE-INLINING.md)接续；下面保留本项交付时的设计与测量。

相对 `d3ca82976`，补齐普通小正文中的值构造、字段写入和 callable-owned 字符串字面量，并让已选依赖的主值构造器进入普通内联选择。用户原始 strmap 的快照方法、主值构造和部分版本检查调用消失；三目标整进程墙钟中位数降低约 3.1%～4.9%。本项沿用 release 默认能力，导出结构对各 profile 一致，没有函数名特判，也没有增加诊断代码、用户开关或内联额度。

## 实现范围

普通非泛型正文沿用 shared-source 的可移植 HIR，96 节点的导出选择继续有界。新增接受字段赋值、struct／class 构造表达式、enum／Option 构造及 callable-owned 字符串字面量；字段 receiver 和实参仍按原顺序各求值一次，保留原写屏障、异常和初始化下降。取得正文后，由既有 MIR 执行成本、累计增长和展开深度共同决定是否内联。closure、Context、协程和局部 EH 等原有限制保持。

普通 struct 主值构造器按已有 `PrimaryValueConstructor` lowering role、exact owner 和完整参数签名建立独立 MIR 优化正文，按字段顺序组装值。复用原类型和 callable 身份，不在 consumer 生成外部 Strong 定义；空 struct 和含 managed 引用的值采用相同规则。默认实参在 caller 求值，secondary constructor 的真实正文和 class 初始化入口保留。泛型继续沿既有具体化路径处理。

导入正文的字符串常量保留原 owner／ordinal 身份与实际 provider，经已有物理依赖选择引用 provider 定义、登记的同一个 immortal object。consumer 只生成 managed 地址空间的外部常量声明，不复制字符串对象或重复登记，也不增加 GC root。物理 subject 新增 `ImmortalObject`（tag 12），相应不透明地址合同为 tag 8；引用闭合沿既有 Compile／Link 流程完成。目标 ABI、对象布局、shared-source 正文格式和 runtime 保持。折叠后归属于独立 property 的字符串暂不进入普通正文选择，保留原调用。

新编译器可以消费既有兼容库：旧库已有的主值构造角色足以建立正文；缺少普通成员正文时正常保留调用。更广的成员导出需要用新编译器重建 provider。新的物理依赖种类需要对应 reader，不承诺旧编译器消费带有该种类的新产物。

## 实际调用变化

三目标的 strmap consumer 机器码得到相同的直接调用位置变化；以下统计代码中的调用位置数量，不是动态执行次数。

| 调用 | 旧 → 新 | 实际选择 |
| --- | ---: | --- |
| `ModificationCounter.snapshot` | 3 → 0 | 三处展开，内部 `RevisionStamp` 组装继续展开 |
| `HashLocation.<init>` | 1 → 0 | MIR 两处主值构造展开；旧代码经 LLVM 合并后为一处机器调用 |
| `ModificationCounter.check` | 3 → 2 | 一处展开，其他位置继续受执行／增长成本限制 |
| `ModificationCounter.advance` | 7 → 7 | 已取得完整正文，执行成本 41，仍未通过相应调用点上限 |
| `ModificationCounter.<init>` | 2 → 2 | class 初始化入口保留 |

普通 class factory 也能导出构造表达式，但专测中的执行成本 21 超过非循环上限 20，因此保留调用，并由优化报告确认原因。没有用单独放宽阈值来掩盖剩余成本。

## 定向验证

两机格式化与 workspace Clippy 通过。本机通过 4 项 HIR 正文导入测试，两机各通过 4 项物理 subject wire 和 3 项字符串身份测试。新增 HIR 用例检查构造／字段写入／字符串完整导入，并确认不会产生本地函数定义副本。

Darwin／GNU／musl 各通过既有 `ordinary-member-inlining` 专测的两个反向 provider／consumer profile 组合，共 6 个 variant、24 次进程调用。覆盖主／次构造器、空 struct、引用字段、receiver／实参求值次数、跨 Cone 字段修改、异常消息、字符串对象身份、泛型、未知分派及 moving／minor GC。快照专测跨未知 callback 保留旧值：只触发 GC 时通过检查，callback 修改版本并触发 GC 时必须报告失效。release IR 断言合格调用消除，debug 和不合格调用保留，并核对实际外部 immortal 引用及精确 roots。

三个目标各通过 `scoop.std.tests.hash.` 的 basic、callbacks、iteration、model 四个 Cone 测试；本机另用新编译器直接消费旧 sysroot 编译、运行原始 strmap，输出正确，主值构造展开而缺失成员正文正常回退。未运行完整 fixture 或全库测试。开发 sysroot 按依赖顺序更新已有完整库集合（本机 22 个、Linux 每目标 21 个），以保持新导出与消费结果一致。

## 性能与成本

使用用户 `/tmp/scoop-bench` 的原始 dispatch、values、strmap 源码。旧／新编译器分别消费自己重建的 core／std，runtime 和 native 输入相同。每个负载一次预热、三对交错独立进程；关闭 GC stress 和诊断，逐次核对输出。原始记录在仓库忽略的 `tmp/program-performance/body-export/`，Linux 原始记录在 `nuc12` 的同名目录。

| 负载 | Darwin：旧 → 新 | GNU：旧 → 新 | musl：旧 → 新 |
| --- | --- | --- | --- |
| strmap | 3.431220 → 3.261878 s（−4.94%） | 4.613048 → 4.422647 s（−4.13%） | 6.463240 → 6.260917 s（−3.13%） |
| dispatch 对照 | 1.743927 → 1.738540 s（−0.31%） | 1.353690 → 1.282936 s（−5.23%） | 1.293866 → 1.289425 s（−0.34%） |
| values 对照 | 0.129874 → 0.132806 s（+2.26%） | 0.150083 → 0.150649 s（+0.38%） | 0.162899 → 0.163067 s（+0.10%） |

**三个目标的 dispatch 和 values 旧／新完整二进制分别逐字节相同**，上表对照变化属于运行波动。strmap 在 GNU／musl 的三对样本均改善，Darwin 一对回退 0.69%、另外两对改善 4.94%／6.86%；保留这些有限样本的波动，不把中位数当作所有工作负载的固定收益。strmap 运行峰值 RSS 的中位数分别变化 +1.89%／−0.01%／+0.20%，本项没有减少堆内存的结论。

| 目标 | strmap 代码段变化 | 相关静态数据变化 | 完整文件变化 |
| --- | ---: | ---: | ---: |
| Darwin | +52 B | −728 B | −560 B |
| GNU | +64 B | −728 B | −600 B |
| musl | +64 B | −728 B | −600 B |

静态数据按相关节汇总：Darwin 为两个 `__const` 加 `__llvm_stackmaps`，GNU 为 `.data.rel.ro.scoop`，musl 为 `.scoop_metadata`；包含实际内联导致的 site／stackmap 变化。dispatch／values 的上述空间指标均不变。

库产物会多携带可导入正文。以下是**完整 `.slib`** 的增长，包括 HIR／MIR／LIR metadata 和实际机器码变化，并非仅测 HIR 正文。

| 库 | Darwin | GNU | musl |
| --- | ---: | ---: | ---: |
| core | +36,000 B（+0.086%） | +33,252 B（+0.067%） | +34,466 B（+0.069%） |
| std | +194,130 B（+0.797%） | +191,176 B（+0.662%） | +199,950 B（+0.692%） |
| test | +182,898 B（+7.629%） | +204,296 B（+7.019%） | +203,010 B（+6.974%） |

consumer 编译成本直接运行 `scoopc build`，排除 dump 和最终链接，一对预热后取三对交错中位数；墙钟变化如下，峰值 RSS 变化范围 −0.57%～+1.79%。

| 负载 | Darwin | GNU | musl |
| --- | ---: | ---: | ---: |
| dispatch | −0.74% | −0.50% | +0.07% |
| values | −0.32% | +0.78% | −2.50% |
| strmap | −0.87% | +0.58% | +2.54% |

本机另独立测量 std producer，相同的一对预热加三对交错编译：墙钟 9.408277 → 9.594443 s（+1.98%），峰值 RSS +0.11%。这部分代价发生在导出库重建时。通用能力在现有有界选择下获得收益，生产库增长小于 1%，样本程序没有明显代码膨胀，沿用 release 默认设置。minor 脏区枚举／清零、survivor 与晋升年龄，以及 callable／stackmap 裁剪继续作为后续优化。
