---
name: "audio-processing"
description: "使用 game-asset-cli 对游戏音频资源进行加工处理：格式转换（wav/ogg/flac/mp3）、音量调整与响度归一化、截取片段与淡入淡出、多音频拼接、无缝循环 BGM、查看音频元信息。纯 Rust 内核可独立运行（输出 WAV），检测到 ffmpeg 时解锁压缩格式。当用户需要处理 AI 生成或已有的音频素材（音效、BGM、语音）时使用。"
---

# 游戏音频资源加工（game-asset-cli）

通过本仓库内置的 `game-asset-cli` 对音频资源进行批处理加工，**优先使用 CLI 而非手写 ffmpeg 命令**。

CLI 路径：`game-asset-cli/target/release/game-asset-cli.exe`（本仓库 worktree 内）。

## 前置条件

1. **音频内核为纯 Rust 实现（symphonia 解码 + rubato 重采样 + hound WAV 编码），单个 exe 即可独立运行，无需安装任何外部工具。** 独立运行下输出格式为 WAV（无损）。
2. 需要输出 **mp3 / ogg / flac** 等压缩格式时，CLI 会自动探测 ffmpeg，按此顺序查找：`FFMPEG_PATH` 环境变量 → **game-asset-cli.exe 同目录 / 其 `bin/` 子目录** → 系统 PATH。把 ffmpeg 的 `ffmpeg.exe` 放到 exe 同目录（或 `bin/`）即可解锁全格式，无需安装到系统、无需设 PATH。
3. 转码已内置 `-vn` 自动剥离封面等视频流，输出的 ogg/mp3 保证是纯音频，无需额外处理。
4. 所有加工类子命令支持传入多个输入文件批量处理；单文件时可用 `-o` 指定输出路径，批量时使用 `--out-dir`（默认 `output/`）。

## 子命令一览

### audio convert — 格式转换

```bash
game-asset-cli audio convert <输入...> --format <wav|ogg|flac|mp3> [flags]
```

- `wav` 由内置纯 Rust 内核直接输出（独立运行可用）。
- `ogg / flac / mp3` 需要检测到 ffmpeg（见前置条件 2），否则报错并提示仅可输出 WAV。

| 参数 | 说明 |
|---|---|
| `--bitrate <rate>` | 目标码率，如 `128k`、`192k`（有损格式生效） |
| `--sample-rate <hz>` | 目标采样率，如 `44100`、`48000` |
| `--channels <n>` | 声道数：`1`（单声道，语音/音效用）或 `2` |
| `-o <path>` | 输出文件（仅单输入） |
| `--out-dir <dir>` | 输出目录（批量，默认 output/） |

### audio volume — 音量调整 / 响度归一化

```bash
game-asset-cli audio volume <输入...> (--gain <dB> | --factor <倍数> | --normalize) [flags]
```

| 参数 | 说明 |
|---|---|
| `--gain <dB>` | 增益，正放大负减小，如 `6` 或 `-3` |
| `--factor <f>` | 音量倍数，如 `1.5` |
| `--normalize` | EBU R128 响度归一化，统一多个音效响度 |
| `--target-lufs <v>` | 归一化目标响度，默认 `-16` LUFS |

### audio trim — 截取片段（支持淡入淡出）

```bash
game-asset-cli audio trim <输入...> [--start <秒>] (--duration <秒> | --end <秒>) [--fade-in <秒>] [--fade-out <秒>]
```

- `--fade-out` 必须与 `--duration` 或 `--end` 搭配（需要确定淡出起点）。
- 常用于从长 BGM 截取循环段、截取音效有效部分并加淡出防爆音。

### audio merge — 多音频拼接

```bash
game-asset-cli audio merge <输入1> <输入2> ... -o <输出> [--bitrate 192k]
```

- 按传入顺序拼接，自动统一采样率与声道；输出 `.wav` 由内核直接生成，压缩格式经 ffmpeg 转码。

### audio loop — 无缝循环 BGM

```bash
game-asset-cli audio loop <输入...> [--crossfade <秒>] [-o <输出>]
```

- `--crossfade`（默认 2 秒）：把结尾与开头做交叉淡化混合并裁掉多余尾部，消除循环接缝，制作可无缝循环的 BGM。
- 纯 Rust 内核实现，独立运行可用。

### audio info — 查看元信息

```bash
game-asset-cli audio info <输入...>
```

输出编码格式、采样率、声道、时长、码率、文件大小。

## 典型游戏场景示例

```bash
# AI 生成的 wav 音效批量转 ogg（减小体积，游戏引擎通用）
game-asset-cli audio convert sfx_01.wav sfx_02.wav sfx_03.wav --format ogg --bitrate 128k --out-dir assets/audio/

# 统一一批音效的响度，避免游戏中忽大忽小
game-asset-cli audio volume assets/audio/*.ogg --normalize --out-dir assets/audio/norm/

# 语音转成单声道 22050Hz 减小体积
game-asset-cli audio convert voice.mp3 --format ogg --channels 1 --sample-rate 22050 -o assets/voice_intro.ogg

# 从 3 分钟 AI BGM 截取 60 秒循环段，首尾淡化便于无缝循环
game-asset-cli audio trim bgm_full.mp3 --start 30 --duration 60 --fade-in 1 --fade-out 2 -o assets/bgm_loop.wav

# 一键制作无缝循环 BGM（首尾交叉淡化，消除接缝）
game-asset-cli audio loop bgm_full.wav --crossfade 3 -o assets/bgm_seamless.wav

# 单个音效音量提升 6dB
game-asset-cli audio volume hit.wav --gain 6 -o assets/hit_loud.wav

# 把多个短音效拼接成连击音效
game-asset-cli audio merge hit1.wav hit2.wav hit3.wav -o assets/combo.ogg --bitrate 160k
```

## 执行指引

1. 音频加工任务一律优先调用 `game-asset-cli audio ...`，不要自行拼 ffmpeg 命令。
2. 用户未指定输出位置时，使用默认 `output/` 目录或 `--out-dir assets/` 之类的资源目录。
3. 执行前先运行一次 `audio info` 了解源素材参数（时长、采样率），再决定截取区间或目标格式。
4. 命令失败时原样向用户展示错误；若提示需要 ffmpeg（输出压缩格式），引导用户把 `ffmpeg.exe` 放到 exe 同目录（或 `bin/` 子目录）后重试。
5. 执行成功后向用户报告输出文件路径及关键参数（时长、大小、格式）。
