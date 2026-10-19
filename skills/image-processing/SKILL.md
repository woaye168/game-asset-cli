---
name: "image-processing"
description: "使用 game-asset-cli 对游戏图像资源进行加工处理：格式转换（png/jpg/webp/tga 等）、缩放、裁剪、自动裁透明边缘、画布扩展与 2 的幂对齐、精灵表切割、纯色背景抠图、查看图像信息。当用户需要处理 AI 生成或已有的图像素材（精灵图、贴图、UI 图素、精灵表）时使用。"
---

# 游戏图像资源加工（game-asset-cli）

通过本仓库内置的 `game-asset-cli` 对图像资源进行批处理加工（纯 Rust 实现，无外部依赖）。

CLI 路径：`game-asset-cli/target/release/game-asset-cli.exe`（本仓库 worktree 内）。

## 前置条件

1. 图像处理为纯 Rust 实现，无需安装任何外部工具。
2. 所有加工类子命令（slice 除外）支持传入多个输入文件批量处理；单文件时可用 `-o` 指定输出路径，批量时使用 `--out-dir`（默认 `output/`）。

## 子命令一览

### image convert — 格式转换

```bash
game-asset-cli image convert <输入...> --format <png|jpg|webp|bmp|tga|ico> [--quality 1-100]
```

- `--quality` 仅 jpg 生效（默认 90）；webp 输出为无损。
- jpg 不支持透明通道，含 alpha 的图转 jpg 会丢失透明。

### image resize — 缩放

```bash
game-asset-cli image resize <输入...> [--width <px>] [--height <px>] [--scale <比例>] [--filter <算法>]
```

- 单独指定宽或高时等比缩放；`--scale 0.5` 表示缩小一半。
- 采样算法：`nearest`（像素风游戏保边缘）、`triangle`、`catmullrom`、`gaussian`、`lanczos3`（默认，质量最高）。

### image crop — 裁剪区域

```bash
game-asset-cli image crop <输入...> [--x <px>] [--y <px>] [--width <px>] [--height <px>]
```

- 宽高缺省时裁剪到右/下边缘。

### image trim-alpha — 自动裁透明边缘

```bash
game-asset-cli image trim-alpha <输入...> [--threshold 0-255] [--padding <px>]
```

- AI 生成精灵图常有大量透明空白边框，此命令自动裁掉，`--padding` 可保留少量留白。
- 场景：AI 出图 → trim-alpha → resize，得到干净的游戏 sprite。

### image pad — 画布扩展 / 2 的幂对齐

```bash
game-asset-cli image pad <输入...> [--width <px> --height <px> | --pot] [--anchor <锚点>] [--background <RRGGBB[AA]>]
```

- `--pot` 自动扩展到大于等于原图的最近 2 的幂尺寸（如 1000x700 → 1024x1024），老式引擎/纹理压缩常用。
- 锚点：`center`（默认）、`top-left`、`top`、`top-right`、`left`、`right`、`bottom-left`、`bottom`、`bottom-right`。
- 背景默认透明。

### image slice — 精灵表切割

```bash
game-asset-cli image slice <输入> (--tile-width <px> --tile-height <px> | --cols <n> --rows <n>) [--margin <px>] [--spacing <px>] [--out-dir <dir>] [--prefix <前缀>]
```

- 按瓦片尺寸或行列数均分切割，输出 `<前缀>_行_列.png`。
- `--margin`：图像四周边距（Tiled margin）；`--spacing`：相邻瓦片间距（Tiled spacing），处理带黑线/间隔的瓦片图（如 tmw_desert_spacing.png）时必用。
- 图像尺寸不是瓦片整数倍时会提示并忽略余量。

### image remove-color — 纯色背景抠图

```bash
game-asset-cli image remove-color <输入...> [--color RRGGBB] [--tolerance 0-255] [--soften]
```

- 不指定 `--color` 时自动取左上角像素作为背景色。
- `--tolerance` 控制颜色相近范围（默认 30）；`--soften` 让边缘半透明过渡更自然。
- 输出始终为 png（保留 alpha）。AI 出图纯色背景去除的核心命令。

### image pack — 图集打包（slice 的逆操作）

```bash
game-asset-cli image pack <输入...> -o <atlas.png> [--cols <n>] [--spacing <px>] [--margin <px>] [--cell <px>] [--json]
```

- 把多张散帧/瓦片按网格打成一个精灵表；`--cols` 缺省按 √n 取近似正方形布局。
- `--cell` 把所有瓦片统一缩放到指定正方形尺寸（缺省取最大瓦片尺寸）。
- `--json` 同时输出同名 `.json` 元数据（含每帧 x/y/w/h），可直接被游戏引擎/代码读取。

### image quantize — 调色板量化/减色

```bash
game-asset-cli image quantize <输入...> [--colors <2-256>] [--dither] [-o <输出>]
```

- 把图像降到 N 色（默认 16），像素风素材常用；`--dither` 启用 Floyd–Steinberg 抖动让减色过渡更平滑。
- 输出为 png。

### image info — 查看图像信息

```bash
game-asset-cli image info <输入...>
```

输出尺寸、色彩类型、是否含透明通道、是否为 2 的幂尺寸、文件大小。

## 典型游戏场景示例

```bash
# AI 生成的纯色背景角色图抠成透明 sprite
game-asset-cli image remove-color hero.jpg --tolerance 40 --soften -o assets/hero.png

# 抠图 → 裁透明边 → 缩放到目标尺寸，一条龙
game-asset-cli image remove-color hero.jpg -o tmp/hero.png
game-asset-cli image trim-alpha tmp/hero.png --padding 4 -o tmp/hero_trim.png
game-asset-cli image resize tmp/hero_trim.png --height 128 --filter lanczos3 -o assets/hero_128.png

# 批量把 UI 图素转成 webp 减小包体
game-asset-cli image convert ui_01.png ui_02.png ui_03.png --format webp --out-dir assets/ui/

# 像素风素材放大 4 倍保持硬边
game-asset-cli image resize tileset.png --scale 4 --filter nearest -o assets/tileset_4x.png

# 把 1024x1024 精灵表切成 16 帧（4x4）
game-asset-cli image slice walk_sheet.png --cols 4 --rows 4 --out-dir assets/walk/ --prefix walk

# 非 2 的幂贴图对齐到引擎要求
game-asset-cli image pad bg_1000x700.png --pot --anchor center -o assets/bg_pot.png

# 九宫格/UI 底板制作：裁出有效区域
game-asset-cli image crop panel.png --x 100 --y 50 --width 400 --height 300 -o assets/panel_core.png
```

## 执行指引

1. 图像加工任务一律优先调用 `game-asset-cli image ...`。
2. 用户未指定输出位置时使用默认 `output/` 目录。
3. 处理前先用 `image info` 查看源图尺寸与通道信息，再决定参数（如是否需要抠图、是否已是 2 的幂）。
4. 抠图后必须输出 png；批量同名文件写到同一 `--out-dir` 且扩展名相同时会互相覆盖，需要保留多步中间结果时使用 `-o` 指定不同文件名。
5. 执行成功后向用户报告输出路径与关键信息（尺寸变化、瓦片数量等），图片可直接展示。
