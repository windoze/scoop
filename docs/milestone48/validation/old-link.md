# 退役旧 single-cone Link 重放 API

本批基于 `61feb20c`，在正常 ProgramLink 改用保存的成员与符号数据之后，删除无生产调用方的旧 reader 资格链。其他遗留生产投影比对 API、只服务验证的持久字段、语义／布局重放、dump、历史清理和最终全量／性能验收仍待完成。

## 删除与保留

删除旧 single-cone Link 的逐级状态、production/object/registration/Code 重建入口和错误类型；删除 Link identity wire 的 materialization、digest patch、object、symbol、final object 重放比较及其资格包装；删除从 final 对象复制并清零摘要生成 provisional 对象的路径。相关私有函数和专用测试 helper 一并删除，没有另建入口保留这些已删除的重放。

保留正常 Compile／ProgramLink 所需的 section 解码、typed 引用解析和对象生产端数据。归档 helper 不再提供修改最终 image digest 的开关；manifest 读取范围与记录往返测试使用正常完整产物。现有对象生产、归档可复现性、扩展成员和 wire 格式断言继续保留；writer 的 Link identity 检查直接比较测试中实际编码的字段，不在生产 reader 建立比对路径。

退役测试清单列出 20 个旧名称：19 项为被删除 API 的专用测试，另 1 项迁移为普通 wire 往返。并未删除源码语言、真实 ODR、ABI、GC 或 FFI 功能用例。reader 主文件从 728 行缩到 212 行，Link identity wire 从 642 行缩到 418 行；无新的运行时格式或 ABI 变更。

## 本机验证

workspace clippy、545 项 slib、release 构建通过。删除旧 API 后修正了一处 writer 测试调用，继续保留实际编码字段断言。最后仅修正了已过时的对象资格注释，再次 fmt／workspace clippy 通过，无语义改动。

6 项正式 CLI、报告通过：8 variants、93 processes，包含普通链接、稳定对象复用／缺失输出补产、泛型 adapter 合并、ODR 冲突、native C 和 Test 用途。此处只覆盖本批相关路径，不代替最终全量。测试完成后另清理 1,316.7 MiB incremental。

## Linux 验证

使用独立的 `target/m48-summary-reader`，历史迁移的 `target/release` 保持原版本。fmt、workspace clippy、release和 546 项 slib通过。GNU 与 musl 各 6 项 CLI、8 variants、93 processes 全部通过，分别见 GNU 报告与musl 报告。执行参数见脚本。
