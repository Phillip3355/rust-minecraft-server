//! Dependency-free decompression used by Anvil region files and `.dat` files.
//!
//! * RFC 1951 DEFLATE inflate (stored / fixed / dynamic Huffman blocks)
//! * RFC 1950 zlib wrapper (Adler-32 verified)  -> Anvil compression type 2
//! * RFC 1952 gzip wrapper (CRC-32 + ISIZE verified) -> Anvil type 1, level.dat, playerdata
//! * lz4-java `LZ4BlockOutputStream` framing ("LZ4Block" magic, XXH32 checksum)
//!   -> Anvil compression type 4 (introduced by Mojang in 24w04a)
//!
//! Encoders here are *stored-only* / *literal-only*: they exist to build test
//! fixtures, never to rewrite a real world.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeError(pub String);

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DecodeError {}

fn err<T>(msg: impl Into<String>) -> Result<T, DecodeError> {
    Err(DecodeError(msg.into()))
}

/// Default ceiling for any single decompressed payload (anti zip-bomb). A
/// vanilla chunk NBT is typically < 1 MiB; 64 MiB leaves ample headroom.
pub const DEFAULT_MAX_OUTPUT: usize = 64 * 1024 * 1024;

// ---------------------------------------------------------------- checksums

pub fn crc32(data: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, slot) in t.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            *slot = c;
        }
        t
    });
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c = table[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

pub fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5552) {
        for &x in chunk {
            a += x as u32;
            b += a;
        }
        a %= MOD;
        b %= MOD;
    }
    (b << 16) | a
}

pub fn xxh32(data: &[u8], seed: u32) -> u32 {
    const P1: u32 = 2_654_435_761;
    const P2: u32 = 2_246_822_519;
    const P3: u32 = 3_266_489_917;
    const P4: u32 = 668_265_263;
    const P5: u32 = 374_761_393;
    let rd = |i: usize| u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
    let round = |acc: u32, lane: u32| acc.wrapping_add(lane.wrapping_mul(P2)).rotate_left(13).wrapping_mul(P1);
    let len = data.len();
    let mut i = 0usize;
    let mut h: u32;
    if len >= 16 {
        let mut v1 = seed.wrapping_add(P1).wrapping_add(P2);
        let mut v2 = seed.wrapping_add(P2);
        let mut v3 = seed;
        let mut v4 = seed.wrapping_sub(P1);
        while i + 16 <= len {
            v1 = round(v1, rd(i));
            v2 = round(v2, rd(i + 4));
            v3 = round(v3, rd(i + 8));
            v4 = round(v4, rd(i + 12));
            i += 16;
        }
        h = v1
            .rotate_left(1)
            .wrapping_add(v2.rotate_left(7))
            .wrapping_add(v3.rotate_left(12))
            .wrapping_add(v4.rotate_left(18));
    } else {
        h = seed.wrapping_add(P5);
    }
    h = h.wrapping_add(len as u32);
    while i + 4 <= len {
        h = h.wrapping_add(rd(i).wrapping_mul(P3)).rotate_left(17).wrapping_mul(P4);
        i += 4;
    }
    while i < len {
        h = h.wrapping_add((data[i] as u32).wrapping_mul(P5)).rotate_left(11).wrapping_mul(P1);
        i += 1;
    }
    h ^= h >> 15;
    h = h.wrapping_mul(P2);
    h ^= h >> 13;
    h = h.wrapping_mul(P3);
    h ^= h >> 16;
    h
}

// ------------------------------------------------------------------ inflate

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    bitbuf: u32,
    bitcnt: u32,
}

impl<'a> Bits<'a> {
    fn new(data: &'a [u8]) -> Self {
        Bits { data, pos: 0, bitbuf: 0, bitcnt: 0 }
    }

    fn need(&mut self, n: u32) -> Result<u32, DecodeError> {
        let mut val = self.bitbuf;
        while self.bitcnt < n {
            let Some(&b) = self.data.get(self.pos) else {
                return err("deflate: unexpected end of input");
            };
            self.pos += 1;
            val |= (b as u32) << self.bitcnt;
            self.bitcnt += 8;
        }
        self.bitbuf = if n == 32 { 0 } else { val >> n };
        self.bitcnt -= n;
        Ok(if n == 32 { val } else { val & ((1u32 << n) - 1) })
    }

    fn align(&mut self) {
        self.bitbuf = 0;
        self.bitcnt = 0;
    }

    /// Number of input bytes consumed so far (partial bytes count as consumed).
    fn consumed(&self) -> usize {
        self.pos
    }
}

