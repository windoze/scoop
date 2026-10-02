# M23-11 fixture 迁移记录

本记录按已迁移批次更新。历史单文件覆盖与剩余 driver／stage／program-link 文件测试分别计数，不能用 Rust workspace 总通过数代替 Python 总验收。

## 1. 历史端到端用例

原 `compiler/driver/tests/fixtures.rs` 已在 `c02bd245c` 退役，留下的合并快照此前不再被执行。此次从原已接受快照和 runner 的 moving 清单恢复全部条件，统一调用正式 `scoop build`，从同一次 root 编译取得四个 stage dump，再执行同次产物。

| 原范围 | 源码数 | 当前覆盖 |
| --- | ---: | --- |
| M1–M25 合并 fixture | 480 | 186 个编译成功（含 trap）、294 个 negative |
| M0 smoke | 1 | 正常运行，精确检查原始 stdout |
| M23 source headers | 4 | 1 个正常运行、3 个 negative |
| 合计 | 485 | 178 个正常运行、10 个 SIGABRT、297 个 negative |

逐源码对应见 [LEGACY-COVERAGE.md](LEGACY-COVERAGE.md)。467 份新增声明位于 `tests/fixtures/legacy-acceptance/`；18 份源码复用先前修复中已经建立的 CLI 条件，其中重复声明错误等在同一用例中有独立命名步骤。发现和覆盖检查均来自 TOML 与源码扫描，Python 中没有历史 case 注册表。

新增的 467 个用例在只读验收中全部通过，共 497 个变体、737 次进程执行和 824 次 stage golden 比较。485 份旧合并快照已删除，原源码全部保留。完整声明中保留 649 条 error、2 条 note 和 2 条 warning。

原 35 份 moving-GC 用例全部保留。native companion 的每份 C 源、逻辑库名、SDK、target、deployment、编译和归档参数都在对应 TOML 中明确给出；CLI 仅消费 `--library-path` 下满足真实 typed requirement 的对象／归档。普通运行与 moving 运行使用独立工作目录并比较相同输出。

### 1.1 快照结构

旧合并 `.scoop.snap` 中拼接的 core AST 不再是 single-file 编译输入。当前 AST 只包含原单文件；HIR 分别记录 Export 与 LocalConcrete，MIR／LIR 使用当前完整 typed identity 和独立 core 依赖。四阶段期望分别保存，既不删除类型身份，也不按用例清理差异。运行／诊断期望与可更新的 stage golden 分开，更新 golden 不会接受新的失败类别。

negative 由完整 JSON 数组检查 severity、code、message、canonical cone/source/span 和每条 note；warning 同样精确检查。原以行列呈现的位置对应当前规范的 byte span，显示路径不参与 canonical 期望。两份原 warning（safe-call 的 `inner`、enum 模式的 `Raedy`）保留原消息和位置。

### 1.2 已接受的历史差异

以下差异均逐项对照原快照审核。263 个 negative 的消息、主位置和 note 与旧期望相同；其余 34 个保留原拒绝规则，并按当前共有前端与入口协议记录具体变化。

