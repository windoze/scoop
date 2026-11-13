# Scoop PGO

`scoop-pgo` 是配套主驱动的外部命令，安装到 `<sysroot>/commands`，通过 `scoop pgo` 调用；Windows 对应 `sco pgo`。它保存构建设置、运行训练程序并管理计数合并，编译和链接仍使用正常 `scoop build`。详细合同见 [profiling 设计](../../docs/profiling/DESIGN.md)。

## 使用

先安装当前目标的可选 `profiling` 组件。该组件包含 Profiling/full 训练库与 Release/full 无探针库；普通系统库无需替换。使用支持本功能的安装器和发行包：

```sh
scoop-install component add profiling --target aarch64-apple-darwin
scoop pgo instrument . -- data/train-small.json
scoop pgo instrument . -- data/train-large.json
scoop pgo optimize -o dist/app
```

数据缺省保存在调用 cwd 下的 `target/pgo`。可用 `--data-dir` 选择另一集合；`--target-dir` 仅控制普通构建输出和缓存，不移动训练数据。PGO 要求一个 executable 根，workspace 用 `-p group:name` 收窄选择。instrument 对源码 Cone 默认 `--export-ir full`，也可显式选择 off/default/full；系统组件的模板固定为 full。

后续命令复用实际选择的根、target、导出策略及路径已解析的构建选项；optimize 省略 root 时不会重新选择当前 workspace 的默认成员。切换 cwd 时显式指向原 `--data-dir`。jobs、输出、缓存、诊断、工具及配套预编译资源位置可以覆盖；不同根、目标、导出策略或语义选项使用另一数据目录。源码修改允许继续训练，过期正文的数据按正常匹配规则回退。

程序参数只放在 `--` 后，包括空参数和以 `-` 开头的参数。PGO 保留程序的 stdin/stdout/stderr、原生 argv 和业务退出状态。非零业务退出仍可提供有效样本；程序成功但没有产生计数会报告采集失败。PGO 摘要写 stderr，结构化信息写入数据目录。

```sh
# 需要外部宿主、手动启动或另一台机器执行时，仅生成训练程序和采集目录。
scoop pgo instrument main.scoop --data-dir training --no-run -- input.json
# 按输出提示设置 SCOOP_PROFILE_DIR 并运行程序，完成后合并及反馈构建。
scoop pgo optimize --data-dir training

# 观察哪些内联/字符串融合决策实际消费了动态频次。
scoop pgo optimize --data-dir training --emit optimizations --dump-dir observations
```

optimize 自动选择 Release/full 系统库，要求至少一个正常优化决策实际使用反馈，成功后生成无探针的 binary；不会自动运行结果。保留为外部调用的系统函数使用组件中的普通 Release 机器码，并未按当前应用数据重编译。第三方预编译库的缺探针或缺映射情况会单独报告。

## 数据与失败处理

`build.json` 保存最近一次成功训练构建；`builds/<link-fingerprint>/` 保存其独立映射副本；`runs/<run-id>/run.json` 记录运行配置与状态，其进程子目录由 runtime 写入 `counts.profraw` 和 `runtime.jsonl`。`merged/` 保存 `counts.profdata`、可直接用于低层 `--profile-use` 的 `profile.json`、`summary.json` 和输入更新记录。`optimization.json` 保存反馈消费者和最终输出路径。runtime 时间信息不混入执行频次。

`runtime.jsonl` 同时保存 GC 总指标与事件时间线：`gc-cycle-begin/end` 描述完整或重启的收集周期，`gc-slice` 分开记录停稳等待、暂停和 worker CPU，`gc-phase` 关联周期与暂停片段。增量周期可以跨多次暂停；plan 包含 copy，reclaim 包含 vm-return，嵌套区间不要重复相加。时间以 runtime 初始化为原点，单位为纳秒。时间线最多保留 8,192 条事件，`gc-trace.dropped` 明确报告截断；总指标仍覆盖完整运行。

训练默认追加；合并累加整数计数并保留全零记录。输入未变化且合并结果存在时直接复用；实际输入缺失、损坏或合并失败时停止，保留已有合并结果与 binary。编译失败不启动旧程序，配置不匹配不替换现有 binary。同一目录通过普通文件锁串行操作；不包含后台任务或隐式数据上传。

## 开发

```sh
cargo build -p scoop -p scoopc -p scoop-pm -p scoop-pgo --bins
python3 scripts/build_scoop_pgo.py --sysroot /path/to/development/sysroot
```

主驱动分发时向子进程传递 `SCOOP_DRIVER` 和 `SCOOP_SYSROOT`。直接调用独立程序可传 `--scoop`、`--sysroot`；开发目录还可通过 `--llvm-profdata` 指定配套 LLVM 22.1 工具。正式安装从自身 LLVM 工具目录定位，不从任意 PATH 选择另一版本。`--profile`、`--release`、`--profile-use`、`--profile-use-required` 由命令管理，显式传入会报错；`--emit-slib` 和 `--tests` 不属于训练入口。
