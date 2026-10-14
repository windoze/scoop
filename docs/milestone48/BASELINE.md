# M48 Darwin 基线

日期：2026-10-15。工具实现基线为 M46 的 `6571bb4a`（其最后生产代码提交为 `1b53d060`）。使用 Apple M3 Ultra、本机 release 工具和 debug Scoop profile。测量源码与工具一直保留，生成的程序均实际运行并检查输出。

每项五组独立命令。冷指新 Scoop 缓存，不刷新 OS 文件缓存；各组的临时缓存与 target 在结束时清理。RSS 为 `ps` 同一轮全部存活后代之和的峰值，约每 20 ms 加查询耗时采样，不能代表未采到的瞬时峰值。

| 场景 | 阶段 | 时间中位数（范围，秒） | RSS 中位数（范围，MiB） | compiler build 次数 |
| --- | --- | --- | --- | --- |
| application | cold | 63.620 (63.405–64.903) | 4566.5 (4546.4–4584.4) | 6 |
| application | warm | 10.499 (10.358–10.560) | 4478.4 (4464.4–4527.8) | 0 |
| core | cold | 7.595 (7.464–7.854) | 1318.7 (1292.1–1346.4) | 1 |
| core | warm | 0.492 (0.448–0.498) | 176.2 (153.7–190.2) | 0 |
| empty | cold | 15.341 (15.215–15.386) | 1473.5 (1469.0–1517.4) | 2 |
| empty | warm | 3.069 (3.023–3.094) | 1428.1 (1382.0–1428.5) | 0 |
| native | cold | 15.682 (15.580–15.762) | 1537.9 (1507.3–1562.7) | 2 |
| native | header-change | 5.209 (5.151–5.295) | 1495.3 (1490.4–1502.0) | 1 |
| native | warm | 3.199 (3.167–3.223) | 1438.3 (1419.3–1457.0) | 0 |
| std | cold | 25.271 (24.775–25.537) | 2787.9 (2784.0–2802.7) | 2 |
| std | warm | 1.171 (1.159–1.177) | 355.0 (354.8–355.0) | 0 |

compiler build 次数来自正式 CLI 的实际 child invocation 记录，全部暖构建为零。原始 `sampled_processes` 可能漏掉短命工具，不能用它给出精确 native 编译／链接调用数；M48 最终结构验收仍须记录实际 native／linker 调用，不能用零 Scoop child 代替零重新链接。

证据：[统计与工具摘要](validation/darwin-baseline-summary.json)、[逐轮命令、构建结果与进程样本](validation/darwin-baseline-report.json.gz)、[原始 stdout/stderr](validation/darwin-baseline-logs.tar.gz)。测量脚本见 [tests/benchmarks/m48](../../tests/benchmarks/m48/README.md)。

M48-1 验证：Rust 格式化／workspace clippy、固定版本 Ruff format/check、fixture runner 公共规则测试 49 项及现有正式用例发现 4,091 项通过；本批未重复运行 M46 已完成的全量语言测试。此处数量是新增 M48 用例前的基线。

GNU 补充结构基线：`nuc12` 在原 `6571bb4a8` 工具上，以真实空 main 执行一组冷／暖构建，`strace` 捕获全部成功 `execve`，源码与工具保持存在。暖构建虽然 Scoop build child 为 0，仍运行 1 次 scoopc 能力查询、C 工具配置／能力探测、startup 编译和最终链接；`ld` 总计运行 2 次（含链接能力探测）。[精确调用统计](validation/gnu-baseline-empty-exec.json)，[冷构建 trace](validation/gnu-baseline-empty-cold.exec.gz)，[暖构建 trace](validation/gnu-baseline-empty-warm.exec.gz)。这是一组结构证据，不计作五轮性能测量或最终 M48 验收。
