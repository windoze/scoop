# 接口父表直接投影

状态：实现与定向验收完成。属于当前程序性能优化，不依赖 PGO。

本记录对应 `a3d3b0bb1` 的交付与测量；其中仍独立执行的接口 `is` 与首次取表，已由后续[接口检查与视图建立](INTERFACE-CHECKS.md)进一步合并。以下基线、查找次数及空间数据保留原测量含义。

## 问题与现有语义

接口值已是 `{ object, itab }`。修改前的 `Child → Parent` 在实际对象类型未知时通过对象 TD 的目录取得 Parent 表；缓存只能减少第二次查找。这里改为第一次上转型就直接从 Child 的表得到父视图。

`concretize/interfaces/members.rs` 目前按父声明序收集方法，对 exact interface application 的菱形路径去重，删除被 typed override 压制的槽，再追加当前声明。class、value 和 default 的实现选择已在 HIR 完成，MIR 按这些选择生成 receiver adapter。这个逻辑方法序不是完整父表布局：override 和多继承都可能改变索引，不能把当前 Child 表直接作为 Parent 表。

保留语言的逻辑方法序与实现选择。物理布局单独由 LIR lowering 确定，使用原方法身份、完整参数／结果／GC ABI 和 exact interface application，不以方法名或擦除后的签名匹配。

## 物理布局

采用完整内嵌父表。一个 exact interface 的布局与具体实现类型和构建 profile 无关：

1. 按直接父接口的声明序，依次放入每个父接口的完整物理布局。单继承的父表从 offset 0 开始；多继承的后续父表从前面完整父表的末端开始。
2. 菱形的重复片段保留。每个片段内部的槽序与独立父表完全相同；从当前视图直接投影到某个 exact 祖先时，选其声明序中第一次出现的片段。先投影到不同直接父接口再到共同祖先，可以得到不同地址的等价片段；接口身份仍只比较 object。
3. 对 Child 的每个逻辑方法，若已有片段包含同一原方法、同一 exact 声明实例且调用 ABI 相同，映射到其已有物理槽。否则追加一个槽。typed override 的新方法身份不覆盖父表位置；父表位置仍使用实际类型对原父契约的最终实现／adapter。
4. 有多个不同 exact 声明实例、无法唯一对应的逻辑方法保留自己的槽，不因物理签名相同合并不同泛型契约。

例如 `Root=[r]`、`Left:Root=[r,l]`、`Right:Root=[r,q]`、`Child:Left,Right` 的物理表是 `[r,l,r,q,...Child 新槽]`。Parent 表不是某个方法集合，而是可直接被既有父接口调用 ABI 消费的完整连续片段。

每个具体实现类型 T 都使用已经选定的 `T implements I` 的逻辑入口填充这些片段。内嵌 `T.Parent`，不是接口声明的 default 表，也不是另一个类型的表。同一 exact 父接口的各片段复制同一组入口指针；不复制方法机器码，不新增 receiver wrapper 或对象字段。

## 存储与产物

对 T 的实际接口闭包，只为没有更派生接口覆盖的非空表分配完整数组。父接口目录项和正常 typed 表符号使用这些数组内的连续片段；多个最派生表都包含同一父接口时，按 exact interface identity 排序选择第一个数组及其中第一次出现的片段。选择不依赖 arena 编号、Cone 导入次序或 profile。必要的菱形槽重复存在，但不再同时保留独立扁平父表。

空接口保持合法 null itab，目录中仍有其 exact TD。地址化的空表身份沿用既有空定义，不参与非空片段运算。

LIR 保留逻辑入口及完整的物理槽来源、存储表和固定偏移。`.slib` 的 layout/ABI section 保存这套实际布局，逻辑方法位置和物理位置明确区分；跨 Cone 消费根据同一 exact 父关系和原方法身份形成相同布局。旧 layout/ABI capability `/16` 升为 `/17`，旧产物及相关缓存重建。其他语言、对象和 GC 表示不变。

