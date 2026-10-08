# M33 实施与验收记录

开始日期：2026-10-08。基线：`670a45477`（M32 完成），分支：`codex/m33`。

本文件只记录实际完成的实现和验证；设计方案与验收要求见 [DESIGN.md](DESIGN.md)，未完成项目不计为通过。

| 批次 | 能力 | 状态 |
| --- | --- | --- |
| M33-1 | NativeSafe/collector、GCLeaf、DirectC | 完成并通过三平台验收；errno bridge 组合在 M33-5 补验 |
| M33-2 | 作用域数据借用、计数 pin | 完成并通过三平台验收 |
| M33-3 | 严格／可空／lossy UTF-8、C 字符串 | 完成并通过三平台验收 |
| M33-4 | main、argv、退出码、输出与 ABI 11/7 | 完成并通过三平台验收 |
| M33-5 | errno 捕获 | 完成并通过三平台验收 |
| M33-6 | native C/C++、系统库与源码选择 | 完成；Darwin/GNU C++、musl 拒绝与纯 C 回归通过 |
| M33-7 | sysroot 默认定位、Equality | 默认定位、NoGc 接口适配、显式及非泛型派生 Equality 完成；泛型与 tuple 派生实施中 |
| M33-8 | 原子类型、内存序与 GC | LLVM AS1 原子操作技术验证完成；语言与 runtime 实现待继续 |
| M33-9 | 线程退出规则与组合验收 | 遗留资源诊断与退出规则完成；里程碑组合总验收待继续 |

开发验证先格式化、lint，再执行受影响测试。运行真实 CLI fixture，覆盖源码、产物消费、链接与运行；新增行为保留独立／组合／negative／golden。全量测试集中在必要的回归节点，已有通过结果在输入不变时复用。

Linux 使用 `nuc12`。其 `~/repos/scoop` 有既存未提交变更，M33 测试使用其 `target/m33-linux/` 下的独立源码副本和构建目录，不覆盖既有工作。所有本机临时文件放在仓库 `tmp/m33/`，工具通过 `TMPDIR=<repo>/tmp` 使用仓库临时目录；Linux 副本采用同一约定。阶段结束后清理过期构建产物。

## M33-1a：NativeSafe 与 collector

- `MANAGED_PENDING` 并入原子 mode，移除独立 `managed_segment`；scanner 只在停稳后读取 anchor、根链和 parked 来源。公开 debug mode 保持既有编号。
- NativeSafe 发布与 RETURNING/phase 检查使用 seq_cst，与 collector 的 STOPPING/mode 检查配对。常见进出不取 world lock、不广播；阻塞返回保持冻结根并在唤醒后重新握手。NativeBorrowed 的非停止路径也不取 world lock。
- collector 条件等待采用 50 微秒的超时复查，等待期间释放 world lock，以 C11 `timespec_get` 取得截止时间，兼容严格 C11 的 musl 构建。保留 GC 开始、park、结束及初始化通知。world lock 下的 phase 转换已经保证单 collector，因此删除会与 world lock 反向取得的冗余 collector mutex。
- release runtime 使用 `-O2 -DNDEBUG`，完整 transition 链查重仅保留在 debug；必要边界与 LIFO 检查继续执行。编译命令和 runtime cache key 复用同一组优化参数。没有改变公共 runtime ABI；11/7 升级在 M33-4 实施。
- 新增三个受控交错：扫描中出现 RETURNING、返回线程先读到 RUNNING 后发起 GC、NativeSafe 发布不通知 collector。每个配置运行 20 轮，覆盖 O0/O2 及 minor/full relocation；检查真实对象和冻结栈根的回写。

已完成的验证：

- `cargo fmt --all`、工作区 `cargo clippy --workspace --all-targets`，后续仅改动 scoop runtime 构建参数时补做该 crate 的 clippy；C 文件经格式化和 `-Wall -Wextra -Werror` 检查。
- Darwin：18 项 `scoop-codegen` runtime collector 测试通过；删除冗余锁后同组复验通过。3 项 `scoop` runtime 构建／缓存测试通过。
- Linux GNU / clang 22.1 TSan：新增交错的 full/minor 两个配置，以及 moving-thread、nursery-thread、gateway、initialization-GC、nursery-mutators，共 7 个进程通过；`halt_on_error=1`，无 TSan suppression。
- Darwin 正式 CLI 回归 31 项全部通过：53 个变体、170 个进程、117 份 golden，覆盖 M13/M15/M27/M31 与新增 transition fixture。最终可移植时钟改动后另复验四个受控交错配置通过。
- Linux GNU / musl 分别定向回归 6 项：各 12 个变体、52 个进程、31 份 golden 全部通过，包含 callback、moving roots、context lifetime、nursery GC 以及新增 transition fixture。三平台新增 fixture 均覆盖 debug/release × normal/moving/minor，HIR/MIR 内容一致，LIR 按 target/profile 保存并在非更新模式下校验。
- musl 另有 PIE + artifact-only 重链接 fixture，debug/release 两个变体、16 个进程通过；原构建和独立链接的 link-plan fingerprint 一致，重新链接的程序通过 moving/minor GC 运行。最初 fixture 漏写工具的正常 stdout 预期，补齐后仅重跑此项。
- fixture runner 的 44 项公共规则测试通过。没有运行与本批 runtime 变化无关的全量语言 fixture。

性能使用 `runtime/tests/native_transition_benchmark.c` 和独立编译的 `native_transition_leaf.c`，同一 native 操作比较直接 C、NativeSafe、NativeBorrowed。GC 测量通过只在测试构建中存在的同步点区分 STOPPING→停稳与 STOPPING→RUNNING；production 无测试回调。完整条件、原始样本、吞吐与等待分布见 [PERFORMANCE.md](PERFORMANCE.md)：常见调用的全局争用显著减少，但超时复查使部分 GC 等待尾部增加，未把吞吐提升报告为所有 GC 延迟改善。

## Nursery 分配竞争修复

GCLeaf 并发 fixture 在 release + minor 压力模式下暴露了既有分配重试问题：其他 mutator 可以在收集结束后抢先占用单块 nursery，旧逻辑以尝试一次 minor、一次 full 的次数上限误报 arena OOM。nursery 容量竞争现在继续收集和重试，只有实际完成 full 后仍无法取得物理块才报告耗尽。内部 full 请求区分自己完成收集与加入另一轮收集，避免把加入 minor 当成已完成 full；完整收集后的尝试也不再受软 threshold 阻挡。公共 runtime ABI 不变。

验证覆盖真实双线程分配和 GC：测试在收集后连续三次安排另一线程抢先占用 nursery，再确认等待方成功分配；旧实现的临时副本在 O0/O2 均稳定复现原 OOM，修复后 O0/O2 均通过。其余 18 项 runtime collector 测试通过。Linux GNU / clang 22.1 TSan 的原 7 个进程与新增竞争测试均通过，未使用 suppression。C 格式化、严格编译检查及 `scoop-codegen` clippy 通过。GCLeaf 的真实 CLI 并发 fixture 在 debug/release × normal/moving/minor 下均通过。

## M33-1b：GCLeaf 调用模式

- `@GCLeaf` 只允许 C ABI extern，保留已有签名、unsafe、top-level、non-generic 和非 suspend 规则；C extern 显式 `@NoGC` 现在报错。旧 fixture 中的冗余注解已删除，普通 C extern 仍使用 NativeSafe。
- `CAbiCallMode` 从源码声明、可调用接口和泛型模板经 MIR 保留至 LIR。模式独立于 native symbol 的物理合同，同 symbol 的普通与 GCLeaf 声明可并存，链接合并不传播调用模式；跨产物 release 的 C 目标同样保留声明模式。
- LIR 的 `NativeGcLeaf` 是独立的完整调用分支，不带 safepoint/root plan。LLVM 调用保持 `gc-leaf-function` 和 `nounwind`，不发射 native 进出、statepoint、根发布或 relocation reload，也不添加纯函数／无内存副作用属性。实参求值中的分配、异常和正常 GC 继续执行。物理调用本批仍采用 StorageBridge，DirectC 单独实现。

