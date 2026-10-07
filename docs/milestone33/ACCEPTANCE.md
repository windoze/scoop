# M33 实施与验收记录

开始日期：2026-10-08。基线：`670a45477`（M32 完成），分支：`codex/m33`。

本文件只记录实际完成的实现和验证；设计方案与验收要求见 [DESIGN.md](DESIGN.md)，未完成项目不计为通过。

| 批次 | 能力 | 状态 |
| --- | --- | --- |
| M33-1 | NativeSafe/collector、GCLeaf、DirectC | 完成并通过三平台验收；errno bridge 组合在 M33-5 补验 |
| M33-2 | 作用域数据借用、计数 pin | 完成并通过三平台验收 |
| M33-3 | 严格／可空／lossy UTF-8、C 字符串 | 完成并通过三平台验收 |
| M33-4 | main、argv、退出码、输出与 ABI 11/7 | 待实现 |
| M33-5 | errno 捕获 | 待实现 |
| M33-6 | native C/C++、系统库与源码选择 | 待实现 |
| M33-7 | sysroot 默认定位、Equality | 待实现 |
| M33-8 | 原子类型、内存序与 GC | 待实现 |
| M33-9 | 线程退出规则与组合验收 | 待实现 |

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
