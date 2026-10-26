# fmt／check 验证记录

日期：2026-10-26～27。本机 macOS/AArch64、LLVM 22.1.8；Linux 为独立工作目录中的 Ubuntu 26.04.1/x86_64、LLVM 22.1.2，分别使用 GCC 和 musl-gcc 验证 GNU／musl 目标。

## check

- `cargo fmt --all`、`cargo clippy --workspace --all-targets` 通过。
- protocol 全部 39 项测试、低层 check 与文档 HIR 回归、公共 check 参数解析、18 项图准备／并行调度测试通过。
- native 工具记录与对象更新的 5 项定向测试通过；fixture runner 的 Ruff 与公共规则测试通过。
- `tests/fixtures/driver/check/` 的 12 项 fixture（含 generator 错误变体）通过：库、可执行文件、单文件、默认输入、direct 输入、Production/Test 分离、生成测试入口、测试依赖隔离、Ignore 正文、普通／生成片段 warning、语法／名称／effect／具体化条件／递归诊断、依赖机器产物、native 头文件更新、generator 更新、同键并发及失败后修复。
- 受影响的 `doc-cli-basic`、`doc-cli-multiple`（含变体）通过。

未优化的完整 HIR Rust 测试使用 `RUST_MIN_STACK=33554432`；源码行为 fixture 使用 release。源码、配套工具、sysroot 和既有产物始终保留；通过实际调用观察和输入变化检查复用。

## 格式库

- workspace 格式化和 Clippy 通过；6 组定向测试覆盖格式输出、操作符／控制体、注释／字符串／CRLF、Unicode 标量行宽和敏感换行。
- 7,546 份可成功解析的仓库源码通过非空白 token、完整位置归一化 AST、文档注释目标和幂等性检查；有意损坏的 UTF-8 或语法输入不属于格式库成功输入。
- `cargo test -p scoop-parser -p scoop-analysis -p scoop-lsp` 全部通过，覆盖新增 CST 角色及其现有消费者。

## 格式化 CLI

- workspace 格式化／Clippy、27 项源码选择测试、公共 CLI 参数及文件写回测试通过；覆盖普通可执行权限、符号链接与第二次写回不改 mtime。有效非 UTF-8 文件名的写回用例在 Linux 验证，APFS 不接受该文件名。
- fixture runner 的 Ruff 和公共规则测试通过；8 项格式化 fixture、15 次进程调用通过，覆盖所有平台的声明并集、重复选择、默认与显式输入、空集合、只检查、语法／UTF-8 错误聚合、整个 root 选择失败、目录循环、实际写入失败及退出码 2。
- `cli-external-command`、`cli-run-process-contract` 共 21 次进程调用通过，覆盖命令结果分发、运行参数、退出码、信号及中断。

## 合并 main 前的 Linux 扩展验证

- workspace 格式化／Clippy 及 fixture runner 的 57 项公共规则测试通过；四个配套 release 命令和 PTY 测试工具构建成功。GNU／musl 各显式准备 25 个系统 Cone 及普通／stress／startup-stats runtime，普通 fixture 共享各目标的预编译资源和 Cone cache。
- `cargo test --workspace --no-fail-fast` 覆盖 6,117 项，首轮 6,097 通过、20 失败。失败涉及既有 ABI／布局／默认方法／优化结果的过期断言，以及一个依赖仓库临时目录的测试；修正后 codegen 全部 344 项、其他受影响的 16 项定向回归通过。该结果是全量运行加失败项闭合，不是重新执行一轮全量测试。
- 20 项新命令 fixture 在 GNU／musl 各全部通过：每个目标 21 个变体、60 次进程调用。GNU 额外复验修正后的 `access-indexing`，合计 21 项、22 个变体、62 次进程调用；musl 完整验收中的该用例也已通过。
- Linux 文件系统上的非 UTF-8 文件名写回测试通过。

两轮完整文件 fixture 在合并 main 时主动中断，避免新旧编译器和 ABI 资源混用。GNU 为 2,185 通过、63 失败、6 配置错误、33 不适用、1,998 中断；musl 为 2,016 通过、63 失败、36 不适用、2,170 中断。这些数字属于合并前的运行，不代表新 main 的结果。原始日志和工作目录保存在仓库外，不提交逐次报告。

## 合并前失败调查

