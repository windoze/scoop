# M30 实施记录

目标与验收矩阵见 [设计](DESIGN.md)。按独立功能完成实现、针对性验证并提交，最后执行正式整体验收；未经实际运行的项目不记作通过。

## 实施计划

- A：Float/Double 标量、literal、alias、完整 IR/meta/ABI 与最短字符串化。
- B：算术、IEEE 比较、分类、totalOrder、显式转换与 const。
- C：候选隔离与直接舍入、默认值、annotation、递归 pattern、泛型及派生相等。
- D：值存储、GC/协程与 C/Scoop FFI、fmod native support。
- E：single-value 协议、companion codec 与 JSON。
- F：跨 Cone/ODR、artifact-only、Darwin 与 Linux glibc/musl 验收。

## 2026-10-06：开始实施

以 `93ae1e671`（M29 完成）为代码基线，保留并采用工作区已有的 M30 设计、调研及三份规范修订，分支为 `codex/m30`。

本机 `target` 初始约 31 GiB，其中旧 debug incremental 约 16 GiB；确认没有活动构建后清理该 incremental 目录，保留现有编译依赖与历史验收资料。开发期优先受影响 crate 和选定文件 fixture，不反复运行全量测试。Linux 验证使用 `nuc12:~/repos/scoop`。

## A：浮点标量闭环

实现 Float/Double 的真实 intrinsic nominal、透明 alias、未舍入十进制 AST、目标精度 literal 与负零，贯通 HIR/MIR/LIR、持久化记录、标量 ABI、C storage 及 LLVM `float`/`double` 常量。以固定版本 `rustc_apfloat 0.2.3` 直接解析目标精度，以 Ryu 固定 commit 提供最短字符串化；新增自有模块按职责拆分，第三方源码保留原样。

实际 CLI 暴露并补齐了 intrinsic struct application、具体化以及 LLVM 局部常量池的对象记录。常量池沿真实重定位保存字节并进入已有对象摘要，不增加源码实体或来源认证。受影响 section 及兼容版本见实现规范 §2.18。

Darwin/AArch64 验证：

- `cargo fmt --all` 与 `LLVM_SYS_221_PREFIX=/opt/homebrew/opt/llvm@22 cargo clippy --workspace --all-targets` 通过。
- 三个配套 CLI 的 release 构建通过。
- 选定单元测试通过：parser 浮点语法 3 项、identity 浮点 wire 2 项、slib profile 19 项、Mach-O section 7 项、HIR persistent function 3 项。
- `m30-floating` 的 10 个正式文件 fixture 通过，包括普通／移动 GC 运行、四阶段 golden 和 9 个精确诊断负例。直接舍入反例得到 F32 `0x4b800001`，正负零、最小 subnormal 和下溢均经实际执行验证。
- Char 基础组合回归通过普通／移动 GC 与四阶段 golden；HIR golden 仅同步新增 core 声明导致的临时 imported identity index 变化，已确认其余内容相同。

本批尚不声明算术、const 求值、pattern、完整 FFI 或 JSON 已完成；它们继续按 B～F 实施。未运行全量测试。随后在 `nuc12` 的 Linux/glibc x86_64 完成 release 构建和上述 10 个 M30 fixture，包含普通／移动 GC 与四阶段 golden，全部通过。

## B1：浮点运算与显式转换

新增封闭的 unary/binary/conversion 数据节点，贯通默认模板、具体化、MIR、LIR、闭包引用及各自遍历。core 提供算术、IEEE equals、分类、totalOrder 与整数／浮点显式转换。四种关系比较直接按真实浮点表示降低，不借 compareTo；div/rem 保持 NoGC。LLVM 使用严格浮点操作和直接目标位宽饱和转换；totalOrder 与分类使用原始位表示。取余产生的 fmodf/fmod 沿已有 native-support 表及平台链接闭包处理。

同步 HIR core-bootstrap `/11`、interface `/59` 与 LIR link-identity-closure `/14`、profile vectors 和实现规范。新增自有 Rust 模块为 18～137 行，按数据、前端、降低、LLVM 运算／比较／转换拆分。

Darwin/AArch64 实际验证：格式化、workspace clippy、三个 release CLI 构建通过；profile 单元测试 19 项、target-support 单元测试 3 项通过。M30 文件 fixture 增至 19 个并全部通过，29 个实际进程、16 份阶段 golden。新增算术、八种整数转换、混合精度与非法运算诊断；totalOrder 对每种精度各 18 个代表位型执行 324 对比较，包含正负 signaling/quiet NaN 及不同 payload，同时检查一元符号和同型转换保留位型。四个正例均通过移动 GC 运行。

