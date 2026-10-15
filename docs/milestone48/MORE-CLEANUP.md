# M48：Rust 测试与 CLI fixture 的进一步清理

日期：2026-10-15。状态：清理、完整复验与三平台验收完成；原审查统计保留，实际删减见[实施记录](validation/more-cleanup.md)，最终验证见[结果概要](validation/README.md)。

审查基于 `9edf9060` 及当时的未提交工作区，已排除工作区中删除的旧测试。扫描了全部 4,107 个 CLI fixture 声明，并重点检查 Rust 的对象验证、产物读取及端到端测试调用链；未逐一审定全部 Rust 测试，审查阶段未重跑全量测试或测量清理后的耗时；后续实测结果见上述记录。本文件补充 [CLEANUP.md](CLEANUP.md)，下文观察描述审查时的状态。

## 1. 判断原则与统计口径

按测试实际验证的合同判断，不按 `replay`、`proof`、`rejects`、`legacy` 或 milestone 目录名批量删除。

- 普通语言 fixture 保留编译诊断、运行结果、独立功能与组合覆盖。
- 类型、格式、typed 引用、ABI、内存边界、GC、真实输入变化、缺失输入、失败不发布及普通并发协调测试继续保留。
- 已完成边界验证且未变化的数据应复用结果，不要求下游不断证明前一步的数据没有被替换或篡改。
- 公共打包、编码、链接计划及完整产物读取的交叉验证集中到少量代表用例；各语言功能保留与自身有关的断言。
- 普通运行与 GC stress 运行有不同目的，不能仅因执行相同程序就判为重复。
- 相同的 fixture TOML 模板不等于相同测试；必须同时检查实际源码、输入、运行条件及期望。

| 审查观察 | 数量 | 口径与限制 |
| --- | ---: | --- |
| CLI fixture 声明 | 4,107 | 全部声明，包含目标平台不适用项；不是某平台通过数 |
| 同时执行 `scoop build` 和显式链接 | 690 | 需逐例判断，不能全部删除显式链接 |
| 生成 `built-program` 后显式 `scoop link`，后续未引用该可执行输出 | 513 | 根据声明中的输出路径及 `${program.result.output}` 引用统计；原 build 产生的 `.slib` 等仍被使用 |
| 固定包含内部 `scoop$$1$$` 符号的检查 | 524 | 包含专测，不能全部删除相关断言 |
| `m23-cli-core-layouts` | 43 | 其中 41 个共用通用 consumer |
| Rust core-layout 测试 | 19 | 覆盖 36 份源码，这 36 份全部另有对应 CLI fixture |
| 对象验证链中含 `ObjectBytesMismatch` 断言的测试文件 | 10 | 文件数，不是独立测试数；混合测试需保留其他有效断言 |

以上集合可能重叠，不能相加作为可删除用例数。当前没有可承诺的提速比例。

## 2. 已确认：对象字节的重复防篡改验收

典型流程是先完成对象验证或 fingerprint 计算，再翻转对象最后一个字节，要求后续步骤报 `ObjectBytesMismatch`。它验证的是前一步证明与任意新传入字节的绑定，而不是新的语言、ABI 或 GC 事实。


- [stackmap verification tests](../../compiler/slib/src/link_object/stackmap_normalization/verification/tests.rs)：`rejects_object_bytes_changed_after_the_builtin_object_proof`。
- [type registration tests](../../compiler/slib/src/link_object/type_registrations/tests.rs)：`rejects_object_bytes_changed_after_all_input_proofs`。
- [immortal registration tests](../../compiler/slib/src/link_object/immortal_registrations/tests.rs)：同名的输入证明后篡改测试。
- [digest patch tests](../../compiler/slib/src/link_object/digest_patch_sites/tests.rs)：`rechecks_the_exact_object_bytes_after_definition_verification`。
- [body fingerprint tests](../../compiler/slib/src/link_object/callable_registrations/body_fingerprints/tests.rs)：`rechecks_body_object_bytes_before_hashing`。
- [registration finalization tests](../../compiler/slib/src/link_object/strong_registration_finalization/tests.rs)：`rejects_provisional_bytes_changed_after_fingerprint_computation`。

