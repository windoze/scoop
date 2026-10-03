# Core 运算用例迁移记录

本批使用统一的 `tests/run_fixtures.py` 和 schema 1，原 `.scoop` 均保留，
不把源码或断言转移到 Rust 内联注册。M23-11 的全仓验收尚未完成。

## 单文件组合

`m23-cli-core-operations` 通过公共 `concat(path,parts)` 将原文件与原显式换行
组成同一个 single-file 输入，保留 `scoop:single-file` 身份、源码位置及声明顺序。
core 包含原 driver 用例的六个扩展文件与两个 reference source-fields 文件，
由真实源码生成 `.slib`，随后删除 core 和消费者源码。独立复制的 `scoop` 在
没有 `scoopc` 与 LLVM 工具的 PATH 下从产物重新链接，比较 link plan 指纹，
并执行普通与强制移动 GC 两次运行。core 与根产物均固定实际 artifact 指纹。

| 功能 | 原输入 | 用例／进程／golden | 与原断言对照 |
| --- | --- | --- | --- |
| 成员调用组合 | `m23-imported-core-members/methods.scoop`、`combined.scoop` | 1／5／5 | 原 HIR export、完整 MIR/LIR 逐字相同；新 HIR 同时锁定 LocalConcrete |
| 整数相等与模式组合 | `m23-imported-core-members/equality.scoop`、`equality-patterns.scoop`、`equality-combined.scoop` | 1／5／5 | 原 HIR export、完整 MIR/LIR 逐字相同；新 HIR 同时锁定 LocalConcrete |

公共文件动作的 29 项单元测试通过，包含原始 UTF-8／二进制字节、显式分隔符、
目标作为输入、后续 patch、缺失输入不覆盖目标，以及 schema 与步骤引用校验。

## 单文件请求边界

组合迁移时核对了 DESIGN §3.1 的 single-file 规则：`scoop build/run <file>`
现在在请求配置边界拒绝 `--cone-path`，并明确区分 `scoop link` 的正常产物查找。
新增 `m23-cli-inputs/single-file-cone-path-{build,run}`，在缺失 compiler/sysroot 时
仍先报告完整的 `SCOOP_CLI_CONFIG`，不建立 cache/target。组合用例从普通 sysroot
artifact slot 取得已经编译的 core，不传额外 Cone locator。完整输入 suite
21 项／72 进程／8 份 golden 通过；两项 single-file 组合的产物和阶段期望均未变化。