本批完成运行期数值操作；const 与 companion 常量接着实施，不重复执行无关全量测试。

## B2：const、companion 常量与静态初值

以 rustc_apfloat 的 Single/Double 实现目标精度的 const 算术、比较、分类、totalOrder 和全部显式转换，算术 NaN 规范化为正 quiet NaN；一元符号和同精度转换保留位型。Float/Double companion 各提供六个普通 const val，含精确边界。修复依赖库 companion 常量在 const initializer 中的读取，沿普通 imported const 名称、可见性和类型规则消费；静态初值与 const 共用浮点求值函数，浮点全零初值按真实标量表示保存。

将 const 表达式处理和静态整数调用按职责拆分，原相关大文件降至 404 / 282 行，拆出的文件为 240 / 265 行；新增浮点求值及调用模块为约 30～165 行。再次清理约 6.8 GiB 的 debug incremental 目录。

Darwin/AArch64：格式化、workspace clippy 和 release CLI 构建通过。新增 const 正例包含普通／移动 GC、四阶段 golden、通过 C ABI 读取精确位型的检查；固定 NaN、负 NaN 复制、直接舍入、最大值、最小 subnormal 和最小 normal 位型均通过。五个 const 负例及两个既有整数 const CLI 回归通过，本轮共 8 fixture、14 进程、12 golden；两个整数 HIR golden 仅临时 imported identity index 改变。12 个既有 const 单元测试通过，期间补齐旧测试用 core 缺失的 Float/Double 与转换声明，并复用实际 core 的浮点源码。

运行期与 const 的 B 批完成，下一批处理候选字面量定型、annotation 和递归 pattern 组合。

B 批 Linux/glibc x86_64 的 release 构建与四个正例通过：普通／移动 GC、16 个进程、16 份 golden。同步前三个 HIR 快照的 imported identity 临时编号，并为两个 C ABI 用例按 target 保存 LIR 桥接符号快照；Darwin 同组四个用例再次通过。未增加无关全量测试。

## C1：候选隔离、默认精度与 receiver 推导

无后缀浮点 literal 在参数、泛型 fixed point、array 和控制流分支中保留 contextual 状态；普通 MSC 后的 literal 默认优先级扩展至 Double。局部与依赖 callable、nominal constructor 共用同一个数值候选记录。receiver 仅探测两种真实 core 表示，以最终 typed intrinsic 判断资格；算术结果可传递 expected Float，显式转换不以结果类型反推源精度。const 采用相同的候选选择和直接舍入。新增推导模块分别为 79 / 117 行。

实际组合用例发现并修复普通泛型 struct 构造器形状查询的提前返回，使嵌套 `Cell(字面量)` 可以从另一实参获得 Float 约束。

Darwin 验证：格式化、workspace clippy、release CLI 构建通过。新增 inference 正例及两个负例通过，共 5 进程和 4 份 golden；覆盖声明顺序、失败的溢出候选、默认 Double、直接舍入反例、receiver 关系运算、const、泛型及嵌套构造器、array、tuple、分支、vararg、默认参数、return 和 lambda。普通与移动 GC 输出一致；用户 extension 不获得反向定型资格，固定 f 后缀不适配 Double。五个既有整数联合推导单元测试通过；浮点 operations/const 与整数 const CLI 回归另有 3 fixture、10 进程、12 golden 通过。

## C2：annotation、递归 pattern 与派生相等

annotation 的 parser 与 lowering 复用原始浮点语法，支持带符号 literal 和同类型 const 引用，metadata 保存目标精度 bits。literal pattern 通过真实 equals intrinsic 保存 FloatKind，默认模板 equality 增加 tag 4，HIR interface 更新至 `/60` 并同步 profile vectors；MIR 复用既有 FloatBinary/Equal。浮点列的剩余域由 wildcard 行覆盖，因为任何 literal 都不能匹配 NaN；递归 product 继续用既有矩阵检查。匹配与 binding 共用标量形状检查，避免把 Float/Double 当作空 struct 解构。按语言规范修正设计文档对可选 usefulness 诊断的过度要求，保留 first-match 语义。

Darwin 验证：格式化、workspace clippy、release CLI 构建通过；19 个 profile 测试、14 个 parser 整数字面量回归通过。新增 annotation/pattern 两个正例通过普通／移动 GC 及 8 份 golden，确认直接舍入、负零、NaN、泛型字段、默认值与递归 enum/struct/tuple，以及含 NaN 的派生值不等于自身。14 个新负例覆盖 annotation 类型／范围、pattern 类型／不可穷尽／guard／解构和 Hash 缺失，正式 runner 全部通过。既有 annotation 源码、参数类型负例、整数递归穷尽与 Char 解构回归通过；两个 HIR 快照仅更新 imported identity 临时编号，已完整归一化比较。期间清理约 5 GiB 旧 incremental 缓存。

