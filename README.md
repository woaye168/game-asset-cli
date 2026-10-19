# game-asset-cli

游戏资源加工命令行工具：对 AI 生成或已有的**音频 / 图像**素材做批处理。纯 Rust 内核可独立运行（音频输出 WAV、图像全功能），检测到 ffmpeg 时解锁音频压缩格式输出（ogg / flac / mp3）。

## 构建

```bash
cargo build --release
```

产物为单个 `game-asset-cli.exe`，拷贝即用。

## ffmpeg 配置（可选，仅音频压缩格式需要）

按以下顺序自动探测：

1. `FFMPEG_PATH` 环境变量
2. exe 同目录 / 其 `bin/` 子目录
3. 系统 PATH

把 `ffmpeg.exe` 放到 exe 同目录（或 `bin/`）即可。内置转码自动带 `-vn` 剥离封面等视频流，保证 ogg 为纯音频。

## 音频命令

```bash
# 格式转换（批量用 --out-dir，单文件用 -o）
game-asset-cli audio convert <输入...> --format <wav|ogg|flac|mp3> [--bitrate 128k] [--sample-rate 44100] [--channels 1]

# 音量调整 / EBU R128 响度归一化
game-asset-cli audio volume <输入...> (--gain <dB> | --factor <倍数> | --normalize [--target-lufs -16])

# 截取片段，支持淡入淡出
game-asset-cli audio trim <输入...> [--start 秒] (--duration 秒 | --end 秒) [--fade-in 秒] [--fade-out 秒]

# 多音频按顺序拼接
game-asset-cli audio merge <输入...> -o <输出> [--bitrate 192k]

# 无缝循环 BGM（首尾交叉淡化消除接缝，纯 Rust 实现）
game-asset-cli audio loop <输入...> [--crossfade 2] [-o 输出]

# 查看元信息
game-asset-cli audio info <输入...>
```

## 图像命令

```bash
game-asset-cli image convert <输入...> --format <png|jpg|webp|bmp|tga|ico> [--quality 90]
game-asset-cli image resize <输入...> (--width px --height px | --scale 倍数)
game-asset-cli image crop <输入...> --x px --y px --width px --height px
game-asset-cli image trim <输入...>                 # 自动裁透明边缘
game-asset-cli image pad <输入...> [--anchor 锚点] [--pot]   # 扩展画布，可对齐 2 的幂
game-asset-cli image slice <输入...> --tile-width px --tile-height px   # 精灵表切割
game-asset-cli image pack <输入...> --cols n -o <输出>        # 散帧打成精灵表（slice 逆操作）
game-asset-cli image chroma <输入...> --color '#00FF00' [--tolerance 30]  # 纯色背景抠图
game-asset-cli image quantize <输入...> --colors 16 [--dither]            # 调色板减色
game-asset-cli image info <输入...>
```

## 技术栈

- 音频：symphonia 解码 + rubato 重采样 + hound WAV 编码（纯 Rust），压缩格式经 ffmpeg 转码
- 图像：image crate（png / jpeg / webp / gif / bmp / tga / ico / pnm）
- CLI：clap derive
