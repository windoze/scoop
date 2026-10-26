# 日志库实施记录

状态：已完成（2026-10-26）。分支：`codex/log-library`。依据：[设计](DESIGN.md)、[公开规范](../specs/SCOOP-LOG-SPEC.md)。

## 交付

- `scoop.log` 提供级别、事件、provider 协议、Logger、静默默认实现和原子配置。一次调用保留同一 provider，先过滤，再执行消息闭包一次并提交。
- 消息闭包异常成为同级别的替代事件，保留原位置、业务 cause 和闭包异常对象；provider 失败独立执行固定格式 stderr fallback。fallback 的构造和写入再次失败时直接放弃，不重试、不递归、不向业务传播。
- `scoop.log.console` 提供同步标准流输出、最低级别、可替换 Clock 和格式函数。同一标准流的多个实例共享锁，覆盖完整写入；用户格式函数在锁外执行。
- 两个普通 Cone 均有 API 注释、README 和可运行示例。现有准备、发行收集和文档脚本按 manifest 自动发现它们，无需新增注册表或专用构建入口。

## 验证

普通行为与组合测试使用库自身的 `scoop.test`。单个 `log-output` fixture 仅承担真实输出字节、关闭描述符、断管和并发完整记录等必须由外部进程控制的验证；它复用预编译库和共享缓存，只构建一个消费者程序。

| 目标 | facade 测试 | console 测试 | 标准流 fixture |
| --- | --- | --- | --- |
| aarch64-apple-darwin | 10 / 10 | 4 / 4 | 1 / 1 |
| x86_64-unknown-linux-gnu | 10 / 10 | 4 / 4 | 1 / 1 |
| x86_64-unknown-linux-musl | 10 / 10 | 4 / 4 | 1 / 1 |

所有普通测试使用 release。覆盖五个级别、急切与惰性求值、只执行一次、原异常身份、业务 catch/finally 控制流、调用处位置及跨 Cone 转发、配置更换、并发发布、固定时钟与自定义格式。输出 fixture 覆盖查询／提交／刷新失败、闭包与 provider 连续失败、部分输出后失败、时钟／格式失败、Unicode／NUL／换行，以及多个 provider 的大记录写入。stderr 关闭或断管时，fallback 失败后仍执行后续业务并返回指定退出码。

Darwin 另完成两个示例的实际运行，以及两库 `scoop doc --no-deps --deny-doc-warnings`；生成站点的本地资源链接均可解析。Rust 格式化和 workspace Clippy、Ruff 格式化和检查通过；fixture runner 公共规则测试 55 / 55 通过。

Linux 使用 `nuc12:~/repos/scoop-log-library` 独立目录，不修改原有 `~/repos/scoop`。仅执行上述日志定向测试，不运行 Linux 完整 Rust 或 fixture 套件。musl 按准备脚本已有说明补充 Linux UAPI 头搜索目录，这是宿主构建配置。

本轮不做完整发行归档、无关全量回归或 GC stress。原始日志、构建缓存和测试工作目录均保存在仓库外：Darwin 为仓库旁的 `scoop-work/log-library`，Linux 为 `~/repos/scoop-log-work` 及同级日志文件。

## 问题闭合

未发现需要修改语言、编译器、runtime 或平台 API 的能力缺口，日志实现不包含底层绕行。测试过程中修正了 fixture runner 测试辅助代码对 macOS `/var` 与 `/private/var` 的路径表示差异：准备测试目录时统一 resolve，与 runner 的现有路径合同一致，随后全部 55 项公共规则测试通过。此修复独立提交为 `1d614d7ad`。

首轮日志测试发现的 override 可见性、Option 比较和 f-string 插值写法问题已按现有语言规则修正，并通过上述重验；不为库增加语言特例。
