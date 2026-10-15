# Compile 依赖直接读取已保存的定义数据

本批移除 Compile 闭包的机器对象解析、materialization 重放、final／provisional 对象复制，以及 Code／RuntimeImage 重建；读取已有 defined owners 和 ODR member ABI，typed 引用解析使用共享 identity graph。实际跨产物符号归属、ODR key／ABI 合并仍保留。读取 API 不再要求 C bridge toolchain profile。

原有 Compile／Link 摘要只由测试消费，已删除；生产与消费者共享 manifest 摘要。不存在为对象数量而扫描目录的路径。新增定义数据与读取模块为 54、102、58 行，按实际数据职责划分。

19 个原对象重放、materialization 或其专用测试 helper 文件退役；真实发布、I/O 失败、初始化、泛型、默认参数、re-export、native 和 GC 场景保留。不同优化级别的 ODR 测试直接合并已保存的 ABI，不再从对象证明中重算 body 指纹。类型位置和 ODR 两个 Rust 测试同时保留源码，相关迁移随本批提交。

## 本机验证

- `cargo fmt --all` 与 [workspace clippy](compile-definitions-darwin-clippy.log.gz) 通过。
- [568 项 slib](compile-definitions-darwin-slib.log.gz)、[正式产物发布](compile-definitions-darwin-formal.log.gz)、[不同优化级别 ODR](compile-definitions-darwin-odr.log.gz)、[属性初始化组合](compile-definitions-darwin-initialization.log.gz)、[机器 callable／默认参数](compile-definitions-darwin-callables.log.gz)、[链式 re-export](compile-definitions-darwin-reexports.log.gz)通过。属性初始化组合耗时 716 秒，未因长时间无输出而重启。
- [release 构建](compile-definitions-darwin-release.log.gz)通过。CLI [第一组 8 项](compile-definitions-darwin-cli.log.gz)共 9 variants、90 processes；GC leaf 选择名称修正后单独[补跑 1 项](compile-definitions-darwin-gc.log.gz)，共 2 variants、16 processes。合计 9 个不同用例、11 variants、106 processes。
- 最后删除重复摘要后，重新通过 slib、正式发布、clippy 和 release；只对受影响的 core artifact、泛型 siblings、GC leaf、M46 序列化与输入错误做 [5 项复验](compile-definitions-darwin-final.log.gz)，共 6 variants、50 processes。未重复耗时的属性大组合。
- 确认没有 cargo／rustc 进程后[清理 incremental 4,233.4 MiB](compile-definitions-darwin-cleanup.log.gz)。

## Linux 验证

使用 `target/m48-summary-reader`，没有覆盖历史 fixture 回归使用的 `target/release`。完整命令和选择见[脚本](compile-definitions-linux.py.gz)。[fmt](compile-definitions-linux-fmt.log.gz)、[workspace clippy](compile-definitions-linux-clippy.log.gz)、[569 项 slib](compile-definitions-linux-slib.log.gz)、[正式发布](compile-definitions-linux-formal.log.gz)、[不同优化级别 ODR](compile-definitions-linux-odr.log.gz)和 [release 构建](compile-definitions-linux-release.log.gz)通过。[GNU](compile-definitions-gnu-cli.log.gz) 和 [musl](compile-definitions-musl-cli.log.gz) 均通过 9 个 CLI 用例、11 variants、106 processes，覆盖正常 core／artifact 消费、泛型 siblings、真实 ODR 输入冲突、native、GC leaf、M46 序列化、稳定对象及真实输入错误。正常消费用例保留源码、工具和原始 `.slib`；输入错误与缺失对象补产按各自场景验证。首次同步漏掉同时含源码保留迁移的类型位置测试调用，Linux clippy 因旧参数数量失败；补齐该文件后通过，保留[首次输出](compile-definitions-linux-clippy-first.log.gz)。

## 范围与后续

本批删除 Compile 的对象重放，不代表整个 M48-3 完成。输入加载层仍会读完整归档，Compile 仍保留语义／布局重放，ProgramLink 的对象证明、注册数据重建和双份对象路径仍待退役。统一文件式按需读取、dump 最小阶段复用、历史测试清理、最终全量回归与五轮性能复测继续属于 M48；M38 未启动。
