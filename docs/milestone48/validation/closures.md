# 删除重复的跨 Cone Link closure section

本批基于 `fa2d9faf`，完成两项重复 Link section 的退役与三平台验证。M48 尚未完成，M38 未启动。

## 实现与格式

删除 `cross-cone-link-closure/1`、`cross-cone-layout-link-closure/8` 的持久 section、coverage digest、对象目录与 relocation use 副本，以及对应 decoder、发布比对和旧 Code 包装接口。实际 ordinary／layout import 分类继续保留；Code contribution 直接使用这两类既有数据，保留原指纹字段名称和公式。两个名称不再注册为 metadata section contract。旧 cross-Cone profile 按 descriptor／fingerprint 变化失效，runtime ABI 不变。

顺带删除此前已失去调用方的 native Code projection reader 和两类 relocation use 的专用编码实现。没有新增 reader 或严格验证路径。[变更统计](closures-change-summary.json.gz)：47 个代码／规范文件，净删 1,962 行；ordinary import 主文件 199 行、external shape use 主文件 28 行、layout writer 294 行。

[测试名称清单](closures-test-changes.json.gz)记录 14 项随退役功能删除的专测，以及两项迁移到实际 LinkSupport 的 inventory／decode 测试。真实 Code、符号分类、非空 physical imports、共享布局往返、ODR、native 和 GC 覆盖保留。[profile 向量](closures-profile-vectors.json.gz)独立计算删除对应 descriptor 条目后的指纹。

## 已完成的本机验收

[workspace clippy](closures-darwin-clippy.log.gz)无警告；[528 项 slib 与 26 项 protocol](closures-darwin-unit.log.gz)通过。删除最后两段无调用方编码后，重新完成 fmt、clippy、[528 项 slib](closures-darwin-final-unit.log.gz)与 [release 构建](closures-darwin-release.log.gz)。

[实际泛型机器产物](closures-darwin-generic.log.gz)和 [core 跨 Cone 产物](closures-darwin-layout.log.gz)通过，后者覆盖本批调整的非空共享布局读取路径，运行 351.43 秒。[17 项 CLI](closures-darwin-cli.log.gz)、[报告](closures-darwin-cli.json.gz)通过，22 variants、245 processes；包含独立链接、ODR 冲突、稳定对象缺失补产、runtime image、argv、M46 序列化、C/C++、GC 与 Test 用途。CLI 使用工作区已迁移的 fixture 描述，源码与工具保持存在。

清理已归档且无进程使用的[上一批 fixture 目录](closures-cleanup.json.gz) 1,680,380,017 字节；本机 Cargo 全部结束后清理 [incremental](closures-incremental-cleanup.json.gz) 4,193,050,415 字节。

## Linux 验收与实际读取

独立 `target/m48-summary-reader` 下 [fmt](closures-linux-fmt.log.gz)、[workspace clippy](closures-linux-clippy.log.gz)、[release](closures-linux-release.log.gz)、[529 项 slib 与 26 项 protocol](closures-linux-slib.log.gz)、[实际泛型](closures-linux-generic.log.gz)和 [core 跨 Cone 产物](closures-linux-layout.log.gz)均通过。core 测试耗时 446.64 秒。[GNU](closures-gnu-cli.json.gz)为 17 项通过、22 variants、245 processes；[musl](closures-musl-cli.json.gz)为 16 项通过、1 项 native C++ 不适用、20 variants、231 processes。命令见[验证脚本](closures-linux.py.gz)。

[GNU strace](closures-gnu-link.trace.gz)与[摘要](closures-gnu-link.json.gz)确认三个产物各打开一次，manifest 和三个 metadata 成员无重复读取，4,458 个 ELF 成员的读取量为零。实际 scoopc 调用为零，程序输出 `42\n`。[采样脚本](closures-trace.py.gz)保留源码与原产物。最大产物为 39,726,908 字节，实际读取 27,612,495 字节，均比前批同一用例减少 286,292 字节；这不是最终五轮性能验收。

本批不代表 M48 最终验收。Compile／ProgramLink 的语义与布局重放、writer 的其余复核、冷 locator 的文件入口复用、dump 按需生产、历史迁移闭合、最终当前代码全量回归和五轮性能验证仍待完成。
