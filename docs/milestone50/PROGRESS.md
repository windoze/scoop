# M50 实施记录

依据 [设计](DESIGN.md)，实施日期：2026-10-17。原始日志与测试工作文件在仓库外 `~/.cache/scoop-m50`；用户原有 M47 文档修改不纳入提交。

## 分批实施

1. 工作文件生命周期：删除 generated-C 源码／对象及 LLVM 对象的只读封存和权限断言；保留实际编译、对象使用和失败清理。
2. CBOR：删除解码后的整包重编码比较、只服务比较的输出限额及解码 trait 对编码 trait 的依赖。现行字段读取、内存边界和 deterministic writer 保留。
3. 诊断源码：删除 source record／context 与 provider 副本的逐项来源证明、专属错误和两项证明测试；保留正常诊断数据和引用。
4. 待完成：native 工具选择与 include 路径、自产 LLVM／对象复核、native 对象额外拒绝、发布证明链及关联历史要求。
5. 最终本机定向回归与 Linux 完整验证尚未开始，不以静态检查宣称验收。

## 验证

工作文件批次：`cargo fmt --all`、workspace clippy 通过；release codegen 的 generated-C 3 项与对象分区 9 项通过。临时文件保持存活与 drop 清理、实际对象生成及 stackmap／registration 断言继续覆盖。

CBOR 批次：格式化、workspace clippy 通过；release wire／identity 共 368 项测试和 1 项 doctest 通过。

诊断源码批次：格式化、workspace clippy 与 release slib 的 layout compile 10 项通过。跨 Cone 源码诊断的 CLI 组合将在工具链批次构建配套命令后验证。
