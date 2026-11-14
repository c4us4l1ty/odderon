//! Extract the four J2000 type-2 segments from a little-endian SPK kernel.
//! The compact format is described in docs/ephemeris.md.
use std::{env, fs, process};

const WANTED: [(i32, i32); 4] = [(3, 0), (10, 0), (301, 3), (399, 3)];

fn bytes<const N: usize>(data: &[u8], offset: usize) -> Result<[u8; N], String> {
    data.get(offset..offset.checked_add(N).ok_or("offset overflow")?)
        .ok_or_else(|| "truncated SPK".to_string())?
        .try_into()
        .map_err(|_| "invalid bytes".to_string())
}
fn i32_at(data: &[u8], offset: usize) -> Result<i32, String> {
    Ok(i32::from_le_bytes(bytes(data, offset)?))
}
fn f64_at(data: &[u8], offset: usize) -> Result<f64, String> {
    Ok(f64::from_le_bytes(bytes(data, offset)?))
}
fn natural(value: f64, max: usize) -> Result<usize, String> {
    if !value.is_finite() || value < 0.0 || value > max as f64 || value.fract() != 0.0 {
        return Err("invalid SPK integer".into());
    }
    Ok(value as usize)
}
fn word_offset(word: i32) -> Result<usize, String> {
    let word = usize::try_from(word).map_err(|_| "negative DAF address")?;
    word.checked_sub(1)
        .and_then(|x| x.checked_mul(8))
        .ok_or_else(|| "invalid DAF address".into())
}
fn put_f64(out: &mut Vec<u8>, v: f64) {
    out.extend(v.to_le_bytes());
}
fn put_i32(out: &mut Vec<u8>, v: i32) {
    out.extend(v.to_le_bytes());
}
fn put_u32(out: &mut Vec<u8>, v: usize) -> Result<(), String> {
    out.extend(u32::try_from(v).map_err(|_| "size overflow")?.to_le_bytes());
    Ok(())
}

