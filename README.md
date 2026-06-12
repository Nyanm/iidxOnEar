# IIDX on Ear

## 简介

IIDX on Ear 是一款将 beatmania IIDX 游戏文件中的歌曲提取为组织好的音乐文件仓库的程序，同时为每首歌附加对应的元信息与专辑封面。

程序读取你的 `contents/` 文件夹，遍历游戏内的全部歌曲（含 omnimix 复活曲），为每首歌：

- **定位音源**：依据 song_id 找到该曲的歌曲文件（30 代及以后为散放文件夹，更早的版本为 `.ifs` 打包）；
- **渲染全曲**：调用 [IIDX on Knitting](https://github.com/Nyanm/iidxOnKnitting) 把谱面键音按 BMS 方式混音并编码为完整的 Opus 歌曲；
- **下载并内嵌封面**：自动下载该曲所属版本的专辑封面并嵌入；
- **归类输出**：按版本放入对应文件夹（如 `IIDX 32 Pinky Crush`）。

在游戏更新到新版本后，可直接再次运行，程序将**增量转换**新增的歌曲（输出目录中已存在的歌曲会跳过）。

本程序不包含任何所属 ©Konami Arcade Games 版权所有的信息。

每首歌附加的元信息：

| 元信息   | Vorbis 标签     | 来源                                       |
|--------|---------------|------------------------------------------|
| 曲名    | `TITLE`       | 游戏内曲名                                    |
| 艺术家   | `ARTIST`      | 游戏内艺术家名                                  |
| 流派    | `GENRE`       | 游戏内流派                                    |
| 专辑    | `ALBUM`       | 所属版本全名（如 `beatmania IIDX 32 Pinky Crush`） |
| 专辑艺术家 | `ALBUMARTIST` | 固定为 `BEMANI`，保证同一版本归为一张专辑                |
| 音轨号   | `TRACKNUMBER` | 版本内序号（song_id 后三位 + 1）                   |
| 封面    | 内嵌图片          | 所属版本的专辑封面                                |

## 使用

推荐通过直接下载Release版本使用该软件。

### 编译

本程序通过 path 依赖嵌入 [IIDX on Knitting](https://github.com/Nyanm/iidxOnKnitting)（负责键音渲染与 Opus 编码）。请将两个仓库克隆为同级目录：

```
RustroverProjects/
├─ iidxOnEar/
└─ iidxOnKnitting/
```

随后 `cargo build -r` 即可（`FFMPEG_DIR` 已在 `.cargo/config.toml` 中配好，指向 `../iidxOnKnitting/vendor`；若放在别处需相应修改）。
首次 clean 构建依赖 LLVM + VS 2022 MSVC 工具链（这是 on knitting 静态链接裁剪版 FFmpeg 所需，`vendor/` 内已带预编译二进制，无需自行编译 FFmpeg），增量构建不需要。构建出的可执行文件已静态链接全部依赖，**运行时无需系统 FFmpeg 或任何外部库**。

### 运行

`IidxOnEar -s <contents> [-d 输出] [-v 版本…] [-f] [-j N]`

| 参数                       | 说明                                                            |
|--------------------------|---------------------------------------------------------------|
| `-s, --src <路径>`         | **必填**。IIDX 的 `contents` 文件夹；程序自动拼接 `data/info/0/music_data.bin`、`data/sound`，并搜索 omnimix 补丁 |
| `-d, --dst <路径>`         | 输出目录。省略时默认为当前工作目录                                       |
| `-v, --version <版本…>`    | 只转换指定版本（如 `-v 30 31 32`）。省略时转换全部版本                       |
| `-f, --force`            | 全量转换：对已存在于输出目录的歌曲也重新转换（默认只增量转换新增歌曲）             |
| `-j, --jobs <N>`         | 并发 worker 数量。省略时默认为逻辑 CPU 核心数                            |
| `--test-jacket`          | 仅下载全部专辑封面到 `./jacket` 后退出（不解析数据库、不转换）               |
| `--skip-jacket`          | 完全跳过封面环节（不下载、不内嵌），适用于离线或封面源失效时                    |

封面会缓存到运行目录下的 `jacket/` 文件夹，跨次运行复用。

使用案例：

`IidxOnEar -s E:\Arcade\IIDX\contents -d E:\Arcade\MUSIC`：读取 contents 文件夹中的游戏数据，转换全部版本，输出到 MUSIC 文件夹。

`IidxOnEar -s E:\Arcade\IIDX\contents -d E:\Arcade\MUSIC -v 30 31 32 -j 8`：只转换 30、31和32 代，开 8 个并发 worker。

## 维护与自定义

游戏更新或封面源变动时，通常只需改两张表：

- **新版本上线**：在 `src/common.rs` 的 `VERSION_ALBUM_NAMES` / `VERSION_FOLDER_NAMES` 填入新版本的专辑名与输出文件夹名；在 `src/jacket.rs` 的 `VERSION_JACKET_URL` 对应下标填入该版本封面的完整 URL。
- **封面换源 / 链接失效**：直接修改 `VERSION_JACKET_URL` 对应行的完整 URL（留空字符串则该版本不下载封面）。本地缓存按 `<版本号>.<扩展名>` 命名，改完 URL 后删掉 `jacket/` 下对应文件即会重新下载。

---

# IIDX on Ear

## Overview

IIDX on Ear extracts the songs out of the beatmania IIDX game files into a neatly organized music library, attaching the matching metadata and album cover to every track.

It reads your `contents/` folder, walks every song in the game (including omnimix-revived ones), and for each song:

- **Locates the audio**: finds the song's files by its song_id (a loose folder for gen 30+, an `.ifs` archive for earlier versions);
- **Renders the full song**: calls [IIDX on Knitting](https://github.com/Nyanm/iidxOnKnitting) to mix the chart's keysounds BMS-style and encode them into a complete Opus track;
- **Downloads and embeds the cover**: automatically fetches and embeds the album jacket of the song's version;
- **Sorts the output**: places it in a per-version folder (e.g. `IIDX 32 Pinky Crush`).

After the game updates to a new version, just run it again — newly added songs are converted **incrementally** (songs already present in the output are skipped).

This program contains no information whose copyright belongs to ©Konami Arcade Games.

The metadata attached to each song:

| Field        | Vorbis tag    | Source                                                          |
|--------------|---------------|-----------------------------------------------------------------|
| Title        | `TITLE`       | in-game song title                                              |
| Artist       | `ARTIST`      | in-game artist name                                             |
| Genre        | `GENRE`       | in-game genre                                                   |
| Album        | `ALBUM`       | full name of the version it belongs to (e.g. `beatmania IIDX 32 Pinky Crush`) |
| Album artist | `ALBUMARTIST` | fixed to `BEMANI`, so one version groups as a single album      |
| Track number | `TRACKNUMBER` | within-version index (last three digits of song_id + 1)         |
| Cover        | embedded image | the album jacket of the song's version                         |

## Usage

Downloading a release build is the recommended way to use this program.

### Build

This program embeds [IIDX on Knitting](https://github.com/Nyanm/iidxOnKnitting) (which handles keysound rendering and Opus encoding) as a path dependency. Clone the two repositories as sibling directories:

```
RustroverProjects/
├─ iidxOnEar/
└─ iidxOnKnitting/
```

Then `cargo build -r` (`FFMPEG_DIR` is already set in `.cargo/config.toml` to point at `../iidxOnKnitting/vendor`; adjust it if you put the repo elsewhere). A first clean build requires the LLVM + VS 2022 MSVC toolchains (needed by on knitting's statically-linked, trimmed FFmpeg; the prebuilt binaries ship in `vendor/`, so there is no need to compile FFmpeg yourself); incremental builds do not. The resulting executable statically links every dependency, so **no system FFmpeg or any external library is needed at runtime**.

### Run

`IidxOnEar -s <contents> [-d output] [-v versions…] [-f] [-j N]`

| Argument                      | Description                                                                                              |
|-------------------------------|----------------------------------------------------------------------------------------------------------|
| `-s, --src <path>`            | **Required.** IIDX's `contents` folder; the program appends `data/info/0/music_data.bin` and `data/sound`, and searches for an omnimix patch |
| `-d, --dst <path>`            | Output directory. Defaults to the current working directory when omitted                                 |
| `-v, --version <versions…>`   | Convert only the given versions (e.g. `-v 30 31 32`). Converts all versions when omitted                 |
| `-f, --force`                 | Full conversion: re-convert songs even if they already exist in the output (by default only newly added songs are converted, incrementally) |
| `-j, --jobs <N>`              | Number of concurrent workers. Defaults to the logical CPU core count when omitted                        |
| `--test-jacket`               | Only download all album jackets into `./jacket`, then exit (no database parse, no conversion)            |
| `--skip-jacket`               | Skip the cover step entirely (no download, no embedding) — for offline use or when a cover source is down |

Covers are cached in the `jacket/` folder under the working directory and reused across runs.

Examples:

`IidxOnEar -s E:\Arcade\IIDX\contents -d E:\Arcade\MUSIC`: read the game data from the contents folder, convert all versions, and output to the MUSIC folder.

`IidxOnEar -s E:\Arcade\IIDX\contents -d E:\Arcade\MUSIC -v 30 31 32 -j 8`: convert only versions 30, 31 and 32, with 8 concurrent workers.

## Maintenance & Customization

When the game updates or a cover source changes, you usually only need to edit two tables:

- **A new version ships**: fill in the new version's album name and output folder name in `VERSION_ALBUM_NAMES` / `VERSION_FOLDER_NAMES` in `src/common.rs`; fill in the full cover URL at the matching index in `VERSION_JACKET_URL` in `src/jacket.rs`.
- **Switching cover source / a dead link**: just edit the full URL on the matching row of `VERSION_JACKET_URL` (an empty string means no cover is downloaded for that version). The local cache is named `<version>.<ext>`, so after changing a URL, delete the corresponding file under `jacket/` to re-download it.
