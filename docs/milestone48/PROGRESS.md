# M48 实施进度

基线：`6571bb4a`，M46 已完成。按用户要求，本期完全实现、提交并验收后才推进 M38；此前 M38 TextWriter 批次已撤回。

## 1. 分批状态

| 批次 | 状态 | 当前证据与剩余工作 |
| --- | --- | --- |
| M48-1 合同、清理范围、release 基线 | 完成 | 已同步三份规范、AGENTS 与 runner 说明；复扫 1,628 份迁移候选并核对活动设计。Darwin 五组各五轮、native 另五轮头文件变化共 55 次测量完成，原始证据见 [基线](BASELINE.md)。精确 native／linker 调用计数作为最终结构验收补齐。 |
| M48-2 提前更新判定与普通构建记录 | 进行中 | 配套编译器的 metadata／能力记录已落地，本机验证通过（见下）；Preparer 输入镜像、产物收据与路径审计尚待删除。 |
| M48-3 按需读取与删除消费重放 | 未开始 | 尚未移除验证资格、对象 replay 和重复闭包。 |
| M48-4 对象／native／runtime／最终链接复用 | 未开始 | 现有暖程序构建仍重新链接、发布和制作运行副本。 |
| M48-5 全量测试迁移与最终验收 | 未开始 | 扫描清单须逐项记录迁移或删除，保留实际语言与运行覆盖。 |

## 2. 测量口径

[测量脚本](../../tests/benchmarks/m48/measure.py)使用 release 版 scoop/scoopc 和 debug Scoop profile。core、std、空 main、真实多 Cone 标准库／JSON 应用、C native／runtime 程序分别执行五组独立冷／暖命令；native 组再改变实际头文件执行增量构建。源码和工具全程保留，每组完成后清理该组缓存和 target。

冷缓存指新的 Scoop 缓存，不刷新 OS 文件缓存。每次保存真实命令、原始 stdout/stderr、构建结果及精确的 compiler build child 数；进程树 RSS 为 ps 抽样的同时存活后代总和。进程抽样可能漏掉短进程，不能把抽样工具列表当作精确 native／链接调用数。最终结构验收还须用实际调用观察证明这些工作已经消失。

## 3. 清理纪律

原始 [扫描清单](SOURCE-ISOLATION.tsv) 是迁移候选，不是删除语言测试的名单。后续每批保存旧路径、保留断言、新落点或删除原因；M48-5 重新扫描新增 case、内联描述、Rust helper 和活动设计。历史已完成记录保留当时事实，当前执行合同采用本期规范。

## 4. M48-2：配套编译器记录

首次使用按实际路径、mtime、大小及配套配置查询能力并保存普通 `tools-v1` 记录；连续构建直接复用记录，不再捕获整个 scoopc、读取程序内容计算摘要或在 child 执行前后重捕获。实际工具启动与协议错误仍传播；并发首次使用经普通文件锁共享一次探测，记录写后普通 rename，不读回或同步落盘。路径在记录中保存原始字节，语言身份与 runtime ABI 不变；compile key domain 更新为 v4。

本机 workspace fmt/clippy 通过，[scoop 单元测试 106 项](validation/compiler-cache-darwin-rust.log.gz)通过，fixture runner 公共规则 49 项通过，正式用例发现 4,093 项通过。新增[独立库用例](../../tests/fixtures/m48-compiler-cache/library/fixture.toml)与[native 组合](../../tests/fixtures/m48-compiler-cache/native/fixture.toml)均通过：冷编译 2 次、暖编译 0 次、仅更新实际工具 mtime 后 2 次、再次暖编译 0 次；native 每轮执行均输出 `native-c-ok`。工具与源码全程保留。[CLI 原始结果](validation/compiler-cache-darwin-fixtures.log.gz)，[lint 结果](validation/compiler-cache-darwin-clippy.log.gz)。Linux fmt/clippy 和 [scoop 单元测试 109 项](validation/compiler-cache-linux-rust.log.gz)通过（含非 UTF-8 工具路径）；[GNU](validation/compiler-cache-gnu-fixtures.log.gz)、[musl](validation/compiler-cache-musl-fixtures.log.gz)各 2 个正式 CLI 用例通过。非 UTF-8 文件名的实际工具运行用例只适用于 Linux 文件系统。

此子批只完成配套 scoopc 复用，当前仍存在源码镜像、缓存产物复核、native 预处理和最终重新链接，不能据此判定 M48-2 或 M48 已完成。已清理本机 `target/debug/incremental` 约 15 GiB，保留后续构建所需依赖与工具。

GNU 的空程序暖构建另经 `strace -f -e trace=execve,openat` 核对：没有启动 scoopc 能力查询或编译 child，也没有打开 scoopc 文件内容。对照原基线的 1 次能力查询，本批该调用已消失；native／最终链接调用仍存在，留在后续批次处理。[原始 trace](validation/compiler-cache-gnu-warm.trace.gz)。

## 5. M48-2：Scoop 源码读取

manifest／single-file 的实际 Scoop 源码只读取一次供 UTF-8 解码与 parser 输入使用，不再在读取前后比较 inode、ctime、mtime、长度与重新解析后的路径。删除无剩余调用的 `StableFileObservation`。源码发现允许普通 symlink 指向 Cone 外的文件／目录；选中路径类型、缺失输入、重复选择、逻辑路径、目录遍历循环及 UTF-8 错误仍正常报告。此处不代表 Preparer 已改为提前命中，也不代表其他 native／产物路径审计已清除。

本机 workspace fmt/clippy、[manifest 测试 56 项](validation/source-inputs-darwin-rust.log.gz)通过；[2 个新 CLI 用例](validation/source-inputs-darwin-fixtures.log.gz)验证显式外部文件、默认目录内 symlink、外部测试目录、冷／暖构建与真实源码变化；[M33 源码选择 33 个用例](validation/source-inputs-darwin-regression.log.gz)通过（34 variants、44 processes），保留缺失、循环、重复与语法／编码诊断。当前用例总数 4,094，较前一批减少 1 个退役的逃逸拒绝 case、增加 2 个实际编译用例。Linux fmt/clippy、[manifest 测试 56 项](validation/source-inputs-linux-rust.log.gz)通过；GNU 与 musl 各通过 [2 个新用例](validation/source-inputs-gnu-fixtures.log.gz)和 [33 个 M33 回归用例](validation/source-inputs-gnu-regression.log.gz)（[musl 新用例](validation/source-inputs-musl-fixtures.log.gz)、[musl 回归](validation/source-inputs-musl-regression.log.gz)）。
