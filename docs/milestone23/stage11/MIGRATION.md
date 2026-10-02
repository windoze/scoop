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

## 4. 归档损坏与 fixture 配置预检

原 `archive_corrupt_containers_and_incompatible_members_fail_at_input` 已迁到 `m23-cli-native-link/archive-corrupt/fixture.toml`，保留 thin archive、截掉 payload 末尾 8 字节、坏 member header 和嵌套 archive 四种输入。正常归档先通过公开 build 并输出 42，之后移走源码和 loose object；每次独立 link 失败都严格比较完整 canonical 诊断，并确认原 `previous executable` 字节未被覆盖。原 Rust 测试删除，其他 archive 断言留待后续批次。

fixture 的 `truncate(path,size)` 只缩短现有文件；对应归档的初始完整摘要明确写在数据中，避免编译工具变化后误把其他损坏当成原 payload 截断。这个动作与现有 copy／patch 共用文件步骤，不包含归档解析器或 case 专用代码。

统一预检补齐嵌套进程／文件／断言值类型、每个变体的变量作用域、后台结果必须等 wait 完成，以及独立诊断文件中的前向引用。20 项 infra 测试全部通过。新增归档用例只读运行通过（1 变体、9 次进程、4 个 golden）；已有 CLI 4 项和 native 13 项分别再次只读通过，保留 5／14 个变体、42／24 次进程与 28／26 个 golden。完整发现仍明确报告 1691 份尚未迁移的源码，未将部分验收计为全仓通过。

## 5. Native 输入选择、动态库与格式错误

其余 21 个 `native_inputs` Rust 测试迁为 29 份声明，原 `m23-native-link/` 的 54 份 Scoop 源码现已全部有归属。包含前两批在内，64 项 native 用例在新工作目录的只读验收全部通过，共 86 个变体、512 次进程和 601 次阶段／plan golden 比较。本批增加 29 个变体、235 次进程、129 个 golden；需要 moving GC 的组合显式再次执行同一个已链接程序。

| 原 Rust 测试 | 当前用例 | 保留的断言 |
| --- | --- | --- |
| `direct_candidates_merge_identical_bytes_and_reject_ambiguous_or_corrupt_inputs` | `direct-candidates` | 相同对象 bytes 去重后的完整 plan／fingerprint 相等，不同内容产生歧义，坏 Mach-O 不能被另一有效候选掩盖 |
| `direct_object_checks_real_symbol_kind_target_and_effects` | `direct-contracts` | data/function 不匹配、缺失 helper、constructor、common/tentative 和 x86_64 五种独立拒绝 |
| `archives_resolve_member_chains_and_back_edges_without_unused_effects` | `archive-chain` | 跨 archive 回边闭包、3 个 selected／1 个 unused、原 42/7 输出，完整 nm 符号列表且没有 `m23_unused` |
| `archive_same_names_and_identical_members_keep_physical_identity` | `archive-members` | 两个同名同 bytes 成员仍有独立 ordinal/header，恰好选择一个 |
| `archive_selected_member_conflicts_and_missing_helpers_report_chain` | `archive-conflicts` | missing chain、strong／weak 冲突和 constructor，完整消息保留 member 与 `_helper` |
| `archive_symbol_table_offsets_and_member_definitions_are_checked` | `archive-toc` | long member name、有效 TOC 正常运行、TOC 改为 `_m23_bad` 时准确拒绝 |
| `native_calls_data_and_code_pointers_run_with_debug_and_optimized_objects` | `formats-pointers` | O0 与 O2+debug 的真实对象均运行得到 42，native 输入改变导致新 link fingerprint |
| `selected_native_objects_reject_corrupt_relocation_tables_and_unwind_records` | `formats-relocations` | 原 11 种 symbol/load-command/relocation/unwind 错误，分别命中原拒绝原因 |
| `native_tls_descriptors_are_checked_before_linking` | `formats-tls` | 有效 TLS control、非零 initial key、指向普通 global 的 template relocation |
| `explicit_dynamic_bindings_choose_different_owners_for_overlapping_exports` | `dynamic-overlap` | 重叠 exports 仍按显式库选择不同 owner，11/22 输出、rpath 和 moving GC |
| `native_reexport_and_framework_keep_the_declared_library_owner` | `dynamic-reexport`、`dynamic-framework` | facade／framework 保留 owner，同时记录真实 leaf 的 re-export |
| `ordinary_dynamic_dependency_does_not_become_a_public_export` | `dynamic-ordinary` | 普通 load 不转为 public export，缺少 leaf 时报告 0 candidates |
| `native_absolute_install_name_and_standalone_text_stub_run` | `dynamic-absolute` | 绝对 install name 与独立 TBD 均运行得到 42，不生成 rpath，同时提供二者时歧义 |
| `renamed_reexport_retains_the_public_name_and_actual_source` | `dynamic-renamed` | alias 的公开名字与真实 `_m23_add` source 同时保留 |
| `conflicting_install_names_and_unsupported_load_paths_are_input_errors` | `dynamic-install-errors` | 相同 install name 冲突、拒绝 `@executable_path`；共享绝对名字为固定数据，不在该路径创建文件 |
| `default_namespace_reports_multiple_visible_dynamic_providers` | `dynamic-default` | default symbol 有两个实际可见 provider 时报告歧义与 Cone origin |
| `loader_relative_dependency_uses_the_original_provider_directory` | `dynamic-loader` | `@loader_path/private` 相对原 facade 解析，独立源码移走后仍运行 |
| `unused_extern_contracts_conflict_before_library_lookup_and_report_all_cones` | `contract-*`（8 项） | library／ABI／effect／parameter／result／function-data／TLS／mutability，保留 left、right、also-left 全部 origin，空 library path 下先报 contract conflict |
| `later_archive_selection_cannot_take_over_an_explicit_dynamic_binding` | `resolution-late-archive` | 后选 archive 不能接管已经显式绑定的 dynamic symbol |
| `default_namespace_does_not_scan_files_from_library_paths` | `resolution-default` | 原 default namespace 不扫描未声明的 native 对象 |
| `native_runtime_location_and_install_name_change_the_link_plan` | `resolution-runtime-location` | 移动实际 dylib、改为绝对 install name 都改变 plan／fingerprint，三次运行保持 42 |

