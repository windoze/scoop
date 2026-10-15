# 精简 production 与 Link identity 的持久字段

本批基于 `5e81e275`，删除前两批 reader 重放退役后已无消费用途的数据。M48 的其他语义／布局重放、dump、历史迁移及最终全量／性能验收仍待完成。

## 格式与实现

`single-cone-production/8` 只保存 ODR 目录和 runtime image records；删除 distribution、output、image owner、typed registration、优化模式副本、重复的 RuntimeImage／Code、native contract／library／C++ 与 C bridge 投影。顶层 manifest 已保存所需指纹，native 需求来自 LIR foundation，入口来自实际语义与 production 数据。删除旧 C bridge 资格包装和 production 投影重建比较 API。

`link-identity-closure/16` 只保存 Scoop member 到 typed definition 的分配、generated-C member id、defined symbol owners 和 undefined symbol uses；不再生成或保存已无消费用途的 generated-C unit 列表、消费者重放专用的 atom 范围、patch sites、对象目录副本及 image／entry owner 投影。生产端 Code 输入及指纹公式、runtime ABI 保持不变。旧 section 按格式版本失效，没有兼容或重放分支。

本批代码与规范共净删 1,276 行；Link identity 主文件从 460 行缩到 162 行，wire 从 418 行缩到 145 行，production wire 从 412 行缩到 52 行。测试变更清单列出名称变化：3 项专测随被删除字段／API 退役，3 项迁移或更名后保留。真实 Code 生产、ODR、native C/C++、GC 和运行结果覆盖继续保留。

profile descriptor 的固定向量按两个 major 版本变化更新，指纹记录使用独立的 Python SHA-256 计算。首次本机测试发现 compatibility 测试还有一处旧 profile 指纹期望，修正后重新执行 fmt、workspace clippy 与相关测试。

优化模式副本与 generated-C unit 列表也已随调用方审计删除。下列结果均来自最终格式；中间版本的 Linux CLI 已停止，不计入验收。

## 本机验证

workspace clippy、542 项 slib 与 26 项 protocol、release 构建和实际泛型机器产物均通过。

17 项 CLI、报告通过：22 variants、245 processes，覆盖独立链接、依赖图与输入错误、稳定对象缺失补产、泛型 ODR 合并与冲突、runtime images、argv、M46 序列化、C/C++、GC 和 Test 用途。源码、工具与原始 `.slib` 保持存在；缺失对象输出的用例仍验证正常补产。CLI 使用当前工作区已迁移的 fixture 描述，历史迁移整体尚在独立验收。

本机 cargo 工作结束后清理 3,704.5 MiB incremental。补完后的本机验证结束后又清理 2,254.9 MiB incremental。Linux 验证使用独立的 `target/m48-summary-reader`，保留历史迁移使用的旧 `target/release`；命令见验证脚本，I/O 采样见脚本。

## Linux 与实际读取

Linux 最终版本 fmt、workspace clippy、release、543 项 slib 与 26 项 protocol和实际泛型产物通过。GNU 报告为 17 项通过、22 variants、245 processes；musl 报告为 16 项通过、native C++ 1 项声明不适用，20 variants、231 processes，无失败。

GNU 显式链接的 strace与摘要确认三个产物各打开一次，每份只读取 manifest 与三个 metadata 成员，无重叠读取；4,458 个 ELF 成员的读取量为零。实际 scoopc 调用为零，程序输出 `42\n`。本例最大归档为 40,013,200 字节，实际读取 27,898,787 字节；上一批同一用例分别为 42,412,404 与 30,297,991 字节，减少 2,399,204 字节。这是本例的产物体积与读取量记录，不替代最终五轮性能验收；语义／布局重放尚未全部退役。
