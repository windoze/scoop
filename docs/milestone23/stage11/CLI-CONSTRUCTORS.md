# M23-11 构造器 CLI 迁移

按原功能核对语言规则、位置和结构后，删除对应 Rust 文件 harness。原源码保留。

## @NoGC 诊断

原 `m19-constructor-nogc/errors` 的 20 份源码迁为相邻 schema 1 描述，经正式 single-file CLI 构建。原合并快照中的 28 条消息、起止行列均逐项换算并核对实际 UTF-8 byte span，完整 JSON 还检查 code、severity、canonical source 和 notes。仅将原 core 拼接后的 file index 改为 single-file 的真实 source identity；不接受其他错误替代。每个失败均要求不发布输出。

关闭更新的只读验收通过：20 个用例／变体、20 次进程。删除原 `m19_nogc/errors.rs`、注册和合并诊断快照，期望保存为各用例的完整诊断文件。

## Safety 诊断

原 `m19-constructor-safety/errors` 的 14 份源码迁为相邻 schema 1 描述，经正式 single-file CLI 构建。原合并快照中的 14 条消息、起止行列均逐项换算并核对实际 UTF-8 byte span，完整 JSON 还检查 code、severity、canonical source 和 notes。仅将原 core 拼接后的 file index 改为 single-file 的真实 source identity；不接受其他错误替代。每个失败均要求不发布输出。

关闭更新的只读验收通过：14 个用例／变体、14 次进程。删除原 `m19_safety/errors.rs`、注册和合并诊断快照，期望保存为各用例的完整诊断文件。
