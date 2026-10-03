# M23-11 构造器 CLI 迁移

保留 `m19-constructor-nogc` 的 scalar／generic 和 `m19-constructor-safety` 的 safety／generic 四份原源码。每个 schema 1 用例先用正式 single-file 编译并运行，再将同一原源码发布为 library，移走 provider 源码后由真实 consumer 调用构造器；全部源码移走后，隔离编译器独立链接，并验证普通／移动模式。新的 consumer 使用原公开构造器，不改变原成员可见性。

真实产物发现泛型 `@NoGC` struct 次构造器的 application binding 将 source GC effect 固定写成了 Managed，导致合法源码无法发布。binding 现在使用具体次构造器的实际 effect；主构造器保持 Managed 的源码合同、PrimaryValueConstructor 角色及 NoGc 的物理值组装。该修复不改变构造器身份、wire 格式或 runtime ABI。

从正式 CLI HIR／MIR golden 提取原合同观察，scalar 和 generic 的各 30 行 GC 合同、两种 safety 的各 6 行合同均与旧快照逐字节相同。核对 primary／secondary effect、Native<String> 含引用结果、Phantom<String> 无 GC 条件、各 owner 的 NoGC 类型参数要求，以及 NoGc 次构造器参数和结果确实 GC-free；source／concrete safety 集合一致。provider 共享 callable 表仍有 6 个 NoGc 构造器和 4 个 Unsafe 构造器。

退役前运行原构造器结构与 wire 断言，53 项 Rust 回归全部通过，包括完整声明表编解码等价和按真实 constructor identity 核对 source／concrete safety。上述数据由新的完整 HIR／MIR／LIR golden 与实际下游读取持续覆盖。删除 `m19_nogc` 文件 runner、render helper、`m19_safety` 的文件测试、两项 source constructor wire 文件测试、注册及四份旧快照；保留直接修改 typed AST 的 compiler exception constructor 单元测试。

最终只读验证：4 个用例、32 次进程、52 份阶段／link-plan golden 全部通过。原 negative 构造器用例已在此前的 CLI 迁移中保留，本次不修改任何诊断或 negative 期望。