验证记录：

- workspace fmt/clippy 通过；codegen、HIR、HIR-lower、MIR-lower、LIR-lower 的受影响测试组已执行，wire 固定向量随显式模式更新后定向复验通过。新增模式编码测试覆盖两种模式、缺失字段和未知 tag；新增 LLVM 测试在 statepoint rewrite 后检查 native 副作用仍在且没有 GC 边界操作。
- 三平台各 10 项正式 CLI fixture、13 个变体、51 个进程、24 份 golden 均在非更新模式下通过。覆盖标量／aggregate、同 symbol 双模式、显式／缺省实参的 GC、实参抛异常、NoGC helper、泛型、真实 foreign-thread 并发，以及移除 provider/consumer 源码后的 `.slib` 消费和独立链接。debug/release 均执行 normal/moving/minor；8 份公共 HIR/MIR 在三个目标间一致。
- 7 类 negative fixture 断言完整诊断与源码位置：非法声明目标、语法位置、generic/suspend、managed 签名、注解参数／重复、C extern 上的 NoGC，以及 unsafe 调用。nursery 修复后另有 20 次 release minor 并发运行全部通过。
- 旧 fixture 只回归受影响的 imported value、Char、浮点、优化 profile 与 release 调用路径。AST/HIR golden 的变更对应删除注解、源码位置变化和 C 声明显式记录 NativeSafe 模式；对既有默认模式的调试拼写做机械更新，MIR/LIR 指令未改变。未重复执行无关的全量语言验收。

## M33-1c：DirectC

- 三个 target 的固定 cdecl 标量签名直接调用 native symbol。LIR 明确区分 DirectC 的值参数／返回与 StorageBridge；窄整数和 Boolean 的扩展属性按 LLVM 22.1 C ABI probe 确定。Char、handle/pin 的透明整数表示和 nullable data/code pointer 保留实际类型。按值 C-layout aggregate 继续使用 storage bridge。
- DirectC 不再创建 bridge 专用参数／结果局部存储，不发布无用途的 outbound bridge recipe、定义或对象；实际 native contract 和 library requirement 继续保留。目标 C lowering contract 更新到 `ScalarDirectOrSystemCBridge`，同步格式固定向量与缓存／产物指纹。
- NativeSafe、GCLeaf 与 release 的调用协议保持独立。DirectC 在声明和调用处携带必要的 signext/zeroext 与 nobuiltin，禁止按名称替换实际选中的 native symbol；未增加 memory/nosync 等纯函数属性。ABI 检查在模块边界完成，发射阶段复用结果，删除旧的重复完整 C 合同检查。调用 lowering 与检查分别抽出小模块，新增实现文件均控制在 130 行以内。

已完成的验证：

- workspace fmt/clippy 通过；增加 LLVM 测试后补做 codegen clippy。codegen 原组 339 项通过，随后 DirectC 和 StorageBridge 的两项 GCLeaf LLVM 测试通过；LIR 467 项、LIR-lower 154 项通过，slib 565 项及更新固定向量后的定向 1 项通过。C companion 严格警告检查、基准 Python 的 Ruff 格式化／检查通过。
- Darwin/GNU/musl 同组各 8 项正式 CLI、15 个变体、99 个进程、54 份 golden，在非更新模式下通过。覆盖全部标量宽度、Boolean、Float/Double、混合与超寄存器参数、void、nullable pointer、Char、pin/handle、release，以及同 symbol 两种 GC 协议。`abs` 同名 C 实现使用可区分的结果，验证优化没有 builtin 替换。
- 跨 Cone、泛型与 artifact-only fixture 保留 scalar direct 调用和 aggregate bridge，移除源码后的独立链接成功。14 份公共 HIR/MIR 与 Darwin 一致，GNU/musl 的 34 份实际 HIR/MIR dump 逐字节一致。musl 额外通过 PIE 的 debug/release 两个变体、16 个进程。
- Darwin 另有 6 项旧 FFI／callback／release 回归、7 个变体、33 个进程、30 份 golden 通过。已有不变的 negative 诊断测试与 runtime TSan 结果继续复用；未重复执行全量语言 fixture。
- 检查三个 target 的优化后机器码：小整数参数直接进入 C ABI 寄存器，调用真实 native symbol；GCLeaf 不含该调用专用的 transition/root/safepoint，NativeSafe 仍发布根并协调返回。实际 Scoop benchmark 的 scalar 循环不含 storage bridge 或 bridge 缓冲区，正常循环 poll 仍在。
- Darwin/GNU 各记录 72 个无 GC 样本和 8 个真实 Scoop collector 压力样本；直接 C、NativeSafe DirectC、GCLeaf DirectC、aggregate bridge 分开报告，见 [PERFORMANCE.md](PERFORMANCE.md)。清理 4,674 个已链接的 Rust 中间对象，共 6.52 GiB，保留编译库与配套 CLI。

## M33-2a：计数 pin

- 对同一对象的每次显式 pin 独立计数，最后一次 unpin 才解除保活和地址固定。PinnedPtr 的值复制不改变计数。普通对象头大小和公共 runtime ABI 不变，GC word 的私有高位保存 pin registry 的索引。
- pin registry 每个对象只占一项；unpin 直接按索引访问，删除时交换末项并修正被交换对象的索引。pin 为摊还 O(1)，unpin 为 O(1)。heap lock 串行化计数、索引与 bitmap 更新，移除额外的 roots lock；pin 不提供 payload 的线程同步。
- 新增独立 C collector 测试和两个真实 Scoop fixture，覆盖重复 pin、GC 后可达性、内部项交换、小对象／large object、泛型、数组、handle、release，以及多线程对同一对象嵌套 pin。

已完成的验证：

- C 格式化与严格警告检查、Rust fmt 与 codegen clippy 通过。原 19 项 runtime collector 测试通过；新增测试在 O0/O2 × full/minor 四个配置通过，并检查实际 GC 计数以区分两种收集路径。
- GNU / clang 22.1 TSan 的 full/minor 两个进程通过，四个 mutator 与 collector 并发操作同一对象，未使用 suppression。
- Darwin/GNU/musl 各 4 项 CLI fixture、7 个变体、43 个进程、22 份 golden 通过。新增两项在非更新模式下各复验 4 个变体、26 个进程、12 份 golden；debug/release 均运行 normal/moving/minor。
- 4 份公共 HIR/MIR 在三平台一致，8 份 Linux LIR 单独保存。旧 roots/handles fixture 的 LIR 同步已有 DirectC 调用表示。复用已构建的 core 与 CLI，只运行 pin、handle、release 和相关 FFI 回归。

## M33-2b：作用域数据借用

- core 提供 Array / MutableArray 的 `withDataPointer` 和 String 的 `withUtf8Bytes`。元素遵守既有 GC-free Ptr 约束；回调是普通非 suspend 函数，可以分配、调用 C 或抛异常。receiver / block 各求值一次，返回值保留完整类型；长度分别为元素数和 UTF-8 字节数，空数据仍提供非零地址。
- 三个内部 intrinsic 经普通 core 声明、HIR metadata 和泛型物化传递，MIR 展开为 typed pin 帧、数据地址／长度、closure 调用和普通 finally cleanup。LIR 使用实际数组／String layout，codegen 在 caller 栈上分配帧，并发射明确的 AS1→AS0 数据借用转换。没有增加逃逸检测或并发访问授权机制。
- push/pop 只更新线程局部帧链，不分配、不取锁。collector 停稳后把帧对象作为根并临时固定，收集完成后仅清除没有显式 pin 的临时状态；支持嵌套、跨线程同时借用、immortal 字符串和显式 pin 交叠。退出最后一个借用后对象可再次移动。
- 新增实现模块最长 132 行。复用既有 closure、EH、根扫描和数组 layout，没有平行的调用或 GC 实现。

