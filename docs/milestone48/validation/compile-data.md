# Compile 直接读取 production 数据，集中退役重放 API

实现基线：`aa5c2cd0`。本机针对性验证通过；整体本机复验和 Linux 验收留待 M48 剩余修改、清理完成后统一执行。

## 变更

CompileProduction 只保存消费者实际使用的 definition symbol plan、静态存储语义、初始化语义及 cell／registration 定义引用。解析已有 typed 引用，复用 provider 的布局导出及初始化定义；不构造对象生产专用的 digest node／patch 或完整注册计划。初始化定义只读取一次。定义表保留正常顺序检查，静态存储保留所引用 scan 与 layout 的关系检查；不重新计算布局或摘要。

删除完整 V2 production／registration 重放、独占的 foreign type reference 目录、digest graph 重建及 foundation resolver。LIR dependency records 直接解析 typed 引用，不再从 HIR materialized roots、static storage 和初始化注册表重算闭包。生产端完整注册、digest 生成、对象发射与实际物理引用选择继续保留。没有 wire／profile 或 runtime ABI 变化。

集中删除或替换 [23 个显式测试函数](compile-data-retired-tests.json.gz)，以及 generic／digest／dependency 等测试中的重放专用 helper 与重复断言。保留正常 wire 格式与往返检查、生产端检查、实际跨产物读取和语言功能 fixtures；未删除普通 negative fixtures。122 个代码／规范文件新增 740 行、删除 3,962 行，净删 3,222 行；现存生产端 layout join 从 replay 目录移出。

## 本机验证与调度调整

本机执行 `cargo fmt --all` 与 [workspace clippy](compile-data-darwin-clippy.log.gz)，随后通过 [454 项 LIR](compile-data-darwin-lir.log.gz)、[527 项 slib](compile-data-darwin-slib.log.gz)、[release 三命令构建](compile-data-darwin-release.log.gz)和[实际泛型库／共享 ODR 对象测试](compile-data-darwin-generic.log.gz)。此前的生产专用 digest 引用失败已由缩减读取数据消除，实际泛型测试通过。

用户要求所有修改、清理先在本机完成，完整复验后才统一到 Linux 验证。因此停止本批组合脚本继续调度，没有启动本批 Linux 测试，也没有执行本批 CLI 组合。已启动的属性初始化长测试随调度停止而中断，[日志没有完成结果](compile-data-darwin-initialization-interrupted.log.gz)，不计为通过；它和初始化／静态存储、re-export、runtime image、继承／接口／装箱、native、M46 等组合覆盖将在整体本机复验中统一执行。

后续迭代只保留格式化、lint 与受影响的针对性测试；不再按每个清理批次进行 Linux 验收。本批没有进行性能复测或整仓全量测试，也不以已有针对性结果替代最终 M48 验收。

## target 与后续范围

[清理](compile-data-cleanup.json.gz)上批已结束且报告已保存的 fixture 目录：本机 2,340,954,112 字节，Linux GNU／musl 共 4,746,137,600 字节，总计 7,087,091,712 字节磁盘分配空间。近期增量缓存、release 工具及仍复用的工作目录保留。

Compile 中其他 HIR／MIR／LIR 布局、ABI、语义与物理重放，ProgramLink identity／foundation 边界，writer 其余复核，dump 复用与最小阶段仍待完成。所有修改清理完成后，先本机完整复验，再统一 Linux GNU／musl 验收及五轮性能复测。M48 保持实施中，M38 未推进。