所有损坏／冲突步骤比较完整 canonical 诊断，并逐次确认旧输出 bytes 不变。Mach-O 和 archive 字节修改的位置只在迁移时从真实编译结果定位一次，提交的数据包含原始文件 SHA-256 前置条件与明确的 offset/hex；Python 中没有 Mach-O、Slib 或语言 reader。`not_contains` 是公共内容断言，用于 nm 的未选择符号和绝对 install name 的无 rpath 条件，独立 infra 测试随之增至 21 项并通过。

需要先有 runtime index 的失败路径先构建有效 control，再显式修改 native 输入或源码声明。contract/default cases 使用普通 `scoopc build` 重建待拒绝的完整 `.slib`，随后移走源码并调用独立 `scoop link`；没有临时入口、隐藏 manifest 或绕过 producer 的产物。三个 contract provider 的包头明确写在 TOML，正文在运行时从原源码读取，保留原坐标、包和 native declaration 身份。

绝对 install name 会进入真实 dylib 内容及 native input identity；这类 plan 用具体 binding、rpath 缺失和跨步骤完整 plan／fingerprint 比较，保留位置改变应影响链接结果的语义，不删除摘要来制作固定 golden。其余稳定 plan 直接保存完整文本。最后 9 份旧阶段正文逐一核对不变，迁为同次编译的四阶段输出，HIR 另记录 LocalConcrete。

删除整个 `compiler/scoop/tests/program_link/native_inputs.rs` 及其 7 个子模块、7 个专用构建／快照 helper、对应注册和最后 9 份旧 native stage 快照；简化已没有非空 support 调用的 `build_with_support` 包装。该目录不再保留 native 用例注册器或 `SCOOP_UPDATE_NATIVE_LINK_SNAPSHOTS` 入口。其余 program-link 基础与产物测试仍继续迁移。

## 6. 后续批次

parser／HIR／MIR／LIR 的文件加载与 golden 编排、driver end-to-end、program-link/native 以及需要外部构建的 runtime 测试仍按原断言逐批迁移。直接构造内存 typed IR 的内部单元测试保留。完成每批后补充对应关系、删除的 helper 与实际验证结果；全部完成前不将 M23-11 标记完成。