已完成的验证：

- Rust fmt、workspace clippy 及后续受影响 crate 的 clippy 通过；C 格式化和严格警告检查通过。新增 intrinsic 编码／非法 tag、六类 core 签名错误，以及四种 LLVM 地址空间边界测试通过。
- Darwin 的 21 项 runtime collector 回归通过。新增 scoped pin C 用例覆盖 O0/O2 × full/minor，检查嵌套、显式 pin、内部引用、最后一帧退出后的真实移动，以及四个 mutator 与 collector 并发。
- GNU / clang 22.1 TSan 的 full/minor 两个进程通过，无 suppression；线程局部帧操作与 collector 扫描未报告数据竞争。
- Darwin/GNU/musl 各 8 项正式 CLI、11 个变体、47 个进程、24 份 golden 在非更新模式下通过。三项正例覆盖普通／零长度／ZST 数组、静态和动态 String、嵌套、异常 cleanup、泛型与 aggregate 返回、NativeSafe / GCLeaf、foreign-thread 并发，以及删除 provider/consumer 源码后的独立链接。debug/release 均执行 normal/moving/minor。
- 五项 negative fixture 固定 unsafe、reference／含引用值元素、缺失 value bound、suspend callback 与 NoGC 的完整诊断和源码位置。8 份公共 HIR/MIR 在三平台一致，16 份 Linux LIR 单独保存。
- Darwin 另有 4 项 pin、handle、release 与裸指针回归，6 个变体、33 个进程、20 份 golden 通过。四份旧 HIR 的差异经检查仅为新增 core 声明导致的导入 arena 索引变化；裸指针用例在 GNU/musl 也分别通过，并同步三个 target 已有的 DirectC LIR 表示。复用热缓存，没有执行无关全量语言测试。
- 清理 182 个已链接的 Rust 中间对象，共 1.67 GiB，保留编译库、CLI 和 fixture 缓存。

## M33-3a：受检 UTF-8

- String 提供 Array<UInt8> 的严格、Option 和 lossy 转换，以及 unsafe 指针／Long 长度重载；CharacterCodingException 是普通 core 异常，公开首个非法子序列的零基 `byteOffset`。
- runtime 扩充现有 UTF-8 模块，严格与 lossy 共用有边界的单步解码器。非法起始字节、截断、过长编码、代理项和超出 Unicode 范围均按规范处理；lossy 消费 maximal subpart，保留失配位置之后的合法输入、U+0000 和原有 U+FFFD，不做 normalization。
- 严格后备通过普通 16-byte `(String?, Long)` Scoop 间接返回传递结果，core 只选择抛异常或返回 None，不再验证同一内容。Array 入口复用作用域 pin；指针入口先检查负长度，零长度不读地址。结果存入独立 String；lossy 先计算实际长度，再分配和转换，溢出与分配失败不转换成编码错误。
- 新增 runtime 解码存储模块 61 行、core helper 30 行；既有 UTF-8 模块扩充至 83 行，没有增加编译器 intrinsic 或异常角色。

已完成的验证：

- Rust fmt、受影响 toolchain/codegen clippy 与 C 严格警告检查通过。Darwin/GNU 的 C 测试均在 O0/O2 下遍历全部 1,112,064 个 Unicode 标量及其截断前缀；保护页用例检查真实内存末尾不会被读越界。GNU 的 ASan/UBSan 同组通过，原有 amd64 String sret／真实 LLVM frame 测试通过。
- Darwin/GNU/musl 各 6 项正式 CLI、9 个变体、35 个进程、24 份 golden 在非更新模式下通过。覆盖边界标量、全部错误类别、精确 byteOffset/maximal subpart、NUL、空输入、原生静态内存、借用的可写数组和存储独立性；debug/release 均运行 normal/moving/minor。
- 跨 Cone 泛型回调返回含 String 的 aggregate，provider/consumer 源码删除后独立链接并运行。8 份公共 HIR/MIR 在三平台一致，16 份 Linux LIR 单独保存。三个 negative fixture 固定 unsafe、数组元素／容器类型与长度类型诊断。
- Darwin 的两项旧 String／unchecked 转换回归通过，2 个变体、6 个进程、8 份 golden。旧 HIR 只同步导入 arena 索引与扩充 String companion 后的源码范围，MIR/LIR 未变化。复用上一批缓存，新增 core 每个 target/profile 只构建一次；没有重复全量语言测试。

## M33-3b：C 字符串

- String.withCString 在回调前拒绝嵌入 NUL，将 UTF-8 内容和终止符复制到普通 MutableArray，再通过已有作用域借用提供 Ptr<Int8>。空串、嵌套、异常 cleanup、泛型和 aggregate 返回沿用普通规则；原 String 保持不变。
- String.fromCString 用 GCLeaf strlen 取得首个 NUL 前的长度，再调用受检 UTF-8 指针入口，保留 CharacterCodingException 的精确 byteOffset。没有引入 CString 类型、编译器 intrinsic 或第二套 runtime 解码器；新 core 模块 21 行。
- 泛型 core 正文的真实产物消费暴露了既有构造器候选问题：编译器异常零参适配器遮蔽了带默认参数的源码构造器。所有源码构造、this/super delegation 候选现在只包含 Source constructor；编译器异常边仍使用其专用适配器。导出的泛型正文因此保存可消费的源码调用和已物化默认实参。

已完成的验证：

- Rust fmt、受影响 crate clippy、C fixture 格式化和严格警告检查通过。7 项构造器身份／候选测试和 12 项 core contract 测试通过。
- Darwin/GNU/musl 各 5 项正式 CLI、7 个变体、27 个进程、18 份 golden 在非更新模式下通过；debug/release 均运行 normal/moving/minor。覆盖字节内容与 NUL、严格错误偏移、嵌套与异常 cleanup、NativeSafe/GCLeaf 调用，以及删除 provider/consumer 源码后的泛型产物消费和独立链接。
- 三项 negative fixture 固定 unsafe、Int8/UInt8 指针类型与 suspend callback 的诊断和源码位置。6 份公共 HIR/MIR 在三平台一致，12 份 Linux LIR 单独保存；复用热缓存，没有运行无关全量测试。
- 清理 16 个已链接的 Rust 中间对象，共 0.36 GiB；保留库、CLI 和当前 fixture 缓存。

## M33-4a：NativeSafe 标准输出

- core 的 write 改为普通 String 作用域借用与 C ABI 调用；新增 writeError、eprint、eprintln 和 flushOutput。ToString 恰好求值一次，正文或换行不会先于失败的转换输出；UTF-8 与嵌入 NUL 原样写出。
- runtime 的字节输出／stdout 刷新后备集中在 13 行 io.c，继续使用 stdio 缓冲。普通 C FFI 负责 NativeSafe，作用域 pin 负责 String 地址稳定；没有新增编译器 intrinsic 或输出专用 GC 协议。
- 规范移除旧的直接 borrowed String 输出示例，明确可能阻塞的写入和刷新、缓冲、并发行边界与安全调用接口。

已完成的验证：

- Rust fmt、受影响 toolchain clippy、C 格式化和严格警告检查通过。
- Darwin/GNU/musl 各 2 项正式 CLI、4 个变体、22 个进程、12 份 golden 在非更新模式下通过；debug/release 均运行 normal/moving/minor。
- 输出用例覆盖两个流、空串、NUL、Unicode、ToString 次数与异常，并删除源码后独立链接、执行。并发用例先填满真实 pipe，确认 writer 持有 stdio 锁后，在另一线程完成 GC，再开始排空；分别验证 stdout 写入、stderr 写入与 stdout 刷新不会阻止 GC，并核对全部输出字节。
- 4 份公共 HIR/MIR 在三平台一致，8 份 Linux LIR 单独保存。复用前一批依赖缓存，没有进行无关全量回归。

