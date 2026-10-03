# M23-11 泛型成员 CLI 迁移

## 成员与接口错误

保留 `m23-generic-member-consumption` 的 9 份原 negative 源码：interface-missing、interface-wrong-type、interface-readonly、interface-invariant、interface-super-abstract、ambiguous-overload、bad-method-kind、bad-method-arity、bad-owner-argument。新 schema 1 用例从原 provider 源码构建普通 slib，移走源码后通过正式 CLI 编译各 consumer。

逐项核对原测试要求的消息和最后一个目标 token 的 UTF-8 byte span，确认诊断仍指向当前 consumer 的 `src/main.scoop`；随后锁定完整 JSON，包括全部诊断、code、severity、origin 和 notes，不用其他错误替代。所有失败均要求不发布 consumer 产物。

只读验证通过 9 个用例、27 次进程。删除原 `imported_generic_members_enforce_source_rules` 文件测试；新增用例只使用现有 runner schema，不增加专用执行分支。