原有 typed 父表符号可作为只读数组片段的普通 native alias。其范围可以与包含它的只读 dispatch 表重合；对象 reader 保留各视图的实际范围和 relocation，不能把共享字节误判为两个独立存储，也不能放宽普通函数、可写存储或其他 atom 的重叠规则。数组和所有父表视图放在同一个 native object member，包含 ODR 表；typed definition 身份保持独立。元数据拆分只由数组定义管理实际存储，视图只管理自己的符号。布局范围和调用签名在各自正常产物边界检查，不引入授权、来源证明或第二套 runtime 校验。

## 表达式与调用

- 已知 Child 的真正 Parent 值：保留当前 object，从已有 itab 进行固定偏移调整；单继承前缀偏移为零，直接复用。transitive parent 使用预先计算的总偏移。
- 通过 Child 调用继承方法：直接在 Child 表中读取原目标契约对应的物理槽；lowering 不为这次调用物化 Parent 聚合值。
- smartcast 成功分支已经形成的 Child 视图使用相同路径。不能为了折叠 Retype 把这个视图退化为 Any 后再查父表。
- Any 首次取得接口、无静态父关系的接口转换、需要从单字引用恢复接口的 ABI adapter，仍按其语义取得目标表。已知实际具体类型继续可直接使用静态表。`is` 返回 Boolean 的协议本项不改。
- 动态路径的块内缓存只保存 GC-free itab，不额外保存 object；修改局部值、取址或切换 LIR 块时遵守已有失效规则。

接口身份、值装箱、单字隐藏 receiver、接口双字参数／结果、大值传参与返回、NoGc→Managed adapter、移动 GC 仅扫描 object 的合同保持。方法 override 继续要求既有的精确返回类型，不新增协变 override。这个布局是所有 profile 的共同 ABI；不为 debug/release 创建不同布局或运行期开关。

## 验证与默认策略

优化／ABI 专测覆盖单继承、多继承、菱形、override/default、不同泛型实例、空接口、装箱及跨 Cone 产物组合；普通库行为使用所属 Cone 的测试。定向 GC 测试覆盖 safepoint 后的视图，检查 roots 不因表复用增加。

用 typed LIR 和机器码分别确认上转型与继承调用没有 `itable_lookup`。性能对照保留旧编译器与旧程序，采用相同输入、预构建程序交错多次运行，并报告每项耗时而非只报告最佳值。另比较完整表指针数量、最终二进制体积和编译成本，包含标准库中的 MutableSortedMap／MutableSortedSet 菱形。

固定布局和静态投影作为正常默认能力；高成本的图变换或其他不稳定优化另行实验。若真实层次测量显示完整内嵌造成明显膨胀，再以固定父表指针共享替换布局，而不是预先建立多套布局插件。本项不引入 PGO。

## 生成代码核对

基线为 `7971a2dcc`，候选同时包含静态父表投影和块内动态 itab 复用。基准源码见 [interfaces](../../tests/benchmarks/interfaces/README.md)，provider 的公开入口从独立 consumer 调用，避免调用方内联抹去未知实际类型。以下是同一函数中的 `itable_lookup` 调用点数量，不是整个程序的查找总量：

| 入口 | 基线 | 候选 | 候选路径 |
| --- | ---: | ---: | --- |
| `parentView`、`diamondLeft` | 各 1 | 各 0 | 前缀直接复用 itab |
| `secondaryView`、`diamondRight` | 各 1 | 各 0 | 分别调整 8／16 bytes |
| `repeatedParent`、`repeatedSecondary`、`repeatedDiamond` | 各 2 | 各 0 | 直接加载 Child 中的物理槽，无中间 Parent 聚合值 |
| `smartParent`，Any 成功收窄后两次调用 | 2 | 1 | 第一次动态取表，后续复用 |
| `smartChild`，Any 成功收窄后两次继承调用 | 4 | 1 | 一次取得 Child，继承调用直接使用其表 |