## M33-4b：显式退出与失败退出

- core 提供安全的 exit(Int): Nothing，作为普通 Scoop ABI extern 保留实际底类型与 reference 返回 carrier；没有 Unit 占位或返回后的伪控制流。退出请求先进入单向 NativeSafe 状态，刷新 stdout/stderr 后调用 _exit，保留完整 Int 参数。
- 单向终止不恢复 managed 执行；已发布根、TaskContext、pin 帧和外层冻结段继续供其他线程的 GC 使用。不会执行 finally、release hook、atexit、callback/token shutdown 或 join；stdio 刷新允许正常等待消费者。
- 未捕获异常、eager 失败和语言级 panic 刷新输出后以 1 退出。异常报告继续读取已发布 failure root 的稳定类型名，不调用用户 toString；内部 gateway/ABI 违例保留 abort。退出实现集中在 42 行 process.c。

已完成的验证：

- Rust fmt、受影响 toolchain/codegen clippy、C 格式化和严格警告检查通过。Darwin/GNU 的 startup gateway 测试确认合法失败以 1 退出、三个内部协议错误仍为 SIGABRT，并在报告前移动实际异常对象。
- Darwin/GNU/musl 各 5 项正式 CLI、9 个变体、69 个进程、24 份 golden 在非更新模式下通过；debug/release 均运行 normal/moving/minor。所有正例删除源码后独立链接、执行；Nothing 通过普通包装函数流入 Int 位置，非法 Long 参数固定精确诊断。
- 显式退出覆盖 0、1、37、255、256、负数和 Int 两端，父进程按 POSIX 低 8 位观察结果；验证缓冲内容保留，finally、release 和 atexit 均未调用。失败用例验证用户 toString 不参与诊断。阻塞用例保留活动 callback 和 C 字符串 pin 帧，分别在 exit 与 managed panic 的 stdout 刷新期间完成另一线程的真实 GC，再排空 pipe。
- Darwin 定向复验 16 项既有异常、初始化与 program-link 用例，19 个变体、67 个进程、92 份 golden 通过。13 份旧 HIR 仅同步导入 arena 索引和 io.scoop 源码范围，eager 用例的旧 LIR 同步既有 DirectC。GNU/musl 另各通过 eager 失败的产物独立链接回归，2 个变体、8 个进程、8 份 golden。
- 8 份公共 HIR/MIR 在三平台一致，16 份 Linux LIR 单独保存。旧产物复制用例复验前清理其只读临时文件，保留热缓存；没有进行全量语言回归。

## M33-4c：四种 main、argv 与入口 ABI

- HIR 从 root Cone 选择唯一的无参数／Array<String> 参数、Unit／Int 返回入口，展开透明 alias，保留完整源码签名和实际函数 identity。参数名任意，普通默认参数仍受原有声明规则约束；gateway 始终传入实际 argv，不执行默认表达式。library 和依赖的同名函数不参与选择。
- 删除仅表达无参 Unit 的签名包装，沿 HIR/MIR/LIR、core protocol 和产物引用传递完整 ExactCallableSignature。argv 构造由 22 行普通 internal core helper 完成，复用 Array、String.fromCString、Option 与异常处理。runtime 在 eager 初始化前保存原始 argc/argv，NoGC 访问器原样读取，越界返回 null；无参入口不构造或解码 argv。
- root gateway 使用三参数 C ABI，向独立 Int32 槽写入完整 main 返回值，以自身返回值区分成功 0／异常 1。参数构造与 main 共用失败出口；有参入口先构造数组，再调用 main 和写出返回值。eager gateway 保持原有无参数 ABI。runtime ABI 升为 11，metadata ABI 升为 7，RootEntry 大小保持 192 bytes；Rust 的发射、读取和链接入口共用版本常量。
- 源码入口形态在 HIR 边界检查；后续保持身份、签名、引用与 ABI 校验，移除对象发布入口重复执行的无参 Unit 限制。新增实现按职责拆分，入口主模块 192 行，其两个辅助模块分别为 44、81 行。

已完成的验证：

- Rust fmt、受影响 crate 的 clippy 与 C 格式化／严格警告检查通过。入口选择、完整签名 identity、core protocol 的 wire／导入、MIR production／104 项结构校验、LIR gateway、codegen metadata 和链接器最终映像的定向测试通过。
- Darwin/GNU 的 C startup 测试验证精确原型、原样 argv、独立输出槽及完整 Int32 边界值；合法失败仍走退出码 1，内部协议违例仍为 SIGABRT。
- Darwin/GNU/musl 各 11 项正式 CLI、15 个变体、107 个进程、24 份 golden 在非更新模式下通过。四个正例覆盖 debug/release、Managed/NoGC、参数 alias、默认参数、eager raw argv、空串／空格／Unicode、原样 argv[0]、非法 UTF-8 下标、Int 正常返回 1、负数和两端值，以及 finally 与 normal/moving/minor GC；删除源码后均可独立链接、运行。父进程按 POSIX 低 8 位观察退出码，C 测试另验证完整 Int32 传递。
- 七个 negative fixture 固定错误参数数量、vararg、可空／可变／元素不匹配的数组、错误返回类型和重复入口的完整诊断及源码位置。8 份公共 HIR/MIR 在三平台一致，16 份 Linux LIR 单独保存。
- Darwin 另有 8 项旧入口、初始化、独立链接和退出回归，在非更新模式下通过 13 个变体、73 个进程、44 份 golden。旧 HIR 同步完整签名表示、导入索引及既有 core 源码范围；旧 LIR 只增加 root gateway 的 C 参数和退出码槽写入。
- 清理增量目录 2.85 GiB 和 481 个已链接的 Rust 中间对象 2.93 GiB，保留库、CLI、测试二进制及热缓存。没有执行无关全量测试。

## M33-5：按次捕获 errno

- `@Extern` 支持默认 false 的编译期 Boolean `captureErrno`，复用现有常量求值，支持位置／命名参数、当前 Cone 与导入的 const val。true 仅适用于 C ABI 函数，返回类型在透明 alias 展开后必须为 `(R, Int)`；C-FFI-safe 检查只分类 native 返回 `R`，完整 Scoop 返回仍参与普通类型、GC-free 与 ReleaseValue 规则。
- HIR/MIR 以 `ExternResult` 区分普通结果及捕获结果，显式保存 native／Scoop 类型。LIR 保留结果适配及完整 C storage signature，捕获始终使用 StorageBridge；非 void 的 native 结果写入本次调用的精确 caller-owned storage，bridge 自身返回 Int32 errno，随后重建 tuple。Unit 不分配 native 结果槽。没有新增共享 buffer、Scoop TLS 错误槽或 runtime errno getter/setter。
- generated-C 在参数解包之后、真实 C 调用之前清零 errno，返回后立即保存到局部 `int`，再复制 native 结果。宏由所选目标头文件展开，Darwin／GNU／musl 的实际 libc accessor 引用进入普通 target-support requirements。NativeSafe 在返回握手前捕获；GCLeaf 与 release 不增加 GC 边界操作。
- native symbol 合并只比较投影后的实际 ABI，普通和捕获声明可共用 native 定义。OutboundFunction recipe identity 含 Direct／CaptureErrno，避免合并不同私有返回方式。HIR interface 升为 62，LIR foundation 升为 7，OutboundWrappers 模板升为 2；旧的缺字段声明／bridge 及旧 capability 明确拒绝。安全包装函数的引用保留 tuple，unsafe extern 仍遵守原有函数值规则。
- 注解 schema、errno 类型投影、bridge 生成和 C 调用 lowering 分别保持在 129、99、88、104 行的模块中；跨 Cone native 调用信息模块为 88 行。没有添加新的泛用验证或资源计费层。

已完成的验证：

