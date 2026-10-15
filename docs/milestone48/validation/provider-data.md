# 外部物理定义选择复用已有导出

实现基线：`74c1b42d`。本机、Linux GNU／musl 验收完成。

## 变更

ShapeLinkProvider 只借用实际需要的 definition symbol plan、callable ABI、layout／scan、descriptor、dispatch 和初始化／静态存储记录，不再要求整份 ConeProductionSectionV2。实际 subject 经 typed definition 查询后直接取得已保存的 symbol／primary atom；不再重新生成符号、扫描 atom 集合，或反查 type／callable 注册表重复比较布局、scan、继承与 dispatch slot。

缺失 subject、本地／外部引用、provider／target 和实际 callable ABI 选择继续处理。生产导出时的检查保留；未修改 wire、profile 或 runtime ABI。20 个代码／spec 文件新增 128 行、删除 331 行，净删 203 行；provider 主模块、合同选择与初始化支持分别为 99、119、37 行。删除的 types 模块为 177 行。

## 验收

本机先完成格式化与 workspace clippy，随后按脚本通过 17 项 shape-link、7 项 Link closure、release 构建和真实泛型产物。现有测试覆盖十类外部物理 subject；本批未删除测试，也未增加只镜像实现的测试。

Darwin CLI通过 16 项、18 variants、199 processes，覆盖跨 Cone 泛型与 ODR、普通链接、runtime image／初始化、继承、接口、装箱、属性更新、C 产物、M46 序列化、生成测试入口与稳定对象文件。所有输入源码、工具和原产物保持存在。

Linux 使用独立 `target/m48-summary-reader`，在本机验收完成后按脚本通过 fmt、workspace clippy、17 项 shape-link、7 项 Link closure、release 构建及真实泛型产物。GNU和 musl各通过同样 16 项、18 variants、199 processes；compiler 同步 dry-run 无差异。本批没有改动文件读取路径，未重复上一批 ProgramLink I/O 采样；未据此声称已有最终性能数据。

## 清理与剩余范围

确认没有运行进程引用后，清理上一批两个已完成且报告已归档的本机 CLI 工作目录，共释放 2,152,275,968 字节磁盘分配空间；记录。本批测试目录、release 工具和增量编译结果保留。

本批仅消除每次实际选择导出时的重复比对。Compile reader 仍先重放 strong production，再向 provider 传入其中三个必要字段；该重放与其余语义／布局重建仍待退役。ProgramLink identity／foundation 边界、writer 其余复核、dump 复用和最小阶段、最终当前代码全量回归及五轮性能复测仍属 M48 范围。未以本批定向结果代替完整验收，M38 未启动。
