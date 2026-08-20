# M23-11 泛型成员 CLI 迁移

## 成员与接口错误

保留 `m23-generic-member-consumption` 的 9 份原 negative 源码：interface-missing、interface-wrong-type、interface-readonly、interface-invariant、interface-super-abstract、ambiguous-overload、bad-method-kind、bad-method-arity、bad-owner-argument。新 schema 1 用例从原 provider 源码构建普通 slib，移走源码后通过正式 CLI 编译各 consumer。

逐项核对原测试要求的消息和最后一个目标 token 的 UTF-8 byte span，确认诊断仍指向当前 consumer 的 `src/main.scoop`；随后锁定完整 JSON，包括全部诊断、code、severity、origin 和 notes，不用其他错误替代。所有失败均要求不发布 consumer 产物。

只读验证通过 9 个用例、27 次进程。删除原 `imported_generic_members_enforce_source_rules` 文件测试；新增用例只使用现有 runner schema，不增加专用执行分支。

## 成员、protected 访问和扩展属性

复用 `m23-cli-generics/generic-member-consumption-*` 的 49 个既有用例，不复制另一套源码或执行规则。35 个正例包含普通泛型成员 18 项、protected 成员 9 项、扩展属性 8 项；14 个负例包含 protected receiver／setter／override 9 项和扩展属性 5 项。以 `hir-generic-members` tag 标记原 HIR 断言的覆盖范围。

成员正例的完整 HIR 保留外来 Box 不进入当前 Export、真实 Method application 以及 signature-support 只物化所需 SignatureValue、不物化 Unrelated 的结构。TOML 另显式要求 Method application 并排除 Export 中的 Box 声明。扩展属性的 LocalConcrete 保留真实 Accessor 和 Application 身份、Extension receiver、参数与正文；write-only 只有一个双参数 setter，不物化 getter。

退役前原五个 Rust 测试全部通过，包括扩展访问器 canonical key 的相同 Accessor origin、NoOwner、非空 callable arguments 和 canonical foundation 构建。新完整 golden 持续锁定这些已核对的 Accessor／Application 标识及结构，正式产物 reader 继续执行身份和引用检查；dump 本身不另造 canonical key 表。所有正例通过 provider → consumer → downstream 的真实产物消费，并运行普通和 moving GC 程序。

14 个错误逐项核对原消息、最后一个目标 token 的 UTF-8 byte span 和 consumer 的 `src/main.scoop`；扩展属性仍各有且仅有一个错误。完整 JSON 保留所有诊断及来源，失败不发布 consumer 产物。

只读验证通过 **49 个用例、462 次进程、455 次阶段／计划 golden 比较**。删除 `hir-lower` 的 `members.rs`、`members/access.rs`、`members/extensions.rs` 和注册；原 `.scoop`、完整 CLI golden 及其他 typed 单元测试保留。
