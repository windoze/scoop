# M31 实施记录

日期：2026-10-07。基线：`6e62514da`（M30）。设计见 [DESIGN.md](DESIGN.md)。

## 实施顺序

1. 配置与基线：保存 M30 可复跑数据；贯通 debug/release、child 请求、producer 与缓存。
2. ODR 与 image：删除正文判等，按身份和 ABI 选择物理实现，迁移 metadata ABI 5。
3. 首批优化：局部传播、LLVM 函数内 pass、最终 GC 发射计划。
4. 分配与屏障：跨 line 普通分配、精确 size、引用范围写屏障。
5. nursery：年轻代 TLAB、minor、首次存活晋升、pin 与 full fallback。
6. 三目标验收及性能对照。

每项功能完成后单独提交；先格式化与 lint，再执行受影响的单元测试、正式 CLI fixture 和必要平台组合。复用未变化的通过结果，不逐批重跑全量。阶段快照保留实际类型、ABI、GC 与引用结构，不固定无关代码摘要。

## 当前状态

- 已读取设计与对应规范；工作区原有 M31 文档作为实施基准保存。
- Linux `nuc12:~/repos/scoop` 留有 M30 测试变更；Linux 验证将使用独立目录，保留原目录内容。
- 尚未完成 M31 功能；后续在此记录每批实际改动、版本与验证结果。