| 原源码 | 原因与保留的断言 |
| --- | --- |
| [m1-hello/errors/no-main.scoop](../../../tests/fixtures/m1-hello/errors/no-main.scoop) | 正式 executable 入口检查要求唯一 ordinary main(): Unit。 |
| [m1-hello/errors/print-arity.scoop](../../../tests/fixtures/m1-hello/errors/print-arity.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m10-coroutines/errors/suspend-main.scoop](../../../tests/fixtures/m10-coroutines/errors/suspend-main.scoop) | 入口错误附独立 source note，说明 main 为 suspend。 |
| [m10-coroutines/suspend-variance-bridge.scoop](../../../tests/fixtures/m10-coroutines/suspend-variance-bridge.scoop) | 保留原不合法赋值，后续未绑定变量诊断归入 core 重载候选。 |
| [m12-ffi/errors/no-gc-operations.scoop](../../../tests/fixtures/m12-ffi/errors/no-gc-operations.scoop) | 真实依赖 receiver 临时值和 managed 依赖调用沿同一 NoGC 检查。 |
| [m12-ffi/errors/sizeof-requires-type.scoop](../../../tests/fixtures/m12-ffi/errors/sizeof-requires-type.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m13-callback/errors/c-unsafe-signature.scoop](../../../tests/fixtures/m13-callback/errors/c-unsafe-signature.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m13-callback/errors/closure-signature.scoop](../../../tests/fixtures/m13-callback/errors/closure-signature.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m13-callback/errors/context-index.scoop](../../../tests/fixtures/m13-callback/errors/context-index.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m13-callback/errors/generic-signature.scoop](../../../tests/fixtures/m13-callback/errors/generic-signature.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m13-callback/errors/missing-signature.scoop](../../../tests/fixtures/m13-callback/errors/missing-signature.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m13-callback/errors/safe-context.scoop](../../../tests/fixtures/m13-callback/errors/safe-context.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m13-callback/errors/suspend-signature.scoop](../../../tests/fixtures/m13-callback/errors/suspend-signature.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m14-generics/errors/intrinsic-types.scoop](../../../tests/fixtures/m14-generics/errors/intrinsic-types.scoop) | core intrinsic 类型在共有模型中没有可访问源码构造器。 |
| [m14-generics/errors/operator-equals.scoop](../../../tests/fixtures/m14-generics/errors/operator-equals.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m2-values/errors/string-plus-int.scoop](../../../tests/fixtures/m2-values/errors/string-plus-int.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m21-delegate-roles/errors/role-contracts.scoop](../../../tests/fixtures/m21-delegate-roles/errors/role-contracts.scoop) | 先拒绝 delegate operator 的非法默认值，不再追加该默认 lambda 的级联类型错误。 |
| [m22-binding-patterns/errors/class-component-ambiguity.scoop](../../../tests/fixtures/m22-binding-patterns/errors/class-component-ambiguity.scoop) | 保留两个歧义候选，使用现行 MSC 判定措辞。 |
| [m22-integers/errors/array-index-requires-long.scoop](../../../tests/fixtures/m22-integers/errors/array-index-requires-long.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m22-integers/errors/callback-context-index-requires-long.scoop](../../../tests/fixtures/m22-integers/errors/callback-context-index-requires-long.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m22-integers/errors/mixed-signedness.scoop](../../../tests/fixtures/m22-integers/errors/mixed-signedness.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m22-integers/errors/mixed-width.scoop](../../../tests/fixtures/m22-integers/errors/mixed-width.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m22-integers/errors/shift-count-requires-long.scoop](../../../tests/fixtures/m22-integers/errors/shift-count-requires-long.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m22-iteration/errors/protocol-resolution.scoop](../../../tests/fixtures/m22-iteration/errors/protocol-resolution.scoop) | 无适用项报告最近的失败候选；歧义仍列出全部并列候选及 MSC 原因。 |
| [m22-ranges/errors/contracts.scoop](../../../tests/fixtures/m22-ranges/errors/contracts.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m23-source-headers/errors/public-current-unit.scoop](../../../tests/fixtures/m23-source-headers/errors/public-current-unit.scoop) | 诊断定位 current-unit 标记，从整条声明起点收紧到第 15 列。 |
| [m3-generics/errors/conflicting-type-args.scoop](../../../tests/fixtures/m3-generics/errors/conflicting-type-args.scoop) | 共同约束求解顺序调整，冲突为 String/Int，位置为第 18 列。 |
| [m3-generics/errors/none-no-context.scoop](../../../tests/fixtures/m3-generics/errors/none-no-context.scoop) | 共有 Option 变体报告缺少预期 enum 类型，规则不变。 |
| [m3-generics/errors/print-generic.scoop](../../../tests/fixtures/m3-generics/errors/print-generic.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m5-arrays/errors/conversion-method-arity.scoop](../../../tests/fixtures/m5-arrays/errors/conversion-method-arity.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m5-arrays/errors/index-not-int.scoop](../../../tests/fixtures/m5-arrays/errors/index-not-int.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m6-classes/errors/generic-interface-call.scoop](../../../tests/fixtures/m6-classes/errors/generic-interface-call.scoop) | 不合法泛型接口方法未进入继承表，显式记录后续 override 无对应成员错误。 |
| [m7-overload/errors/duplicate-return-only.scoop](../../../tests/fixtures/m7-overload/errors/duplicate-return-only.scoop) | 保留重复声明错误，删除由 Error 表达式再触发的 println 级联错误。 |
| [m7-overload/errors/duplicate-signature.scoop](../../../tests/fixtures/m7-overload/errors/duplicate-signature.scoop) | 保留重复声明错误，删除由 Error 表达式再触发的 println 级联错误。 |