struct Huffman {
    count: [u16; 16],
    symbol: Vec<u16>,
}

impl Huffman {
    fn new(lengths: &[u8]) -> Result<Self, DecodeError> {
        let mut count = [0u16; 16];
        for &l in lengths {
            count[l as usize] += 1;
        }
        if count[0] as usize == lengths.len() {
            // No codes: valid only if the table is never used.
            return Ok(Huffman { count, symbol: Vec::new() });
        }
        let mut left: i32 = 1;
        for len in 1..16 {
            left <<= 1;
            left -= count[len] as i32;
            if left < 0 {
                return err("deflate: over-subscribed Huffman code");
            }
        }
        let mut offs = [0u16; 16];
        for len in 1..15 {
            offs[len + 1] = offs[len] + count[len];
        }
        let mut symbol = vec![0u16; lengths.len()];
        for (sym, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbol[offs[l as usize] as usize] = sym as u16;
                offs[l as usize] += 1;
            }
        }
        Ok(Huffman { count, symbol })
    }

    fn decode(&self, bits: &mut Bits<'_>) -> Result<u16, DecodeError> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..16 {
            code |= bits.need(1)? as i32;
            let count = self.count[len] as i32;
            if code - count < first {
                return Ok(self.symbol[(index + (code - first)) as usize]);
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        err("deflate: invalid Huffman code")
    }
}

const LBASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258,
];
const LEXT: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DBASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145,
    8193, 12289, 16385, 24577,
];
const DEXT: [u8; 30] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

fn inflate_codes(
    bits: &mut Bits<'_>,
    out: &mut Vec<u8>,
    lit: &Huffman,
    dist: &Huffman,
    max_out: usize,
) -> Result<(), DecodeError> {
    loop {
        let sym = lit.decode(bits)?;
        if sym < 256 {
            if out.len() >= max_out {
                return err(format!("deflate: output exceeds limit {max_out}"));
            }
            out.push(sym as u8);
        } else if sym == 256 {
            return Ok(());
        } else {
            let s = (sym - 257) as usize;
            if s >= 29 {
                return err("deflate: invalid length symbol");
            }
            let len = LBASE[s] as usize + bits.need(LEXT[s] as u32)? as usize;
            let ds = dist.decode(bits)? as usize;
            if ds >= 30 {
                return err("deflate: invalid distance symbol");
            }
            let d = DBASE[ds] as usize + bits.need(DEXT[ds] as u32)? as usize;
            if d > out.len() {
                return err("deflate: distance too far back");
            }
            if out.len() + len > max_out {
                return err(format!("deflate: output exceeds limit {max_out}"));
            }
            let start = out.len() - d;
            for k in 0..len {
                let b = out[start + k];
                out.push(b);
            }
        }
    }
}

/// Raw DEFLATE. Returns (output, input bytes consumed).
pub fn inflate_raw(data: &[u8], max_out: usize) -> Result<(Vec<u8>, usize), DecodeError> {
    let mut bits = Bits::new(data);
    let mut out = Vec::new();
    loop {
        let last = bits.need(1)?;
        let btype = bits.need(2)?;
        match btype {
            0 => {
                bits.align();
                let p = bits.pos;
                if p + 4 > data.len() {
                    return err("deflate: truncated stored block header");
                }
                let len = u16::from_le_bytes([data[p], data[p + 1]]) as usize;
                let nlen = u16::from_le_bytes([data[p + 2], data[p + 3]]) as usize;
                if len != (!nlen & 0xFFFF) {
                    return err("deflate: stored block LEN/NLEN mismatch");
                }
                if p + 4 + len > data.len() {
                    return err("deflate: truncated stored block");
                }
                if out.len() + len > max_out {
                    return err(format!("deflate: output exceeds limit {max_out}"));
                }
                out.extend_from_slice(&data[p + 4..p + 4 + len]);
                bits.pos = p + 4 + len;
            }
            1 => {
                let mut l = [0u8; 288];
                l[..144].fill(8);
                l[144..256].fill(9);
                l[256..280].fill(7);
                l[280..].fill(8);
                let lit = Huffman::new(&l)?;
                let dist = Huffman::new(&[5u8; 30])?;
                inflate_codes(&mut bits, &mut out, &lit, &dist, max_out)?;
            }
            2 => {
                let nlen = bits.need(5)? as usize + 257;
                let ndist = bits.need(5)? as usize + 1;
                let ncode = bits.need(4)? as usize + 4;
                if nlen > 286 || ndist > 30 {
                    return err("deflate: bad dynamic block counts");
                }
                const ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
                let mut cl = [0u8; 19];
                for &o in ORDER.iter().take(ncode) {
                    cl[o] = bits.need(3)? as u8;
                }
                let clh = Huffman::new(&cl)?;
                let mut lengths = vec![0u8; nlen + ndist];
                let mut i = 0;
                while i < nlen + ndist {
                    let sym = clh.decode(&mut bits)?;
                    if sym < 16 {
                        lengths[i] = sym as u8;
                        i += 1;
                    } else {
                        let (val, rep) = match sym {
                            16 => {
                                if i == 0 {
                                    return err("deflate: repeat with no previous length");
                                }
                                (lengths[i - 1], 3 + bits.need(2)? as usize)
                            }
                            17 => (0, 3 + bits.need(3)? as usize),
                            _ => (0, 11 + bits.need(7)? as usize),
                        };
                        if i + rep > nlen + ndist {
                            return err("deflate: too many code lengths");
                        }
                        lengths[i..i + rep].fill(val);
                        i += rep;
                    }
                }
                if lengths[256] == 0 {
                    return err("deflate: missing end-of-block code");
                }
                let lit = Huffman::new(&lengths[..nlen])?;
                let dist = Huffman::new(&lengths[nlen..])?;
                inflate_codes(&mut bits, &mut out, &lit, &dist, max_out)?;
            }
            _ => return err("deflate: invalid block type 3"),
        }
        if last == 1 {
            break;
        }
    }
    Ok((out, bits.consumed()))
}