- Rust fmt、受影响 crate 的 clippy，以及 C 格式化和 `-Wall -Wextra -Werror` 检查通过。11 项 bridge identity／wire、5 项 callable implementation 编码、16 项 C toolchain／target support、19 项产物 profile、12 项既有 extern 前端测试通过。C 生成测试检查清零／调用／快照／复制顺序、void 与非 void 私有签名及普通／捕获 recipe 分离；3 项 GCLeaf LLVM 测试经 statepoint rewrite 和 verifier 检查，捕获返回同样无 statepoint、relocation、native 握手及 caller-root 操作。
- Darwin／GNU／musl 各 17 项正式 CLI、20 个变体、58 个进程、24 次 golden 比较通过。三个正例均覆盖 debug/release × normal/moving/minor；包含 Int、Boolean、Unit、可空 pointer、C-layout struct、丢弃结果、安全函数引用、泛型、NoGC、release、原样错误值与同 symbol 的普通调用。
- 产物组合先删除 provider 源码，consumer 从 `.slib` 读取捕获声明、常量与泛型；再删除 consumer 源码独立链接并运行。覆盖导入 extern 在 release 中直接调用、泛型 release owner、协程挂起／恢复与 finally GC。
- 嵌套回调及四线程回调分别在本次调用中设置不同 errno，GC 和其他 native 调用之后仍保留原 tuple；outer C 函数在回调后设置的最终 errno 正确返回。14 个 negative fixture 固定形态、native 返回约束、ABI／变量限制、常量类型／常量性、重复参数、unsafe 及真实 native 签名冲突的诊断和位置。
- 8 份公共 HIR/MIR 在三平台一致，GNU debug/release 的实际 dump 另逐字节核对，16 份 Linux LIR 独立保存。三平台均在非更新模式下完成最终复验。
- Darwin 另通过 10 项旧 GCLeaf／FFI 回归，13 个变体、51 个进程、24 次 golden 比较。旧 HIR 同步完整 main 签名与 Direct 结果表示；旧 LIR 同步既有 root gateway 参数和本批 bridge identity，原 MIR 未改变。
- 清理 369 个已链接的 Rust 中间对象约 2.58 GiB、增量目录约 2.38 GiB，保留库、CLI、测试二进制与热缓存。未执行无关全量测试。


## M33-6a：按 target 选择 Scoop 源码

- manifest 以显式数据区分默认 `src/` 扫描与完整 `sources` 清单。`os`／`arch`／`env` 从现有 target 定义取得合法值，支持字符串、数组的“或”和键之间的“与”；全部条目的 schema、相对路径和条件先验证，再检查当前 target 选中的文件系统路径。
- 支持 Cone 内目录与单个 `.scoop` 文件，保留完整的规范化 Cone-relative identity 和确定性顺序。重复／互含目录、文件与目录重叠、symlink 重复源、逃出 Cone 根、循环、缺失路径、类型错误、非 UTF-8 与最终空集合在 parser 前失败；未选中源码或不存在路径不参与编译。路径归一化后继续检查盘符前缀，未选中条目也不能藏入非法路径。
- `scoop` 与直接 `scoopc` 通过共享 manifest discovery 接收实际 target。外层缓存命中前完成选择与枚举；key 纳入规范化选择语义和实际源码，未选中文件的内容不进入 key。输入快照保留选中的空目录，子编译器读取同一清单和文件内容；清单重排或等价路径写法保持缓存命中。没有修改 runtime 或产物 ABI。
- 原 766 行的 discovery 拆为 198 行的数据／入口模块与独立的选择、遍历、读取和诊断模块；manifest semantic 为 343 行，声明解析为 237 行。target 谓词解析和路径处理各自独立，没有新增资源计费或通用插件框架。

已完成的验证：

- Rust fmt、受影响 crate 的 clippy 通过；35 项 manifest、5 项 cache key、7 项直接编译器输入及 13 项构建快照测试通过。
- Darwin／GNU／musl 各覆盖 34 项新增正式 CLI fixture、35 个变体、45 个进程和 6 次 stage golden 比较。平台选择正例覆盖 debug/release、泛型、file-private 同名函数、Cone 内 source alias、空目录、普通／moving GC，并逐字节比较 `scoop` 与直接 `scoopc` 的产物。HIR/MIR/LIR 按 target 保存，LIR 同时按 profile 保存，均完成非更新复验。
- 32 个 negative fixture 固定条件与字段错误、路径形态、重复／重叠、空集合、文件类型、缺失、symlink 边界、非法编码以及正常的重名声明诊断。缓存用例证明未选中内容变化和清单重排仍命中，选中内容变化改变 key／产物，且缓存命中前仍检查选中目录存在性。GNU 复验后将缓存变化断言改为 key 和产物变化，允许修改后内容已在热缓存中；只补跑受影响用例与新发现的盘符路径边界，其余已通过结果复用。
- Darwin 另通过 3 项已有 CLI 输入回归，共 10 个进程。清理 32 个已链接 Rust 中间对象与增量目录，共约 2.30 GiB，保留库、命令与测试缓存。

## M33-6b：Cone 自带 native C

- manifest 支持条件 native 源码、Cone 相对 include 和有序 C 编译参数。选中源码按归一化路径排序，拒绝重复物理文件、越界路径和 driver 管理的参数；未选中源码不读取。注入实际 target 的 OS、arch、env 宏，使用 profile 的 O0/O2 默认值并保留 PIC。
- 外层缓存查询前，所选 C driver 生成完整预处理输入及 depfile。源码、传递头文件、公开 runtime 头、系统头的内容摘要和生效参数进入缓存键；child 编译同一份不可变预处理内容。源码行号保留，工作区、公开头与系统目录映射为稳定逻辑路径。既有文件快照移至 manifest crate 共享，没有增加平行快照实现。
- 每个 C 源码生成独立的普通 native LinkObject，实际对象内容进入 Code 指纹；不建立 Scoop Strong/ODR 身份或 bridge recipe。最终链接按 canonical Cone/member 顺序消费，跨 Cone 即使内容相同也保留独立定义并正常报告冲突。直接 scoopc、scoop 构建和 artifact-only 链接共用该产物合同。
- host protocol 升为 5，field 10 传递排序后的预处理输入 locator，严格拒绝旧请求或数量不符。runtime ABI 与 metadata ABI 保持 11/7。新增实现按 manifest、预处理、路径映射、产物成员、链接输入分成小模块；既有大文件只增加必要接线。

已完成的验证：

- Rust fmt 和受影响 crate 的 clippy 通过；C fixture 经 clang-format 和严格警告编译。manifest 42 项、protocol 24 项、编译缓存键 5 项、Code 指纹 9 项、native 链接器 7 项定向测试通过。
- Darwin 与 Linux GNU 各 5 项真实 C driver 测试通过，覆盖不可变预处理输入、头文件失效、公开 runtime 头失效、越界、hardlink 重复，以及搬迁目录后的缓存投影和带调试信息对象字节稳定性。
- Darwin/GNU/musl 各 29 项正式 CLI fixture、31 个变体、57 个进程全部通过；每个 target 保存 18 份 HIR/MIR/LIR golden。正例覆盖 C 标量、aggregate bridge、NativeSafe/GCLeaf、errno、BSS/TLS、C 字符串、泛型与 moving/minor GC，debug/release 均运行。直接 scoopc 与 scoop 构建的产物逐字节一致。
- 缓存 fixture 分别修改传递头、forced include、C 编译参数和 C 源码，确认缓存失效及运行结果改变；未选源码、无关头和清单重排保持命中。产物 fixture 删除 provider/consumer 源码后完成跨 Cone 消费与独立重链接，link-plan fingerprint 一致。
- 26 类 negative fixture 使用真实 CLI 采集的完整诊断，包含非法条件/参数/路径、缺失/错误源码、symlink/hardlink、头文件越界、预处理/语法错误、C 初始化段和跨 Cone 强符号冲突；平台相关的编译器诊断分别保存。
- 清理 428 项已链接中间对象与 incremental 目录，共 2,525,560,831 bytes，保留可复用编译库和配套 CLI。没有运行无关的全量语言测试；C++ 与系统库作为后续功能单独实现和验收。

