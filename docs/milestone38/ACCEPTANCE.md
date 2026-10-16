# M38 验收记录

状态：已完成并验收，2026-10-16。实现基于 M48 最终提交 `f75704bac`，各功能与修复分别提交；过程及失败闭合情况见 [PROGRESS.md](PROGRESS.md)。

## 功能范围

新增 52 个正式 CLI fixture，每组包含独立功能、组合或跨 Cone 消费，以及对应的精确负例。正例按实际声明执行普通与 moving-GC 运行，不保存普通功能的完整 IR 快照。

| 功能 | 数量 | 代表用例与覆盖 |
| --- | ---: | --- |
| TextWriter／StringWriter | 3 | [独立](../../tests/fixtures/m38-text-writer/basic/fixture.toml)、[跨库组合](../../tests/fixtures/m38-text-writer/combined/fixture.toml)：Unicode、快照、异常、闭包及 GC |
| TypeInfo 普通数据 | 3 | [独立](../../tests/fixtures/m38-type-model/basic/fixture.toml)、[组合](../../tests/fixtures/m38-type-model/combined/fixture.toml)：typed 身份、惰性描述、注解值与独立引用种类 |
| typeInfo 自动查询 | 5 | [类型形状](../../tests/fixtures/m38-type-query/types/fixture.toml)、[局部类型](../../tests/fixtures/m38-type-query/local-types/fixture.toml)、[产物消费](../../tests/fixtures/m38-type-query/artifacts/fixture.toml)：完整类型、成员、构造关系、原声明 Option 事实及描述专有依赖 |
| annotationKey 查询 | 15 | [独立](../../tests/fixtures/m38-annotation-query/basic/fixture.toml)、[三 Cone](../../tests/fixtures/m38-annotation-query/artifacts/fixture.toml)：默认值、repeatable、alias／re-export、共享正文与访问诊断 |
| Inspectable／DebugPrinter | 20 | [手写观察](../../tests/fixtures/m38-debug/manual/fixture.toml)、[合成](../../tests/fixtures/m38-debug/derived/fixture.toml)、[产物组合](../../tests/fixtures/m38-debug/artifacts/fixture.toml)：普通可见性、惰性读取、截断、循环、基类投影和 moving GC |
| 独立 scoop.cli | 6 | [基本解析](../../tests/fixtures/m38-cli/basic/fixture.toml)、[数值](../../tests/fixtures/m38-cli/numbers/fixture.toml)、[稳定 Decoder](../../tests/fixtures/m38-cli/stable-decoder/fixture.toml)、[产物组合](../../tests/fixtures/m38-cli/artifacts/fixture.toml)：schema／help、argv、泛型默认值、codec 重试及 JSON／debug 组合 |

TextWriter、TypeInfo 数据、值视图、debug 和 CLI 库使用普通 Scoop 值、接口及闭包。编译器查询和 inspect 合成生成普通 typed HIR，复用已有身份、依赖与成员访问，不增加 runtime ABI、动态注册表或额外访问授权。

## 完整验证

| 范围 | 结果 |
| --- | --- |
| Darwin Rust | 全量 5,302 项通过，零忽略；后续 codegen 377 项通过，含新增 3 项，当前共覆盖 5,305 项。 |
| Darwin 正式 CLI | 4,141 项通过、18 项不适用；4,665 variants、16,463 processes、88 项 stage／plan golden。完整轮次 3,253.56 秒，8 项旧用例配置／期望按实际补验结果替换，无未闭合失败。 |
| Linux Rust | 全量 5,338 项一次通过；后续 codegen 392 项通过，含新增 3 项，当前共覆盖 5,341 项。零失败、零忽略，使用优化测试构建。 |
| Linux GNU 正式 CLI | 4,130 项通过、29 项不适用；4,650 variants、16,290 processes、70 项 stage／plan golden。 |
| Linux musl 正式 CLI | 4,126 项通过、33 项不适用；4,646 variants、16,256 processes、70 项 stage／plan golden。 |
| 格式、lint 与 runner | 两机 fmt／整仓 Clippy、各 50 项 runner 规则通过。 |

Linux 的完整轮次使用 `b9e80ce4` 已提交快照，后续补齐 canonical manifest 文件名修正及 `9aa27875e` 查询优化；LLVM 为 22.1.2，本机为 22.1.8。Linux Rust 使用 `cargo test --workspace --release --no-fail-fast`，两个 CLI 目标各 8 并发，独立工作目录同时执行完整 `--all`。CLI 与 Rust 子进程使用固定的同批工具副本，避免测试构建改写正在使用的配套命令。

Linux 完整轮次各选择 4,159 项，用时 GNU 11,038.32 秒、musl 11,004.99 秒；两轮均只有 10 项 M38 manifest 大小写失败。修正文件名后，九项直接补验通过，CLI 基本组合随后通过下述查询优化闭合。最终结果逐项合入同一批 315 项补验（GNU 1,813.92 秒、musl 1,839.24 秒），没有未执行、被忽略或未闭合失败项计入通过。

本机完整复验修复了一处生产问题：依赖补齐再次推进正文队列时，导入构造器现在复用已完成的正文，保持闭包捕获的 binding 身份。相关 HIR lowering 全组 1,465 项通过。其余修正均为测试：同步必要的 HIR 声明及格式向量、去除 core 内部实体总数的绑定，并更新 8 个 CLI 用例的源码准备或诊断期望；每个实际产物描述符的布局、引用与链接检查仍保留。

Linux 发现 CLI 基本组合的冷编译超过 300／600 秒；调用栈采样定位到 GC 指针检查反复全函数扫描。`9aa27875e` 改用实际 LLVM 使用链并复用 CFG／指令位置，保留 operand bundle、PHI 与循环重新定义检查，恢复默认超时。本机随后集中补验 M38 全组与 gc 标签：308 项通过、7 项不适用，389 variants、1,859 processes、3 项结构 golden，用时 368.66 秒；其中 CLI 基本组合冷编译及普通／moving-GC 运行共 35.01 秒。Linux 同批补验分别为 GNU 310 项通过、5 项不适用（393 variants、1,871 processes），musl 315 项全部通过（400 variants、1,915 processes）；各有 3 项结构 golden。CLI 基本组合分别用 52.82／56.82 秒通过，默认超时保持 300 秒。

HIR 产物格式已随新增查询及声明信息更新，旧 `.slib` 需要重建；runtime ABI 保持不变。精确版本和各批验证范围见实施记录。

原始日志、逐用例 JSON 和补验报告均在仓库外。文档只记录范围、结果及必要限制。本机完整轮次与查询优化补验分别清理约 176 GiB、16 GiB；Linux 两个完整轮次分别清理约 178 GiB、176 GiB，最后补验各清理约 16 GiB。共享编译缓存和近期 Cargo 缓存继续保留。
