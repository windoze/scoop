# MORE-CLEANUP 集中实施记录

日期：2026-10-15；状态：实现与三平台统一验收完成。M38 尚未推进。最终结果见[概要](README.md)。

## 已完成范围

- [普通 CLI 名称级清单](../CLEANUP-FIXTURES.tsv)：488 次重复链接退役，保留原输入／多 Cone 构建、依赖数量及全部运行输出与 GC stress；2 个 program-read 无用首次 build 删除。native、ODR、显式链接入口及失败不发布专测保持。
- [core CLI 清单](../CLEANUP-FIXTURES.tsv)：34 个通用 consumer 删除，产物生产用例与 Rust typed 结构断言保留；base 承担通用读取 smoke，6 个 ABI／TD／初始化 consumer 改为实际导出调用并执行普通／moving-GC 程序。已有 heap ZST 和 ordinary consumer 保留。
- Rust core-layout 的 19 个测试／36 份源码保留。各自 HIR／MIR／LIR、ZST 参数、间接返回、dispatch、box、descriptor、初始化等结构断言仍在原模块；完整对象／打包仅 base、shared-aliases-combined、property-initialization-provider。base 完整读取链仅在最终结果检查保存记录，不逐阶段重复完整相等性断言。
- [共享类 core 输入清单](../CLEANUP-FIXTURES.tsv)：172 个普通类用例用完全相同的声明消费共享目录；不同 core 输入／真实失效专测不改。另有 [842 个普通语言／产物用例](../CLEANUP-FIXTURES.tsv)共享原 stage3 helper core，检查各自声明未写入共享输入后统一迁移；保留各自全部产物／诊断／运行断言。固定目录在启动 fixture 并发前准备，源码与工具存在；缓存及编译／链接仍由真实命令负责。
- [对象复核退役](../CLEANUP-TESTS.tsv)：7 个纯字节替换测试删除，3 个混合用例仅删相应分支。10 处重复整包哈希移除；registration、image、entry 回填不再清零／重读解析全部对象，沿用最初 section、symbol、relocation 数据，实际内容摘要随回填更新。格式范围、GC root、relocation、frame chain、record 和 patch 槽位检查保留。
- 旧 single-cone-strong／cross-cone-semantics-strong 的 descriptor、fingerprint 正向矩阵及只供旧拒绝用例构造完整产物的辅助删除。通用格式测试改用当前 profile；当前 descriptor 固定向量和 required inventory 保留。兼容性 reader 用两条旧 profile 身份验证拒绝，identity 的旧版本检查覆盖至 5。
- @GCLeaf／enum C ABI／calling-convention 的旧 M12 字样清理，只改诊断表述，保留 negative 覆盖。

## 验证策略与当前结果

遵从用户最新指示：删除用例不重跑，修改用例只跑受影响用例；本批还有对象生产、profile 识别及诊断实现变更，集中完成本机必要完整复验后再上 Linux。全量过程中发现的问题集中处理，只补跑受影响包／用例，不无理由重新跑已通过部分。

本机格式化、workspace Clippy 无警告，Ruff 0.16.10 格式／检查及 50 项 runner 测试通过；全部 4,107 份声明与源码归属发现通过。Rust 完整复验 5,302 项通过。首轮仅两条旧诊断／profile 期望失败，修正后各补验一次；不重复全套。正式 CLI 4,089 项通过、18 项不适用，4,613 variants、16,356 processes、88 项 stage／plan golden。首轮仅三个新增 consumer 失败：ABI 用例的 UInt8 字面量改为显式转换，两个 descriptor 用例改用 core 实际公开的对象与接口；只修改 consumer，不放宽 provider 可见性。三个用例集中补验通过，其余首轮结果保留。

耗时与工作量：本轮完整首轮墙钟 1,688.1 秒（28.1 分钟），另加三个用例补验；同 M4 Pro、10 并发下，最终有效用例累计耗时从 32,324.5 降至 16,770.0 秒，减少 48.1%。累计耗时是并行用例各自耗时之和，不是墙钟；上一批未记录可靠的完整墙钟，因此不报告墙钟加速比例。进程调用从 17,040 降至 16,356，减少 684；variants 与 golden 数量保持。最终代码的五轮构建性能测量已完成，共 55 次构建，生成程序输出正确；全部暖构建零编译子进程，详见[性能记录](../PERFORMANCE.md)。本机修改、清理及完整复验完成后统一进行 Linux 验收，结果如下。

Linux 上一轮中断结果：825 通过、29 不适用、3,253 中断，无失败；musl 未开始。中断结果不计为完整通过。已清理其结束的 cases 目录，保留近期编译缓存。追加清理前的结果仅作为历史记录，不标作本批最终验证。

当前 Linux 整仓格式／Clippy、release、完整 Rust 5338 项和 50 项 runner 规则测试一次通过，未跳过或重复测试。Rust 中需要 paired 编译器的 linker 测试直接使用当前 release 工具。GNU 正式 CLI 4,078 项通过、29 项不适用，零失败、无需补验。musl 正式 CLI 4,074 项通过、33 项不适用，零失败、无需补验。两套结束的工作目录均已清理，近期 Rust／release 编译缓存保留。功能验收结束后的[外部调用与 I/O 观察](final-io.md)全部通过；原始日志、逐用例报告与 trace 保存在仓库外，本文只保留结果概要。