## M33-6c：manifest native 库与系统库解析

- `native.libraries` 支持必需的逻辑库名、default/dynamic/static/framework kind 和既有 target 条件。筛选结果排序去重并进入外层缓存键，合并到既有 LIR foundation 的 native library 表；native C 可以是唯一使用者。完整要求继续由 Code、production manifest 和依赖产物传递，没有另建平行的库元数据。
- 显式 library roots 优先；该层没有候选时，Darwin 查询所选 SDK 的库与 framework，Linux 通过所选 C driver 和 linker 解析系统库。一个逻辑库可以映射到多个真实输入；GNU 系统 linker script 的内容与目标文件均进入链接计划。损坏、歧义或不兼容的显式输入直接报错。
- 已包含在固定系统输入中的 ELF archive/DSO 复用其符号索引，避免重复解析和重复候选。musl 的固定空兼容 archive 只转向实际 libc 的符号；显式 roots 中的空 archive 不得到该别名。Darwin SDK 的 previous 指令按 platform 和 deployment 区间选择实际加载库，并保留 compatibility version。
- 新代码按清单解析、显式／系统查找、SDK 指令和 ELF 系统符号索引拆为小模块，后者为 98 行。runtime ABI、metadata ABI 和 host protocol 仍为 11/7/5。

已完成的验证：

- Rust fmt、受影响 crate 的 clippy 通过。manifest native 6 项、LIR 库要求 4 项、SDK stub 5 项、缓存键 5 项定向测试通过；GNU/musl 的两个真实工具链测试验证 libc/libm 复用及别名不能提供 unwind 符号。
- Darwin/GNU/musl 分别通过 20/20/21 项适用正式 CLI fixture、25/24/25 个变体、66/59/60 个进程；保存 36/30/30 份 golden。12 份公共 HIR/MIR 在本机与 Linux 副本一致。覆盖 SDK CoreFoundation、系统 cos、显式库优先、NativeSafe/GCLeaf、errno、泛型及 moving/minor GC，debug/release 均运行。
- 产物 fixture 删除 provider/consumer 源码后完成跨 Cone 消费和独立链接；缓存 fixture 验证无 C 源码时库重排、重复及未选条目仍命中，新增库要求使缓存和产物改变。16 类 negative fixture 固定完整诊断，覆盖名称、kind、条件、缺失系统库、显式候选损坏／空库／歧义和 target 限制。
- Darwin previous fixture 的公开 stub 指向实际旧 dylib，公开名称对应的 dylib 不存在，最终程序仍成功加载运行。另有两项既有动态库重导出／符号重命名回归通过，14 个进程、10 份 golden；绑定仍保留 facade 与实际 source symbol，旧 golden 同步已完成的 M33 入口和 DirectC 变化。
- musl 完整运行暴露 libc archive 被重复按普通 native 对象解析的问题；修复后只复验失败缓存项、GNU 系统绑定及受影响诊断。清理 246 项已链接中间对象和 incremental 目录，共 2,019,638,990 bytes，保留配套 CLI 与热缓存；没有运行无关全量测试。

## M33-6d：C++ 源码、产物要求与最终链接

- `native.cxx` 是显式布尔开关，`cxx_flags` 与 `c_flags` 分别作用于 C++／C。默认采用 C++20／C11；`.cc`、`.cpp`、`.cxx`、`.C` 必须开启 C++。C/C++ 共用已实现的头文件发现和不可变预处理快照，编译阶段消费对应 `.ii`／`.i` 内容。
- C++ driver 从已选 C driver 的配套位置解析。GNU 检查 target、版本、frontend 和 libstdc++；Darwin 保留 clang++ 的调用名称并使用所选 SDK 的 libc++。driver、标准库及实际宏进入 native 输入指纹；配置开启 C++ 时，即使没有选中的 C++ 源码也保留该要求。工具链错误标明要求 C++ 的 Cone。
- LIR foundation 增加必需的 `native_cxx` 字段，进入 Code 和 production manifest。完整 Link 闭包确定最终模式，root 无需重复声明依赖的 C++ 配置。LIR identity-foundation 升为 8，production manifest 升为 6；相关固定编码和指纹同步。runtime／metadata ABI、host protocol 仍为 11/7/5。
- GNU 使用配套 g++、libstdc++ 和唯一的 libgcc_s unwind provider；Darwin 使用 clang++、libc++ 与 SDK 中实际提供 ABI 符号的库。C++ 初始化／析构、TLS、原生异常与普通 weak/COMDAT 定义走目标工具链规则。Mach-O 的普通绑定和 weak coalescing 分开解释；`__dso_handle` 按映像头地址解析。GNU UNIQUE 定义按其可合并语义处理。公开 runtime 头兼容 C++ 的 C linkage、noreturn、static_assert 和 alignof，结构布局不变。
- native、C++ driver、最终运行库配置及 Mach-O binding 校验分别放在小模块中。已有较长的 foundation/cone 文件移出 native 配置操作，final_image 文件也移出 binding 检查，未增加通用框架。

已完成的验证：

- Rust fmt 与受影响 crate 的 clippy 通过。Darwin/GNU 的 native 输入测试各 6 项通过，覆盖真实混合编译、公开头、标准库、原生异常及修改头文件后仍编译原快照。另有 LIR 模式编码／三个 target 投影、11 项 Code 投影、7 项 production manifest、19 项 capability profile、5 项缓存键和 2 项 SDK previous 指令测试通过。新增字段接受 0/1 并拒绝其他值，旧格式版本仍被拒绝。
- 新增 15 项正式 fixture。Darwin 通过 10 项适用用例、13 个变体、52 个进程；GNU 通过 14 项、17 个变体、56 个进程。两者各验证 30 份 golden，10 份公共 HIR/MIR 完全一致。其余项按 target 标为不适用。
- values 用例覆盖五种源文件后缀、C/C++ 参数隔离、模板／inline 共享定义、标准库、原生 throw/catch、全局构造／析构、TLS、NativeSafe／GCLeaf、errno 和 Scoop 异常组合；debug/release 下运行普通、moving、minor GC。三层产物用例在每层构建后删除源码，最终无 C++ 源码／配置的 consumer 通过中间 Cone 消费 provider，再独立链接运行。
- cache 用例验证传递头、强制 include、C++ 参数和源码变化后的失效，以及重排、未选输入和无关头的复用。mode-cache 验证仅切换 `cxx`、没有 native 源码时，缓存键、产物和链接计划变化，重复构建命中。exit 用例确认显式退出刷新输出、返回 37，并且不执行 C++ 全局析构。
- negative 用例覆盖字段类型、受 driver 管理的参数、缺少开关、参数不隐式开启 C++、缺配套 driver／标准库、target／版本不匹配。musl 通过 6 项适用配置用例、7 个变体，包含未选中 C++ 源码时 static/dynamic 均拒绝的检查。Linux 另有真实产物读取测试：删除源码后读取带 C++ 要求的 GNU Link 闭包，分别选择 musl static/dynamic final-link profile，均在解析 C++ 工具链前拒绝并指出 Cone。
- 三平台各通过 1 项既有纯 C fixture、2 个变体、8 个进程与 6 份 golden，验证混合功能改动后原有 C 路径。最后清理本机 262 项 target 中间对象／incremental 目录，共 2,369,074,897 bytes，保留热缓存和配套 CLI；没有运行无关全量测试。
- 最后补充工具链错误中的当前 Cone 来源，只复验 GNU 的 4 项工具链诊断、musl 的 static/dynamic 两个拒绝变体和混合 native 输入测试，均通过；其余已通过结果直接复用。

