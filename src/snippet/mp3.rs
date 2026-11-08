//! Cutting an MP3 after its first N seconds, at a frame boundary. The browser only ever receives the part of the song
//! the player has unlocked, so the rest cannot be peeked at in the network tab.

const ID3_HEADER_BYTES: usize = 10;
const ID3_FOOTER_BYTES: usize = 10;
/// Deezer previews are 30 seconds; used to cut proportionally if a file has no readable MP3 frames.
const PREVIEW_SECONDS: f64 = 30.0;
const BITRATES_MPEG1: [u32; 15] = [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320];
const BITRATES_MPEG2: [u32; 15] = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160];

/// The first `seconds` of the song (rounded up to whole frames), including its ID3 tag.
pub fn cut(data: &[u8], seconds: f64) -> &[u8] {
    let start = id3_size(data).min(data.len());
    let mut offset = start;
    let mut elapsed = 0.0;
    while elapsed < seconds {
        match frame_at(data, offset) {
            Some((length, duration)) => {
                offset += length;
                elapsed += duration;
            }
            None => break,
        }
    }
    if offset == start && seconds > 0.0 {
        let share = (seconds / PREVIEW_SECONDS).clamp(0.0, 1.0);
        offset = start + ((data.len() - start) as f64 * share) as usize;
    }
    &data[..offset.min(data.len())]
}

fn id3_size(data: &[u8]) -> usize {
    if data.len() < ID3_HEADER_BYTES || &data[..3] != b"ID3" {
        return 0;
    }
    // Tag sizes are "syncsafe": 7 bits per byte.
    let size = data[6..10].iter().fold(0usize, |size, byte| (size << 7) | usize::from(byte & 0x7F));
    let footer = if data[5] & 0x10 != 0 { ID3_FOOTER_BYTES } else { 0 };
    ID3_HEADER_BYTES + size + footer
}

/// Length in bytes and duration in seconds of the Layer III frame at `offset`, if there is one.
fn frame_at(data: &[u8], offset: usize) -> Option<(usize, f64)> {
    let header = data.get(offset..offset + 4)?;
    if header[0] != 0xFF || header[1] & 0xE0 != 0xE0 {
        return None;
    }
    let version = (header[1] >> 3) & 0b11;
    let layer = (header[1] >> 1) & 0b11;
    if layer != 0b01 || version == 0b01 {
        return None;
    }
    let mpeg1 = version == 0b11;
    let bitrate_index = usize::from(header[2] >> 4);
    let rate_index = usize::from((header[2] >> 2) & 0b11);
    let padding = usize::from((header[2] >> 1) & 1);
    let kbps = if mpeg1 { BITRATES_MPEG1 } else { BITRATES_MPEG2 }.get(bitrate_index).copied().filter(|kbps| *kbps > 0)?;
    let sample_rate = match version {
        0b11 => [44_100, 48_000, 32_000],
        0b10 => [22_050, 24_000, 16_000],
        _ => [11_025, 12_000, 8_000],
    }
    .get(rate_index)
    .copied()?;
    let samples: u32 = if mpeg1 { 1152 } else { 576 };
    let length = (samples / 8 * kbps * 1000 / sample_rate) as usize + padding;
    Some((length, f64::from(samples) / f64::from(sample_rate)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// MPEG-1 Layer III, 128 kbps, 44.1 kHz, no padding: 417 bytes and 26.1 ms per frame.
    const FRAME_HEADER: [u8; 4] = [0xFF, 0xFB, 0x90, 0x64];
    const FRAME_BYTES: usize = 417;

    fn song(frames: usize) -> Vec<u8> {
        let mut data = b"ID3\x04\x00\x00\x00\x00\x00\x05hello".to_vec();
        for _ in 0..frames {
            let mut frame = vec![0u8; FRAME_BYTES];
            frame[..4].copy_from_slice(&FRAME_HEADER);
            data.extend(frame);
        }
        data
    }

    #[test]
    fn cuts_at_whole_frames_after_the_tag() {
        let data = song(200);
        // One second is 38.3 frames, so 39 whole frames.
        assert_eq!(cut(&data, 1.0).len(), 15 + 39 * FRAME_BYTES);
        assert_eq!(cut(&data, 0.0).len(), 15);
        assert_eq!(cut(&data, 60.0).len(), data.len());
    }

    #[test]
    fn unreadable_files_are_cut_proportionally() {
        let data = vec![0u8; 3000];
        assert_eq!(cut(&data, 15.0).len(), 1500);
    }
}
