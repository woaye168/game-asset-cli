use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod audio;
mod image_ops;
mod util;

#[derive(Parser, Debug)]
#[command(
    name = "game-asset-cli",
    version,
    about = "AI 生成的游戏资源加工 CLI 工具（音频 / 图像批处理）",
    long_about = "面向游戏场景的音频与图像资源加工工具。\n音频处理依赖 ffmpeg/ffprobe（需安装并在 PATH 中）；图像处理为纯 Rust 实现。\n所有加工类子命令均支持传入多个输入文件进行批量处理。"
)]
struct Cli {
    #[command(subcommand)]
    command: TopCommand,
}

#[derive(Subcommand, Debug)]
enum TopCommand {
    #[command(subcommand, about = "音频资源处理（格式转换 / 音量 / 截取 / 拼接 / 信息）")]
    Audio(AudioCommand),
    #[command(subcommand, about = "图像资源处理（格式转换 / 缩放 / 裁剪 / 抠图 / 精灵表切割 / 信息）")]
    Image(ImageCommand),
}

#[derive(Subcommand, Debug)]
enum AudioCommand {
    #[command(about = "音频格式转换（wav/ogg/flac/mp3），可指定码率、采样率、声道")]
    Convert {
        #[arg(required = true, help = "输入音频文件（支持多个，批量处理）")]
        inputs: Vec<PathBuf>,
        #[arg(short, long, help = "目标格式：wav（独立运行）、ogg/flac/mp3（需 exe 旁 bin/ 或 PATH 中有 ffmpeg）")]
        format: String,
        #[arg(short, long, help = "输出文件路径（仅单输入时可用）")]
        output: Option<PathBuf>,
        #[arg(short, long, default_value = "output", help = "输出目录（批量时使用）")]
        out_dir: PathBuf,
        #[arg(short, long, help = "目标码率，如 128k、192k（有损格式生效）")]
        bitrate: Option<String>,
        #[arg(long, help = "目标采样率，如 44100、48000")]
        sample_rate: Option<u32>,
        #[arg(long, help = "声道数：1（单声道，语音/音效常用）、2（立体声）")]
        channels: Option<u8>,
    },
    #[command(about = "调整音量：增益（dB 或倍数）或响度归一化")]
    Volume {
        #[arg(required = true, help = "输入音频文件（支持多个）")]
        inputs: Vec<PathBuf>,
        #[arg(short, long, allow_hyphen_values = true, help = "增益（dB），正数放大、负数减小，如 6 或 -3")]
        gain: Option<f64>,
        #[arg(short, long, help = "音量倍数，如 1.5 表示放大到 1.5 倍")]
        factor: Option<f64>,
        #[arg(long, help = "响度归一化（EBU R128 loudnorm），统一多个音效的响度")]
        normalize: bool,
        #[arg(long, default_value_t = -16.0, help = "归一化目标响度（LUFS），默认 -16")]
        target_lufs: f64,
        #[arg(short, long, help = "输出文件路径（仅单输入时可用）")]
        output: Option<PathBuf>,
        #[arg(short, long, default_value = "output", help = "输出目录（批量时使用）")]
        out_dir: PathBuf,
    },
    #[command(about = "截取音频片段，支持淡入淡出（制作循环 BGM / 截取音效）")]
    Trim {
        #[arg(required = true, help = "输入音频文件（支持多个）")]
        inputs: Vec<PathBuf>,
        #[arg(short, long, default_value_t = 0.0, help = "起始时间（秒），也支持 00:01:23.5 格式")]
        start: f64,
        #[arg(short, long, help = "截取时长（秒），与 --end 二选一")]
        duration: Option<f64>,
        #[arg(short, long, help = "结束时间（秒），与 --duration 二选一")]
        end: Option<f64>,
        #[arg(long, help = "淡入时长（秒）")]
        fade_in: Option<f64>,
        #[arg(long, help = "淡出时长（秒）")]
        fade_out: Option<f64>,
        #[arg(short, long, help = "输出文件路径（仅单输入时可用）")]
        output: Option<PathBuf>,
        #[arg(short, long, default_value = "output", help = "输出目录（批量时使用）")]
        out_dir: PathBuf,
    },
    #[command(about = "将多个音频按顺序拼接为一个（串联合成 BGM / 连击音效）")]
    Merge {
        #[arg(required = true, help = "输入音频文件，按传入顺序拼接")]
        inputs: Vec<PathBuf>,
        #[arg(short, long, required = true, help = "输出文件路径")]
        output: PathBuf,
        #[arg(short, long, help = "输出码率，如 192k")]
        bitrate: Option<String>,
    },
    #[command(about = "制作无缝循环 BGM：交叉淡化首尾，消除循环接缝")]
    Loop {
        #[arg(required = true, help = "输入音频文件（支持多个）")]
        inputs: Vec<PathBuf>,
        #[arg(short, long, default_value_t = 2.0, help = "交叉淡化时长（秒），用于把结尾混入开头实现无缝循环")]
        crossfade: f64,
        #[arg(short, long, help = "输出文件路径（仅单输入时可用）")]
        output: Option<PathBuf>,
        #[arg(short, long, default_value = "output", help = "输出目录（批量时使用）")]
        out_dir: PathBuf,
    },
    #[command(about = "查看音频元信息（时长、采样率、声道、码率、编码格式）")]
    Info {
        #[arg(required = true, help = "输入音频文件（支持多个）")]
        inputs: Vec<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
enum ImageCommand {
    #[command(about = "图像格式转换（png/jpg/webp/bmp/tga/ico），可指定压缩质量")]
    Convert {
        #[arg(required = true, help = "输入图像文件（支持多个）")]
        inputs: Vec<PathBuf>,
        #[arg(short, long, help = "目标格式：png、jpg、webp、bmp、tga、ico")]
        format: String,
        #[arg(short, long, help = "压缩质量 1-100（jpg 生效；webp 默认无损）")]
        quality: Option<u8>,
        #[arg(short, long, help = "输出文件路径（仅单输入时可用）")]
        output: Option<PathBuf>,
        #[arg(short, long, default_value = "output", help = "输出目录（批量时使用）")]
        out_dir: PathBuf,
    },
    #[command(about = "缩放图像：按目标尺寸或缩放比例")]
    Resize {
        #[arg(required = true, help = "输入图像文件（支持多个）")]
        inputs: Vec<PathBuf>,
        #[arg(long, help = "目标宽度（px），单独指定时等比缩放")]
        width: Option<u32>,
        #[arg(long, help = "目标高度（px），单独指定时等比缩放")]
        height: Option<u32>,
        #[arg(short, long, help = "缩放比例，如 0.5 表示缩小一半（与宽高参数二选一）")]
        scale: Option<f64>,
        #[arg(long, default_value = "lanczos3", help = "采样算法：nearest（像素风）、triangle、catmullrom、gaussian、lanczos3")]
        filter: String,
        #[arg(short, long, help = "输出文件路径（仅单输入时可用）")]
        output: Option<PathBuf>,
        #[arg(short, long, default_value = "output", help = "输出目录（批量时使用）")]
        out_dir: PathBuf,
    },
    #[command(about = "裁剪图像指定区域")]
    Crop {
        #[arg(required = true, help = "输入图像文件（支持多个）")]
        inputs: Vec<PathBuf>,
        #[arg(long, default_value_t = 0, help = "裁剪区域左上角 X")]
        x: u32,
        #[arg(long, default_value_t = 0, help = "裁剪区域左上角 Y")]
        y: u32,
        #[arg(long, help = "裁剪宽度（px），缺省裁剪到右边缘")]
        width: Option<u32>,
        #[arg(long, help = "裁剪高度（px），缺省裁剪到下边缘")]
        height: Option<u32>,
        #[arg(short, long, help = "输出文件路径（仅单输入时可用）")]
        output: Option<PathBuf>,
        #[arg(short, long, default_value = "output", help = "输出目录（批量时使用）")]
        out_dir: PathBuf,
    },
    #[command(about = "自动裁剪透明边缘（AI 生成精灵图去空白边框）")]
    TrimAlpha {
        #[arg(required = true, help = "输入图像文件（支持多个）")]
        inputs: Vec<PathBuf>,
        #[arg(short, long, default_value_t = 0, help = "alpha 阈值 0-255，大于该值视为有效像素")]
        threshold: u8,
        #[arg(short, long, default_value_t = 0, help = "裁剪后保留的边缘留白（px）")]
        padding: u32,
        #[arg(short, long, help = "输出文件路径（仅单输入时可用）")]
        output: Option<PathBuf>,
        #[arg(short, long, default_value = "output", help = "输出目录（批量时使用）")]
        out_dir: PathBuf,
    },
    #[command(about = "扩展画布：按锚点对齐放置，可自动对齐到 2 的幂尺寸（贴图导入游戏引擎前常用）")]
    Pad {
        #[arg(required = true, help = "输入图像文件（支持多个）")]
        inputs: Vec<PathBuf>,
        #[arg(long, help = "目标宽度（px），需不小于原图")]
        width: Option<u32>,
        #[arg(long, help = "目标高度（px），需不小于原图")]
        height: Option<u32>,
        #[arg(long, help = "自动扩展到大于等于原图的最近 2 的幂尺寸")]
        pot: bool,
        #[arg(long, default_value = "center", help = "锚点：center、top-left、top、top-right、left、right、bottom-left、bottom、bottom-right")]
        anchor: String,
        #[arg(long, help = "背景色（RRGGBB 或 RRGGBBAA 十六进制），默认透明")]
        background: Option<String>,
        #[arg(short, long, help = "输出文件路径（仅单输入时可用）")]
        output: Option<PathBuf>,
        #[arg(short, long, default_value = "output", help = "输出目录（批量时使用）")]
        out_dir: PathBuf,
    },
    #[command(about = "切割精灵表为单个帧/瓦片（sprite sheet slicing）")]
    Slice {
        #[arg(help = "输入精灵表图像")]
        input: PathBuf,
        #[arg(long, required_unless_present = "cols", help = "单个瓦片宽度（px）")]
        tile_width: Option<u32>,
        #[arg(long, required_unless_present = "rows", help = "单个瓦片高度（px）")]
        tile_height: Option<u32>,
        #[arg(long, help = "按列数均分切割（与 --tile-width 二选一）")]
        cols: Option<u32>,
        #[arg(long, help = "按行数均分切割（与 --tile-height 二选一）")]
        rows: Option<u32>,
        #[arg(long, default_value_t = 0, help = "外边距（px），图像四周边缘跳过的宽度（Tiled margin）")]
        margin: u32,
        #[arg(long, default_value_t = 0, help = "瓦片间距（px），相邻瓦片之间跳过的宽度（Tiled spacing）")]
        spacing: u32,
        #[arg(short, long, default_value = "output", help = "瓦片输出目录")]
        out_dir: PathBuf,
        #[arg(long, default_value = "tile", help = "输出文件名前缀，生成 tile_行_列.png")]
        prefix: String,
    },
    #[command(about = "纯色背景抠图：将指定颜色（及容差内相近色）转为透明（AI 出图纯色背景去除）")]
    RemoveColor {
        #[arg(required = true, help = "输入图像文件（支持多个）")]
        inputs: Vec<PathBuf>,
        #[arg(short, long, help = "要去除的颜色（RRGGBB 十六进制），默认取左上角像素颜色")]
        color: Option<String>,
        #[arg(short, long, default_value_t = 30, help = "颜色容差 0-255，越大去除范围越广")]
        tolerance: u8,
        #[arg(long, help = "边缘柔化：容差边缘像素按距离比例半透明")]
        soften: bool,
        #[arg(short, long, help = "输出文件路径（仅单输入时可用）")]
        output: Option<PathBuf>,
        #[arg(short, long, default_value = "output", help = "输出目录（批量时使用）")]
        out_dir: PathBuf,
    },
    #[command(about = "图集打包：把多张散帧/瓦片按网格打成一个精灵表（slice 的逆操作）")]
    Pack {
        #[arg(required = true, help = "输入图像文件（支持多个，按传入顺序从左到右、从上到下排列）")]
        inputs: Vec<PathBuf>,
        #[arg(short, long, required = true, help = "输出图集路径（如 atlas.png）")]
        output: PathBuf,
        #[arg(long, help = "列数（缺省按 √n 自动取近似正方形布局）")]
        cols: Option<u32>,
        #[arg(long, default_value_t = 0, help = "瓦片间距（px）")]
        spacing: u32,
        #[arg(long, default_value_t = 0, help = "图集四周外边距（px）")]
        margin: u32,
        #[arg(long, help = "背景色（RRGGBB 或 RRGGBBAA），默认透明")]
        background: Option<String>,
        #[arg(long, help = "把所有瓦片统一缩放到该尺寸（px，正方形），缺省取最大瓦片尺寸")]
        cell: Option<u32>,
        #[arg(long, help = "同时输出同名 .json 图集元数据（帧坐标）")]
        json: bool,
    },
    #[command(about = "调色板量化/减色：把图像降到 N 色（像素风常用），可选有序抖动")]
    Quantize {
        #[arg(required = true, help = "输入图像文件（支持多个）")]
        inputs: Vec<PathBuf>,
        #[arg(short, long, default_value_t = 16, help = "目标颜色数（2-256）")]
        colors: u32,
        #[arg(long, help = "启用 Floyd–Steinberg 抖动（减色更平滑）")]
        dither: bool,
        #[arg(short, long, help = "输出文件路径（仅单输入时可用）")]
        output: Option<PathBuf>,
        #[arg(short, long, default_value = "output", help = "输出目录（批量时使用）")]
        out_dir: PathBuf,
    },
    #[command(about = "查看图像信息（尺寸、色彩类型、是否含透明通道）")]
    Info {
        #[arg(required = true, help = "输入图像文件（支持多个）")]
        inputs: Vec<PathBuf>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        TopCommand::Audio(cmd) => audio::run(cmd),
        TopCommand::Image(cmd) => image_ops::run(cmd),
    }
}
