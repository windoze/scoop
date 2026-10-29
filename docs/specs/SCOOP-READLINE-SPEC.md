# Scoop Readline 库规范

`scoop.readline` 是显式依赖的普通 Scoop Cone，以 rustyline 19.0.0 为移植基线。
编辑、历史、补全与布局使用 Scoop，复用 terminal 会话、charset Unicode 分段和
platform 文件 API。目标为 Darwin、Linux 和三个 Windows 环境；不包含 SQLite
backend 或 Rust derive 宏。不新增 intrinsic、runtime ABI 或编译器特殊处理。

## 1. 编辑与扩展

`Editor(config, history)` 保存配置、History、补全器、提示器、高亮器、验证器与
自定义按键。默认 FileHistory，不自动读写文件。`readLine(prompt, initial, cursor)`
返回文本；EOF/Ctrl-C 分别抛出 ReadlineEofException/ReadlineInterruptedException。
autoAddHistory 默认关闭。索引与回调位置使用 Unicode 标量 Long，移动和删除使用
扩展字素簇，保持组合字符、ZWJ emoji 与旗帜完整。

提供 Emacs 与 Vi insert/command/replace 模式、重复计数、字符/词/行移动、删除、
kill ring、yank/yank-pop、undo/redo、转置、单词大小写、历史导航与前缀/增量搜索、
候选循环/列表/fuzzy 筛选、提示接受、引用插入与多行编辑。Completer 返回替换起点与
display/replacement 候选，提供文件名补全。Hinter 返回显示与接受文本，提供历史
提示。Highlighter 分别处理输入、提示与候选，提供配对括号高亮。Validator 返回
Valid/Invalid/Incomplete；Incomplete 插入换行，Invalid 保留缓冲区和错误信息。
提供配对括号验证。EditingSession 是生产事件处理核心，测试直接驱动同一入口。
文件名补全使用目标平台的路径语法。Unix 保留 shell 风格反斜线转义；Windows
接受 `/` 和 `\` 分隔符，保留盘符、UNC 及 extended 路径中的反斜线，有空白等
分隔字符的未加引号候选使用双引号包围。已打开的引号保持，Windows 引号内以
连续两个相同引号表示一个字面引号，不把路径分隔符当作转义前缀。
Vi 的一次修改及其后连续 insert/replace 输入组成一个撤销单位，`.` 重放整个修改。
`.` 的显式计数覆盖上次修改的重复计数；连续插入文本作为整体重复。
Vi 的 j/k 在多行间移动，到达边界后继续导航历史；缩进操作消费对应 motion。
带计数的字符查找先定位第 n 个匹配，再应用 f/F/t/T 的包含或排除边界。
Fuzzy 模式以 Unicode case folding 后的子序列筛选候选，优先连续匹配，再保持提供方
顺序；方向键选择、Enter 插入、Esc/Ctrl-G 取消。它复用普通编辑循环，不依赖 skim。
自定义按键序列支持共享前缀，以最长匹配执行；等待超时后执行已有的完整匹配，
剩余按键回到普通键表。Editor 与 EditingSession 均可移除绑定。

非 TTY 输入使用普通 ByteSource 逐行读取。交互会话拥有终端资源，开启 bracketed
paste，所有正常/异常出口恢复模式。粘贴是一次可撤销编辑，控制字符可见绘制，
resize 重新布局。按字素簇宽度、制表位、列数计算折行，高亮序列不占显示宽度；
超过屏幕高度时显示包含光标的窗口。终端的实际字体可能改变 emoji 的视觉宽度。
Windows 复用 platform 的 console 会话，恢复实际输入／输出模式与代码页；真实
ConPTY 子进程覆盖交互、粘贴、resize、异常和外部输出。Unix job-control 暂停只在
Unix 提供；Windows 的 Suspend 命令在恢复会话后报告 PlatformErrorKind.Unsupported。
当单个字素宽于整个视口时，仅显示占一列的替代字符，编辑文本保持原值。

Editor.createExternalPrinter() 返回可跨线程共享的 ExternalPrinter。输入活动期间，
print 将消息排队，由同一终端循环清除当前输入、输出消息后恢复 prompt 和光标；
空闲时写标准输出。消息与队列由普通 Mutex 保护，终端输入仍只有一个消费者。
编辑器支持 Ctrl-Z：离开 raw/paste 作用域后请求 Unix job-control 暂停，继续时重绘。

## 2. History backend

History 是普通非泛型接口：size、isEmpty、get、add、clear、setMaxLength、
setIgnoreDuplicates、setIgnoreSpace、search、startsWith。Editor 仅依赖该接口，
允许自定义实现。PersistentHistory 继承 History 并增加 load/save/append；
MemoryHistory 不实现持久化接口，不暴露无作用或未实现的文件方法。

MemoryHistory 默认保留最近 100 项，忽略空字符串和连续重复项，默认保留空白开头。
ignoreSpace 使用 Unicode White_Space。最大长度零拒绝新项，负值报参数错误；
缩小长度保留最新项。get 越界返回 None。搜索包含起点，空 term 或越界返回 None。
substring 搜索结果位置为匹配起点，startsWith 为前缀末端。结果包含完整 String、
历史索引与匹配位置，读取结果不随后续删除改变。
historyCaseSensitive 默认 true；关闭后使用 Unicode case folding 匹配，返回位置仍为
原文的标量索引，前缀位置为原文中匹配部分的末端。

FileHistory 复用内存规则，记录未保存的新项。load 追加磁盘条目并将状态标记为
已保存；save 替换磁盘内容；append 在排他文件锁内重读磁盘、合并未保存项、应用
过滤和长度限制后写回，避免并发会话覆盖。读取/解析先完成再修改内存；成功写完
才清除未保存计数。clear 不删除磁盘文件。save 可以保存空历史；没有新增项的
append 不访问文件。创建权限请求 0600，受 umask 限制，不审计文件来源/所有者。

兼容 rustyline UTF-8 V1（每行一项）和首行 #V2 的格式；V2 转义换行与反斜杠，
未知或残缺转义保留整条原文。写出统一为 V2。接受 CRLF、无末尾换行和空文件，
无效 UTF-8 按平台文本读取异常处理。锁沿平台 advisory 契约，I/O 失败传播，
不承诺断电事务。

## 3. 验收

只使用所属 Cone 的 scoop.test，不新增 fixture。覆盖第三方 History、两种 backend、
V1/V2、Unicode、多行、过滤/长度/搜索、并发追加及失败重试，编辑核心的模式、撤销、
kill、补全/提示/验证/高亮与组合行为。按用户要求，遇到编译器、链接器及其他底层缺口时只修复实现，不为这些补缺新增正例或负例测试；文本后端与 readline 自身的公开功能仍由 scoop.test 验证。