Darwin ARM64、Linux GNU／musl x86-64 的 typed LIR 和最终机器码均已核对上述路径。正常 Managed 入口 poll、间接调用和必要 roots 仍存在；没有把整个函数描述成单条指针调整。`is` 返回 Boolean 的协议没有改变，其类型检查成本仍计入运行时间。两个重复 Managed 调用的代表性根计划没有因缓存增加对象根；定向移动／minor GC fixture 验证 safepoint 后对象与视图继续有效。

## 运行与空间测量

同一台 Apple M3 Ultra，macOS 27.0.1，LLVM 22.1.8，release。每种模式 2,000 万轮、每轮两次 UInt 变换，已知输出 `4287603176`。先构建双方程序，再用 A1 的 `compare.py` 预热一对、交错执行九对；关闭 GC stress 和统计。表中是独立进程总墙钟中位数，耗时下降按 `1 - candidate / baseline` 计算，不混用 kernel 计时或声称统计显著性。

| 模式 | 基线 ms | 候选 ms | 耗时下降 |
| --- | ---: | ---: | ---: |
| 4：Child 单继承调用 | 144.928 | 116.799 | 19.41% |
| 5：Any → Parent smartcast | 176.519 | 164.872 | 6.60% |
| 6：Any → Child smartcast 后继承调用 | 223.660 | 175.676 | 21.45% |
| 7：多继承的第二父接口调用 | 164.563 | 117.012 | 28.90% |
| 8：菱形共同父接口调用 | 165.163 | 117.302 | 28.98% |

各模式进程 CPU 中位数同向下降。峰值 RSS 中位数增加 48～64 KiB，不能把静态表缩小解释成进程 RSS 必然下降。这是当前接口热点的累计改进，尚不代表一般应用或相对 Go／Java／Kotlin/Native 的综合结论。

Linux 在同一台 Core i7-12700H、Ubuntu 26.04.1、LLVM 22.1.2 上复用相同的输入、预热及交错次数，双方进程都固定在逻辑 CPU 0。以下仍为独立进程总墙钟中位数，不跨平台比较绝对耗时。

| 模式 | GNU 基线 → 候选 ms | GNU 耗时下降 | musl 基线 → 候选 ms | musl 耗时下降 |
| --- | ---: | ---: | ---: | ---: |
| 4：Child 单继承调用 | 127.813 → 81.143 | 36.51% | 153.877 → 107.321 | 30.26% |
| 5：Any → Parent smartcast | 174.070 → 138.511 | 20.43% | 198.761 → 169.138 | 14.90% |
| 6：Any → Child smartcast 后继承调用 | 243.303 → 146.579 | 39.75% | 268.914 → 178.128 | 33.76% |
| 7：多继承的第二父接口调用 | 164.522 → 102.587 | 37.65% | 200.803 → 107.350 | 46.54% |
| 8：菱形共同父接口调用 | 164.933 → 103.206 | 37.43% | 199.565 → 107.298 | 46.23% |

GNU／musl 各模式的 CPU 中位数同向下降，峰值 RSS 中位数不变。每个模式的九对样本均为候选更快；保留全部波动样本，不以此推断其他负载的收益或统计显著性。

静态表按原生对象中的 typed dispatch 符号及真实 boundary 范围计数，重叠视图按字节并集只算一次；空表的 1-byte 地址化定义单列，不计作指针槽。节内对齐、符号和 relocation 不计作表定义 bytes，另用完整文件体积观察总成本。没有用逻辑 LIR dump 中的槽数估计实际存储。

| 对象范围 | 非空物理指针槽：基线 → 候选 | 包含空表的实际 bytes：基线 → 候选 |
| --- | ---: | ---: |
| 三个目标的接口基准 provider | 15 → 9 | 181 → 133 |
| 三个目标的完整接口基准程序 | 667 → 547 | 6,915 → 5,955 |
| Darwin `scoop.core.slib` | 652 → 538 | 6,735 → 5,823 |
| GNU 标准库集合探针全部表 | 278 → 167 | 2,383 → 1,495 |

集合探针通过真实 `MutableSortedMap<Int, String>`／`MutableSortedSet<Int>` 工厂物化 TreeMap、TreeSet 及相关 iterator／view；278 → 167 是探针全部表的合计。各范围的 typed 表身份数量和空表数量保持，减少来自共享父片段。上述实际层次未出现总表空间膨胀，因此本轮不增加固定父表指针模式。

