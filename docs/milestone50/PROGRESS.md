# M50 实施记录

依据 [设计](DESIGN.md)，实施日期：2026-10-17。原始日志与测试工作文件在仓库外 `~/.cache/scoop-m50`；用户原有 M47 文档修改不纳入提交。

## 分批实施

1. 工作文件生命周期：删除 generated-C 源码／对象及 LLVM 对象的只读封存和权限断言；保留实际编译、对象使用和失败清理。
2. CBOR：删除解码后的整包重编码比较、只服务比较的输出限额及解码 trait 对编码 trait 的依赖。现行字段读取、内存边界和 deterministic writer 保留。
3. 诊断源码：删除 source record／context 与 provider 副本的逐项来源证明、专属错误和两项证明测试；保留正常诊断数据和引用。
4. Native 工具输入：include 搜索参数直接交给 C/C++ 工具；删除目录存在性／类型预审，将缺失目录和文件搜索路径改为运行正例，组合覆盖共享头文件与 forced include。删除仅比较 C/C++ 完整版本号／目标名称的配对拒绝及两个伪造文本的 fixture。实际选中源码、工具、头文件、目标和链接失败继续覆盖。
5. LLVM IR：删除生产路径完整 module 验证、自建 RS4GC verifier、专用 pointer witness 和独占 negative tests；保留优化后实际根分组、RS4GC、GC 元数据发射及直接输出／运行回归。
6. Native 对象：删除段名、load command、C++ 符号和可执行栈黑名单及完整 relocation/EH/TLS 复查；删除选择阶段资格包装。支持普通 COMMON 合并和平台强弱绑定；runtime index／缓存规则相应升级。重复 archive TOC 比较及其破坏测试退役。
7. 自产机器对象：删除 codegen 完整机器对象／frame chain／stackmap／LSDA 验证器及独占破坏测试；优化后按实际 invoke 保留 EH atom，删除独立 EH manifest 和仅用于验证的 target 字段。终结调用去重等回归直接断言机器 stackmap 数量。
8. 发布入口：删除无生产调用的旧 single-Cone 资格流水线；C bridge 改为实际成员数据读取，删除 profile/template/flags/unit 和 SDK/tool 版本凭证比对。Code 目录直接投影已生成的 member records，production 投影不深比较计划副本；同步删除独占错误、测试和历史资格要求。
9. 待完成：物理数据与 registration 的重复证明链、其余同类要求及最终验收。
10. 最终本机定向回归与 Linux 完整验证尚未开始，不以静态检查宣称验收。

## 验证

工作文件批次：`cargo fmt --all`、workspace clippy 通过；release codegen 的 generated-C 3 项与对象分区 9 项通过。临时文件保持存活与 drop 清理、实际对象生成及 stackmap／registration 断言继续覆盖。

CBOR 批次：格式化、workspace clippy 通过；release wire／identity 共 368 项测试和 1 项 doctest 通过。

诊断源码批次：格式化、workspace clippy 与 release slib 的 layout compile 10 项通过。跨 Cone 源码诊断的 CLI 组合将在工具链批次构建配套命令后验证。

Native 输入批次：格式化与 workspace clippy 通过；native 工具链 9 项测试通过，C++ 修改后定向补验混合语言／头文件变化 1 项通过。本机正式 CLI 41 项、45 变体、101 进程通过，包含 native C、共享头文件和跨 Cone 迭代诊断；无需重复前一批的全量 Rust 测试。

LLVM IR 批次：格式化、workspace clippy 无警告；本机 release codegen 全部 356 项通过，包含三个目标的 IR／机器发射、真实 runtime／GC／异常测试。此前已清理本机 debug target 2.7 GiB，保留 release 工具。

Native 对象批次：格式化、workspace clippy 通过，release linker 3 项测试通过。本机 CLI 66 项中 65 项首次通过；新增 COMMON／弱定义组合暴露 Darwin 与 ELF 的优先级差异，按目标规则修正后两个新增用例复测通过。完整批次为 87 变体、505 进程、53 个专门链接 plan golden；补验 2 变体、4 进程。Linux COMMON 行为留待最终目标验证。

自产机器对象批次：格式化、workspace clippy 无警告，本机 release codegen 331 项全部通过；含真实 GC／异常运行、三目标机器发射、终结调用与条件 poll 的实际 site 数量。

发布入口批次：格式化、workspace clippy 无警告，release slib 334 项全部通过。配套 release CLI 重建后，C ABI、Scoop ABI、回调重入和 native storage 4 项／8 变体／40 进程通过，保留 8 个专门链接 plan golden。
