# M24 实施与验收记录

状态：实现中，尚未完成 M24。本文只记录已执行的检查，不把设计、内部测试或版本迁移视为完整语言交付。

## 已落实的基础

- 按 DESIGN 第 4.3 节迁移共同 profile、section、outer schema、language/runtime/composite ABI。
- 所有 callable body 改用 `scoop-callable-body-v2`；新增 tag 5 `ReleaseHook(exact owner)`，保留既有 RuntimeEncode ABI。
- C/LLVM TypeDescriptor 固定部分为 152 bytes，release entry 位于 144，related types 从 152 开始；第一批仅发射 null entry，第三批已接通源码 hook。
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

其余 GC 根组合、缓存检查、最终格式基线和全量正式 CLI 回归仍须完成。已经完成的源码和依赖闭环见第三至六批；最终以 DESIGN 第 6 节的源码、产物消费、独立链接和运行闭环验收为准。

## 第二批：ReleaseValue 与 NoTransition

- 已实现基于实际表示和 compiler protocol 的 ReleaseValue 条件，以及 NoGc callable 的定义侧 NoTransition 推导。递归调用按 SCC 传播，泛型条件保存 typed binder 引用；跨 Cone 消费导出的摘要，普通 NoGc 行为不变。
- 共有 callable effect、binder 引用检查和 HIR dump 同步更新。旧 fixture 的摘要、golden 和 corruption 向量已随格式更新；corruption 用例保持原来的损坏位置和诊断目标。
- 格式化与全 workspace、all-targets clippy 通过。HIR 的 869 项测试通过；HIR lowering 的 1,302 项测试中，1 项旧 dump 期望更新后单独复核通过。新增的 9 项 release effect 测试通过。
- 新增独立、组合和跨 Cone 三项正式 CLI fixture，锁定 HIR/MIR/LIR 输出；跨 Cone 用例在删除源码后仍完成产物消费、链接并运行返回 42。
- 使用这一批源码构建并保存于 `tmp/m24/effects-tools/` 的三个命令，执行原 fixture runner 的无更新模式 `--all`，退出码为 0：发现并选择全部 2,070 项，全部通过，覆盖 2,160 个变体、11,612 次进程执行及 11,024 份 stage/plan golden。
- 完整报告位于 `tmp/m24/effects-all-acceptance/report.json`，命令日志位于 `tmp/m24/logs/effects-all-acceptance.log`。这次全量结果仅对应本批工具，不代表后续 release hook 编译路径已经完成。

## 第三批：源码 hook 与参数自由依赖

- `release` 从独立 AST 成员进入 Export/LocalConcrete HIR、MIR 和 LIR 的独立 typed arena。字段引用保留实际 owner 与字段身份；机器入口使用 AS0 私有参数，既有 CFG 和值操作共用原 lowering。
- 最外层 class 构造的正常出口在 initializer 返回后发布 ready；委托与失败出口不发布。LIR/codegen 接通无 transition 的 C leaf、原 NoGc helper，以及非 TLS native global 的纯存储 bridge。受限 MIR/LIR 输出检查覆盖错误 owner/字段、过早发布、managed 值、接收者逃逸和异常边。
- 共有 HIR nominal 声明保存 release policy 和必要 binder 条件；共有 MIR exact class 保存同一 owner 的 policy，LIR TD 投影直接消费它。参数自由 hook 由 provider 提供 Strong 定义，TD 的外部 relocation 保留精确 body symbol。
- 新增 4 个运行 fixture，锁定 21 份 AST/HIR/MIR/LIR 快照。它们覆盖直接 C leaf、native global/helper、显式与重复 close、惰性取得资源、stored 自定义 getter、Unit/空 block、base/this 构造、initializer 中 GC 和失败构造；所有运行同时使用普通 GC 与 moving stress。
- A→B→C fixture 删除 core/provider/middle/consumer 源码后完成下游消费和纯产物独立链接。provider 的私有 helper 随其原 Strong hook 发射，consumer 仅通过 middle 消费 Owner；普通与压力运行均得到 42。
- 新增 63 个独立 negative fixture，精确断言 canonical source/span、code、message 和 notes，覆盖语法、owner、字段/receiver、控制流、实际 helper/default/TLS 效果、类型用途条件，以及 handle/pin/function-pointer/callback 的直接、Option、aggregate 和 pointer 包装。
- `cargo fmt --all`、全 workspace/all-targets clippy 通过；完整 `cargo test --workspace --no-fail-fast` 执行了 5,238 项测试，其中 5 处旧字段数量/摘要断言迁移后，对相关 3 个 crate 的 2,379 项测试复核全部通过。原全量日志为 `tmp/m24/logs/hooks-workspace-tests.log`，复核日志为 `tmp/m24/logs/hooks-workspace-recheck.log`；原全量命令保留非零退出事实。
- Python runner 的 29 项单元测试通过。使用 `tmp/m24/hooks-tools/` 的配套命令执行无更新模式 `--filter 'release-blocks-*'`，67/67 通过，覆盖 89 次进程执行及 21 份 stage golden；报告为 `tmp/m24/hooks-source-acceptance/report.json`。
- 再次用 `cargo clean --profile dev --target-dir target` 清理默认开发产物，释放约 2.5 GiB。所有工具副本、测试目录和日志继续位于已忽略的仓库 `tmp/m24/`。