对照使用任务基点 `4298bca4d` 的独立 checkout 与 release 编译器，复用相同 Linux 工具链和本次已准备的 sysroot。没有为对照重新准备整套基线 sysroot；下表区分直接复现与同诊断归组。

| 类别 | 对照结果 |
| --- | --- |
| ELF 符号缺失 | `add-extensions`、`annotations-scalar-artifacts`、`arrays-bounds-trap`、`file-gc` 和 `client-basic` 在基线复现相同缺失符号。其余具有相同符号和诊断的用例归为对应组，未逐项重复长时间编译。 |
| 运行异常 | `application` 的 DecodingException、`generic-delegate-siblings-consumer` 的 IllegalStateException 均在基线复现。 |
| IR 断言 | 两项 array-inlining 及 `counted-array-bounds` 均在基线复现；后者仍在 `rebound` 中包含期望已消除的 GC poll。 |
| MIR／产物快照 | 两项 devirtualization、`generic-machine-standalone` 和两项 generic-reader 在基线产生完全相同的快照差异；其余三项 generic-machine 具有相同成员列表差异。 |
| JSON 库测试超时 | musl 完整运行与三次定向复验均超过原 600 秒限制，包括固定性能核的复验；基线一次运行在 565 秒通过。延长观察的独立诊断运行在 658 秒完成真实编译及全部 99 项测试，生成程序与基线逐字节相同；原 fixture 正常复用后在 0.67 秒通过。两版 Rust 构建 profile、feature 与 flags 一致；冷编译的耗时差异尚未归因，原超时仍单独保留。 |

`access-indexing` 的过期 String 索引期望已修正并回归通过。另有 6 项 imported-class-primitive-inherited fixture 的旧 core 源码替换片段无法匹配；合并前尚未闭合。上表记录历史调查，不能据此判断合并后的编译器仍有相同问题。

## 合并 main 后的验证

- `59cedb4ea` 合并远端 main `86dbefa95`，包含 ELF dead constant users／unused declarations 清理和 runtime ABI 20；上游删除的旧整数 C runtime 测试保持删除。
- macOS workspace 格式化、Clippy 通过。格式库（含更新后的仓库源码 corpus）、protocol、scoop、slib、codegen 共覆盖 752 项测试；4 项 ABI 19 固定向量更新为 ABI 20 后，slib 全部 246 项及 compile-cache 向量复验通过，修改单独提交为 `28615e7fe`。
- Linux workspace 格式化、Clippy 通过；相同五个 package 的 771 项测试全部通过，包括 345 项 codegen 测试、ELF 优化后符号清理的实际链接／运行和非 UTF-8 文件名写回。
- 合并后的 macOS、Linux GNU／musl 各通过 20 项新命令 fixture、21 个变体、60 次进程调用。
- GNU／musl 的 `add-extensions`、`annotations-scalar-artifacts`、`arrays-bounds-trap`、`file-gc`、`client-basic` 均全部通过，每个目标 6 个变体、16 次进程调用；此前这五项的 ELF 缺失符号错误已闭合。
- 配套 release 命令已重建，三个目标的 ABI 20 sysroot 均已显式准备完成。musl 使用新命令及原 ELF 失败用例的定向覆盖，不重复启动第二轮完整验收。
- GNU 完整文件 fixture 按收尾要求主动中断：4,284 项中 501 通过、3 失败、33 不适用、3,747 中断；共执行 514 个变体、989 次进程调用。未完成全量验收，不把中断项计为通过。

## 合并后单独处理

按 2026-10-27 的收尾决定，以下三项在本批改动合并后分别修复。当前诊断均与合并前完整一致，合并前已用任务基点 `4298bca4d` 复现。

| fixture | 当前失败 |
| --- | --- |
| `application` | 运行时抛出 `DecodingException`，实际退出 1，期望退出 0。 |
| `array-inline-ranges` | `compiler-correctness/optimization/lowering/array-inlining/inspect.py:33` 的 IR 断言失败。 |
| `array-inline-ranges-roots` | GC poll 专测复用同一检查脚本，在相同 IR 断言失败。 |

分别用 `python3 tests/run_fixtures.py --filter '<fixture>'` 复现，沿用已准备的 GNU sysroot、配套 release 命令和共享 cache；正式断言和超时限制保持现状。原始日志与报告位于仓库外的 `scoop-fmt-check-work-20261026` 工作目录。
