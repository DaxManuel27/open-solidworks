//! Reading files out of a zip archive (stored or deflated entries), for 3MF
//! and zipped AMF.

use super::io;
use crate::Result;

fn u16_at(b: &[u8], at: usize) -> Result<usize> {
    b.get(at..at + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]) as usize)
        .ok_or_else(|| io("the archive is cut short"))
}

fn u32_at(b: &[u8], at: usize) -> Result<usize> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as usize)
        .ok_or_else(|| io("the archive is cut short"))
}

/// Every file in the archive whose name `want` accepts: (name, contents).
pub fn files(bytes: &[u8], want: impl Fn(&str) -> bool) -> Result<Vec<(String, Vec<u8>)>> {
    // The end of central directory record is in the last 64 KiB.
    let start = bytes.len().saturating_sub(65_557);
    let eocd = (start..bytes.len().saturating_sub(21))
        .rev()
        .find(|&i| bytes[i..i + 4] == [0x50, 0x4b, 0x05, 0x06])
        .ok_or_else(|| io("not a zip archive"))?;
    let entries = u16_at(bytes, eocd + 10)?;
    let mut at = u32_at(bytes, eocd + 16)?;
    let mut out = Vec::new();
    for _ in 0..entries {
        if u32_at(bytes, at)? != 0x0201_4b50 {
            return Err(io("the archive's directory is damaged"));
        }
        let method = u16_at(bytes, at + 10)?;
        let compressed = u32_at(bytes, at + 20)?;
        let name_len = u16_at(bytes, at + 28)?;
        let extra_len = u16_at(bytes, at + 30)?;
        let comment_len = u16_at(bytes, at + 32)?;
        let local = u32_at(bytes, at + 42)?;
        let name = String::from_utf8_lossy(
            bytes
                .get(at + 46..at + 46 + name_len)
                .ok_or_else(|| io("the archive is cut short"))?,
        )
        .to_string();
        at += 46 + name_len + extra_len + comment_len;
        if !want(&name) {
            continue;
        }
        let lname = u16_at(bytes, local + 26)?;
        let lextra = u16_at(bytes, local + 28)?;
        let data_at = local + 30 + lname + lextra;
        let data = bytes
            .get(data_at..data_at + compressed)
            .ok_or_else(|| io("the archive is cut short"))?;
        let contents = match method {
            0 => data.to_vec(),
            8 => miniz_oxide::inflate::decompress_to_vec(data)
                .map_err(|_| io(format!("{name} in the archive is damaged")))?,
            _ => return Err(io(format!("{name} uses an unsupported compression"))),
        };
        out.push((name, contents));
    }
    Ok(out)
}

#[cfg(test)]
pub(crate) fn store(files: &[(&str, &[u8])], deflate: bool) -> Vec<u8> {
    // A minimal writer for tests.
    let mut out = Vec::new();
    let mut dir = Vec::new();
    for (name, data) in files {
        let body = if deflate {
            miniz_oxide::deflate::compress_to_vec(data, 6)
        } else {
            data.to_vec()
        };
        let offset = out.len() as u32;
        let method: u16 = if deflate { 8 } else { 0 };
        out.extend(0x0403_4b50u32.to_le_bytes());
        out.extend([20u8, 0, 0, 0]);
        out.extend(method.to_le_bytes());
        out.extend([0u8; 8]);
        out.extend((body.len() as u32).to_le_bytes());
        out.extend((data.len() as u32).to_le_bytes());
        out.extend((name.len() as u16).to_le_bytes());
        out.extend(0u16.to_le_bytes());
        out.extend(name.as_bytes());
        out.extend(&body);
        dir.extend(0x0201_4b50u32.to_le_bytes());
        dir.extend([20u8, 0, 20, 0, 0, 0]);
        dir.extend(method.to_le_bytes());
        dir.extend([0u8; 8]);
        dir.extend((body.len() as u32).to_le_bytes());
        dir.extend((data.len() as u32).to_le_bytes());
        dir.extend((name.len() as u16).to_le_bytes());
        dir.extend([0u8; 12]);
        dir.extend(offset.to_le_bytes());
        dir.extend(name.as_bytes());
    }
    let dir_at = out.len() as u32;
    out.extend(&dir);
    out.extend(0x0605_4b50u32.to_le_bytes());
    out.extend([0u8; 4]);
    out.extend((files.len() as u16).to_le_bytes());
    out.extend((files.len() as u16).to_le_bytes());
    out.extend((dir.len() as u32).to_le_bytes());
    out.extend(dir_at.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn stored_and_deflated() {
        for deflate in [false, true] {
            let z = super::store(&[("a.txt", b"hello hello hello"), ("b", b"x")], deflate);
            let got = super::files(&z, |n| n == "a.txt").unwrap();
            assert_eq!(
                got,
                vec![("a.txt".to_string(), b"hello hello hello".to_vec())]
            );
        }
    }
}