## 第四批：泛型模板与外来 C leaf

- 泛型 nominal initialization 的共有格式显式保存 release policy、定义来源和完整正文。下游按原 nominal binder 替换并实例化独立 hook；共享表达式遍历、来源和调用引用包含 hook，provider 无须预先物化 consumer 才使用的 exact owner。
- 依赖选择保留 C extern 的原 source native contract。release 内直接使用其 native leaf，普通调用仍使用原 provider Scoop 入口；同一声明同时用于这两种调用也保持各自 ABI。共有 HIR 的 `NativeLeaf` 分支明确连接该调用，避免为它保留未使用的 Scoop wrapper。
- 新增本地泛型组合，覆盖标量、值聚合、Option、Unit、nested owner、phantom 引用参数和循环；新增跨 Cone 签名、别名、字段及构造四项条件失败 fixture，精确断言 canonical 诊断。
- 新增 provider、left、right、consumer 组合：两个库和下游共同实例化 `Owner<Int>`，另有独立 `Owner<Long>`；不同库使用不同 method，release 还调用私有普通与泛型 NoGc helper。删除所有源码后，独立 artifact-only link 与普通/moving stress 运行均输出 `3`、`69`。
- 格式化和全 workspace/all-targets clippy 通过。六个相关 crate 共执行 3,382 项测试，其中一处旧未知 tag 断言迁移后，HIR 的全部 873 项复核通过；日志为 `tmp/m24/logs/generic-hooks-regression.log` 与 `tmp/m24/logs/generic-hooks-hir-recheck.log`，首轮非零退出事实保留。
- 使用 `tmp/m24/generic-hooks-tools/` 的三个固定命令运行原 runner，无更新模式 `--filter 'release-*'` 退出码为 0：76/76 通过，覆盖 128 次进程执行、50 份 stage/plan golden；报告为 `tmp/m24/generic-hooks-acceptance/report.json`。最终全量验收仍须使用最终版本的工具重新执行。

## 第五批：聚合 native ABI 与值 helper 组合

- 新增 24-byte C struct 的参数与返回值 fixture，release 字段复制后经已有纯 storage bridge 传入 C。同一 C 声明在普通 caller 和 hook 中分别保持原 transition 协议与 native leaf 协议。
- 跨 Cone 泛型 owner 保存三份聚合字段，覆盖大对象回收；release 同时调用外来私有泛型值类型的次构造函数和 method、泛型 NoGc helper，以及外来值 method。`Owner<Int>` 与 `Owner<Unit>` 的实际字段布局和 C 结果均正确。
- 独立及跨 Cone 两个用例均通过普通和 moving stress 运行；跨 Cone 用例删除源码后重新 artifact-only link，输出仍为 42。原 runner 无更新模式 `--filter 'release-blocks-native-aggregate-*'` 为 2/2，通过 15 次进程执行、10 份 stage golden；报告为 `tmp/m24/aggregate-acceptance/report.json`。
- 本批未改变编译器实现。格式化与全 workspace/all-targets clippy 通过；继续使用第四批固定工具。此前分别清理 `target/m24-cli` 和默认 `target` 的开发产物，释放约 2.9 GiB、2.2 GiB。

## 第六批：实际机器属性与 ODR 冲突

- driver 测试复用四份正式 fixture 源码，经完整 HIR/MIR/LIR 和 LLVM 生成路径检查每一个 hook：入口为 raw pointer、返回 void、nounwind、地址有意义；函数正文无 AS1/addrspacecast、statepoint/relocate、GC strategy、异常边或 runtime transition。普通函数的外部声明不被错误当作 hook 正文。
- 同一测试发射实际对象，确认 hook 所在物理 member 不含 LSDA 或 stackmap section；普通函数的 EH/CFI 仍由既有对象 verifier 负责。
- 复用实际双 consumer ODR 测试：两个独立构建使用相同 provider identity 和泛型 owner，但 hook 正文不同。ABI、group 和 member identity 不变，ReleaseHook 与 registration 的定义摘要发生变化，原 ODR 合并路径明确拒绝冲突。
- 格式化和全 workspace/all-targets clippy 通过；`cargo test -p scoopc release_hooks` 的两项端到端测试均通过，日志为 `tmp/m24/logs/machine-tests.log`。没有增加生产侧验证管线或新的来源机制。
