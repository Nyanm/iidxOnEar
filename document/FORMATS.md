# IIDX 文件格式逆向笔记 (script 阶段产物)

用 Python 试水验证过的三种容器格式，将来移植到 Rust。所有偏移均小端。

## .2dx — 预览音频容器 (RIFF/WAVE)

```
头部:
  0x00  char[16]   name              "<id>_pre" 等名字 + 0 填充
  0x10  uint32     header_size       = 偏移表结束 = 首个 2DX9 块偏移
  0x14  uint32     file_count        块数量 (预览通常为 1)
  0x18..           padding(0)
  ????             uint32[file_count]  各 2DX9 块的文件内绝对偏移 (本例在 0x48)
每个 2DX9 块:
  +0x00 char[4]    "2DX9"
  +0x04 uint32     block_header_size 块头大小 (0x18)
  +0x08 uint32     data_size         RIFF/WAVE 字节数
  +0x0C ...        其余块头 (track id / 音量 等)
  +block_header_size  RIFF/WAVE 数据
```
内层是标准 RIFF/WAVE, **fmt_tag=2 (MS-ADPCM)**, 立体声 44100Hz。ffmpeg 可直接解码。
验证: 0x64 + 0x7105A == 文件大小 0x710BE。
稳健解法: 扫描全文件 `2DX9` magic, 按各自块头切出 WAV (不依赖偏移表位置)。

## .s3p — 主音频归档 (ASF/WMA)

```
  0x00  char[4]    "S3P0"
  0x04  uint32     count             条目数 (= 一首歌的全部音源: 1 个全曲 + N 个 keysound)
  0x08..           count × (uint32 offset, uint32 size)   条目表
每个条目 -> S3V0 块:
  +0x00 char[4]    "S3V0"
  +0x04 uint32     header_size       0x20
  +0x08 uint32     data_size         = 条目 size - 0x20
  +0x0C ...        其余块头
  +0x20            payload = ASF/WMA (GUID 3026B2758E66CF11...)  即 SDVX 的 .s3v 同格式
```
**关键**: 每个 .s3p 恰有 1 个超大条目 (2.6~4.8MB = 预混全曲), 其余几百个是 keysound (中位 ~17KB)。
取**最大条目**即为可听的完整曲目 -> WMA -> ffmpeg 转码 (与 SDVX 路径一致)。
样本: 30000.s3p=1186 条 (最大 4766KB); 28000.s3p=609 条 (最大 2662KB)。

## .ifs — Konami 打包 (每首歌一个)

magic `0x6CAD8F89`; 头后跟 KBin (二进制 XML) 清单, 再跟打包数据。复杂, 现用 `ifstools` 解。
28000.ifs 解出 3 个文件:
- `28000.s3p`      主音频 (见上)
- `28000_pre.2dx`  预览音频 (见上)
- `28000.1`        谱面/note 数据 (12 难度的 offset/len 表), **与音频无关**

### Rust 移植策略: 轻量 magic 扫描 (已验证, 见 probe_ifs_scan.py)

不移植 KBin。在 .ifs 原始字节中:
1. 扫 `S3P0` magic — 实测每个 .ifs **恰好出现 1 次** (28000.ifs @0xC0), 无假阳性; `2DX9` 同样唯一。
2. 在该处读 `count` + 条目表, 找 size 最大的条目, 按其 (offset,size) 直接切出 S3V0 块 -> 去 0x20 头 -> WMA。
3. **无需计算 s3p 总长度** — 只按条目表定位最大块即可。

验证: 从 28000.ifs 直接 magic 扫描切出的全曲 WMA 与走 ifstools 解出再切的 WMA **逐字节完全相同**。
(注: ifstools 导出的 .s3p 比按 max(offset+size) 多 4 字节, 是 16 字节对齐填充, 与音频无关。)

## 音频源分布 (按版本, scan 实测) —— Step 3 解包要点

主音频容器随版本分两族, 但统一处理 = "定位容器 -> 取最大块 -> WMA/WAV -> ffmpeg":

| 来源 | 容器 | 内层块 | 全曲 = |
|---|---|---|---|
| v25+ 散放 `<id5>/<id5>.s3p` (v30+) 或打包 `<id5>.ifs` 内 S3P0 | S3P0 | S3V0 裹 WMA | **最大 S3V0 块** |
| v01-24 打包 `<id5>.ifs` (无 S3P0, 只有一串 2DX9) | 2DX9 序列 | 2DX9 裹 RIFF/WAVE(ADPCM) | **最大 2DX9 块** (实测 ~90-140s 全曲) |
| omni 老歌散放 `<id5>/<id5>*.2dx` | 2DX9 序列 | 同上 | **最大 2DX9 块** |

- `.ifs` magic 扫描顺序: **先 S3P0, 没有再 2DX9** (v25+ 同时含 1 个 S3P0 主音频 + 1 个 2DX9 预览, 故 S3P0 优先才对)。
- 散放 `.2dx` 多音源后缀: 数字 (`<id5>1/2`) 或难度字母 (`<id5>n/h/a/b/7`...); scan 取排序首个 (无后缀优先)。
- `-p0.ifs` (62 个): 无 S3P0 无 2DX9, ~160KB, **非音频** (谱面/辅助), 忽略。
- 边角: 个别 omni 老歌 (如 01011/01002) 最大块为两个等大 ~44s 块, 非完整全曲, 留作后续 special 处理。
- scan 实测: 2373 首有效曲 (含 omni +608) **全部定位成功, 0 skip**。

## 对 iidx-on-Ear 的影响

- 元数据: `music_data.bin` (已有 Python 解析器, 定长二进制, UTF-16LE + CP932)。
- 完整曲目音频: 每首歌 .ifs -> .s3p -> 最大 S3V0 条目 -> WMA -> ffmpeg。
- 封面/jacket: **尚未定位** (28000.ifs 内无封面, 应在游戏数据别处的纹理 ifs 中) — 待设计阶段调查。
- ffmpeg 当前不在 PATH, 转码阶段需补。