完整接口基准可执行文件体积如下。Darwin provider `.slib` 从 1,233,486 降至 1,232,038 bytes；GNU 集合探针 `.slib` 从 7,586,726 降至 7,492,876 bytes。完整文件包含符号、机器码和其他元数据，不能把其差额全部归为表数据。

| 目标 | 基线 bytes | 候选 bytes | 减少 bytes |
| --- | ---: | ---: | ---: |
| Darwin ARM64 | 12,988,312 | 12,978,024 | 10,288 |
| Linux GNU x86-64 | 12,669,120 | 12,658,312 | 10,808 |
| Linux musl x86-64 | 12,265,880 | 12,263,264 | 2,616 |

## 编译成本与验证记录

三个目标用同一 provider 源码、各自兼容的 core 产物、每轮新建普通 cache／target，预热一对后交错五对 release 编译；Linux 同样固定逻辑 CPU 0。以下为独立构建进程中位数，包括其子进程资源统计。

| 目标 | 墙钟 s：基线 → 候选 | CPU s：基线 → 候选 | 峰值 RSS bytes：基线 → 候选 |
| --- | ---: | ---: | ---: |
| Darwin ARM64 | 1.047924 → 1.047487 | 1.012489 → 1.016690（+0.41%） | 476,430,336 → 482,738,176（+1.32%） |
| Linux GNU x86-64 | 1.391752 → 1.371664 | 1.285360 → 1.289199（+0.30%） | 472,768,512 → 474,349,568（+0.33%） |
| Linux musl x86-64 | 1.308772 → 1.303582 | 1.259360 → 1.246621（−1.01%） | 472,821,760 → 474,578,944（+0.37%） |

该负载编译耗时基本持平，峰值 RSS 略增；不能外推为大型应用编译完全没有成本。

本机已通过 MIR／LIR／lowering、codegen、`.slib`、linker 的相关完整测试目标；新增 ELF COMDAT 专测通过。接口优化 debug／release、跨 Cone provider／consumer 反向 profile、独立重新链接和定向 GC 专测通过；core 普通测试 Cone 54 项与 std 104 项通过。ABI 组合包含 class override、interface default、装箱 struct、NoGc → Managed adapter、含引用大值参数／结果、泛型 exact 实例及空接口。同一个 Child 继承 `ExactLabel<Int>`／`ExactLabel<String>` 时显式消解方法冲突，分别核对两份父视图及第三种未实现实例的身份检查。

Linux 完整 Rust 工作区通过 5,953 项，零失败／忽略；默认线程栈在既有 HIR 大测试中溢出，设 `RUST_MIN_STACK=33554432` 后完成全量，没有修改产品代码规避。musl 环境用工作目录中的普通 CC 包装脚本添加系统 Linux UAPI 头，21 个系统 Cone 与 runtime 重建通过，未混入 glibc 用户态头或修改 vendor。

GNU 的跨 Cone 接口产物、接口视图复用和条件接口 GC roots 定向组通过，另补跑条件接口 ODR 用例通过。musl 的 95 个接口／优化／GC／ABI／PIE fixture 全部通过，共 194 个变体、587 个进程；另补跑条件接口 ODR 用例通过。本项正确性验收以这些相关组合为准。已启动的 GNU 完整 CLI 批次按用户收窄验证范围的要求停止，不计作完整验收；后续每项改动只运行足以证实其正确性的定向组。

完整 CLI 首轮发现既有 `conditional-interface-odr` 将依赖数写死为 5，而此前主线已把 std 加入 JSON 的生产依赖。同步为 6，并保留完整依赖逆序重新链接后，三个目标的构建、两次运行及重新链接均通过。产品代码没有为此增加特殊处理。

原始样本、stage dump、机器码、编译日志和探针源码仅保存在两机忽略的 `tmp/program-performance/b2-*`；临时目录名不对应调研路线中的 B2 数组内核。
