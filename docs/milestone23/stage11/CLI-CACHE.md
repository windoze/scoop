# M23-11 公开 CLI 缓存验收

本记录随各项独立迁移更新；总验收仍以统一 Python 入口的完整只读运行结果为准。

## 泛型依赖与消费者变化

`generic_dependency_changes_rebuild_consumers_and_unchanged_inputs_hit_cache` 迁为 `tests/fixtures/m23-cli-cache/generic/`。原 provider、consumer 源码与 `test:dependency:1.0.0`／`test:root:1.0.0` 坐标保持。正式 `scoop build` 首次编译 core、依赖和根三个节点，第二次没有 compiler child；逐节点检查完成来源、缓存键、完整产物 fingerprint 和归档字节。

依次修改公开正文、私有 helper、默认参数、泛型约束和消费者类型实参。前四项重编译 provider 与 root，最后一项只重编译 root；各项随后再次构建均命中缓存，未受影响的 core／provider 产物保持相同。六个状态分别保存根的完整 AST/HIR/MIR/LIR。普通 Scoop 程序消费实际 root 产物，在普通与 moving GC 下检查结果 42、43、44、45、45、45；library 构建不需要 runtime 源码。

原 Rust 进程测试及注册已删除。格式化与全 workspace lint 通过后，关闭更新开关进行只读验收：1 个用例、1 个变体、30 次进程执行、24 次 golden 比较全部通过。
