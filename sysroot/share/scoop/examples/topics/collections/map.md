## 运行

在保存文件的目录执行：

```sh
scoop run --release main.scoop
```

## 预期结果

程序 stdout：

```text
red=2
blue=1
missing=0
```

## 关键说明

LinkedHashMap 保留键首次插入顺序。下标读取返回 Int?；缺失时 `?: 0` 产生起始计数，再加一写回。遍历按条目解构出键和值。

String 可满足这里的键能力要求。自定义键需要按 Map 的 Hash/Equals 合同提供实现；不能因为 struct 可以使用结构 ==，就认为它自动满足这些接口。可以替换词列表，而无需改变缺失键处理。

## 常见误用

下标返回 Option，不能直接对它加一。需要确定顺序时使用此处的 LinkedHashMap，不要把插入顺序套用到所有字典实现。
