# 函数级采样归因

先用普通 release 构建取得程序，再请求同一次构建的符号映射：

```sh
target/release/scoop build PATH --release --emit symbols \
  --dump-dir "$PWD/tmp/profile/dumps" -o "$PWD/tmp/profile/program"
```

保留应用原有的 sysroot、目标、链接和 cache 参数。映射位于
`dump-dir/ConeIdentity/symbols.json`；`--emit all` 也包含它，所有 profile 均可请求。
已有机器产物而缺少映射时，只补到 LIR 的观察；再次请求复用观察缓存。
独立 `scoopc build --emit symbols` 把同一 JSON 写到 stdout。

映射包含当前 Cone 及本次已加载预编译依赖的 callable body。每项保存逻辑机器符号、
producer、typed implementation、模板／实例、可用的完整 exact signature 和声明来源。
泛型实例、重载与 adapter 仍是不同符号；显示名不参与身份判断。它包含可能被最终链接
去掉的正文，不是存活函数清单。声明位置是原始 byte span，**不是采样 PC 的执行行号**；
缺失来源保留 null。本能力不生成指令行表、DWARF 内联栈或 PGO 输入。

## macOS

对足够长的实际负载使用系统 `sample`，保留原始报告和程序输出：

```sh
tmp/profile/program > tmp/profile/stdout.txt 2> tmp/profile/stderr.txt &
profile_pid=$!
sample "$profile_pid" 5 1 -mayDie -file tmp/profile/sample.txt
wait "$profile_pid"
```

这里采样 5 秒、间隔 1 毫秒。负载必须覆盖采样窗口，可通过应用实际输入增加工作量；
不要把进程启动或等待当作稳态内核。`sample` 记录线程状态的时钟采样，后台线程的等待
帧不代表相应 CPU 消耗；读取报告时保留线程与调用树语义。

## Linux

使用本机已配置权限的 `perf`，对用户态 cycles 采样并记录已有物理调用栈：

```sh
perf record -e cycles:u -F 199 --call-graph fp \
  -o tmp/profile/perf.data -- tmp/profile/program
perf report --stdio --no-children --percent-limit 0 \
  -i tmp/profile/perf.data > tmp/profile/perf.txt
```

当前 Scoop 后端保留 frame pointer；原生库的调用栈仍受其自身构建方式限制。
若要使用 DWARF 展开，直接选择 perf 的 `--call-graph dwarf` 并单独测量该采集配置的开销。
perf.data、采集 stderr 和文本报告一同保留，用于核对事件、样本及丢失记录。
该 cycles 百分比不是墙钟时间百分比。PMU 的更深分析按具体问题选择事件。

## 给原始文本添加名字

```sh
python3 tests/benchmarks/symbolize.py \
  --symbols tmp/profile/dumps/ConeIdentity/symbols.json \
  --input tmp/profile/perf.txt --output tmp/profile/perf-named.txt \
  --binary tmp/profile/program --llvm-tools "$LLVM_SYS_221_PREFIX/bin"
```

`--symbols` 可重复传入，省略 output 时写 stdout。
工具仅匹配映射中的完整符号，兼容 Darwin 的单个目标下划线前缀；名称后保留原符号，
不修改计数、比例、地址偏移、线程或调用树，不按名称重新聚合。LLVM 克隆／冷分片、
native／runtime 和未知符号保持原样。声明 byte span 与具体实例签名可在 JSON 中按
保留的符号查询；无法展开的栈和缺失调试信息不补造。

perf 有时选择同址的 `scoop$1$bs$…` 定义起始符号。提供 binary 后，工具用 LLVM nm
读取实际符号地址，仅在同址 callable body 唯一时添加关联，保留原边界符号与对应 body
符号；多个正文同址时不猜测。某些 perf 版本为这类边界符号显示异常偏移，注释保留该
原始值，不将其解释成有效的指令偏移；需要 PC 归因时应读取原 perf.data 中的地址。

macOS 的 sample 会把一些同址符号显示为 `<deduplicated_symbol>`。显式传入本次构建的
Mach-O 程序，让配套 LLVM 工具从报告中的 PC 恢复符号：

```sh
python3 tests/benchmarks/symbolize.py \
  --symbols tmp/profile/dumps/ConeIdentity/symbols.json \
  --input tmp/profile/sample.txt --output tmp/profile/sample-named.txt \
  --binary tmp/profile/program --llvm-tools "$LLVM_SYS_221_PREFIX/bin"
```

省略 `--llvm-tools` 时从 PATH 使用 llvm-nm、llvm-readobj 和 llvm-symbolizer。对 sample
原始报告，工具自动用主镜像的
实际加载范围筛选 PC，以 __TEXT 地址还原 ASLR，再批量符号化。每行仅附加明确列出的
地址所对应的函数及偏移，原始 `<deduplicated_symbol>`、折叠计数与省略号均保留。
一个折叠帧可能覆盖多个地址，不能将该行总计数分别算到这些函数，也不能补造省略样本；
没有地址的汇总行保持原样。分析时需要更精细的分布，应重新选择采集工具或选项。

采样与普通 A/B 计时分别运行。先核对输出，再用相同程序和输入比较开启／关闭采样的
扰动；训练输入、反馈编译与 PGO 消费者不属于这个工作流。函数热点定位后，可用平台
反汇编工具或 `perf annotate` 检查具体指令；没有行表时只报告有证据的函数／指令范围。

## 启动阶段测量

显式构建带启动诊断的程序：

```sh
scoop build program.scoop --release --enable-startup-stats \
  -o "$PWD/tmp/profile/program-startup"
tmp/profile/program-startup > tmp/profile/stdout.txt 2> tmp/profile/startup.jsonl
```

每次启动在 eager 初始化之后、root gateway 之前输出一行 `scoop_startup:1` JSON。
`phases_ns` 分为 `loaded_images`、`collect_records`、`code_and_types`、
`storage_and_units`、`stackmaps`、`runtime_init`、`eager_init`，七项之和等于
`total_ns`；`counts` 直接取自已有注册索引，不额外遍历。应用自己的 stderr
可能出现在记录前后，按 JSON 的 `scoop_startup` 字段选择记录。

该时钟不含进入 runtime 前的 dyld／CRT、managed 参数构造、main、shutdown
及报告输出；完整进程耗时继续用 `compare.py` 外测，dyld 使用系统工具单独归因。
开启与关闭统计必须分别构建，再测量观测开销。默认产物在预处理阶段排除这套
计时、报告、诊断字符串和专用计数访问，不能通过环境变量启用。切换开关只改变
runtime 与最终链接，保留 Scoop Cone 的编译缓存；所有 profile 遵守同一规则。