M0 的源程序使用 `print`，真实 stdout 为 `hello, world`，末尾没有 LF；旧 insta 文件自身的结尾 LF 不等于程序输出。10 个原 trap 仍要求 SIGABRT、空 stdout 和完整 stderr，类型名按现行 canonical TD 名字保存。其中 `m21-delegated-globals/failure`、`m21-initialization/runtime-cycle` 还保留当前 runtime 给出的实际初始化单元上下文。没有用短类型名归一化掩盖 TD 身份，也没有把合法程序失败改成 negative。

### 1.3 迁移发现并修复的编译器缺口

| 能力 | 修复与回归 |
| --- | --- |
| imported enum 派生相等 | 复用定义 Cone 的真实 helper，覆盖本地、依赖与 re-export |
| 拒绝调用的诊断 | Error 表达式不再制造额外 println 级联；ForeignCallback 按真实 typed identity 拒绝源码构造 |
| shared shape | 源码需求先闭合，再生成协议；补泛型 receiver、companion 及实际协程结果类型 |
| SourceLocation | 默认值、泛型、lambda 和协程保留定义／求值位置及 canonical source |
| 整数操作 | 跨 core 调用保留实际失败参数类型与明确转换提示，不重复 lowering |
| 多接口 application | 每张实际接口表独立保存 slot selection 与选中 receiver，继承默认实现不丢失 |
| raw global／TLS | GC-free 存储不进入 managed registration，正确发布 Mach-O TLS descriptor／初值及 bootstrap 引用 |
| Scoop ABI native roots | C 使用真实源码类型描述符；正常 GC 与强制移动分别验证可观察契约 |

## 2. Program-link 值、协程、ODR 和初始化组合

`compiler/scoop/tests/program_link/{combinations,initialization,siblings}.rs` 的 9 个 Rust 测试已迁成 20 个正式 CLI 用例，继续使用原 40 份源码；另加一项 tuple／泛型派生相等的 re-export 组合。当前声明位于 `tests/fixtures/m23-cli-program-link/`，21 项用例的只读验收全部通过，共 42 个变体、172 次进程执行、210 次阶段／link-plan golden 比较。

每项声明保存真实 manifest 与依赖路径，从普通 `scoop build` 取得产物。重新链接前复制 root 和依赖 `.slib`、移走源码，使用独立复制的 `scoop` 和空 sysroot，并令 LLVM 配置路径不可用。所有用例同时检查正常与 moving 运行；link-plan 保存完整文本并与构建时 fingerprint 对照。

