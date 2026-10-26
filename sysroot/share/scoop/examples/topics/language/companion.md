## 运行

在保存文件的目录执行：

```sh
scoop run --release main.scoop
```

## 预期结果

程序 stdout：

```text
origin=(0, 0)
diagonal=(3, 3)
```

## 关键说明

companion object 是依附于类型的单例对象。Point.origin() 和 Point.diagonal(3) 调用它的工厂函数，返回普通 Point 值；它们不是每个 Point 实例的成员。也可以通过 Point.Companion 明确引用对象。

需要作为公开能力使用时，companion 与函数分别声明 public。可以增加有业务意义的工厂，但实例数据仍由 Point 字段保存。JSON 的 companion codec 是同一机制的另一个用途。

## 常见误用

不要把 companion 当作会随每个实例复制的字段，也不要假设声明了类型就自动拥有某个工厂或 codec。