对应实现确实反复计算整份对象的 SHA-256，例如 [type registration physical reader](../../compiler/slib/src/link_object/type_registrations/physical.rs) 与 [stackmap section reader](../../compiler/slib/src/link_object/stackmap_normalization/section.rs)。正常生产入口 [layout/verification.rs](../../compiler/driver/src/object_production/layout/verification.rs) 连续把同一组候选对象交给这些步骤。

建议：

- [x] 删除“前一步已验证，再人为篡改，要求下一步重新发现”的专属测试及混合测试中的对应分支。
- [x] 同步简化重复复核实现，让后续处理复用同一份对象及解析结果；不能只删测试而保留反复哈希。
- [x] 保留首次对象边界的 section 范围、定义、relocation、stackmap、frame chain、GC registration 和实际 digest patch 正确性检查。

处理依据是 [AGENTS.md](../../AGENTS.md) 的验证范围纪律，以及 [实现规范](../specs/SCOOP-IMPL-SPEC.md) 对对象首次检查及后续结果复用的规定。不是禁止所有对象错误测试或所有生产端摘要计算。

## 3. 已确认：Rust core-layout 公共验收重复执行

[core.rs](../../compiler/driver/src/request/preflight/end_to_end_tests/layout_exports/core.rs) 中的 19 个测试通过 `check_core_layout_exports` 覆盖 36 份源码。每份源码都执行 HIR、MIR、LIR、codegen、产物打包及读取。

其中的重复部分包括：

- [artifact/bytes.rs](../../compiler/driver/src/request/preflight/end_to_end_tests/layout_exports/core/artifact/bytes.rs) 分别打开 Compile 和 Link 视图，重编码并比较多个完整 section，再走完整的 closure 读取链。
- `bytes/lir_strong.rs`（已退役） 在连续阶段重复断言相同的 current、dependency 数量、layouts 和 initialization units。

这些公共验收不必跟随每个语言场景完整执行。与此同时，[shared_abis.rs](../../compiler/driver/src/request/preflight/end_to_end_tests/layout_exports/shared_abis.rs) 中针对 ZST 参数消除、间接参数与间接返回的精确断言具有独立价值。

建议：

- [x] 保留各功能自己的 typed IR／ABI 断言，将其停在能够验证该行为的必要阶段。
- [x] 将公共打包、Compile／Link 双视图和完整 round-trip 验收收敛到少量代表用例。
- [x] 删除同一记录穿过多个不改变它的阶段时重复的完整相等性断言。

不能因为某个 Rust 测试和 CLI 使用同一份源码就直接删除 Rust 测试；结构断言与黑盒运行可能承担不同职责。此处主要应收缩公共流水线和公共断言。

## 4. 已确认：core-layout CLI consumer 未使用被测导出

[m23-cli-core-layouts](../../tests/fixtures/m23-cli-core-layouts) 的 43 个 fixture 中，41 个使用同一个 [support/consumer.scoop](../../tests/fixtures/m23-cli-core-layouts/support/consumer.scoop)：

```scoop
public fun useCore(): Long = stage3CoreAnswer()
```

例如 [shared-abi-standalone](../../tests/fixtures/m23-cli-core-layouts/shared-abi-standalone/fixture.toml) 把 ABI 场景源码加入 core 后，只编译这个通用 consumer，没有调用对应 ABI 导出，也没有运行相关程序。它能发现生产或读取整体失败，却不能证明该功能被下游正确消费。

上述 Rust core-layout 测试中的 36 份源码全部另有对应 CLI fixture，因此这里还重复了完整源码生产过程。

建议：

- [x] 将通用 consumer 的重复编译收敛到少量产物读取 smoke test。
- [x] 需要保留为独立功能 fixture 的场景，改用实际调用对应导出的 consumer，并验证运行结果。
- [x] 为每个删减项确认保留的功能覆盖位置，避免以“去重”为由丢失独立功能和组合 fixture。

## 5. 收缩候选：普通 CLI 的重复链接