| 原 Rust 测试 | 当前用例 | 保留的关键断言 |
| --- | --- | --- |
| `cross_cone_value_storage_survives_boxing_arrays_and_moving_gc` | `values` | ZST、大值、含引用 tuple／enum、Option、Array、装箱与转换，原五行输出 |
| `coroutine_frames_helpers_closures_and_finally_are_consumed_from_artifacts` | `coroutine-closure-finally`、`coroutine-failure-finally`、`coroutine-function-values`、`coroutine-lifecycle`、`coroutine-value-task` | 协程 frame/helper、函数值、closure、成功／异常 finally，每项返回 42 |
| `odr_member_unions_keep_independently_demanded_function_adapters` | `union-adapters`、`union-adapters-combined` | sibling 独立需求的 callable member 并集，动态函数适配、泛型与接口使用 |
| `sibling_generic_delegates_share_storage_and_cached_failures` | `delegate-siblings` | delegate 构建／provide 次数、共享可变状态、失败对象的 `===` 身份和各 specialization 的隔离 |
| `empty_and_nogc_roots_and_early_initialization_chain` | `empty`、`nogc`、`early-root` | 空／NoGC 入口与原初始化序列 6→5→4→3→2→1→7 |
| `lazy_failure_cycle_and_recovery_preserve_the_original_failure` | `lazy-failure-root`、`unused-lazy-root`、`lazy-cycle-root`、`cycle-recovery-root` | 延迟求值、原失败缓存、未使用时不求值、循环和恢复 |
| `diamond_uses_the_current_ready_set_and_has_a_locator_independent_plan` | `diamond-ready-order` | 六 image、10→20→30→40→50 当前 ready-set 顺序；移动所有产物、重排并重复依赖后的 plan 与 fingerprint 完全相等 |
| `uncaught_root_and_eager_failures_stop_at_the_runtime_gateway` | `root-failure`、`eager-failure` | SIGABRT、原 stdout、完整 canonical 异常 stderr 及 eager 单元上下文 |
| `failed_dependency_prevents_later_eager_initializers_and_main` | `failed-dependency-order` | 仅输出 1，后续 eager 和 main 均未执行，实际失败初始化单元保留 |
| 新增派生相等组合 | `equality-roots` | tuple、generic application、enum、facade 和 closure 中的比较，移走源码后仍可链接运行 |

值类型用例暴露的 reader 问题在共同 HIR/MIR 调用根关联中修复：包含完整 exact owner 的派生 helper 使用 `NoSubstitution`，同时可以有真实 ODR 定义。调用根按其已发布 generated identity 关联，仍拒绝缺失定义以及错误的词法／初始化 application 上下文；未增加产物字段或新的资格检查。587 项 `scoop-slib` 单元测试通过，其中能力描述符和摘要只同步已登记的 HIR 44、type-semantics 12、link-identity-closure 9。

三个原 Rust 编排模块及其 `mod` 注册已删除。其余 program-link/native 用例仍在后续批次逐项迁移。

## 3. Native ABI、回调和线程存储

`tests/fixtures/m23-cli-native-link/` 新增 22 个正例、12 个源码 negative，覆盖原 native harness 的 17 个 Rust 测试和 39 份 Scoop 源码。关闭快照更新后的 34 项用例全部通过，共 56 个变体、268 次进程执行和 468 次阶段／link-plan golden 比较。

正例用真实 manifest 保留原 `dev.programlink` 坐标；native C 文件先显式复制并编译，归档保留无索引的 `ar qcS` 形式，使用 `ZERO_AR_DATE=1` 固定非语义日期。普通 `scoop build --emit all --dump-scope sources` 一次捕获每个源码节点的四阶段输出，分别检查 provider、facade 和 root。随后复制全部 `.slib`、删除 Scoop/C 源码与不再需要的归档成员对象，在无 compiler／LLVM 的独立 `scoop link` 环境重新链接和运行。所有正例同时运行普通与 moving-GC 变体。

