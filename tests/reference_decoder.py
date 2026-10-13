"""Offline decoder for Ulanzi Dial vendor messages; input is controller-att.tsv."""
import csv
import struct
import sys

KEY = bytes.fromhex('cabaa5ca6d8a2abcba9e5acaca8bb89b')
WORDS = struct.unpack('<4I', KEY)
MASK = 0xffffffff

def transform(data, encrypt=False):
    out = bytearray(data)
    for offset in range(0, len(data) // 8 * 8, 8):
        a, b = struct.unpack_from('<2I', data, offset)
        total = 0 if encrypt else 0xc6ef3720
        for _ in range(32):
            if encrypt:
                total = (total + 0x9e3779b9) & MASK
                a = (a + (((b << 4) + WORDS[0]) ^ (b + total) ^ ((b >> 5) + WORDS[1]))) & MASK
                b = (b + (((a << 4) + WORDS[2]) ^ (a + total) ^ ((a >> 5) + WORDS[3]))) & MASK
            else:
                b = (b - (((a << 4) + WORDS[2]) ^ (a + total) ^ ((a >> 5) + WORDS[3]))) & MASK
                a = (a - (((b << 4) + WORDS[0]) ^ (b + total) ^ ((b >> 5) + WORDS[1]))) & MASK
                total = (total - 0x9e3779b9) & MASK
        struct.pack_into('<2I', out, offset, a, b)
    return bytes(out)

if __name__ == '__main__':
    with open(sys.argv[1], newline='') as source:
        rows = csv.DictReader(source, delimiter='\t')
        writer = csv.DictWriter(sys.stdout, fieldnames=rows.fieldnames + ['decoded_hex'], delimiter='\t')
        writer.writeheader()
        for row in rows:
            row['decoded_hex'] = ''
            if row['attribute_handle'] in ('0x004d', '0x0050'):
                raw = bytes.fromhex(row['value_hex'])
                decoded = transform(raw)
                assert transform(decoded, encrypt=True) == raw
                row['decoded_hex'] = decoded.hex()
            writer.writerow(row)
