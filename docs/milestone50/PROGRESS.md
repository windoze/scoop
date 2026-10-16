# M50 实施记录

依据 [设计](DESIGN.md)，实施日期：2026-10-17。原始日志与测试工作文件在仓库外 `~/.cache/scoop-m50`；用户原有 M47 文档修改不纳入提交。

## 分批实施

1. 工作文件生命周期：删除 generated-C 源码／对象及 LLVM 对象的只读封存和权限断言；保留实际编译、对象使用和失败清理。
2. CBOR：删除解码后的整包重编码比较、只服务比较的输出限额及解码 trait 对编码 trait 的依赖。现行字段读取、内存边界和 deterministic writer 保留。
3. 诊断源码：删除 source record／context 与 provider 副本的逐项来源证明、专属错误和两项证明测试；保留正常诊断数据和引用。
4. Native 工具输入：include 搜索参数直接交给 C/C++ 工具；删除目录存在性／类型预审，将缺失目录和文件搜索路径改为运行正例，组合覆盖共享头文件与 forced include。删除仅比较 C/C++ 完整版本号／目标名称的配对拒绝及两个伪造文本的 fixture。实际选中源码、工具、头文件、目标和链接失败继续覆盖。
5. LLVM IR：删除生产路径完整 module 验证、自建 RS4GC verifier、专用 pointer witness 和独占 negative tests；保留优化后实际根分组、RS4GC、GC 元数据发射及直接输出／运行回归。
6. 待完成：自产机器对象复核、native 对象额外拒绝、发布证明链及关联历史要求。
7. 最终本机定向回归与 Linux 完整验证尚未开始，不以静态检查宣称验收。

## 验证

工作文件批次：`cargo fmt --all`、workspace clippy 通过；release codegen 的 generated-C 3 项与对象分区 9 项通过。临时文件保持存活与 drop 清理、实际对象生成及 stackmap／registration 断言继续覆盖。

CBOR 批次：格式化、workspace clippy 通过；release wire／identity 共 368 项测试和 1 项 doctest 通过。

诊断源码批次：格式化、workspace clippy 与 release slib 的 layout compile 10 项通过。跨 Cone 源码诊断的 CLI 组合将在工具链批次构建配套命令后验证。

Native 输入批次：格式化与 workspace clippy 通过；native 工具链 9 项测试通过，C++ 修改后定向补验混合语言／头文件变化 1 项通过。本机正式 CLI 41 项、45 变体、101 进程通过，包含 native C、共享头文件和跨 Cone 迭代诊断；无需重复前一批的全量 Rust 测试。

LLVM IR 批次：格式化、workspace clippy 无警告；本机 release codegen 全部 356 项通过，包含三个目标的 IR／机器发射、真实 runtime／GC／异常测试。此前已清理本机 debug target 2.7 GiB，保留 release 工具。