## M33-7a：sysroot 默认库源码定位

- 已声明的普通 `scoop` 依赖没有显式 locator，且 artifact search roots 没有候选时，从 `<sysroot>/lib/<name>` 读取普通 library Cone。复用已有 coordinate、manifest、来源冲突和循环检查；其他 group 不参与，core 保持原规则。新发现源码的显式依赖继续优先展开，直至依赖工作队列为空。
- 显式 path／artifact 和搜索根中的有效产物优先；损坏、歧义或不兼容候选直接报错。默认定位不注入依赖或扩大可见性，独立链接仍要求完整的产物闭包。定位逻辑为 22 行独立模块，依赖图复用普通 source 节点、缓存和编译流程。

已完成的验证：

- Rust fmt 与受影响 crate 的 clippy 通过；依赖发现 16 项测试通过，其中新增 4 项覆盖默认源码的传递显式依赖、两种显式来源优先和其他 group 不回退。
- Darwin／GNU／musl 各通过 15 项正式 CLI fixture、17 个变体、43 个进程、12 次 stage golden 检查。两个正例完成非更新复验，4 份公共 HIR/MIR 在 Darwin 与 Linux 完全一致。
- 组合用例覆盖默认库、传递依赖、泛型、native C 与普通／moving／minor GC；重复构建命中，头文件变化使结果由 42 变为 43。删除全部用户源码后独立链接运行成功且链接计划相同；缺少依赖产物时即使 sysroot 中仍有源码也正常报错。
- 13 个 negative fixture 固定完整诊断，覆盖 coordinate 的 group/name/version、不合法 library、缺失／损坏默认源码、其他 group、未声明依赖、显式来源失败、损坏／歧义搜索候选和依赖循环。复用已有热缓存与已通过结果，没有增加无关全量测试。

## M33-7b：NoGc 值方法实现 Managed 接口

- struct／enum 的 NoGc 方法可实现普通 Managed interface slot，保留相同的参数、结果、访问、安全性和 ordinary/suspend 规则；本地和导入接口使用相同的实现匹配。class vtable override 保持原 effect。slot 与实际实现的导出合同分别保留 Managed／NoGc，没有抹去实现属性。
- 复用 MIR 已有的 Managed boxing adjust：适配函数解箱并以目标方法的 NoGc 合同调用。具体值直接调用仍为 NoGc；接口和仅有 interface bound 的调用保持 Managed。没有新增 runtime API 或分派机制。

已完成的验证：

- Rust fmt 与受影响 crate 的 clippy 通过。13 项 slot 合同测试、12 项既有 NoGc 及泛型 effect 测试通过；新增 slot 测试确认 effect 收紧只允许指定方向，class vtable 的差异仍被拒绝。
- Darwin／GNU／musl 各通过 3 项正式 CLI fixture、5 个变体、21 个进程与 18 次 golden 检查。覆盖 struct／enum、接口继承、直接调用、泛型 bound、装箱、`is/as` 和普通／moving／minor GC，debug/release 均运行；6 份公共 HIR/MIR 一致。
- 跨 Cone 用例删除 provider 源码后继续消费其接口、值方法和 generic body，并以本地 NoGc 值方法实现导入接口；删除 consumer 源码后独立链接，链接计划与原构建一致。negative 用例固定 NoGc generic body 调用 Managed interface slot 的诊断，实际 NoGc 实参不能改变声明处的调用合同。
- 清理 196 项 target 中间对象与 incremental 内容，共 1,697,653,676 bytes，保留库、CLI 和热缓存。运行结果与 golden 均复验，不运行无关全量测试。

## M33-7c：显式 Equality 与可调用的标量实现

- core 以普通 invariant `Equality<T>` 接口定义相等合同，compiler protocol 保存其实际 interface、source callable 与 dispatch slot 身份。整数、Boolean、Char、Float/Double、String、Unit 和 Ptr 显式实现对应 application；相等运算只从实际 core 合同收集手写成员，同形或同名用户接口不获得该能力。接口继承、普通 override、多个不同参数的 application 和独立 Hash bound 沿用普通类型规则。
- 整数、Char 和浮点 intrinsic equals 增加可寻址的普通实现入口，复用直接比较使用的 primitive operation。直接调用保留 NoGc，接口表继续使用 Managed value adapter。Unit 使用普通 core 方法。Ptr 保留真实声明、完整 interface conformance、装箱接口表和跨 Cone callable 绑定；没有增加地址相等 fallback 或新的 runtime API。
- 产物保存必要的 Equality 协议字段，core-bootstrap-interface 与 cross-cone-interface 升为 13/63，固定编码、版本拒绝和 profile 指纹同步。结构表示的装箱表使用 LocalConcrete HIR 已完成的接口决议，产物边界继续检查引用、slot 和 callable 签名，删除旧的“结构表示必须没有接口”假设。
- GNU 的 release moving GC 暴露 LLVM X86 call-frame size optimization 在按值参数处临时 push/pop 的问题，实际 SP 与固定 stackmap frame size 不一致。codegen 在创建 target machine 前统一关闭该项优化，保留其余 O2 及原 ABI；不放宽 runtime 栈边界检查。新增后端模块 25 行，独立机器码测试覆盖 GNU/musl 的按值参数和跨调用 managed root。

已完成的验证：

- Rust fmt 与受影响 crate 的 clippy 通过；20 项相等相关 HIR 测试、14 项 core protocol 测试、19 项 capability profile 测试通过。38 项 statepoint 相关测试覆盖既有 SSA relocation、O0/O2、异常及新的 amd64 固定调用帧；已有通过项复用结果。
- Darwin/GNU/musl 各通过 16 项适用正式 CLI fixture、19 个变体、41 个进程和 24 次 stage golden 检查；8 份公共 HIR/MIR 一致。GNU 修复后的 3 个正例与 musl 的 3 个正例均完成普通模式复验，先前通过的 13 类诊断复用。
- Darwin 的 4 项旧 core 重建／身份、派生相等及 Unit 跨 Cone 回归在非更新模式下通过，共 5 个变体、37 个进程、44 次 golden 检查；相关测试声明同步现有 Equality 和 String API。
- 新增 16 项正式 CLI fixture，覆盖全部标量、手写 struct/class、接口继承、多参数合同、求值顺序、泛型与 Hash 组合、装箱和 `is/as`。Float/Double 的 NaN 与正负零保持原比较语义；指针用例在作用域借用内跨普通／moving／minor GC 比较与装箱。
- 13 个 negative fixture 保存实际完整诊断与源码位置，覆盖缺少合同或 override、同形／同名假接口、Any、仅 Hash、Float 的 Hash bound、缺少泛型 bound、invariant、接口视图操作数、歧义、左右操作数顺序及 NoGc generic body 调用 Managed slot。
- 跨 Cone 用例在移除 provider 源码后消费普通与泛型 Equality，实现导入接口；移除 consumer 源码后独立链接，链接计划保持一致。直接数值比较的 MIR/LIR 仍是 integer/float compare，自定义 NoGc 比较保持直接调用，不引入装箱或 safepoint。
- 本机清理 94 项已链接中间对象及 incremental 内容，共 3,039,163,000 bytes，保留编译库、CLI 和热缓存。条件派生作为下一项继续实现，不将本节的显式合同验收视为整个 Equality 或 M33 完成。

## M33-7d：手写同签名成员与派生冲突

