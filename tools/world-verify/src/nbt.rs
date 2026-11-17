//! Minimal, defensive Java-Edition NBT codec (big-endian, named root tag).
//!
//! The reader is hardened against corrupt input: bounded nesting depth,
//! bounded list/array lengths (checked against remaining bytes before
//! allocating), and rejection of trailing garbage is left to callers.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    /// (element type id, elements)
    List(u8, Vec<Tag>),
    /// Insertion-ordered; duplicate keys are rejected by the reader.
    Compound(Vec<(String, Tag)>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NbtError(pub String);

impl fmt::Display for NbtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for NbtError {}

pub const MAX_DEPTH: usize = 512;

impl Tag {
    pub fn id(&self) -> u8 {
        match self {
            Tag::Byte(_) => 1,
            Tag::Short(_) => 2,
            Tag::Int(_) => 3,
            Tag::Long(_) => 4,
            Tag::Float(_) => 5,
            Tag::Double(_) => 6,
            Tag::ByteArray(_) => 7,
            Tag::String(_) => 8,
            Tag::List(..) => 9,
            Tag::Compound(_) => 10,
            Tag::IntArray(_) => 11,
            Tag::LongArray(_) => 12,
        }
    }

    pub fn get(&self, key: &str) -> Option<&Tag> {
        match self {
            Tag::Compound(v) => v.iter().find(|(k, _)| k == key).map(|(_, t)| t),
            _ => None,
        }
    }

    /// Integer-like value (Byte/Short/Int/Long) widened to i64.
    pub fn as_i64(&self) -> Option<i64> {
        match *self {
            Tag::Byte(v) => Some(v as i64),
            Tag::Short(v) => Some(v as i64),
            Tag::Int(v) => Some(v as i64),
            Tag::Long(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Tag::Float(v) => Some(v as f64),
            Tag::Double(v) => Some(v),
            _ => self.as_i64().map(|v| v as f64),
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Tag::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Tag]> {
        match self {
            Tag::List(_, v) => Some(v),
            _ => None,
        }
    }

    pub fn as_compound(&self) -> Option<&[(String, Tag)]> {
        match self {
            Tag::Compound(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_long_array(&self) -> Option<&[i64]> {
        match self {
            Tag::LongArray(v) => Some(v),
            _ => None,
        }
    }

    /// Structural equality that ignores the given compound keys at any depth.
    pub fn eq_ignoring(&self, other: &Tag, ignore: &[&str]) -> bool {
        match (self, other) {
            (Tag::Compound(a), Tag::Compound(b)) => {
                let fa: Vec<_> = a.iter().filter(|(k, _)| !ignore.contains(&k.as_str())).collect();
                let fb: Vec<_> = b.iter().filter(|(k, _)| !ignore.contains(&k.as_str())).collect();
                fa.len() == fb.len()
                    && fa.iter().all(|(k, v)| {
                        fb.iter().find(|(k2, _)| k2 == k).is_some_and(|(_, v2)| v.eq_ignoring(v2, ignore))
                    })
            }
            (Tag::List(ta, a), Tag::List(tb, b)) => {
                (ta == tb || a.is_empty() && b.is_empty())
                    && a.len() == b.len()
                    && a.iter().zip(b).all(|(x, y)| x.eq_ignoring(y, ignore))
            }
            (Tag::Float(a), Tag::Float(b)) => a.to_bits() == b.to_bits(),
            (Tag::Double(a), Tag::Double(b)) => a.to_bits() == b.to_bits(),
            _ => self == other,
        }
    }
}

struct Reader<'a> {
    d: &'a [u8],
    p: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], NbtError> {
        if n > self.d.len() - self.p {
            return Err(NbtError(format!("nbt: unexpected end of data at offset {} (need {n} bytes)", self.p)));
        }
        let s = &self.d[self.p..self.p + n];
        self.p += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, NbtError> {
        Ok(self.take(1)?[0])
    }
    fn i16(&mut self) -> Result<i16, NbtError> {
        Ok(i16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32, NbtError> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i64(&mut self) -> Result<i64, NbtError> {
        Ok(i64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn len(&mut self, elem_size: usize) -> Result<usize, NbtError> {
        let at = self.p;
        let n = self.i32()?;
        if n < 0 {
            return Err(NbtError(format!("nbt: negative length {n} at offset {at}")));
        }
        let n = n as usize;
        if n.saturating_mul(elem_size.max(1)) > self.d.len() - self.p {
            return Err(NbtError(format!("nbt: length {n} at offset {at} exceeds remaining data")));
        }
        Ok(n)
    }
    fn string(&mut self) -> Result<String, NbtError> {
        let n = self.i16()? as u16 as usize;
        let raw = self.take(n)?;
        // Java "modified UTF-8": treat as UTF-8, replacing invalid sequences
        // (CESU surrogates / 0xC0 0x80) losslessly enough for verification.
        Ok(String::from_utf8_lossy(raw).into_owned())
    }

    fn payload(&mut self, id: u8, depth: usize) -> Result<Tag, NbtError> {
        if depth > MAX_DEPTH {
            return Err(NbtError("nbt: nesting depth limit exceeded".into()));
        }
        Ok(match id {
            1 => Tag::Byte(self.u8()? as i8),
            2 => Tag::Short(self.i16()?),
            3 => Tag::Int(self.i32()?),
            4 => Tag::Long(self.i64()?),
            5 => Tag::Float(f32::from_bits(self.i32()? as u32)),
            6 => Tag::Double(f64::from_bits(self.i64()? as u64)),
            7 => {
                let n = self.len(1)?;
                Tag::ByteArray(self.take(n)?.iter().map(|&b| b as i8).collect())
            }
            8 => Tag::String(self.string()?),
            9 => {
                let et = self.u8()?;
                let n = self.len(if et == 0 { 0 } else { 1 })?;
                if et == 0 && n != 0 {
                    return Err(NbtError("nbt: non-empty list of TAG_End".into()));
                }
                if et > 12 {
                    return Err(NbtError(format!("nbt: invalid list element type {et}")));
                }
                let mut v = Vec::with_capacity(n.min(1 << 16));
                for _ in 0..n {
                    v.push(self.payload(et, depth + 1)?);
                }
                Tag::List(et, v)
            }
            10 => {
                let mut v: Vec<(String, Tag)> = Vec::new();
                loop {
                    let t = self.u8()?;
                    if t == 0 {
                        break;
                    }
                    if t > 12 {
                        return Err(NbtError(format!("nbt: invalid tag type {t} at offset {}", self.p - 1)));
                    }
                    let name = self.string()?;
                    if v.iter().any(|(k, _)| *k == name) {
                        return Err(NbtError(format!("nbt: duplicate compound key {name:?}")));
                    }
                    let val = self.payload(t, depth + 1)?;
                    v.push((name, val));
                }
                Tag::Compound(v)
            }
            11 => {
                let n = self.len(4)?;
                let mut v = Vec::with_capacity(n);
                for _ in 0..n {
                    v.push(self.i32()?);
                }
                Tag::IntArray(v)
            }
            12 => {
                let n = self.len(8)?;
                let mut v = Vec::with_capacity(n);
                for _ in 0..n {
                    v.push(self.i64()?);
                }
                Tag::LongArray(v)
            }
            _ => return Err(NbtError(format!("nbt: invalid tag type {id}"))),
        })
    }
}

/// Parse a complete named-root NBT document. Returns (root name, root tag).
/// Trailing bytes are an error: they indicate a length/framing bug.
pub fn read(data: &[u8]) -> Result<(String, Tag), NbtError> {
    let mut r = Reader { d: data, p: 0 };
    let id = r.u8()?;
    if id != 10 {
        return Err(NbtError(format!("nbt: root tag must be Compound (10), found {id}")));
    }
    let name = r.string()?;
    let tag = r.payload(10, 0)?;
    if r.p != data.len() {
        return Err(NbtError(format!("nbt: {} trailing bytes after root compound", data.len() - r.p)));
    }
    Ok((name, tag))
}

fn write_str(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&(s.len() as u16).to_be_bytes());
    out.extend_from_slice(s.as_bytes());
}

fn write_payload(out: &mut Vec<u8>, t: &Tag) {
    match t {
        Tag::Byte(v) => out.push(*v as u8),
        Tag::Short(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Int(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Long(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Float(v) => out.extend_from_slice(&v.to_bits().to_be_bytes()),
        Tag::Double(v) => out.extend_from_slice(&v.to_bits().to_be_bytes()),
        Tag::ByteArray(v) => {
            out.extend_from_slice(&(v.len() as i32).to_be_bytes());
            out.extend(v.iter().map(|&b| b as u8));
        }
        Tag::String(s) => write_str(out, s),
        Tag::List(et, v) => {
            out.push(if v.is_empty() { *et } else { v[0].id() });
            out.extend_from_slice(&(v.len() as i32).to_be_bytes());
            for e in v {
                write_payload(out, e);
            }
        }
        Tag::Compound(v) => {
            for (k, e) in v {
                out.push(e.id());
                write_str(out, k);
                write_payload(out, e);
            }
            out.push(0);
        }
        Tag::IntArray(v) => {
            out.extend_from_slice(&(v.len() as i32).to_be_bytes());
            for x in v {
                out.extend_from_slice(&x.to_be_bytes());
            }
        }
        Tag::LongArray(v) => {
            out.extend_from_slice(&(v.len() as i32).to_be_bytes());
            for x in v {
                out.extend_from_slice(&x.to_be_bytes());
            }
        }
    }
}

pub fn write(name: &str, root: &Tag) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(root.id());
    write_str(&mut out, name);
    write_payload(&mut out, root);
    out
}

/// Convenience builder for tests/fixtures.
#[macro_export]
macro_rules! compound {
    ($($k:expr => $v:expr),* $(,)?) => {
        $crate::nbt::Tag::Compound(vec![$(($k.to_string(), $v)),*])
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Tag {
        compound! {
            "b" => Tag::Byte(-1),
            "s" => Tag::Short(300),
            "i" => Tag::Int(-70000),
            "l" => Tag::Long(i64::MIN),
            "f" => Tag::Float(1.5),
            "d" => Tag::Double(-2.25),
            "ba" => Tag::ByteArray(vec![1, -2, 3]),
            "str" => Tag::String("minecraft:stone".into()),
            "list" => Tag::List(3, vec![Tag::Int(1), Tag::Int(2)]),
            "empty" => Tag::List(0, vec![]),
            "nested" => compound! { "x" => Tag::Int(5) },
            "ia" => Tag::IntArray(vec![1, 2, 3]),
            "la" => Tag::LongArray(vec![-1, 0, 1]),
        }
    }

    #[test]
    fn roundtrip() {
        let t = sample();
        let bytes = write("", &t);
        let (name, back) = read(&bytes).unwrap();
        assert_eq!(name, "");
        assert_eq!(back, t);
    }

    #[test]
    fn rejects_truncation_everywhere() {
        let bytes = write("root", &sample());
        for cut in 0..bytes.len() {
            assert!(read(&bytes[..cut]).is_err(), "truncation at {cut} accepted");
        }
    }

    #[test]
    fn rejects_trailing_bytes_and_huge_lengths() {
        let mut bytes = write("", &sample());
        bytes.push(0);
        assert!(read(&bytes).is_err());
        // Compound { LongArray "a" len=0x7fffffff } with no payload
        let evil = [10, 0, 0, 12, 0, 1, b'a', 0x7F, 0xFF, 0xFF, 0xFF];
        assert!(read(&evil).unwrap_err().0.contains("exceeds remaining"));
    }

    #[test]
    fn rejects_duplicate_keys() {
        let evil = [10, 0, 0, 1, 0, 1, b'a', 1, 1, 0, 1, b'a', 2, 0];
        assert!(read(&evil).unwrap_err().0.contains("duplicate"));
    }

    #[test]
    fn depth_limit() {
        let mut t = Tag::Compound(vec![]);
        for _ in 0..(MAX_DEPTH + 5) {
            t = Tag::List(10, vec![t]);
            t = compound! { "x" => t };
        }
        let bytes = write("", &t);
        assert!(read(&bytes).unwrap_err().0.contains("depth"));
    }

    #[test]
    fn eq_ignoring_keys() {
        let a = compound! { "LastUpdate" => Tag::Long(1), "v" => Tag::Int(2) };
        let b = compound! { "v" => Tag::Int(2), "LastUpdate" => Tag::Long(99) };
        assert!(a.eq_ignoring(&b, &["LastUpdate"]));
        assert!(!a.eq_ignoring(&b, &[]));
    }
}
