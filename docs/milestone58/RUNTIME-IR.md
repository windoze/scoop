# M58 runtime C 到 LLVM IR

已有 TLAB cursor 检查／推进、条件 poll 和单引用卡表屏障不重复实现。候选审计见 [局部消除设计](ALLOCATION-ELISION.md#5-尚未消除的-runtime-调用ir-候选)；本文件记录实际迁移范围。

## String 有界读取

普通 typed `StringByteLength`／`StringByteAt` 调用在 codegen 的现有 call 合同检查后展开，不按符号名识别。长度为 offset 16 的 i64 load；byte-at 先做 unsigned index < length，再从 offset 24 的数据区加载 i8，保留 AS1。String 长度不超过 INT64_MAX，因此一次比较也排除负索引。正常路径没有 C 调用或 safepoint；错误路径复用原 byte-at leaf 的 fatal 诊断。invoke 形态仍走原发射路径，UTF-8 单字符解码和复杂字符串算法保留 C。

没有更改语言、LIR 或 runtime ABI，也没有给一般 NoGc 调用增加 readonly／无副作用属性。原 native 入口继续可用；显式同名 extern 不参与展开。普通优化在展开后运行，最终 roots／RS4GC 仍统一处理外围的真实 GC 调用。对象跨 GC 后使用正常恢复的值，不保留旧 derived pointer。

本机 fmt／workspace clippy、精确 String LLVM 测试通过；测试检查 AS1 load、unsigned bounds check、失败边和没有 native transition。新源码三变体与 M56 String 八变体通过，覆盖空串、NUL、多字节 UTF-8、首次／末尾字节、复制独立性、显式 native ABI、异常、普通／强制移动 GC 与 minor。

这部分主要位于预编译 core。对照以 `e5c1f474a` 为基线，将同一 core 源码分别用前后编译器重建到两个私有 sysroot；其他库、runtime、app release 和机器相同。不能只换应用编译器却继续测旧 core。11 对交替运行中，字符遍历 45.625 → 42.144 ms（约 −7.6%）；字节数组转换 29.410 → 28.630 ms（约 −2.7%）；重复 byteLength 34.906 → 34.305 ms，差异较小且区间重叠。text 只减少 8 bytes，不宣称显著 footprint 收益。完整方法见 [性能记录](PERFORMANCE.md#7-string-读取迁移)。开发与发行 sysroot 的最终准备须使用新编译器。

## 静态布局拆箱

LIR 的已有 boxed descriptor 保留选定 shape 的 payload offset，不另推导布局或增加 wire 字段。codegen 从有效 AS1 对象读取实际 TD，与 expected TD 相等时，将完整 payload 字节复制到已有 typed local；保留 padding 和 packed alignment，含引用值仍进入普通根计划。ZST 只检查类型，不生成虚假存储。非 exact 情况继续调用原 C leaf，保留整型 enum 到父整数的合法转换及原错误行为；C 入口的检查没有删除。

格式化、workspace clippy、LIR descriptor 两项与 codegen boxing 五项测试通过。精确断言覆盖 AS1、payload offset、完整复制、慢路径和 RS4GC。新增源码三变体与原根恢复／unbox-clone 七变体通过，包含真实移动 GC、嵌套引用、packed、ZST、enum 父整数、异常和 padding 字节标记。按值的 LLVM aggregate load/store 不能保证保留可观察 padding，因此最终实现使用 memcpy，再由现有标量管线优化。

真实数组中逃逸 box 的 11 对交替测量中，四 Long 值扫描 248.624 → 200.357 ms（−19.4%），含引用嵌套值扫描 181.524 → 129.450 ms（−28.7%）；text +116 bytes，文件大小不变，编译时间基本持平。方法和限制见 [性能记录](PERFORMANCE.md#8-静态布局拆箱迁移)。

## 静态数组 clone

复用 typed ArrayClone 已有的源 live root、元素 exact type 与完整 layout。源是相同布局的有效数组，其不可变 count 已满足尺寸边界；生成代码复用普通 sizing／TLAB／慢分配路径，按目标 nominal TD 分配新对象并写入 count。分配后从 canonical root 重读源，随后才形成 AS1 payload 地址并复制完整 stride 字节，含引用元素发布前执行既有范围屏障。ZST 只保留 logical count 和独立对象身份。原 C 入口及动态检查继续可用，没有更改语言、LIR wire 或 runtime ABI。

原 site 只标记实际分配慢调用，最终 roots／RS4GC 顺序保持；入口和源码循环 poll 保留。精确测试检查只剩一个实际 site、恢复后的源地址、AS1 memcpy、范围屏障顺序、目标 TD 和 ZST。fmt／workspace clippy、数组相关八项与 statepoint 十八项测试通过；新旧源码八变体通过，覆盖 moving／minor、含引用 large array、复制后修改源、packed、padding、ZST、空数组及异常。

相对 `171b70ed6` 的 11 对生产程序测量，小型无引用／含引用数组复制均约快 15%，大数组没有明确变化；text +380 bytes，文件不变，编译成本基本持平。因为 wrapper 握手改为普通分配慢路径握手，另以保留源码 loop poll 的一百万次 clone 测 stop-wait：单／四 worker 的每轮 p99 中位数为 13 → 11／74 → 60 µs，最坏单样本 99 → 118／161 → 140 µs，没有稳定尾部回退的证据。带观测点的 runtime 与生产计时分开；这不代表通用 GUI 或无调度噪声的暂停上界。完整方法见 [性能记录](PERFORMANCE.md#9-静态数组-clone-迁移)。

首批三个 runtime 迁移候选均完成实现、验证和对照；GNU／musl 完整组合验收通过，范围见 [实施记录](PROGRESS.md)。GC 协调与分配慢路径继续由 runtime 负责。