pub fn zlib_decompress(data: &[u8], max_out: usize) -> Result<Vec<u8>, DecodeError> {
    if data.len() < 6 {
        return err("zlib: stream too short");
    }
    let (cmf, flg) = (data[0], data[1]);
    if cmf & 0x0F != 8 {
        return err(format!("zlib: unsupported compression method {}", cmf & 0x0F));
    }
    if ((cmf as u16) << 8 | flg as u16) % 31 != 0 {
        return err("zlib: header check (FCHECK) failed");
    }
    if flg & 0x20 != 0 {
        return err("zlib: preset dictionary not supported");
    }
    let (out, used) = inflate_raw(&data[2..], max_out)?;
    let p = 2 + used;
    if p + 4 > data.len() {
        return err("zlib: missing Adler-32 trailer");
    }
    let want = u32::from_be_bytes([data[p], data[p + 1], data[p + 2], data[p + 3]]);
    let got = adler32(&out);
    if want != got {
        return err(format!("zlib: Adler-32 mismatch (stored {want:#010x}, computed {got:#010x})"));
    }
    Ok(out)
}

pub fn gzip_decompress(data: &[u8], max_out: usize) -> Result<Vec<u8>, DecodeError> {
    if data.len() < 18 || data[0] != 0x1F || data[1] != 0x8B {
        return err("gzip: bad magic");
    }
    if data[2] != 8 {
        return err("gzip: unsupported compression method");
    }
    let flg = data[3];
    let mut p = 10usize;
    let need = |p: usize, n: usize| -> Result<(), DecodeError> {
        if p + n > data.len() { err("gzip: truncated header") } else { Ok(()) }
    };
    if flg & 0x04 != 0 {
        need(p, 2)?;
        let xlen = u16::from_le_bytes([data[p], data[p + 1]]) as usize;
        p += 2;
        need(p, xlen)?;
        p += xlen;
    }
    for flag in [0x08u8, 0x10] {
        if flg & flag != 0 {
            while p < data.len() && data[p] != 0 {
                p += 1;
            }
            need(p, 1)?;
            p += 1;
        }
    }
    if flg & 0x02 != 0 {
        need(p, 2)?;
        p += 2;
    }
    let (out, used) = inflate_raw(&data[p..], max_out)?;
    let t = p + used;
    if t + 8 > data.len() {
        return err("gzip: missing CRC32/ISIZE trailer");
    }
    let want_crc = u32::from_le_bytes([data[t], data[t + 1], data[t + 2], data[t + 3]]);
    let want_len = u32::from_le_bytes([data[t + 4], data[t + 5], data[t + 6], data[t + 7]]);
    let got_crc = crc32(&out);
    if want_crc != got_crc {
        return err(format!("gzip: CRC-32 mismatch (stored {want_crc:#010x}, computed {got_crc:#010x})"));
    }
    if want_len != out.len() as u32 {
        return err(format!("gzip: ISIZE mismatch (stored {want_len}, actual {})", out.len()));
    }
    Ok(out)
}

// ---------------------------------------------------------------------- LZ4

