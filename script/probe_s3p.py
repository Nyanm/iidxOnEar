# -*- coding: utf-8 -*-
"""
探测并解包 IIDX 的 .s3p 归档。
.s3p 结构（已通过 hexdump 反推）：
    0x00  char[4]   "S3P0"
    0x04  uint32    count                条目数量
    0x08  ...        条目表 count × (uint32 offset, uint32 size)  指向文件内每段数据
每个条目指向的数据块的内部格式尚未确认(可能是 2DX9, 或 S3V0, 或裸 WAV), 本脚本负责探测:
打印每块的前若干字节 magic 与可读化, 用以决定下一步如何转码。
"""
import struct
import sys
import os
from collections import Counter

_HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT_IN = os.path.join(_HERE, "..", ".sample", "unpack", "30000.s3p")


def parse_s3p(data: bytes):
    magic = data[0x00:0x04]
    assert magic == b"S3P0", magic
    count = struct.unpack_from("<I", data, 0x04)[0]
    print(f"magic={magic!r} count={count} total=0x{len(data):X}")

    entries = []
    for i in range(count):
        off, size = struct.unpack_from("<II", data, 0x08 + i * 8)         # 条目表: 每条 (offset, size)
        entries.append((off, size))
    return entries


def main():
    path_in = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_IN
    dump_n = int(sys.argv[2]) if len(sys.argv) > 2 else 0                  # 第二参数: 实际导出前 N 个块的原始数据
    out_dir = os.path.join(_HERE, "out", "s3p")

    data = open(path_in, "rb").read()
    base = os.path.splitext(os.path.basename(path_in))[0]
    entries = parse_s3p(data)

    # 统计每个数据块的前 4 字节 magic 分布, 并打印前几个块的细节
    magics = Counter()
    for i, (off, size) in enumerate(entries):
        head = data[off:off + 16]
        magic4 = bytes(head[:4])
        magics[magic4] += 1
        if i < 5 or i == len(entries) - 1:                                 # 只详细打印头几个与最后一个
            print(f"  entry#{i}: off=0x{off:X} size=0x{size:X} magic={magic4!r} head={head.hex()}")

    print("\n块 magic 分布:")
    for m, c in magics.most_common():
        print(f"  {m!r}: {c}")

    # 可选: 把前 dump_n 个块原样写出, 便于人工检查
    if dump_n > 0:
        os.makedirs(out_dir, exist_ok=True)
        for i in range(min(dump_n, len(entries))):
            off, size = entries[i]
            out = os.path.join(out_dir, f"{base}_{i}.bin")
            with open(out, "wb") as f:
                f.write(data[off:off + size])
            print(f"  wrote {out} ({size} bytes)")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main()