690 个 fixture 同时执行 build 和显式链接，其中泛型与导入类两组共有 388 个。进一步检查发现，513 个 fixture 先生成 `built-program`，随后没有引用该可执行输出，而是显式 `scoop link` 后运行另一份程序。

示例：

- [argument-inference-abi](../../tests/fixtures/m23-cli-generics/argument-inference-abi/fixture.toml)：`program` build 后执行 `link`，最终运行 `${work}/program`。
- [primitive-any](../../tests/fixtures/m23-cli-imported-classes/primitive-any/fixture.toml)：相同包装流程。

build 已经完成一次正常链接，因此普通语言用例又承担了显式链接入口的一致性验收。它不是每个语言规则都需要重复承担的职责。

建议：

- [x] 普通语言 fixture 优先直接运行 build 产出的程序，删除没有独立测试目的的再次链接、产物复制和计划检查。
- [x] 显式链接、依赖选择、ODR、native、归档与动态绑定专测保留必要的独立链接流程。
- [x] 保留必要的普通／GC stress 运行模式，不把它们计为重复链接的附带删除项。

690 和 513 都是候选统计，不是最终删除数量。实施时必须逐例判断显式链接是否承载独有的合同。

## 6. 收缩候选：普通功能固定内部链接符号

524 个 fixture 的检查包含固定的内部 `scoop$$1$$` 符号。例如 [argument-inference-abi](../../tests/fixtures/m23-cli-generics/argument-inference-abi/fixture.toml) 除语言运行结果外，还固定断言 root 的 mangled symbol 与 `String` descriptor alias；[primitive-any](../../tests/fixtures/m23-cli-imported-classes/primitive-any/fixture.toml) 也有同类断言。

这些公共符号与被测参数推导或普通类语义无直接关系，重复冻结它们会扩大公共编码变化的维护范围。

建议：

- [x] 普通功能用例删除与被测语义无关的固定内部符号和公共链接计划断言。
- [x] mangling、链接计划、相关 ABI 和 native 专测保留其合同需要的精确期望。
- [x] 同步收缩 [fixture runner README](../../tests/fixture_runner/README.md) 中要求普通语言组合用例保留构建／独立链接一致性及相关符号检查的约定，与 [CLEANUP.md](CLEANUP.md) 的代表性链接专测原则保持一致。

不能把 524 个 fixture 全部改成只检查 exit code；语言输出、诊断、真实多 Cone 消费和必要的运行合同仍须保留。

## 7. 旧 milestone 残留：历史 profile 的正向冻结测试

[profile/tests.rs](../../compiler/slib/src/profile/tests.rs) 仍固定 `SINGLE_CONE_STRONG` 和 `CROSS_CONE_SEMANTICS_STRONG` 的完整 descriptor 与 fingerprint。[profile/tests/strong_version.rs](../../compiler/slib/src/profile/tests/strong_version.rs) 还要求这两个旧 profile 随当前 production section 版本一起更新。

当前 [实现规范](../specs/SCOOP-IMPL-SPEC.md) 规定的完整生产 profile 是 `org.scoop-lang.slib-profile/cross-cone-generic/5`。本次未在 driver／scoop／linker 中找到上述两个旧 profile 常量的引用。旧 descriptor 随新 section 更新也不是对历史固定字节的兼容性测试。

建议：

- [x] 复核历史 profile 的实际入口与剩余调用，退役仅为旧 milestone 正向验收保留的 profile 测试、golden 及专属构造辅助。
- [x] 保留当前 profile 的格式、required inventory 和兼容性检查。
- [x] 若需要诊断旧产物，保留少量真实的旧版本拒绝测试，不继续维护一套会随当前版本变化的旧正向生产矩阵。

这是优先退役候选；实施前仍需检查共享辅助和实际 reader 的使用关系，不能仅凭常量没有直接出现在命令入口就机械删除实现。

## 8. 已确认：program-read 的无用首次构建