/// Decode one raw LZ4 block whose decompressed size is known exactly.
pub fn lz4_block_decompress(src: &[u8], out_len: usize) -> Result<Vec<u8>, DecodeError> {
    let mut out: Vec<u8> = Vec::with_capacity(out_len);
    let mut i = 0usize;
    let read_ext = |i: &mut usize, mut n: usize| -> Result<usize, DecodeError> {
        loop {
            let Some(&b) = src.get(*i) else { return err("lz4: truncated length") };
            *i += 1;
            n += b as usize;
            if b != 255 {
                return Ok(n);
            }
        }
    };
    while i < src.len() {
        let token = src[i];
        i += 1;
        let mut lit = (token >> 4) as usize;
        if lit == 15 {
            lit = read_ext(&mut i, lit)?;
        }
        if i + lit > src.len() || out.len() + lit > out_len {
            return err("lz4: literal run out of bounds");
        }
        out.extend_from_slice(&src[i..i + lit]);
        i += lit;
        if i == src.len() {
            break; // last sequence has no match part
        }
        if i + 2 > src.len() {
            return err("lz4: truncated match offset");
        }
        let off = u16::from_le_bytes([src[i], src[i + 1]]) as usize;
        i += 2;
        if off == 0 || off > out.len() {
            return err("lz4: invalid match offset");
        }
        let mut mlen = (token & 0x0F) as usize;
        if mlen == 15 {
            mlen = read_ext(&mut i, mlen)?;
        }
        mlen += 4;
        if out.len() + mlen > out_len {
            return err("lz4: match overruns declared size");
        }
        let start = out.len() - off;
        for k in 0..mlen {
            let b = out[start + k];
            out.push(b);
        }
    }
    if out.len() != out_len {
        return err(format!("lz4: decoded {} bytes, header declared {out_len}", out.len()));
    }
    Ok(out)
}

pub const LZ4_BLOCK_MAGIC: &[u8; 8] = b"LZ4Block";
pub const LZ4_BLOCK_SEED: u32 = 0x9747_B28C;

/// Decode an lz4-java `LZ4BlockOutputStream` stream (Minecraft region compression type 4).
pub fn lz4_java_decompress(data: &[u8], max_out: usize) -> Result<Vec<u8>, DecodeError> {
    let mut out = Vec::new();
    let mut p = 0usize;
    loop {
        if p == data.len() {
            // lz4-java normally writes an explicit end mark; accept EOF after >=1 block.
            if p == 0 {
                return err("lz4block: empty stream");
            }
            return Ok(out);
        }
        if p + 21 > data.len() || &data[p..p + 8] != LZ4_BLOCK_MAGIC {
            return err("lz4block: bad or truncated block header");
        }
        let token = data[p + 8];
        let rd = |o: usize| i32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]);
        let clen = rd(p + 9);
        let dlen = rd(p + 13);
        let check = rd(p + 17) as u32;
        p += 21;
        if clen < 0 || dlen < 0 {
            return err("lz4block: negative length");
        }
        let (clen, dlen) = (clen as usize, dlen as usize);
        if clen == 0 && dlen == 0 {
            return Ok(out); // end mark
        }
        if p + clen > data.len() {
            return err("lz4block: truncated block body");
        }
        if out.len() + dlen > max_out {
            return err(format!("lz4block: output exceeds limit {max_out}"));
        }
        let body = &data[p..p + clen];
        let block = match token & 0xF0 {
            0x10 => {
                if clen != dlen {
                    return err("lz4block: raw block with clen != dlen");
                }
                body.to_vec()
            }
            0x20 => lz4_block_decompress(body, dlen)?,
            m => return err(format!("lz4block: unknown method {m:#04x}")),
        };
        let got = xxh32(&block, LZ4_BLOCK_SEED) & 0x0FFF_FFFF;
        if got != check & 0x0FFF_FFFF {
            return err(format!("lz4block: checksum mismatch (stored {check:#010x}, computed {got:#010x})"));
        }
        out.extend_from_slice(&block);
        p += clen;
    }
}

// ------------------------------------------------- fixture-only encoders

pub fn deflate_stored(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + data.len() / 65535 * 5 + 5);
    let mut chunks: Vec<&[u8]> = data.chunks(65535).collect();
    if chunks.is_empty() {
        chunks.push(&[]);
    }
    let n = chunks.len();
    for (i, c) in chunks.into_iter().enumerate() {
        out.push(if i + 1 == n { 1 } else { 0 });
        let len = c.len() as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(c);
    }
    out
}

pub fn zlib_compress_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    out.extend(deflate_stored(data));
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

