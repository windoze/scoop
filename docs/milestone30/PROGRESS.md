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
