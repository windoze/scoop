# M23-11 静态命名空间重导出迁移

保留 `m23-reexported-namespaces` 全部 28 份原源码，用 schema 1 描述 8 个正例和 13 个反例。真实编译 `namespace-origin`，再从同一 facade 源码分别发布 `namespace-first` 与 `namespace-second`；每层源码在下游读取前移走。consumer 同时从两个 facade 查找静态命名空间，随后再发布给独立 downstream，最后由正式 artifact-only link 组成七个 image 的程序。

正例保留 exact／star、deep nested owner、object、companion、enum variant、public exact／star 的完整组合；downstream 覆盖具体 Int、含引用的 Later 与 Unit 实参。每项保存原 provider、两个 facade、consumer 和 downstream 的完整 AST／HIR／MIR／LIR，验证结果 42、普通／移动 GC 和独立进程链接，实际 nm 输出确认所用 weak callable 只保留一个定义。

反例逐项保留 support-only exact／star 查找、private／internal／protected／instance 限制、star 下的 private、public 重导出 private、companion private、owner arity、两个 provider 的同名歧义、最长 package 匹配及再次重导出的隐藏声明。后两种分别引入原 conflict／value-package provider；隐藏声明场景先真实发布 public-star consumer。完整 JSON 保留原消息、consumer 的 UTF-8 span、code、severity 和 notes，失败均不发布产物。

迁移复核：原 24 份 HIR／MIR／LIR 内容及 13 份诊断逐项相同。正式 runner 只读验证 21 个用例、180 次进程、168 份阶段／link-plan golden 全部通过。删除 driver 的 `reexported_namespaces.rs`、注册、专用编排／快照 helper 与 37 份旧快照；新增用例只使用既有 runner schema。
