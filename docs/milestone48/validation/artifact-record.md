# 依赖摘要记录验收

显式 artifact、搜索目录和默认 core 产物共用 `artifact-summaries-v1`。更新键仅包含实际路径、mtime、大小和 target／backend 配置；命中直接读取摘要记录，不打开 `.slib`。首次读取使用已有 manifest 最小 reader，普通锁与原子发布协调并发；真实输入缺失或目标不匹配仍失败。

新增 `m48-artifact-summary` 保留源码、工具和原产物，覆盖首次构建、连续命中、touch 后摘要更新、真实依赖从 48 改为 49 及运行结果。记录 metadata 断言验证命中未重写记录。Rust 两项单测另外覆盖负时间戳、目标变化及缺失输入不发布。

- Darwin：[workspace clippy](artifact-record-clippy-final.log.gz)、[87 项 scoop 测试](artifact-record-scoop.log.gz)、[release 构建](artifact-record-release.log.gz)、[Ruff 格式](artifact-record-ruff-format.log.gz)与[lint](artifact-record-ruff-check.log.gz)、[49 项 runner 测试](artifact-record-runner.log.gz)、[4,106 项完整预检](artifact-record-preflight.log.gz)通过。
- Darwin CLI：[首轮](artifact-record-darwin-cli.log.gz)通过 20 项；新增 fixture 的文本替换同时匹配包名，改为精确的 `= 48` 后[定向复验](artifact-record-darwin-final.log.gz)通过。最终[不同用例汇总](artifact-record-darwin-results.json.gz)为 21 项、21 variants、78 processes；首轮配置错误未计为通过。
- Linux：[fmt](artifact-record-linux-fmt.log.gz)、[workspace clippy](artifact-record-linux-clippy.log.gz)、[摘要记录 2 项](artifact-record-linux-tests.log.gz)、[locator 15 项](artifact-record-linux-locator.log.gz)、[core 17 项](artifact-record-linux-core.log.gz)和 [release 构建](artifact-record-linux-release.log.gz)通过。
- [GNU](artifact-record-gnu-cli.log.gz)和 [musl](artifact-record-musl-cli.log.gz)各通过 21 项、21 variants、78 processes；覆盖源码与 prebuilt 选择、真实输入错误、独立编译及 M46 序列化。

源码、工具、原产物与输出始终存在。[GNU 暖构建 trace](artifact-record-gnu-warm.trace.gz)中，41,595,590 字节的 core 产物打开 0 次、读取 0 字节，compiler child 为 0；摘要记录数量为 1 且 metadata 不变。[摘要和命令](artifact-record-gnu-warm.json.gz)、[stderr](artifact-record-gnu-warm.stderr.gz)、[采样脚本](artifact-record-trace.py.gz)及[跨平台 CLI 脚本](artifact-record-linux.py.gz)保留。采样前先按相同配置正常构建一次，不把首次生成记录当作暖命中。

Linux 使用独立 `target/m48-summary-reader/release`，没有改写正在运行历史迁移回归的 `target/release` 工具。本批只完成依赖发现的摘要复用；Compile／Link 的按需 section 共享、语义与对象 replay 退役、dump 最小阶段、其余历史清理和最终性能／全量验收仍未完成。