## D1：线程浮点环境与浮点 foreign callback

主线程和首次附着的 foreign thread 通过同一创建入口安装 C 默认浮点环境，并显式关闭 x86 SSE 的 FTZ/DAZ 或 AArch64 的 FZ。调用期间不反复保存／恢复 fenv；已附着线程的 native 重入责任保持原规范。函数为 runtime 内部实现，不增加 ABI 字段。

Darwin 的 C 严格警告检查、workspace 格式化及 clippy 通过。新增 environment fixture 经正式 CLI 构建、普通／移动 GC 和四阶段 golden 通过，共 5 个进程；主线程检查 rounding/trap/subnormal 设置，foreign worker 在回调前故意改成向上舍入与 flush-to-zero，回调附着后验证 nearest-even、subnormal、Float/Double 参数／Double 结果及 GC 中的闭包捕获。native archive 仍不接收初始化 section，测试使用正常线程入口改变环境。

随后在 nuc12 的 Linux/glibc x86_64 完成 release 构建；inference、annotation、pattern 与 environment 四个正例通过，共 14 进程、16 golden，并保存环境用例的 GNU LIR 快照。x86 SSE 的 rounding、trap mask 和 FTZ/DAZ 也通过实际线程回调验证。

## D2：FFI、值容器与协程

补齐 LLVM C-layout 字段大小及物理／canonical C storage 匹配中的浮点分支，继续使用既有 C 编译器分类 aggregate。三个独立 fixture 覆盖 C 同型／混合 aggregate、Scoop scalar／aggregate ABI、global/TLS、指针、原始与 managed callback，以及数组、ArrayList、泛型 aggregate、Option、boxing、引用字段、闭包和协程 frame。

Darwin 格式化、workspace clippy、release CLI 构建通过。三个新 fixture 共 16 个进程、12 份阶段 golden 通过，均包含普通及移动 GC 运行。位型检查使用负 signaling NaN、正 signaling NaN、负零和最小 subnormal；协程在两个挂起点之间触发 GC，验证保留值与恢复结果。未改动不允许直接调用 FunPtr 或捕获 mutable local 的既有语言规则，测试通过正常 native 调用与显式引用状态表达这些组合。

同批三个 fixture 随后在 nuc12 的 Linux/glibc x86_64 通过 release 构建、16 个进程和 12 份 golden，含两种 GC 模式；GNU 的 C bridge LIR 快照独立保存。

## E：单值 codec 与 JSON

SingleValueEncodingContainer／SingleValueDecodingContainer 增加 Float 与 Double 的独立读写方法，Float／Double companion 以普通 core body 显式实现 codec；既有 JSON 和手写容器实现同时补齐，不增加默认方法。JSON 用原始 number 文本直接解析目标精度，整数路径保持原有精确解析。解析后备复用 String 结果适配的方式，按实际 16 字节、8 对齐的 tagged Option 布局提供两平台 sret 入口；C locale 通过 pthread_once 初始化，临时 NUL 缓冲区按真实文本长度分配。新增 C 实现 68 行，ABI header 和两个平台适配文件各不超过 35 行，不新增通用 FFI 框架。

Darwin 的格式化、workspace clippy、严格 C 告警检查和 release CLI 构建通过。四个新 fixture 共 14 进程、7 份阶段 golden 通过：独立格式保留 NaN payload／Infinity，JSON 只编码有限数，检查直接 F32 舍入、nearest-even 两侧、边界及抽样位型往返、负零、subnormal／underflow、overflow、411 位数字和逗号 locale 下解析；派生 record／enum、泛型 codec、Option、Array、ArrayList 和嵌套错误 path 均经普通／移动 GC 运行。两个负例锁定缺失 Float／Double 单值方法的精确诊断。旧序列化容器与派生依赖两项回归另有 10 进程、3 份 golden 通过；手写 encoder 的快照同步新增方法、浮点类型和相关函数编号。

Linux/glibc 动态与 musl 静态均已通过这四个新 fixture，各为 14 进程、7 份 golden，包含两种 GC 模式；共有 AST/HIR/MIR 无变化，分别保存目标 LIR。两套 libc 的 strtof_l／strtod_l 均通过直接舍入和边界检查。F 批开始前再次清理了约 1.8 GiB 的闲置 debug incremental 缓存。
