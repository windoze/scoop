# ProgramLink 直接读取 production 与 native ABI 数据

状态：本机、Linux GNU／musl 验收完成。实现基线：`da7c2dec`。

## 变更

ProgramLink 不再调用 strong production replay、dependency production catalog、layout／descriptor／dispatch 重建、shape support 比较或 physical import replay。它从已有字段读取 definition owner／role、image／入口与 trap 符号、六类注册的 definition／symbol，以及 native ABI 匹配所需的标量／指针表示。runtime String alias 直接解析保存的 typed owner 和 contract；InlineBytes 合同仍在生产该别名时检查。

删除失去消费者的布局与 ABI 重建 helper、descriptor／dispatch reader 和 shape-source 推导。普通 param-free bridge 不再由 ProgramLink 解码；完成读取后不再保存完整 LIR foundation，仅保留 native C++ 标记。生产端、Compile、wire/profile、runtime ABI 与 runtime image fingerprint 公式没有变化。ODR 合并、实际符号选择、native contract 匹配和工具错误继续参与本次链接。

代码与 spec 共 46 个文件，新增 373 行、删除 1,487 行，净删 1,114 行。主要新数据读取模块分别为 46、50、85 行；ProgramLink artifact reader 为 120 行，image 组装为 70 行。

## 验收

本机 workspace clippy、528 项 slib、6 项入口、release、4 项 linker及真实产物 ProgramLink通过。入口测试改为断言真实 root descriptor symbol；ProgramLink 测试保留生产／生成入口、别名 wire 往返和实际输入错误，删除四个只服务 alias 消费重建的破坏情形及其编码 helper。

本机组合通过 17 项、22 variants、245 processes；native 补充通过 7 项、7 variants、25 processes。总计 24 项、29 variants、270 processes，覆盖普通／显式链接、初始化依赖顺序、moving GC、ODR 合并与冲突、C／C++、GC leaf、生成测试入口、M46 序列化及真实 native ABI／effect／缺失库／保留符号错误。全部源码、工具与原产物保留。

Linux 使用独立 `target/m48-summary-reader` 构建目录，按脚本完成 fmt、workspace clippy、release、529 项 slib、6 项入口、17 项 linker和真实产物 ProgramLink，全部通过。GNU CLI通过 24 项、29 variants、270 processes。musl CLI通过 23 个适用项，C++ 项按原声明不适用；27 variants、256 processes。最终同步 dry-run 没有 compiler 文件差异。

## 实际 I/O

GNU 采样和系统调用记录确认：三个产物各打开一次，manifest 和 metadata 的读取范围没有重叠；4,458 个机器对象成员读取量为零，compiler child 为零，程序输出 `42`。所有源码、工具、原产物和稳定对象均保持存在。此次读取量不因 production 重放退役而变化；本批删除的是已解码数据的重新构造、比较和长期保留，五轮 wall／进程树 RSS 性能验收仍待完成。命令与范围见采样脚本。

## 清理与边界

本机 Cargo／rustc 进程均结束后，清理 incremental 4,950,100,741 字节；Linux 验收及采样结束后，清理 incremental 13,139,123,342 字节。release 工具、稳定对象与验收工作目录保留。

ProgramLink 的 identity／foundation 读取边界、Compile 的语义与布局重放、writer 其余复核、dump 最小阶段生成与复用、最终当前代码全量回归和五轮性能验收仍待完成。此次定向回归不替代这些要求，M48 尚未完成，M38 未启动。
