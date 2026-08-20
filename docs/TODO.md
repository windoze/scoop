# 待办事项

本清单从规范原有的后续事项中筛选，并对照当前源码、正式 fixture 和里程碑记录核实。这里只列出仍未完成的语言能力、工具与运行时缺口，以及有实际问题依据的优化；不表示这些能力已经可用。涉及新语言行为、公开接口或 ABI 的任务，先修订对应规范，再实现。

## 语言功能

- [ ] **补齐 `do-while` 与带标签的控制流。** 先明确标签作用域、合法跳转目标及 callable 边界，再贯通解析、类型检查和控制流输出。完成后，`continue` 正确到达相应循环条件，跨层跳转只执行真正退出的 `finally`、catch 和 Context 清理；覆盖嵌套循环、异常与挂起组合。

  依据：语言规范 2.1、8.7；[parser](../compiler/parser/src/stmt.rs) 仍明确拒绝 `do-while` 和循环标签，[对应测试](../compiler/parser/src/tests_m22_control_flow.rs) 仍断言 `break@`、`continue@`、`return@` 不支持。

- [ ] **实现源码可命名的 `Nothing` 与一般 jump expression。** 让底类型参与签名、泛型实参、类型转换和分支类型合流，支持 `value ?: break`、`value ?: return` 等表达式位置的控制转移。不可达路径不能伪造普通值；所有阶段保留真实退出目标、异常和清理语义。

  依据：语言规范 3.1；[路线图的 M2 后续事项](ROADMAP.md#来自-m2)；[parser 的 jump-expression 测试](../compiler/parser/src/tests_m22_control_flow.rs) 仍要求拒绝一般 jump expression，[异常 lowering](../compiler/hir-lower/src/stmt/exceptions.rs) 仍未使用完整底类型表示。

## 编译器与产物

- [ ] **提供源码级调试行表。** 将源码文件、函数和行列位置传递到 LLVM 调试信息及最终程序，优先完成当前 Darwin/Linux 目标上的源码断点、单步和栈回溯。默认值、泛型实例及生成的桥接正文应有准确的来源归属；优化后的缺失位置如实表达。

  依据：原编译器规范的调试信息后续项，以及 [M31 的后续范围](milestone31/DESIGN.md#9-明确留待后续)。当前 [codegen](../compiler/codegen/src/function.rs) 未生成源码级 LLVM debug metadata；现有 unwind/stackmap 与 `SourceLocation` 不替代调试行表。

- [ ] **降低实际编译与链接开销。** 使用现有工作负载区分冷构建、热构建和各阶段耗时，定位并减少重复产物读取、查询、对象处理或工具启动等实际热点。记录改动前后的耗时与产物规模，保留必要的类型、ABI、GC/EH 和对象检查。

  依据：[M31 实施记录](milestone31/PROGRESS.md) 已记录对象分区等路径的编译开销；[性能报告](milestone31/PERFORMANCE.md) 保留冷／热构建观测，并明确将编译速度优化留作后续工作。

- [ ] **完善实际查询依赖，缩小无关变更导致的重编译范围。** 记录名称查询的空结果、不适用候选、star/re-export 表面，以及默认值和模板实际使用的依赖。只有覆盖完整后才缩小缓存失效范围；此前继续保守纳入相关依赖，避免漏掉新候选或绑定变化。

  依据：原语言规范 12.5 的 lookup observation 后续项；当前 [缓存输入](../compiler/scoop/src/snapshot/prepared/model/cache_key.rs) 仍逐项使用全部直接依赖的 HIR/MIR/LIR 语义指纹。完成时用真实多 Cone 构建验证“无关变化可复用、影响决议的变化必失效”，不另建查询证明或预算体系。

## 运行时

- [ ] **补齐公开 runtime 入口的签名与错误处理规范。** 按现有入口整理参数、结果、调用约定、GC/异常效果、线程状态、root 与所有权义务，区分源码异常、返回状态和 fatal ABI error。把缺少的契约写入运行时规范，并与公开头文件及编译器调用合同核对。

  依据：原运行时规范的“完整签名表与错误处理矩阵”后续项；[scoop_rt.h](../runtime/include/scoop_rt.h) 和 [runtime ABI 合同](../compiler/lir/src/runtime_abi.rs) 已有真实入口，而运行时规范第 4 章仍主要按能力描述。此项补齐文档及实际合同差异，不要求重写现有 runtime。

- [ ] **解除 GC 堆固定单 arena 的容量限制。** 设计并实现按实际需求增长的堆存储，明确分配失败行为，保持已有对象、pin、card table、对象起点与精确扫描关系有效。验证超过初始容量、增长失败、含 pinned 对象及 minor/full 交替的情况。

  依据：原运行时规范的 arena 扩容后续项；[heap_internal.h](../runtime/src/gc/heap_internal.h) 固定 `GC_ARENA_SIZE = 1 << 30`，[heap.c](../runtime/src/gc/heap.c) 一次建立单个 arena，[allocation.c](../runtime/src/gc/allocation.c) 在其耗尽时终止。扩容方式由真实地址与分配约束决定，不预设多套 allocator。

- [ ] **改善已测得的高存活年轻对象回退。** 以已有高存活图、少量旧代脏边和密集旧数组写入工作负载比较收集频率、晋升与实际复制量，再选择是否采用 survivor、调整晋升或触发策略。保持精确根、写屏障、pin、release hook 和失败出口的既有合同。

  依据：[M31 性能报告](milestone31/PERFORMANCE.md) 记录 `survivors` 在 NUC GNU/musl 上分别慢 26.0% / 20.8%，同版本 nursery/full-only 的实际复制量分别为 6,365,264 / 917,672 bytes；首次存活即晋升的策略仍在使用。以这些实际问题判断方案收益，不把增加代数或并行 collector 本身当成完成条件。

## 基础库接口

- [ ] **提供受检的 UTF-8 字节解码入口。** 明确非法序列、截断、多余字节、越界码点和代理项的失败行为，并提供 safe 调用方式；合法输入生成独立、有效的 String，保留 U+0000。先确定接口与错误表达，再补普通 core/library 实现及正反例。

  依据：原语言规范 11.4 的通用字节解码后续项；当前 [String 接口](../sysroot/lib/scoop.core/src/strings.scoop) 只有 `fromUtf8Unchecked`，调用者必须自行满足合法 UTF-8 前置条件。

- [ ] **定义并实现 `CharRange`。** 明确 Unicode scalar 次序、跨代理项区间的行为、端点、step、正反向迭代与边界，再沿普通 Range/Iterator 接口实现并与 `for` 组合。不能直接把 UTF-16 code unit 或任意整数区间当作合法 Char 序列。

  依据：原语言规范 11.8 明确留待后续的字符区间；当前 [ranges.scoop](../sysroot/lib/scoop.core/src/ranges.scoop) 仅有四种整数 Range，[Char](../sysroot/lib/scoop.core/src/characters.scoop) 已采用 Unicode scalar 语义。

- [ ] **设计并实现 off-heap ByteBuffer 与必要的外部内存反馈。** 先确定所属普通库、容量增长、borrow/view 有效期、显式 close 和失败原子性，再用现有 FFI 与 release hook 完成资源管理。跟踪实际 native 分配与释放，向 managed GC 提供压力反馈；hook 路径只做允许的释放与扣减，不依赖及时 GC 或把 best-effort hook 当作确定性 close。

  依据：[从 M26 移出的事项](ROADMAP.md#从-m26-移出的后续事项) 及原运行时规范后续项。当前 sysroot 没有 ByteBuffer，runtime 没有对应外部内存反馈入口；现有 [GC/FFI 接口](../sysroot/lib/scoop.core/src/gc.scoop) 与 release hook 是可复用基础。

## 筛选边界

已完成的 nursery/minor GC、晋升与 remembered set、精确移动 GC、managed callback、跨 Cone 产物与链接、单文件模式、集合与字符串构建、Task-local Context、Float/Double、自有异常 ABI 和警告基础设施不重复列入。

“异常能否穿越 FFI frame”已有现行规则，不再保留旧 TBD；catch 遮蔽目前明确为编译错误，旧的“等待警告基础设施后改为 warning”也不自动成为待实现要求。

Windows／更多 target、parallel/concurrent GC、接口方法级泛型、generic typealias、class delegation、额外捕获语法、suspend FFI、无 context 槽的任意 closure callback、platform-native integer，以及通用内联／逃逸分析／LTO 等条目，目前只有扩展方向或条件性设想。只有出现明确使用需求或测量依据、并完成相应语言／ABI 决策后，才转为具体任务。运行期反射、GC finalizer、来源授权证明和通用资源预算不列入待办。