- 派生声明在本地及导入路径均检查普通 `equals(Self)` 成员，不再仅检查 operator 标记。普通同签名方法保持普通调用语义，不产生 Equality；私有成员同样占用签名。参数类型不同的 equals 和其他名称的方法不阻止结构派生。
- `tests/fixtures/m33-equality/derivation/collisions` 覆盖 struct、enum、泛型私有成员的精确 negative 诊断，以及移除提供方源码后调用普通／泛型成员、结构派生和再次拒绝相等运算；成功程序在移除消费方源码后独立链接运行。
- Darwin、GNU、musl 均在非更新模式下通过 4 项、5 个变体、13 个进程、12 次 HIR/MIR/LIR golden 检查。三个正式报告保存在 `tmp/m33/equality-collision-{darwin,gnu,musl}-final-report.json`。本地 fmt 和 HIR-lower 全 target clippy、Linux fmt/clippy 及两地主 CLI release 构建通过；此次没有重跑无关全量测试。
- 这项修复只完成同签名冲突边界；自动派生的真实 Equality conformance 和泛型条件仍继续实施。

## M33-7e：非泛型值类型的派生 Equality

- 普通非泛型 struct／enum 在全部字段可比较时建立真实 `Equality<Self>` conformance，并用完整派生正文满足原接口 slot。直接 `==`／`!=`、普通 `.equals`、泛型 bound、接口调用、装箱和 `is/as` 使用同一关系。不可比较字段不限制值的构造，也不会产生空正文或恒真实现。
- 派生接口目标使用 `DerivedEquality(原 nominal declaration)`，exact 实现继续使用已有 generated callable identity。HIR nominal selection、MIR binding、boxing adjust 和 `.slib` 闭包连接真实生成目标；没有伪造 SourceFunctionId。cross-cone-interface／cross-cone-type-semantics 升为 64／24，固定编码、版本拒绝与 profile 指纹同步。
- 导入 `.equals` 保留完整的 Equality application，与手写的其他参数重载共同决议，再经普通接口调用提供方的派生正文。跨 Cone 的接口表可以直接引用提供方生成 callable；绑定装配先收集普通与派生实现，再生成 boxing adjust。
- 派生方法保持 Equality 的 safe 调用合同；InteriorMutable 的 unsafe 类型使用规则在实际构造、读取、传递及比较处执行。删除生成入口上的重复源码 API 暴露检查，并移除旧的发布阶段重复派生准备。
- 正式物化从发布声明闭包出发，实际正文和表示需求继续补齐私有支持类型。新增接口不再把完全未使用的私有类型反向带入发布闭包；已有字节稳定性测试保留。实际装箱的私有类型仍提供完整 Equality 接口和表示支持。

已完成的验证：

- Rust fmt、workspace all-targets clippy 及两地主 CLI release 构建通过。20 项相等测试、26 项类型导出／装箱／派发表测试、4 项自动物化测试、3 项 InteriorMutable 测试和 114 项 MIR 类型桥接测试通过；19 项 capability profile、11 项 selected declaration 编码及新增派生 slot 合同测试通过。
- 新增 7 项正式 fixture，覆盖非泛型 struct／enum、空值、嵌套字段、继承 `Equality<Base>` 的字段、字段求值顺序与短路、enum tag／active payload、多参数 Equality 重载、Ptr、InteriorMutable、NaN 与正负零。四类 negative 固定不可比较 payload、歧义字段、无法满足显式 Equality 及 safe context 使用 InteriorMutable 的诊断。
- Darwin 在普通模式下完整通过 7 项、10 个变体、32 个进程、24 次 golden 检查。GNU／musl 各通过同样的 7 项和计数：发布根调整后，仅重跑受阶段 dump 变化影响的两项，普通模式各通过 4 个变体、16 个进程、12 次 golden，其余五项复用已通过的普通运行。新增 fixture 的 8 份公共 HIR/MIR 在三目标间一致。
- 跨 Cone fixture 在删除 provider 源码后消费其派生接口、手写字段实现和 generic 调用，再删除 consumer 源码独立链接。正例覆盖 debug／release 与普通／moving／minor GC；未运行无关全量 fixture。最终报告及 Linux 分批记录位于 `tmp/m33/equality-derived-*-report.json`。
- 本批两次清理共删除 197 项 target 中间对象与 incremental 内容，释放 3,644,615,617 bytes，保留库、CLI 和热缓存。新增生产子模块分别为 51 行和 66 行；泛型条件及 tuple 的完整 conformance 继续实施，不计为本批完成。

## M33-8a：AtomicRef 的 LLVM 技术验证

- `compiler/codegen/src/statepoint_tests/atomic_refs.ll` 保留最小 IR：AS1 对象 base、expected 和 new 引用跨前一个 safepoint；原子读取／exchange／CAS 的旧值与这三个引用一起跨后一个 safepoint。store 继续保留写入引用，真实 SSA alias 在 rewrite 前合并。
- 四种原子指令覆盖全部合法序：3 种 load、3 种 store、5 种 exchange、9 种 strong CAS，共 20 种操作／顺序组合。每组执行 debug／release × Darwin arm64／GNU amd64／musl amd64，共 120 个组合。
- 每个组合均通过 Scoop 实际优化与 RewriteStatepointsForGC、严格根计划检查、LLVM `verify<safepoint-ir>`／IR verifier 及目标 object 生成；原子指令仍存在，managed reference 不转成整数，CAS 未变为 weak。
- Darwin 的 LLVM 22.1.8 与 nuc12 的 LLVM 22.1.2 分别通过全部 120 个组合；fmt 与 codegen all-targets clippy 通过。日志为 `tmp/m33/atomic-ir-tests.log` 和 `tmp/m33/atomic-ir-linux-tests.log`。本批只完成后端可行性验证，尚不代表 Atomic 类型、内存序源码诊断、写屏障或实际 moving GC fixture 已实现；没有重跑无关 CLI 测试。

## M33-9a：正常 shutdown 的资源诊断

- 主线程正常返回后，thread／callback registry 分别在原锁下关闭登记并取得非主 attachment、活动 callback lease 和 ownership 非零的 token 数。令牌有多份 ownership 时只计一个，OneShot 消费 ownership 后的活动 invocation 仍计入活动数。
- 任一计数非零时，释放锁后输出运行时规范第 7 章规定的完整计数诊断，沿现有 NativeSafe 终止路径刷新 stdout/stderr 并 `_exit(1)`。退出码覆盖 main 的返回值；不等待线程、不进入异常／GC 元数据清理。资源清空时保留正常返回码与既有清理流程。
- callback 自行建立的临时 attachment 保留到 token 状态及 GC handles 清理完成之后，避免 shutdown 在收尾尚未结束时观察到线程与 callback 均为空。shutdown 后的新 invocation 继续按既有 ABI 规则拒绝；没有新增 runtime ABI、线程模式或登记框架。
- 新增 4 项正式 fixture，覆盖仅遗留 attachment、两个有 ownership 的 token、已消费 ownership 的活动 OneShot，以及正常 join／release 与显式 exit。用 C 条件变量确定线程已进入相应状态，不依赖睡眠或忙等。正常退出保留 Int 41；失败覆盖 Int 73／91；显式 exit 保留 23。
- 每个目标均通过 4 项、8 个 debug／release 变体、62 个进程、24 次 golden 检查，包含普通／moving／minor GC；先删除源码再独立链接。诊断与输出逐字断言，正常退出执行 atexit，失败／显式退出不执行。captured struct、String、argv 和 OneShot／Reusable 回调组合验证实际根保活。
- GNU／musl 的普通模式均使用最终实现通过全部四项。Darwin 先完整通过四项，调整 callback detach 顺序后仅重跑受影响的两项，普通模式通过 4 个变体、34 个进程和 12 次 golden 检查，其余两项复用先前结果。8 份公共 HIR／MIR golden 在三目标间一致。
- workspace fmt／all-targets clippy 和 Darwin／GNU／musl 的 C 严格警告检查通过；现有 NativeSafe 返回与发布握手回归通过 O0／O2 × full／minor 四种配置。报告位于 `tmp/m33/shutdown-darwin-*-report.json` 与 `tmp/m33/shutdown-linux-reports/`。本批没有重跑无关全量测试；M33-7 泛型／tuple Equality、M33-8 原子语言实现及最终组合验收仍未完成。