fn pack(data: &[u8], range: Option<(i64, i64)>) -> Result<Vec<u8>, String> {
    if range.is_some_and(|(start, end)| start >= end) {
        return Err("range start must precede end".into());
    }
    if data.len() < 1024
        || &data[..8] != b"DAF/SPK "
        || &data[88..96] != b"LTL-IEEE"
        // This detects common text-mode transfer corruption, but does not
        // authenticate the kernel. A trusted digest is still required.
        || &data[699..727] != b"FTPSTR:\r:\n:\r\n:\r\0:\x81:\x10\xce:ENDFTP"
        || i32_at(data, 8)? != 2
        || i32_at(data, 12)? != 6
    {
        return Err("expected a little-endian DAF/SPK with ND=2, NI=6".into());
    }
    let mut found: [Option<Vec<u8>>; 4] = Default::default();
    let mut common_start = i64::MIN;
    let mut common_end = i64::MAX;
    let mut record = usize::try_from(i32_at(data, 76)?).map_err(|_| "invalid summary pointer")?;
    let mut visited = std::collections::BTreeSet::new();
    while record != 0 {
        if !visited.insert(record) {
            return Err("cyclic summary chain".into());
        }
        let off = record
            .checked_sub(1)
            .and_then(|x| x.checked_mul(1024))
            .ok_or("summary offset overflow")?;
        if off.checked_add(1024).ok_or("summary overflow")? > data.len() {
            return Err("truncated summary record".into());
        }
        let next = natural(f64_at(data, off)?, data.len() / 1024)?;
        let n = natural(f64_at(data, off + 16)?, 25)?;
        for i in 0..n {
            let p = off + 24 + i * 40;
            let start = f64_at(data, p)?;
            let end = f64_at(data, p + 8)?;
            let target = i32_at(data, p + 16)?;
            let center = i32_at(data, p + 20)?;
            if let Some(which) = WANTED.iter().position(|x| *x == (target, center)) {
                if found[which].is_some() {
                    return Err(format!(
                        "multiple segments for {target}; merge or subset the input first"
                    ));
                }
                if i32_at(data, p + 24)? != 1 || i32_at(data, p + 28)? != 2 {
                    return Err(format!("target {target} must be J2000 SPK type 2"));
                }
                let a = word_offset(i32_at(data, p + 32)?)?;
                let last = word_offset(i32_at(data, p + 36)?)?;
                let trailer = last.checked_sub(24).ok_or("invalid segment address")?;
                let init = f64_at(data, trailer)?;
                let interval = f64_at(data, trailer + 8)?;
                let size = natural(f64_at(data, trailer + 16)?, 10_000)?;
                let count = natural(f64_at(data, trailer + 24)?, 10_000_000)?;
                if ![start, end, init, interval].iter().all(|v| {
                    v.is_finite() && v.fract() == 0.0 && v.abs() <= 9_007_199_254_740_992.0
                }) || start >= end
                    || interval <= 0.0
                    || size < 5
                    || (size - 2) % 3 != 0
                    || count == 0
                {
                    return Err("invalid type-2 metadata".into());
                }
                let ncoef = (size - 2) / 3;
                if ncoef > 64
                    || a.checked_add(
                        count
                            .checked_mul(size)
                            .and_then(|v| v.checked_mul(8))
                            .ok_or("segment size overflow")?,
                    )
                    .ok_or("segment end overflow")?
                        != trailer
                    || start < init
                    || (end as i128) > (init as i128) + (count as i128) * (interval as i128)
                {
                    return Err("inconsistent type-2 record layout or coverage".into());
                }
                // Choose every original Chebyshev record intersecting the
                // requested half-open range, without fitting new coefficients.
                let (clip_start, clip_end) = range.unwrap_or((start as i64, end as i64));
                if clip_start < start as i64 || clip_end > end as i64 {
                    return Err(format!("requested range outside target {target} coverage"));
                }
                common_start = common_start.max(clip_start);
                common_end = common_end.min(clip_end);
                let origin = init as i128;
                let stride = interval as i128;
                let first = usize::try_from((i128::from(clip_start) - origin) / stride)
                    .map_err(|_| "invalid first record")?;
                let last_exclusive =
                    usize::try_from((i128::from(clip_end) - origin + stride - 1) / stride)
                        .map_err(|_| "invalid last record")?;
                if first >= last_exclusive || last_exclusive > count {
                    return Err("range outside records".into());
                }
                let clip_init = origin + first as i128 * stride;
                if clip_init.abs() > 9_007_199_254_740_992_i128 {
                    return Err("clipped origin not exactly representable".into());
                }
                let mut block = Vec::new();
                put_i32(&mut block, target);
                put_i32(&mut block, center);
                for v in [
                    clip_start as f64,
                    clip_end as f64,
                    clip_init as f64,
                    interval,
                ] {
                    put_f64(&mut block, v);
                }
                put_u32(&mut block, ncoef)?;
                put_u32(&mut block, last_exclusive - first)?;
                for rec in first..last_exclusive {
                    let pos = a + rec * size * 8;
                    let mid = f64_at(data, pos)?;
                    let radius = f64_at(data, pos + 8)?;
                    if mid != init + (rec as f64 + 0.5) * interval || radius != interval / 2.0 {
                        return Err("type-2 record timing does not match directory".into());
                    }
                    for j in 0..3 * ncoef {
                        let coefficient = f64_at(data, pos + 16 + j * 8)?;
                        if !coefficient.is_finite() {
                            return Err("nonfinite coefficient".into());
                        }
                        put_f64(&mut block, coefficient);
                    }
                }
                found[which] = Some(block);
            }
        }
        record = next;
    }
    if common_start >= common_end {
        return Err("selected segments have no common coverage".into());
    }
    let mut out = b"ODEPHEM1".to_vec();
    put_u32(&mut out, WANTED.len())?;
    for (i, block) in found.into_iter().enumerate() {
        out.extend(block.ok_or_else(|| {
            format!("missing target {} relative to {}", WANTED[i].0, WANTED[i].1)
        })?);
    }
    Ok(out)
}
fn main() {
    if let Err(e) = run() {
        eprintln!("pack-ephem: {e}");
        process::exit(1);
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    let (input, output_path, range) = match args.as_slice() {
        [_, input, output] => (input, output, None),
        [_, flag, start, end, input, output] if flag == "--range" => {
            let start = start.to_str().ok_or("invalid start")?.parse::<i64>()?;
            let end = end.to_str().ok_or("invalid end")?.parse::<i64>()?;
            (input, output, Some((start, end)))
        }
        _ => {
            return Err(
                "usage: pack-ephem [--range START_TDB_S END_TDB_S] INPUT.bsp OUTPUT.ephb".into(),
            )
        }
    };
    let data = fs::read(input)?;
    let output = pack(&data, range)?;
    fs::write(output_path, &output)?;
    println!("packed {} bytes", output.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn synthetic() -> Vec<u8> {
        // A minimal DAF containing four single-record degree-one segments.
        let mut b = vec![0u8; 4096 + 4 * 96];
        b[..8].copy_from_slice(b"DAF/SPK ");
        b[8..12].copy_from_slice(&2i32.to_le_bytes());
        b[12..16].copy_from_slice(&6i32.to_le_bytes());
        b[76..80].copy_from_slice(&3i32.to_le_bytes());
        b[88..96].copy_from_slice(b"LTL-IEEE");
        b[699..727].copy_from_slice(b"FTPSTR:\r:\n:\r\n:\r\0:\x81:\x10\xce:ENDFTP");
        b[2048 + 16..2048 + 24].copy_from_slice(&4.0f64.to_le_bytes());
        for (i, (target, center)) in WANTED.iter().enumerate() {
            let p = 2048 + 24 + i * 40;
            b[p..p + 8].copy_from_slice(&0.0f64.to_le_bytes());
            b[p + 8..p + 16].copy_from_slice(&10.0f64.to_le_bytes());
            b[p + 16..p + 20].copy_from_slice(&target.to_le_bytes());
            b[p + 20..p + 24].copy_from_slice(&center.to_le_bytes());
            b[p + 24..p + 28].copy_from_slice(&1i32.to_le_bytes());
            b[p + 28..p + 32].copy_from_slice(&2i32.to_le_bytes());
            let offset = 4096 + i * 96;
            let first = (offset / 8 + 1) as i32;
            let last = first + 11;
            b[p + 32..p + 36].copy_from_slice(&first.to_le_bytes());
            b[p + 36..p + 40].copy_from_slice(&last.to_le_bytes());
            let words = [5.0, 5.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 0.0, 10.0, 8.0, 1.0];
            for (j, x) in words.iter().enumerate() {
                b[offset + j * 8..offset + j * 8 + 8].copy_from_slice(&f64::to_le_bytes(*x));
            }
        }
        b
    }
    #[test]
    fn clipping_respects_half_open_record_boundaries() {
        let mut b = synthetic();
        const SEGMENT_BYTES: usize = (8 * 3 + 4) * 8;
        b.resize(4096 + 4 * SEGMENT_BYTES, 0);
        for i in 0..4 {
            let p = 2048 + 24 + i * 40;
            b[p + 8..p + 16].copy_from_slice(&30.0_f64.to_le_bytes());
            let offset = 4096 + i * SEGMENT_BYTES;
            let first = (offset / 8 + 1) as i32;
            let last = first + (SEGMENT_BYTES / 8) as i32 - 1;
            b[p + 32..p + 36].copy_from_slice(&first.to_le_bytes());
            b[p + 36..p + 40].copy_from_slice(&last.to_le_bytes());
            for rec in 0..3 {
                let words = [
                    5.0 + 10.0 * rec as f64,
                    5.0,
                    1.0 + rec as f64,
                    2.0,
                    3.0,
                    4.0,
                    5.0,
                    6.0,
                ];
                for (j, value) in words.iter().enumerate() {
                    let pos = offset + (8 * rec + j) * 8;
                    b[pos..pos + 8].copy_from_slice(&f64::to_le_bytes(*value));
                }
            }
            for (j, value) in [0.0_f64, 10.0, 8.0, 3.0].iter().enumerate() {
                let pos = offset + (24 + j) * 8;
                b[pos..pos + 8].copy_from_slice(&value.to_le_bytes());
            }
        }
        let clipped = pack(&b, Some((11, 20))).unwrap();
        assert_eq!(f64_at(&clipped, 20).unwrap(), 11.0);
        assert_eq!(f64_at(&clipped, 28).unwrap(), 20.0);
        assert_eq!(f64_at(&clipped, 36).unwrap(), 10.0);
        assert_eq!(u32::from_le_bytes(bytes(&clipped, 56).unwrap()), 1);
        assert_eq!(f64_at(&clipped, 60).unwrap(), 2.0);
        let two_records = pack(&b, Some((11, 21))).unwrap();
        assert_eq!(u32::from_le_bytes(bytes(&two_records, 56).unwrap()), 2);
        assert_eq!(f64_at(&two_records, 60 + 48).unwrap(), 3.0);
    }

    #[test]
    fn extracts_and_rejects_bad_data() {
        let good = synthetic();
        let packed = pack(&good, None).unwrap();
        assert_eq!(&packed[..8], b"ODEPHEM1");
        assert_eq!(packed.len(), 12 + 4 * (48 + 6 * 8));
        let clipped = pack(&good, Some((2, 9))).unwrap();
        assert_eq!(f64_at(&clipped, 20).unwrap(), 2.0);
        assert_eq!(f64_at(&clipped, 28).unwrap(), 9.0);
        assert_eq!(f64_at(&clipped, 36).unwrap(), 0.0);
        assert!(pack(&good, Some((-1, 5))).is_err());
        assert!(pack(&good, Some((0, 11))).is_err());
        assert!(pack(&good, Some((5, 5))).is_err());
        assert!(pack(&good[..200], None).is_err());
        let mut corrupt_header = good.clone();
        corrupt_header[706] = 0;
        assert!(pack(&corrupt_header, None).is_err());
        // All four segments are valid, but one has disjoint coverage.
        let mut disjoint = good.clone();
        let summary = 2048 + 24;
        disjoint[summary..summary + 8].copy_from_slice(&20.0_f64.to_le_bytes());
        disjoint[summary + 8..summary + 16].copy_from_slice(&30.0_f64.to_le_bytes());
        disjoint[4096..4104].copy_from_slice(&25.0_f64.to_le_bytes());
        disjoint[4096 + 64..4096 + 72].copy_from_slice(&20.0_f64.to_le_bytes());
        assert_eq!(
            pack(&disjoint, None).unwrap_err(),
            "selected segments have no common coverage"
        );
        let mut bad = good.clone();
        bad[2048..2056].copy_from_slice(&3f64.to_le_bytes());
        assert!(pack(&bad, None).is_err());
        let mut bad = good.clone();
        bad[2048 + 24 + 28..2048 + 24 + 32].copy_from_slice(&3i32.to_le_bytes());
        assert!(pack(&bad, None).is_err());
        let mut bad = good.clone();
        bad[4096..4104].copy_from_slice(&99f64.to_le_bytes());
        assert!(pack(&bad, None).is_err());
        let mut bad = good.clone();
        bad[4096 + 16..4096 + 24].copy_from_slice(&f64::NAN.to_le_bytes());
        assert!(pack(&bad, None).is_err());
    }
}
