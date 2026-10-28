//! Counting an MP3's audio frames, for the seek section of its analysis
//! file. The ID3v2 tag and a Xing/Info header frame don't count, as with
//! rekordbox (found by Bake'n Deck's `expressport`, MIT licence, Copyright
//! (c) Masanari Higashi).

use std::path::Path;

/// MPEG-1 Layer III frames in the file; `None` when it has none.
pub fn frames(path: &Path) -> Option<u32> {
    count(&std::fs::read(path).ok()?)
}

fn count(data: &[u8]) -> Option<u32> {
    const BITRATES: [u32; 16] = [
        0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0,
    ];
    const RATES: [u32; 4] = [44100, 48000, 32000, 0];
    let mut pos = 0usize;
    if data.starts_with(b"ID3") && data.len() > 10 {
        let size = ((data[6] as usize & 0x7f) << 21)
            | ((data[7] as usize & 0x7f) << 14)
            | ((data[8] as usize & 0x7f) << 7)
            | (data[9] as usize & 0x7f);
        let footer = if data[5] & 0x10 != 0 { 10 } else { 0 };
        pos = 10 + size + footer;
    }
    let mut frames = 0u32;
    let mut first = true;
    while pos + 4 <= data.len() {
        let h = &data[pos..pos + 4];
        let sync = h[0] == 0xff && (h[1] & 0xe0) == 0xe0;
        let mpeg1_layer3 = (h[1] & 0x18) == 0x18 && (h[1] & 0x06) == 0x02;
        let bitrate = BITRATES[(h[2] >> 4) as usize];
        let rate = RATES[((h[2] >> 2) & 3) as usize];
        if !(sync && mpeg1_layer3) || bitrate == 0 || rate == 0 {
            if frames == 0 && first {
                pos += 1; // junk before the first frame
                continue;
            }
            break;
        }
        let len = (144 * bitrate * 1000 / rate + ((h[2] >> 1) & 1) as u32) as usize;
        if first {
            first = false;
            let body = &data[pos..(pos + len).min(data.len())];
            if body.windows(4).any(|w| w == b"Xing" || w == b"Info") {
                pos += len;
                continue;
            }
        }
        frames += 1;
        pos += len;
    }
    (frames > 0).then_some(frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(padding: bool) -> Vec<u8> {
        let mut f = vec![0u8; 1044 + padding as usize];
        f[..4].copy_from_slice(&[0xff, 0xfb, 0xe0 | ((padding as u8) << 1), 0]);
        f
    }

    #[test]
    fn tags_and_the_info_frame_are_left_out() {
        let tag_size = 5000usize;
        let mut data = b"ID3\x03\x00\x00".to_vec();
        data.extend((0..4).rev().map(|i| ((tag_size >> (7 * i)) & 0x7f) as u8));
        data.resize(10 + tag_size, 0);
        let mut info = frame(false);
        info[36..40].copy_from_slice(b"Info");
        data.extend(info);
        for i in 0..100 {
            data.extend(frame(i % 9 != 0));
        }
        data.extend(b"TAG");
        data.resize(data.len() + 125, 0);
        assert_eq!(count(&data), Some(100));
        assert_eq!(count(b"not audio"), None);
    }
}