[read-basic](../../tests/fixtures/m23-cli-program-read/read-basic/fixture.toml) 和 [read-combined](../../tests/fixtures/m23-cli-program-read/read-combined/fixture.toml) 都先 build，保存无人读取的 `original-program.slib`，然后向源码插入运行断言，再执行 `verified` build。最终只运行第二次产物，也没有对两次构建作输入失效或结果变化的断言。

建议：

- [x] 准备源码时直接包含所需运行断言，删除无用的首次 build 和原产物复制。
- [x] 保留最终功能结果、必要的依赖断言及普通／moving-GC 两种运行。

## 9. 不应误删的测试与文字遗留

- `replay` 名称不证明仍有生产语义重放。例如 `layout_profile_replays_the_legacy_hir_production_before_new_semantics` 的实现路径已经改为读取保存的 core 记录；应按实际行为决定保留、合并或改名。
- `bridge_replays_each_export_signature_from_the_mir_foundation` 检查缺失 implementation 与签名不匹配，不能因名称就删除必要的引用／ABI 关系检查。
- `decoded_record_recomputes_member_id` 检查 Cone 与 stable key 派生的 member 身份；这不同于对完整成员内容重新哈希。本次未把它列为可直接删除项。
- section 越界、无效 relocation、GC root 数量不符、relocation 后使用旧 root 等仍是必要的编译器或 runtime 正确性测试。
- 修改或删除真实输入后要求报错、失败时不覆盖旧输出，以及所需输出缺失后的正常重建，不等于“隐藏输入后仍必须成功”的隔离验收。
- `@GCLeaf` 和 enum C ABI 的部分诊断仍带有 “milestone M12” 字样。应按当前语言规则更新表述；这首先是诊断文字遗留，不足以证明对应 negative case 已过时。

## 10. 建议实施顺序与验证

1. 按第 5、3、4 节减少普通 CLI 重复链接、Rust 公共完整流水线和无针对性的 consumer 编译，同时保留各功能独有断言。
2. 按第 2 节同步清理对象重复复核的测试与实现，避免只减少测试数量却继续维护不必要机制。
3. 收缩普通功能的固定符号断言，退役历史 profile 正向冻结矩阵，删除 program-read 无用步骤，并同步修订相关文档。

清理应分批记录删除／合并／迁移及保留覆盖的位置。实现或脚本变更按 AGENTS.md 先格式化和 lint，再运行受影响的 Rust 测试与 fixture；公共 runner 规则变化需要验证 runner。完成批次后统一安排必要的完整验收，避免每删除几个断言就重复全量测试。

上述实施项已完成代码、用例修改与最终统一验证。实际范围、保留覆盖和验证记录见 [进一步清理记录](validation/more-cleanup.md)。已删除的用例不触发重跑；修改用例只复验受影响项；本批包含对象生产和 profile 的编译器变更，因此统一执行必要的完整验收。

## 11. 实施落点

488 个普通语言 fixture 移除重复独立链接、公共计划与固定内部符号检查；保留原程序结果、诊断、依赖数量、普通／moving-GC 模式。2 个 program-read 用例删除无用首次构建。34 个 core-layout 用例保留产物生产，退役不使用导出的通用 consumer；base 保留读取 smoke，6 个 ABI／TD／初始化用例改为真实导出调用并运行。Rust 36 份源码仍保留 typed IR／ABI 断言，完整对象和打包只执行 base、alias combined、property initialization 三个代表场景，双视图及完整读取链只由 base 覆盖。

同一次对象生产不再反复哈希既有字节；registration、runtime image 与 entry 的 digest 回填复用已解析结构，更新实际内容摘要。首次对象格式／section／relocation／stackmap／frame chain、GC 记录与回填范围检查保留。旧两种 profile 的生产 descriptor、正向矩阵和专属旧产物构造辅助退役；当前 profile inventory 保留，旧 profile 拒绝由小型兼容性测试覆盖。

为消除清理后仍占主要时间的重复输入生产，842 个普通用例共享原有辅助 core 源码，172 个导入类用例共享相同的修改版 core 源码；均执行真实 build 并复用正常缓存。需要不同 core 输入与更新比较的专测仍保留独立输入。语言规则与 runtime／sysroot 实现保持；测试仍执行真实编译。
