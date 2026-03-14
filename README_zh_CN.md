# assfonts

简体中文 | [English](README.md)

一个用 Rust 编写的 ASS 字幕字体索引、匹配、子集化和嵌入的命令行工具。

## 功能特性

- **字体索引**：扫描字体目录并生成 JSON 索引文件，便于快速查询
- **智能字体匹配**：根据 ASS 字体请求匹配可用字体，支持粗体/斜体样式
- **字体子集化**：仅提取所需的字形，大幅减小字体文件体积
- **字体嵌入**：将子集化后的字体直接嵌入到 ASS 文件的 `[Fonts]` 区域
- **多文件处理**：并行处理多个 ASS 文件
- **TTC/OTC 支持**：支持从 TrueType/OpenType Collection 字体中提取单个字体面

## 安装

### 从源码编译

```bash
git clone https://github.com/kotorimiku/assfonts.git
cd assfonts
cargo build --release
```

## 使用方法

### 构建字体索引

生成字体索引文件以便快速查询：

```bash
assfonts build -f /path/to/fonts -d /path/to/db
```

这将在指定目录中创建 `fonts.index.json` 文件。

参数：
- `-f, --fontpath`：要扫描的字体目录（必需，支持多个路径）
- `-o, --output`：`fonts.index.json` 的输出目录（默认：当前目录）

### 处理 ASS 文件

将字体嵌入到 ASS 文件中：

```bash
assfonts -i input.ass -o output_dir -f /path/to/fonts
```

处理多个文件或目录：

```bash
assfonts -i /path/to/ass/files -o output_dir -f /path/to/fonts -d /path/to/db
```

参数：
- `-i, --input`：输入 ASS 文件或目录（必需，支持多个路径）
- `-o, --output`：输出目录（默认：当前目录）
- `-f, --fontpath`：要扫描的字体目录（可选）
- `-d, --dbpath`：包含 `fonts.index.json` 的目录（默认：当前目录）
- `-s, --strict`：遇到字体错误时失败（默认：true）
- `--allow-missing-sample`：允许缺失字符样本
- `--allow-missing-fonts`：允许缺失字体
- `--report`：处理后生成 `run-report.json` 报告
- `--force`：覆盖已存在的输出文件

## 示例

### 基本用法

```bash
# 处理单个 ASS 文件
assfonts -i video.ass -o ./out -f C:/Windows/Fonts

# 处理目录中的所有 ASS 文件
assfonts -i ./subtitles -o ./output -f /usr/share/fonts

# 使用预构建的字体索引
assfonts build -f /usr/share/fonts -d ~/.assfonts
assfonts -i video.ass -o ./out -d ~/.assfonts
```

### 生成报告

```bash
assfonts -i video.ass -o ./out -f C:/Windows/Fonts --report
```

报告（`run-report.json`）包含：
- 缺失字体列表
- 字形覆盖率统计
- 字体文件体积压缩信息

### 宽松模式

允许缺失字体并继续处理：

```bash
assfonts -i video.ass -o ./out -f C:/Windows/Fonts --allow-missing-fonts --allow-missing-sample
```

## 相关项目

- [assfonts](https://github.com/wyzdwdz/assfonts) - 原始 C++ 实现

## 许可证

MIT License - 详见 [LICENSE](LICENSE)。
