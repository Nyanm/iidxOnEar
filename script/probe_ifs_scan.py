# -*- coding: utf-8 -*-
"""
验证轻量策略: 不解析 KBin, 直接在 .ifs 原始字节中扫描 'S3P0' magic 切出主音频 .s3p。
做法:
  1) 统计 'S3P0' 在 ifs 中出现次数 (要确认无误命中 / 无致命假阳性)。
  2) 对每个候选位置, 读 count + (offset,size) 表, 校验合理性 (count 不离谱, 表项不越界),
     s3p 总长 = max(offset_i + size_i)。
  3) 切出 [start, start+total] 与 ifstools 解出的参考 .s3p 逐字节对比。
若吻合, 说明 Rust 端可照此实现, 无需移植 KBin。
"""
import struct
import os

_HERE = os.path.dirname(os.path.abspath(__file__))
IFS = os.path.join(_HERE, "..", ".sample", "unpack", "28000.ifs")
REF_S3P = os.path.join(_HERE, "out", "ifs", "28000_ifs", "28000", "28000.s3p")


def scan_magic(data: bytes, magic: bytes):
    pos, hits = data.find(magic), []
    while pos != -1:
        hits.append(pos)
        pos = data.find(magic, pos + 1)
    return hits


def try_carve_s3p(data: bytes, start: int):
    """在 start 处按 S3P0 结构试切, 返回 (count, total_size) 或 None(不合理)。"""
    if start + 8 > len(data):
        return None
    count = struct.unpack_from("<I", data, start + 4)[0]
    if not (1 <= count <= 100_000):                                       # count 合理性: 一首歌音源数不会离谱
        return None
    table_end = start + 8 + count * 8
    if table_end > len(data):
        return None
    total = 0
    for i in range(count):
        off, size = struct.unpack_from("<II", data, start + 8 + i * 8)    # 偏移相对 s3p 起点
        if off < 8 + count * 8 or off + size > len(data) - start:         # 越界即不是真 s3p
            return None
        total = max(total, off + size)
    return count, total


def main():
    data = open(IFS, "rb").read()
    print(f"ifs size = {len(data)} (0x{len(data):X})")

    for magic in (b"S3P0", b"2DX9"):
        hits = scan_magic(data, magic)
        print(f"\n'{magic.decode()}' 出现 {len(hits)} 次, 位置: {[hex(h) for h in hits[:8]]}{' ...' if len(hits) > 8 else ''}")

    s3p_hits = scan_magic(data, b"S3P0")
    for start in s3p_hits:
        res = try_carve_s3p(data, start)
        print(f"\n候选 S3P0 @0x{start:X}: ", end="")
        if res is None:
            print("结构不合理, 跳过 (疑似假阳性)")
            continue
        count, total = res
        carved = data[start:start + total]
        print(f"count={count} total={total} (0x{total:X})")
        if os.path.exists(REF_S3P):
            ref = open(REF_S3P, "rb").read()
            same = carved == ref
            print(f"  vs ifstools 参考: 参考长度={len(ref)} 切出长度={len(carved)} 逐字节相同={same}")
            if not same:
                # 找首个不同字节, 辅助诊断
                n = min(len(ref), len(carved))
                diff = next((i for i in range(n) if ref[i] != carved[i]), n)
                print(f"  首个差异@{diff} (长度差={len(carved) - len(ref)})")


if __name__ == "__main__":
    import sys
    sys.stdout.reconfigure(encoding="utf-8")
    main()
