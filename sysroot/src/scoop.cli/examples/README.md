# scoop.cli 示例

先完成[运行准备](../../../README.md#运行准备)，以下命令在本目录执行。

| 文件 | 内容 |
| --- | --- |
| [greet.scoop](greet.scoop) | 位置参数、短选项、Boolean flag、默认值和自动帮助 |
| [repeated_options.scoop](repeated_options.scoop) | 重复选项、枚举选择和可选输出路径 |

```sh
scoop run --release greet.scoop -- Ada --count 2 --excited
scoop run --release greet.scoop -- --help
scoop run --release repeated_options.scoop -- -Isrc -Iinclude --mode Thorough -o result.txt
scoop run --release repeated_options.scoop
```

问候程序的参数、帮助和错误退出见 `scoop example cli/options`（[正文](../../../share/scoop/examples/topics/cli/options.md)）。

重复选项示例只打印解析结果，不访问指定目录或写文件；不传参时输出 `mode=Fast` 和 `output=stdout`。
