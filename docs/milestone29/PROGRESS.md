# M29 实施记录

以 `a727ac93c` 为实现基线，在 `codex/m29` 逐功能提交。目标及完成门以
[设计](DESIGN.md) 和三份当前 spec 为准；只在实际通过验收后记录完成。

## 实施顺序

1. 普通编码/解码协议、标量 codec 和 JSON 库，建立显式 codec 的真实运行闭环。
2. 完整宿主限定和泛型 companion，贯通单态化、独立初始化及跨 Cone ODR。
3. 自定义 annotation、共有静态 shape 及产物读写。
4. record 派生、字段定制和正常 constructor/default 求值。
5. enum、tuple、核心容器及显式 generic decoder 组合。
6. final class、递归/异常/Context/GC 组合与正式总验收。

各功能先格式化、lint，再进行相应正式 CLI、negative 和 stage golden 验证。
完整验收包括 workspace tests、fixture runner 单测及全部文件 fixture，并使用
`nuc12:~/repos/scoop` 验证 Linux glibc/musl。

## 构建环境与目录

macOS 使用 `/opt/homebrew/opt/llvm@22`（22.1.8）；默认 PATH 的 LLVM 23.1
不用于 Scoop 后端。开发构建关闭增量缓存与调试信息，定期检查并清理构建中间文件。
`target/m28-darwin` 是已有 Git worktree，清理 Cargo 缓存时不删除该 checkout。

## 普通协议、标量 codec 与 JSON

已实现 `Encodable`、`Decodable<T>`、三种容器协议、带路径的编码/解码异常、
`UnitDecoder` 和普通函数值适配类 `DecodeFunction<T>`。Boolean、八种整数、
Char 和 String 使用普通成员方法编码，各自 companion 实现具体解码接口。
整数源码按类型拆分，每个文件不超过 131 行；原 fixture 的文件路径与源码替换同步调整。

`scoop.json` 是独立普通库，使用有序成员列表和 typed JSON 值树，完成对象、数组、
单值、整数精确范围、JSON 数字语法、转义/Unicode、重复 key、完整消费及路径检查。
新增实现文件不超过 124 行。JSON 与 core 均无新增 runtime C 入口或反射表。

独立 provider/JSON/consumer 验收发现并修复共有继承查询遗漏依赖 identity graph 的问题：
某个依赖的 `Decodable<Identifier>` 不一定被最后一个产物直接引用；共有查询现在按
typed ID 读取原已验证图。修复没有复制机器物化根、增加 wire 字段或修改 runtime ABI。

macOS 已实际通过标量和容器 fixture 的普通/移动 GC 运行、三项带精确位置和消息的
negative，以及删除源码后的 artifact-only link/run。四份 HIR/MIR/LIR golden 保留普通
interface、companion、函数值和构造调用。另通过 40 项共有 HIR 测试、38 项 fixture
runner 单测、workspace 格式化/lint 和三个 release CLI 构建。整数源码拆分相关的
35 项 core operations fixture 与 core builtin binding fixture 也已通过，共 134 个进程。

阶段性清理了旧 `target/m29-scalars` 工作目录和手工 core/JSON `.slib`，保留实际报告。
后续仍须实现完整宿主 companion、annotation/shape、自动派生与核心组合 codec；
本节不代表整个 M29-1 或 M29 已完成。Linux 验证及最终全量结果另行记录。
