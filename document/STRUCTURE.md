# IIDX 游戏数据目录结构 (实测 E:\Arcade\IIDX\contents, IIDX 32)

## song_id 命名

5 位零填充: `format!("{:05}", song_id)`。前 2 位=版本, 后 3 位=版本内序号。
例: 5.1.1.=id 1000 -> `01000`; 冥=12004 -> `12004`; 30000 -> 版本30第0首。

## 音频目录 (核心)

`contents/data/sound/` — **混合两种存放**:
- `id >= 30000` (版本30+): 散放文件夹 `sound/<id5>/` 内含 `<id5>.s3p`(主音频) + `<id5>_pre.2dx`(预览) + `<id5>.1`(谱面)
  实测 265 个文件夹, 范围 30000..32095。
- `id < 30000` (版本<30): 打包 `sound/<id5>.ifs` (内含上面三件套)
  实测 1557 个纯数字 .ifs, 范围 01000..29103。
- **`-p0` 变体**: 62 个如 `12007-p0.ifs`, 是某曲的额外音源 (基础 `12007.ifs` 也在), 类比 SDVX 多音源特殊曲, 后期单独处理。

**Rust 定位规则 (避免硬编码 30000)**: 先看 `sound/<id5>/` 文件夹存在否 -> 散放; 否则 `sound/<id5>.ifs` -> 打包。
两条路最终都汇到 "从 s3p 取最大 S3V0 -> WMA"。散放路无需解 ifs; 打包路用 magic 扫描 (见 FORMATS.md)。

## omnimix (逻辑同 SDVX)

`contents/data_mods/omnimix/` 下有自己的 `sound/` `info/` `graphic/` `movie/`。
- 音频 `omnimix/sound/`: 同样混合, 698 文件夹 + 318 .ifs (范围约 01002..31260)。omni 曲音频在此。
- DB `omnimix/info/0/music_omni.bin`: **与 info/0 同为 v32 格式** (int32 索引/2040 字节), count=2373 (= 基础 1765 的超集, 多约 608 复活曲)。
- 合并逻辑照搬 sdvx: 遍历合并后 DB, 每首歌音频先查 data/sound 再查 omnimix/sound。

## 三个 music_data.bin 定性 (全部 magic "IIDX")

| 文件 | version | count | index | entry | 用途 |
|---|---|---|---|---|---|
| `data/info/0/music_data.bin` | 32 | 1765 | int32×33000 | 2040B | **主用**, 当前版本 |
| `data_mods/omnimix/info/0/music_omni.bin` | 32 | 2373 | int32×33000 | 2040B | omni 超集, 主用合并源 |
| `data/info/1/music_data.bin` | 31 | 1743 | **int16×32000** | **1324B** | **上个版本(IIDX31)遗留, 忽略** |

v31 头与 v32 不同: 0x08=count(int16), 0x0A=index_len(int16); 索引为 int16; 条目布局更小更老 (字段偏移与 v32 不同, 不复用)。
contents 里有 `bm2dx_default.dll` / `bm2dx_omni.dll` 等, 印证装有可切换的多版本游戏体。

## 排序读音 (新需求, SDVX 内嵌在 xml, IIDX 独立成文件)

`data/info/0/music_title_yomi.xml` 与 `music_artist_yomi.xml` 存标题/艺术家读音 (半角片假名),
对应 TITLESORT / ARTISTSORT 标签。需按 song_id 关联 (待解析其结构)。
