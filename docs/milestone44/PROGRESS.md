# M44 实施记录

依据：[设计](DESIGN.md)。实施起点为 M43 完成提交 `4f3ec3be`，不把旧 M34 性能数据当作当前基线。

## 当前状态

| 批次 | 状态 |
| --- | --- |
| M44-1 基线、计时观测与 spec | 已检查现有 collector、线程边界与 benchmark；准备固定工具和基线，尚未测量 |
| M44-2 零搬迁更新分支 | 待实施 |
| M44-3 引用定位、写回和无引用对象 | 待实施 |
| M44-4 编译器 GC 代码生成边界 | 待实施 |
| M44-5 runtime GC 操作与 Scoop 适配 | 待实施 |
| M44-6 组合、平台与性能验收 | 待实施 |

## 工作约定

- 延续每个完整功能先规范、再实现、格式化／lint、验证及提交的顺序。优化与机械迁移分别提交和测量。
- 工具临时目录为仓库 tmp/m44，生成输出为 target/m44；基线可执行文件和配套工具固定保存，已无用途的生成输出定期清理。
- 本机验证后再执行 nuc12 的 Linux GNU／musl 验证。计时前完成同机编译、测试和清理；对照在同一主机交替运行，至少七次进程样本，不跨主机计算加速比。
- 共享 GC/codegen 路径集成完成后，按设计完成工作区、正式 fixture 全量及三目标适用组合；开发各批先执行受影响范围，不反复重跑无关全量。

## 2026-10-14：实施前核对

- 当前 collector 在规划／复制后无条件重扫 roots、remembered set 与 retained live set；verify_heap_object 又要求 scanned 状态。零搬迁分支必须同时调整这两处职责，不能只加一个条件跳过调用。
- 已有线程测试观测点覆盖 STOPPING、COLLECTING 后以及 RUNNING 发布前的 RESUMING。后者尚不等于设计要求的 T2；计时观测需明确发布边界，保留旧 stop_wait_ns/pause_ns 的定义。
- ScoopGcMetrics 的快照按 world／heap 顺序加锁，后续计时实现须保持同步与锁序。现有 benchmark 已有 parallel 图、region、allocation、poll 与 FFI 工作负载，优先复用真实输入及原始输出检查。
- 当前只登记实施和建立基线准备，不宣称任何 GC 优化或性能收益已完成。
