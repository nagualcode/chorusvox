"""Roda o rastreador de pitch original de reference/DecaBuddy.dll (0x10008e60) num emulador x86
e grava as leituras ("amostra midi periodo unvoiced"), o gabarito de src/pitch.rs.

Uso: python3 tools/dll_pitch_reference.py entrada.wav saida.txt [segundos]
Requer: pip install unicorn numpy
"""
import sys, struct, math, wave
import numpy as np
# --- carregador PE minimo ---
import os
D=open(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'reference', 'DecaBuddy.dll'),'rb').read()
pe=struct.unpack_from('<I',D,0x3c)[0]
nsec=struct.unpack_from('<H',D,pe+6)[0]; opt=struct.unpack_from('<H',D,pe+20)[0]
BASE=struct.unpack_from('<I',D,pe+24+28)[0]
SECS=[]
for i in range(nsec):
    o=pe+24+opt+40*i
    name=D[o:o+8].rstrip(b'\0').decode(); vs,va,rs,ro=struct.unpack_from('<IIII',D,o+8)
    SECS.append((name,va+BASE,vs,ro,rs))
def v2o(va):
    for n,sva,vs,ro,rs in SECS:
        if sva<=va<sva+max(vs,rs): 
            off=va-sva
            return ro+off if off<rs else None
def rd(va,n):
    o=v2o(va); return D[o:o+n] if o is not None else b'\0'*n

from unicorn import *
from unicorn.x86_const import *

def load_wav(path):
    w = wave.open(path)
    n, ch, sw, sr = w.getnframes(), w.getnchannels(), w.getsampwidth(), w.getframerate()
    raw = w.readframes(n)
    if sw == 2:
        x = np.frombuffer(raw, '<i2').astype(np.float64) / 32768.0
    elif sw == 3:
        b = np.frombuffer(raw, np.uint8).reshape(-1, 3)
        v = (b[:, 0].astype(np.int32) | (b[:, 1].astype(np.int32) << 8) | (b[:, 2].astype(np.int32) << 16))
        v = np.where(v >= 1 << 23, v - (1 << 24), v)
        x = v / float(1 << 23)
    else:
        x = np.frombuffer(raw, '<i4') / 2.0**31
    return x.reshape(-1, ch)[:, 0].astype(np.float32), sr

def run(x, sr):
    mu = Uc(UC_ARCH_X86, UC_MODE_32)
    lo = BASE; hi = max(s[1] + max(s[2], s[4]) for s in SECS)
    size = ((hi - lo) + 0xfff) & ~0xfff
    mu.mem_map(lo, size)
    mu.mem_write(lo, D[:0x1000])
    for name, va, vs, ro, rs in SECS:
        mu.mem_write(va, D[ro:ro + min(vs, rs) if vs else rs])
    # log(x) natural: fldln2; fld qword [esp+4]; fyl2x; ret
    mu.mem_write(0x1001e7f6, bytes([0xD9, 0xED, 0xDD, 0x44, 0x24, 0x04, 0xD9, 0xF1, 0xC3]))
    THIS, RING, STACK, MAGIC = 0x20000000, 0x20001000, 0x30000000, 0x40000000
    mu.mem_map(THIS, 0x10000); mu.mem_map(STACK - 0x10000, 0x20000); mu.mem_map(MAGIC, 0x1000)
    f32 = lambda a, v: mu.mem_write(a, struct.pack('<f', v))
    i32 = lambda a, v: mu.mem_write(a, struct.pack('<i', v))
    w = 500.0 / sr
    r = math.exp(-math.pi * w * 1.25)
    a1 = math.cos(2 * math.pi * w) * 2 * r
    a2 = math.exp(-2 * math.pi * w * 1.25)
    for base in (THIS + 4, THIS + 0x18):
        f32(base + 8, a1); f32(base + 0xc, a2)
        f32(base + 0x10, (1.0 - a1) + struct.unpack('<f', struct.pack('<f', a2))[0])
    i32(THIS + 0x38, RING)
    f32(THIS + 0x58, 40.374756 / sr); f32(THIS + 0x5c, 107.66602 / sr)
    f32(THIS + 0x70, 0.45); f32(THIS + 0x74, 0.45)
    f32(THIS + 0x80, 0.0248016 * sr); f32(THIS + 0x84, math.log(sr))
    i32(THIS + 0x8c, int(50 * sr * 0.001)); f32(THIS + 0x9c, 1.0); f32(THIS + 0xa0, 100.0)
    def call(addr, args=()):
        sp = STACK
        for a in reversed(args):
            sp -= 4; mu.mem_write(sp, a)
        sp -= 4; mu.mem_write(sp, struct.pack('<I', MAGIC))
        mu.reg_write(UC_X86_REG_ESP, sp); mu.reg_write(UC_X86_REG_ECX, THIS)
        mu.emu_start(addr, MAGIC)
        return mu.reg_read(UC_X86_REG_EAX)
    mu.reg_write(UC_X86_REG_FPCW, 0x027F)
    call(0x10008d90)
    out = []
    for n, v in enumerate(x):
        ev = call(0x10008e60, [struct.pack('<f', float(v))]) & 0xff
        if ev:
            m = mu.mem_read(THIS + 0x90, 9)
            midi, per = struct.unpack('<ff', m[:8])
            out.append((n, midi, per, m[8]))
    return out

if __name__ == '__main__':
    x, sr = load_wav(sys.argv[1])
    if len(sys.argv) > 3:
        x = x[:int(float(sys.argv[3]) * sr)]
    ev = run(x, sr)
    with open(sys.argv[2], 'w') as f:
        for n, midi, per, unv in ev:
            f.write(f'{n} {midi:.6f} {per:.6f} {unv}\n')
    print(len(ev), 'events', sr)
