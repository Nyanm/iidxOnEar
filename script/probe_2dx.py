# -*- coding: utf-8 -*-
"""
探测并解包 IIDX 的 .2dx 容器。
.2dx 结构（已通过 hexdump 反推验证）：
  头部:
    0x00  char[16]  name              任意名字 / 填充
    0x10  uint32    header_size       = 偏移表结束位置 = 第一个数据块的偏移
    0x14  uint32    file_count        内含的音轨数量
    0x18..          padding(0)        填充至偏移表
    偏移表           uint32[file_count]  指向每个 2DX9 块的文件内绝对偏移
  每个 2DX9 块:
    +0x00 char[4]   "2DX9"
    +0x04 uint32    block_header_size 块头大小(本例 0x18), 数据从 块首+此值 开始
    +0x08 uint32    data_size         RIFF/WAVE 数据字节数
    +0x0C ...       其余块头字段(track id / 音量 / 等, 暂不关心)
    +block_header_size  RIFF/WAVE 数据(data_size 字节)

这里采用最稳健的方式: 直接扫描全文件中所有 "2DX9" magic, 按各自块头精确切出 WAV, 不依赖偏移表的具体起始位置。
偏移表只用来交叉验证数量。
"""
import struct
import sys
import os

_HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT_IN = os.path.join(_HERE, "..", ".sample", "unpack", "26078_pre.2dx")


def parse_2dx(data: bytes):
    name = data[0x00:0x10].split(b"\x00")[0].decode("ascii", "replace")  # 取首段可见名字
    header_size, file_count = struct.unpack_from("<II", data, 0x10)        # 0x10 头大小, 0x14 块数量
    print(f"name={name!r} header_size=0x{header_size:X} file_count={file_count} total=0x{len(data):X}")

    # 扫描所有 2DX9 块的位置, 与 file_count 交叉验证
    positions = []
    pos = data.find(b"2DX9")
    while pos != -1:
        positions.append(pos)
        pos = data.find(b"2DX9", pos + 4)
    print(f"found {len(positions)} '2DX9' blocks at offsets: {[hex(p) for p in positions]}")

    tracks = []
    for i, p in enumerate(positions):
        block_header_size, data_size = struct.unpack_from("<II", data, p + 4)  # 块内 +4 头大小, +8 数据大小
        data_off = p + block_header_size
        wav = data[data_off:data_off + data_size]
        ok = wav[:4] == b"RIFF"                                            # 数据应为标准 RIFF/WAVE
        # 读 WAV 的 fmt 标签确认编码类型 (2=MS-ADPCM, 1=PCM, 0x11=IMA-ADPCM ...)
        fmt_tag = struct.unpack_from("<H", wav, 0x14)[0] if ok and len(wav) >= 0x16 else -1
        print(f"  block#{i} @0x{p:X}: hdr=0x{block_header_size:X} size=0x{data_size:X} "
              f"-> data@0x{data_off:X} RIFF={ok} fmt_tag={fmt_tag}")
        tracks.append((i, wav))
    return tracks


def main():
    path_in = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_IN
    out_dir = os.path.join(_HERE, "out", "2dx")
    os.makedirs(out_dir, exist_ok=True)

    data = open(path_in, "rb").read()
    base = os.path.splitext(os.path.basename(path_in))[0]
    tracks = parse_2dx(data)

    for i, wav in tracks:
        out = os.path.join(out_dir, f"{base}_{i}.wav")
        with open(out, "wb") as f:
            f.write(wav)
        print(f"  wrote {out} ({len(wav)} bytes)")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main()
