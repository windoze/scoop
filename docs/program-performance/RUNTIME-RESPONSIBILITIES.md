# Runtime 职责收敛

基线 `01047d8c6`。普通算法使用 Scoop 正文，已知表示操作消费 typed IR／现有 codegen，C 保留 GC、线程、平台与确有消费者的共享慢路径。没有新的清理开关；旧产物沿现有 ABI／输入更新机制重建。

## 实际消费者与分批结果

| 入口组 | 核对结果 | 处理 |
| --- | --- | --- |
| bool equals/hash/toString、long/ulong hash/toString、Char toString、String eq、long/boolean print/println | 13 个旧入口均无生产消费者；仅 stage 测试替身、旧文档和 Char GC fixture 显式引用。生产正文已经迁移。 | 删除 C 实现、声明、Char 构建项、重复整数 writer 及其专属测试。stage 测试使用明确的 synthetic extern；GC 用例继续验证真实 Char 正文。 |
| String 长度／定位／UTF-8 转换／长 hash | core 普通 extern 仍调用 C；短 hash、字节读取与复制已有生成路径。 | 本批迁移普通算法，并按实际使用退役 C 适配。 |
| String 有界 leaf、复制／concat、Array clone、box/unbox | typed IR 和生成路径已存在；需区分失败、未展开调用与仅余旧声明。 | 按真实生成覆盖收敛，保留实际慢路径。 |
| Context 操作 | generated 操作与 runtime callback／线程入口共用部分逻辑。 | 按消费者拆分生成路径与必要 native 协调。 |
| Float／Double | Ryu 核心有实际消费者，C 还承担文本适配；解析有逐次 malloc。 | 保留成熟数字算法，迁移格式规则并消除不必要的解析堆缓冲。 |
| compiler／caller root frame | 编译器已提供完整 entries，C 仍重放 entries 与链表检查。 | 本批收敛生成发布／撤销；native 根边界仍承担自身检查。 |

第一批 runtime ABI contract 升至 20，metadata ABI 9 与对象布局不变。不为历史公开头文件保留无人消费的重复实现。删除死代码只计维护与静态体积收敛，不计运行性能收益。

StringBuilder／join_parts，以及异常记录生命周期、calloc、全局锁、LSDA 与 libunwind 查询优化按约定后置。原始日志和逐次数据保存在仓库外 `scoop-performance-results/2026-10-26/runtime-responsibilities`。

## 第一批验证

Darwin ARM64：release Rust Clippy、C 警告检查、Python Ruff 通过；4 项相关 HIR／MIR／LIR 单测、5 项真实 core 标量行为、String copy 的 debug／release moving GC 两变体，以及重建／重定位 core 的 8 次编译、链接与运行通过。产物测试同步修正旧 helper import 和 String 定义剪切边界，私有 hash helper 随实际定义一起移动。准备出的 91 个 runtime 对象均不再定义这 13 个符号。

同参数 `-O2` 编译的 `rt.c` 加 Char 翻译单元，`__text` 从 2,524 B 降到 1,040 B，减少 1,484 B；含只读数据和 unwind 的各 section 合计减少 2,122 B。没有热路径改动，不声称运行加速。x86_64 的最终 ABI／运行验证随后续批次一起执行。
