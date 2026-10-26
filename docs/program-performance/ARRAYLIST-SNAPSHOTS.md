# ArrayList 的普通快照正文

ArrayList 新增普通 `toArray()`／`toMutableArray()` 成员，一次捕获逻辑长度和 backing，按索引复制已经初始化的前缀。成员正文使用现有 Array initializer 和 `MaybeUninit.assumeInit()`，沿原调用点评分展开；不增加按函数名识别、专用 intrinsic、接口槽位、对象字段或 runtime 后备。

快照保持独立数组存储与元素浅复制，不读取剩余容量；GC 后使用恢复的源引用。静态 `List<T>` 和用户 getter 仍走原扩展，保留 getter 次数、顺序及异常。API 与解析合同见语言 spec §11.10.2～11.10.3。

本项建立在[整字原始存储](RAW-VALUE-STORAGE.md)上。仅添加成员而保留逐字节 LLVM 存储的早期试验发生回退，没有单独采纳。最终实现采用普通 core 算法与通用存储表示；全部 profile 提供相同 API，release 按现有调用点策略优化，无新开关或诊断代码。

对照保持源码、接口布局和同一 runtime，分别构建 GC 修复后的基线、仅整字存储、再加本项成员三个版本。新增[快照负载](../../tests/benchmarks/arrays/list-snapshots.scoop)保留每轮两份实际数组分配，覆盖非满容量及独立写入；`lambdas` 通过普通 std 集合变换消费这些成员。原始日志和逐次数据位于仓库外 `scoop-performance-results/2026-10-26/array-list-snapshots/final`。

三目标最终机器码中，`lambdas` 的目标 `List.toArray` 调用及其逐元素接口 getter 路径被直接数组复制替代，相应独立实例被裁剪；原 inliner、范围证明和有界循环继续工作。快照基准的 `toArray` 展开，`toMutableArray` 仍按成本保留普通共享方法，方法内部也直接复制 backing。其他实际使用的 `List` 扩展、ArrayList getter、分配慢路径和必要 roots 继续保留，没有扩大内联阈值或强制展开所有调用点。

Darwin ARM64、Linux GNU／musl 的三项新 core 快照测试、两项既有列表测试和四项 std 集合组合测试全部通过，覆盖非满容量、clear／remove 后的前缀、独立写入、引用浅复制、ZST、嵌套 Optional，以及自定义 List 的 getter 次数和异常。同批两项原始位测试、跨 profile ABI 和 MaybeUninit GC 专测保持通过。引用数组 GC fixture 增加宽值／可空泛型接口的快照组合，包含 class 与 boxed 实现、身份比较、moving、minor 和 survivor；三个目标均通过，没有扩大为全量验证。

2026-10-26，M3 Ultra 与 i7-12700H 实机、一次预热及五轮正反序交替。下面单列成员正文相对整字存储的收益，并保留两项合计；Linux 固定 CPU 4，工具链与测量限制同前项。

| 负载／阶段 | ARM64 Darwin | x86_64 GNU | x86_64 musl |
| --- | ---: | ---: | ---: |
| lambdas：成员正文 | −19.81% | −28.07% | −33.01% |
| lambdas：两项合计 | −30.06% | −52.74% | −53.39% |
| 快照：成员正文 | −45.01% | −48.06% | −50.11% |
| 快照：两项合计 | −46.25% | −51.49% | −46.53% |

三目标的 lambdas 分配均为 738,736,680→738,096,680 B，minor／full 保持 781／1；复制字节为 6,737,728→4,159,296，root_slots 为 55,366→54,896，旧代引用槽扫描为 314→0。快照内核仍分配 1,644,832,936 B、发生 2,083 次 minor；root_slots 从 143,727 增到 145,810，即每次 minor 多一槽，源引用跨分配及共享调用仍需保活。

成员正文使 lambdas 的 text 再减少 404／512／512 B，完整文件减少 2,304／6,848／6,840 B。快照 text 分别变化 +48／−192／−192 B，完整文件增加 320／648／648 B。四个库的单库增量最多 0.225%，没有 runtime、接口表或对象字段增长。

三次独立 consumer 冷构建的墙钟变化：lambdas −6.18%／+0.91%／+0.60%，快照 −0.33%／−0.45%／+0.13%；CPU 变化 −21.08%～+0.90%，峰值 RSS 最大增加 0.08%。其余七个负载在成员正文前后分别为相同二进制，时差不计为本项收益。原主对照中 ARM64 接口数组合计 +11.44%、musl dispatch／`List<Long>` 合计 +6.73%／+5.96% 等差异与前项的定向复核均保留，未以选择较好样本替换。

采纳为默认 core 能力；core 及已静态解析调用的依赖 Cone 需重建。未知 List、未展开调用及引用／宽值的必要存储和屏障继续沿现有路径；不把普通 initializer 统一替换成 memcpy，不承诺任意 T 或每个调用点都能向量化。

合入 `origin/main` 的 `8caf37694` 后，格式化与相关 release Clippy、HIR 求值位置回归及五项列表行为补充检查通过；该合并未改变本项后端或 runtime 实现，前述三目标 GC 与性能对照继续保留。