pub fn gzip_compress_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x1F, 0x8B, 8, 0, 0, 0, 0, 0, 0, 0xFF];
    out.extend(deflate_stored(data));
    out.extend_from_slice(&crc32(data).to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out
}

/// Literal-only LZ4 block (valid, uncompressed-size-preserving).
pub fn lz4_block_literals(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + data.len() / 255 + 2);
    let n = data.len();
    if n < 15 {
        out.push((n as u8) << 4);
    } else {
        out.push(0xF0);
        let mut rest = n - 15;
        while rest >= 255 {
            out.push(255);
            rest -= 255;
        }
        out.push(rest as u8);
    }
    out.extend_from_slice(data);
    out
}

pub fn lz4_java_compress_literals(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let body = lz4_block_literals(data);
    out.extend_from_slice(LZ4_BLOCK_MAGIC);
    out.push(0x20 | 0x06);
    out.extend_from_slice(&(body.len() as i32).to_le_bytes());
    out.extend_from_slice(&(data.len() as i32).to_le_bytes());
    out.extend_from_slice(&((xxh32(data, LZ4_BLOCK_SEED) & 0x0FFF_FFFF) as i32).to_le_bytes());
    out.extend_from_slice(&body);
    out.extend_from_slice(LZ4_BLOCK_MAGIC);
    out.push(0x10 | 0x06);
    out.extend_from_slice(&[0u8; 12]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_known_vector() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn adler32_known_vector() {
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn xxh32_known_vectors() {
        assert_eq!(xxh32(b"", 0), 0x02CC_5D05);
    }

    #[test]
    fn stored_roundtrip_zlib_gzip() {
        let data: Vec<u8> = (0..200_000u32).map(|i| (i * 7 % 251) as u8).collect();
        assert_eq!(zlib_decompress(&zlib_compress_stored(&data), DEFAULT_MAX_OUTPUT).unwrap(), data);
        assert_eq!(gzip_decompress(&gzip_compress_stored(&data), DEFAULT_MAX_OUTPUT).unwrap(), data);
        assert_eq!(zlib_decompress(&zlib_compress_stored(&[]), DEFAULT_MAX_OUTPUT).unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn zlib_detects_checksum_corruption() {
        let mut z = zlib_compress_stored(b"hello world");
        let n = z.len();
        z[n - 1] ^= 0xFF;
        assert!(zlib_decompress(&z, DEFAULT_MAX_OUTPUT).unwrap_err().0.contains("Adler-32"));
    }

    #[test]
    fn gzip_detects_payload_corruption() {
        let mut g = gzip_compress_stored(b"hello world, this is a test");
        g[16] ^= 0x01; // flip a payload bit inside the stored block
        assert!(gzip_decompress(&g, DEFAULT_MAX_OUTPUT).is_err());
    }

    #[test]
    fn output_limit_enforced() {
        let data = vec![0u8; 10_000];
        assert!(zlib_decompress(&zlib_compress_stored(&data), 1000).is_err());
    }

    #[test]
    fn lz4_literal_and_match() {
        // "abcd" literal + match(offset 4, len 8) -> "abcdabcdabcd"
        let block = [0x44, b'a', b'b', b'c', b'd', 4, 0];
        assert_eq!(lz4_block_decompress(&block, 12).unwrap(), b"abcdabcdabcd");
        assert!(lz4_block_decompress(&block, 13).is_err());
        let bad_off = [0x44, b'a', b'b', b'c', b'd', 9, 0];
        assert!(lz4_block_decompress(&bad_off, 12).is_err());
    }

    #[test]
    fn lz4_java_stream_roundtrip_and_checksum() {
        let data: Vec<u8> = (0..5000u32).map(|i| (i % 97) as u8).collect();
        let mut s = lz4_java_compress_literals(&data);
        assert_eq!(lz4_java_decompress(&s, DEFAULT_MAX_OUTPUT).unwrap(), data);
        // Corrupt the last literal byte (just before the 21-byte end mark). Byte offsets
        // near the header fall inside the LZ4 length-extension run and would surface as
        // a bounds error instead, which is also a detection but not the one tested here.
        let k = s.len() - 21 - 1;
        s[k] ^= 0x55;
        assert!(lz4_java_decompress(&s, DEFAULT_MAX_OUTPUT).unwrap_err().0.contains("checksum"));
        // A corrupted length byte must still be rejected (as a structural error).
        let mut t = lz4_java_compress_literals(&data);
        t[30] ^= 0x55;
        assert!(lz4_java_decompress(&t, DEFAULT_MAX_OUTPUT).is_err());
    }
}
