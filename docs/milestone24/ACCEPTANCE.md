# M24 实施与验收记录

状态：实现中，尚未完成 M24。本文只记录已执行的检查，不把设计、内部测试或版本迁移视为完整语言交付。

## 已落实的基础

- 按 DESIGN 第 4.3 节迁移共同 profile、section、outer schema、language/runtime/composite ABI。
- 所有 callable body 改用 `scoop-callable-body-v2`；新增 tag 5 `ReleaseHook(exact owner)`，保留既有 RuntimeEncode ABI。
- C/LLVM TypeDescriptor 固定部分为 152 bytes，release entry 位于 144，related types 从 152 开始；当前源码编译路径仍只发射 null entry。
- runtime 的 small/large reclaim 仅对逻辑死亡对象清除 ready 并同步调用 hook；搬迁旧副本不调用。清位先于回调，回调先于 poison、元数据退役与块回收。
- runtime 加载阶段从 exact type registration 派生 hook body ID，复用 callable map 与已检查的 executable entry。
- nominal parser 按成员、构造和修饰符拆分；主文件从 1,177 行降到 565 行。此提交未改变语法行为。

## 已执行检查

验证使用本机 LLVM 22.1；为缩短大型产物测试时间，Rust 开发 profile 设置 `OPT_LEVEL=1`、`DEBUG=0`、`CARGO_INCREMENTAL=0`，构建放在 `target/m24-cli`。这些设置只影响编译器工具自身，不改变 fixture 中 Scoop 程序的编译选项。

- Rust 格式化与全 workspace、all-targets clippy 已通过。
- 完整 Rust workspace 回归已执行；两处旧固定输出迁移后，对应泛型产物和外来 boxing 测试单独复核通过。
- codegen 原有 310 项测试通过，包括 C/LLVM 布局、image registry 和 moving collector harness。
- 新增 release collector 测试通过：small/large × ordinary/stress，覆盖 hook/ready 四种组合、clear-before-call、存活搬迁、未发布对象、显式 inert、惰性取得资源、pin、循环引用、多次 GC 及 runtime 返回后不补调。
- image type harness 增加非空 hook 的正向登记与缺失 body、错误 exact owner、错误 entry、非 FixedObject、不可执行 entry 的 negative 检查，已通过。
- parser 432 项测试和 1 项文档测试通过。
- fixture runner 的 29 项 Python 单元测试通过。
- 无更新模式的正式 `--all` 已覆盖全部 2,067 项既有 fixture：1,997 项通过，70 项停在摘要基线差异。差异仅涉及 ABI 迁移和临时目录迁入仓库后 BSD ar 保存的 group id，归档成员 payload 未改变。同步基线后，70 项均已分批用原 runner 无更新模式复核通过（首轮 68 项，随后产物图和归档冲突各 1 项）。这不是一次全量命令的零退出结果；M24 完整交付前仍需对最终工具执行正式全量验收。
- 临时工具副本、测试工作目录和日志统一使用仓库根目录下已忽略的 `tmp/m24/`。已清理旧 fixture 工作目录，并用 `cargo clean --profile dev --target-dir target` 清理默认开发构建，释放约 24.5 GiB。

## 尚未完成

源码 `release` 的 AST/HIR、受限 MIR/LIR、构造成功边 publish、机器 hook 发射、共有模板与参数自由/generic 依赖、ODR 和全部 M24 正负/组合 fixture，均须继续完成。最终以 DESIGN 第 6 节的源码、产物消费、独立链接和运行闭环验收为准。

## 第二批：ReleaseValue 与 NoTransition

- 已实现基于实际表示和 compiler protocol 的 ReleaseValue 条件，以及 NoGc callable 的定义侧 NoTransition 推导。递归调用按 SCC 传播，泛型条件保存 typed binder 引用；跨 Cone 消费导出的摘要，普通 NoGc 行为不变。
- 共有 callable effect、binder 引用检查和 HIR dump 同步更新。旧 fixture 的摘要、golden 和 corruption 向量已随格式更新；corruption 用例保持原来的损坏位置和诊断目标。
- 格式化与全 workspace、all-targets clippy 通过。HIR 的 869 项测试通过；HIR lowering 的 1,302 项测试中，1 项旧 dump 期望更新后单独复核通过。新增的 9 项 release effect 测试通过。
- 新增独立、组合和跨 Cone 三项正式 CLI fixture，锁定 HIR/MIR/LIR 输出；跨 Cone 用例在删除源码后仍完成产物消费、链接并运行返回 42。
- 使用这一批源码构建并保存于 `tmp/m24/effects-tools/` 的三个命令，执行原 fixture runner 的无更新模式 `--all`，退出码为 0：发现并选择全部 2,070 项，全部通过，覆盖 2,160 个变体、11,612 次进程执行及 11,024 份 stage/plan golden。
- 完整报告位于 `tmp/m24/effects-all-acceptance/report.json`，命令日志位于 `tmp/m24/logs/effects-all-acceptance.log`。这次全量结果仅对应本批工具，不代表后续 release hook 编译路径已经完成。
