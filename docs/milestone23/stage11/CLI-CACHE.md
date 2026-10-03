# M23-11 公开 CLI 缓存验收

本记录随各项独立迁移更新；总验收仍以统一 Python 入口的完整只读运行结果为准。

## 泛型依赖与消费者变化

`generic_dependency_changes_rebuild_consumers_and_unchanged_inputs_hit_cache` 迁为 `tests/fixtures/m23-cli-cache/generic/`。原 provider、consumer 源码与 `test:dependency:1.0.0`／`test:root:1.0.0` 坐标保持。正式 `scoop build` 首次编译 core、依赖和根三个节点，第二次没有 compiler child；逐节点检查完成来源、缓存键、完整产物 fingerprint 和归档字节。

依次修改公开正文、私有 helper、默认参数、泛型约束和消费者类型实参。前四项重编译 provider 与 root，最后一项只重编译 root；各项随后再次构建均命中缓存，未受影响的 core／provider 产物保持相同。六个状态分别保存根的完整 AST/HIR/MIR/LIR。普通 Scoop 程序消费实际 root 产物，在普通与 moving GC 下检查结果 42、43、44、45、45、45；library 构建不需要 runtime 源码。

原 Rust 进程测试及注册已删除。格式化与全 workspace lint 通过后，关闭更新开关进行只读验收：1 个用例、1 个变体、30 次进程执行、24 次 golden 比较全部通过。

## core 构建根与默认来源

`real_process_builds_and_caches_an_edited_core_root_without_a_sysroot` 迁为 `core-root/`。在普通目录加入原 core extension／metadata 源码，以缺失 sysroot 构建该 library；cold 只有 core 一个 child，warm 没有 child，产物字节与缓存键相同。复制实际 core 产物到目标平台的 sysroot 路径后移走源码，清空环境的直接 `scoopc` 与公开 `scoop build` 编译原 consumer，所得 `.slib` 字节相同。公开构建只有 consumer 一个 child，core 是普通 prebuilt；程序通过普通与 moving GC 运行。原 Rust 测试与 helper 已删除。

这项迁移发现公开构建此前只支持默认 core 源码。先修订语言规范 12.3、实现规范 2.7 与阶段设计，再接通默认产物定位：显式依赖优先，默认源码存在时继续使用源码，仅在目录不存在时消费目标平台的产物。新增 `core-default/` 用损坏的备用产物证明源码优先；七个负例覆盖错误 manifest、缺失 manifest、dangling 源码 symlink、损坏产物、错误 coordinate、目录产物与缺失产物。各项均比较完整 canonical 诊断，并确认旧输出保持不变。manifest 诊断保留实际 host path／文本范围，普通 I/O 不伪造 span，产物格式错误保留 semantic member。

两个用例只读通过，共 2 个变体、15 次进程、12 次 golden 比较；12 项共有依赖发现单元测试通过。既有 19 项输入与 4 项基础 CLI 回归也全部只读通过（24 个变体、112 次进程、36 次 golden 比较）。格式化与全 workspace lint 通过；新增默认定位模块 31 行，沿已有 source／prebuilt 节点处理，没有额外编译或产物验证流程。

## 默认 core 源码缓存

`edited_core_rebuilds_through_the_common_cache_without_writing_sysroot_artifacts` 迁为 `core-source/`。保留原 extension 与 library consumer，cold 编译 core／root，warm 两者零 child；修改 core 的 `+ 1` 为 `+ 2` 后两个节点重编译，fingerprint／缓存键都变化，再构建命中新的条目。恢复原内容时直接命中原条目，证明两个版本的缓存都仍可用。各状态的普通与 moving GC 程序结果为 42、43、42；sysroot 始终没有 artifacts 目录。

原 Rust 进程测试与注册已删除；已准备源码不受随后宿主文件修改影响的内部断言仍由 `core_uses_the_common_immutable_snapshot_and_cache_key` 及 `prepare_materializes_only_immutable_private_source_inputs` 保留。格式化与全 workspace lint 后，退役状态再次只读通过：1 个用例、1 个变体、14 次进程、8 次 golden 比较。

## 显式 core locator 切换

`explicit_core_source_rebuilds_and_can_be_replaced_by_its_prebuilt_artifact` 迁为 `core-locator/`。原 root／helper 坐标与 core extension／consumer 保留，默认 sysroot 始终不存在。cold 按 core、helper、root 顺序编译，warm 零 child；修改 core 后三者的实际产物和缓存键都变化，再构建命中缓存。把 core 源码 locator 改为刚完成的 `.slib` 并删除源码后，core 为 prebuilt 且没有 source cache key，helper／root 仍命中相同 bytes。通过依赖 root 的真实 executable 再次发现这条显式 core 边，程序在普通与 moving GC 下保持 42、43、43。

原 Rust 进程测试与注册已删除。格式化与全 workspace lint 后，退役状态再次只读通过：1 个用例、1 个变体、14 次进程、8 次 golden 比较。此前五项 CLI cache 用例一起关闭更新开关也全部通过，共 5 个变体、73 次进程、52 次 golden 比较。

## 基本进程与 canonical 调度

原真实进程测试逐项迁为独立声明。保留原坐标和源码文本，cold 的 core／source 完成顺序与来源、warm 的零 child 和相同归档字节／缓存键均通过公开 JSON 检查；全部根从一次编译保存四阶段输出。直接 `scoopc` 以同一 core 产物编译原根，所得 `.slib` 与公开构建逐字相同。executable 另通过正式链接并运行普通／moving GC。diamond 按原 beta 在前的 manifest 声明检查实际 core、alpha、beta、root 的 canonical 顺序。每次完成后私有 staging 为空，返回产物仍可读取。

各批删除对应 Rust 注册及失去调用者的 helper，格式化与全 workspace lint 后进行只读验收。

| 原 Rust 测试 | 新用例 | 进程／golden 比较 |
| --- | --- | --- |
| `real_process_compiles_then_reuses_core_and_source_cache` | `process-library` | 3／4 |