| 原 Rust 测试 | 当前用例 | 保留的条件 |
| --- | --- | --- |
| `direct_native_object_runs_after_source_removal` | `direct` | 一个 native 对象、typed 库需求、源码移走后输出 42 |
| `direct_native_provider_survives_defaults_generics_and_reexport` | `direct-reexport` | 默认参数、generic facade、三 Cone 消费、原两行 42 |
| `four_cones_share_archive_dynamic_odr_and_initializers_after_source_removal` | `mixed` | 两个已选归档成员、一个未选成员、动态库、sibling ODR 和 eager/lazy 输出；native symlink 顺序及依赖顺序改变后完整 plan／fingerprint 相等 |
| `packed_c_layout_round_trips_through_a_native_object` | `cabi-packed` | packed／aligned C layout、Boolean 和结构值传递 |
| `fixed_width_layout_pointer_and_static_callback_abis_survive_three_cones` | `cabi-object`、`cabi-archive`、`cabi-dynamic` | 原完整整数边界、指针、静态 callback 和含引用调用结果 |
| `native_global_and_tls_properties_read_write_and_take_addresses` | `storage` | global／const／TLS 的读写与地址操作，原八行输出 |
| `imported_native_properties_use_provider_storage_for_all_input_kinds` | `storage-object`、`storage-archive`、`storage-dynamic` | provider 存储归属、facade、默认参数与引用保活 |
| `native_scoop_abi_keeps_borrowed_references_and_elides_zst_payloads` | `scoopabi` | ZST 参数、borrowed 引用及 C 内 native roots |
| `imported_scoop_abi_publishes_native_roots_and_indirect_results_across_gc` | `scoopabi-object`、`scoopabi-archive`、`scoopabi-dynamic` | 三 Cone、native root publication、间接结构返回；动态变体仍将 managed 实现放在归档，只有 leaf 使用 dylib |
| `native_same_thread_reentry_preserves_live_callback_roots` | `callback-reentry` | 同线程重入、OneShot／Reusable 回调与 live roots |
| `foreign_thread_callback_failure_keeps_managed_payload_and_detaches` | `callback-exception` | 外来线程异常 payload、失败身份和线程退出 |
| `two_native_threads_keep_tls_independent_across_all_inputs_and_moving_gc` | `callback-tls-object`、`callback-tls-archive`、`callback-tls-dynamic` | 两线程独立 TLS 地址与数值、共享 global、回调及线程退出 |
| `imported_core_runtime_intrinsics_collect_with_live_references` | `runtime-gc` | imported core GC intrinsic 与存活引用 |
| `generic_provider_bodies_preserve_pin_and_handle_results_across_gc` | `runtime-handles` | generic provider、pin／handle 与移动后的实际结果 |
| `native_c_abi_rejects_ordinary_layout_and_managed_static_callbacks` | `reject-cabi-*`（2 项） | 普通 struct 无 C layout、缺少 NoGC 的静态 callback |
| `imported_callbacks_preserve_source_contract_diagnostics` | `reject-callbacks-*`（6 项） | signature、context index/type、constant mode、unsafe 和显式类型参数 |
| `imported_native_storage_preserves_source_diagnostics` | `reject-storage-*`（4 项） | unsafe 读、immutable 写、值类型和 addressOf 的类型实参 |

12 个 negative 均保留原 HIR 错误的消息条件和准确 byte span，升级为完整 canonical JSON 数组，并确认未发布请求的输出文件。没有新增预期错误或调整合法性规则。

78 份旧阶段快照迁为 27 份正例源码的 108 份四阶段 golden，另保存 22 份完整 link plan。原 70 份阶段正文仅去除旧快照文件的额外结尾空行；8 份实际变化分别是 CABI／Scoop ABI／storage provider 已有派生相等修复带来的真实 helper，以及 mixed provider 初始化单元的完整 canonical 来源。HIR 另明确保存 Export／LocalConcrete 两部分。完整 mixed plan 继续锁定两个 selected 成员和一个 unused 成员；重排／别名比较不删除输入摘要或类型身份。

删除 `native_inputs/{cabi,callbacks,mixed,runtime,scoopabi,source,storage}.rs` 七个模块、相应注册以及 `direct.rs` 中已迁移的两个测试；同时删除 78 份失去调用者的旧阶段快照。其余 archive／dynamic／corruption 测试仍使用的 native helper 暂留，继续逐项迁移。

## 4. 后续批次

parser／HIR／MIR／LIR 的文件加载与 golden 编排、driver end-to-end、program-link/native 以及需要外部构建的 runtime 测试仍按原断言逐批迁移。直接构造内存 typed IR 的内部单元测试保留。完成每批后补充对应关系、删除的 helper 与实际验证结果；全部完成前不将 M23-11 标记完成。
