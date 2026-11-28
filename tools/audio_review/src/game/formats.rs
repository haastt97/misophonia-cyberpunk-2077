use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

use anyhow::{Context, Result, ensure};
use byteorder::{ByteOrder, LittleEndian as Le};
use serde::{Deserialize, Serialize};

const INDEX_HEADER: usize = 28;
const ENTRY_SIZE: usize = 56;
const SEGMENT_SIZE: usize = 16;
const WWISE_INDEX_ENTRY: usize = 12;
pub(super) const MAX_BLOCK: u32 = 512 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub(super) struct Segment {
    pub offset: u64,
    pub stored: u32,
    pub size: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Member {
    pub media_id: u32,
    pub start: u64,
    pub length: u32,
}

pub(super) fn read_at(stream: &mut (impl Read + Seek), offset: u64, data: &mut [u8]) -> Result<()> {
    stream.seek(SeekFrom::Start(offset))?;
    stream
        .read_exact(data)
        .context("Audio archive changed or a segment is truncated")?;
    Ok(())
}

// RED4 archive and Opus pack layouts follow WolvenKit's ArchiveReader and OpusInfo.
pub(super) fn archive_entries(path: &Path) -> Result<Vec<(u64, Segment)>> {
    let mut file = File::open(path)?;
    let file_size = file.metadata()?.len();
    let mut header = [0; 20];
    file.read_exact(&mut header)
        .context("Truncated archive header")?;
    ensure!(
        &header[..4] == b"RDAR" && Le::read_u32(&header[4..]) == 12,
        "Unsupported archive version"
    );

    let index_offset = Le::read_u64(&header[8..]);
    let index_size = Le::read_u32(&header[16..]);
    ensure!(
        (INDEX_HEADER as u32..=MAX_BLOCK).contains(&index_size)
            && index_offset <= file_size
            && u64::from(index_size) <= file_size - index_offset,
        "Invalid archive index range"
    );
    let mut index = vec![0; index_size as usize];
    read_at(&mut file, index_offset, &mut index)?;

    let entries = Le::read_u32(&index[16..]) as usize;
    let segments = Le::read_u32(&index[20..]) as usize;
    let dependencies = Le::read_u32(&index[24..]) as usize;
    let tables = &index[INDEX_HEADER..];
    ensure!(
        entries <= tables.len() / ENTRY_SIZE,
        "Truncated archive file table"
    );
    let (entries, tables) = tables.split_at(entries * ENTRY_SIZE);
    ensure!(
        segments <= tables.len() / SEGMENT_SIZE,
        "Truncated archive segment table"
    );
    let (segment_table, dependencies_table) = tables.split_at(segments * SEGMENT_SIZE);
    ensure!(
        dependencies <= dependencies_table.len() / 8,
        "Truncated archive dependency table"
    );

    let mut result = Vec::with_capacity(entries.len() / ENTRY_SIZE);
    for entry in entries.as_chunks::<ENTRY_SIZE>().0 {
        let id = Le::read_u64(entry);
        let start = Le::read_u32(&entry[20..]) as usize;
        let end = Le::read_u32(&entry[24..]) as usize;
        ensure!(
            start < end && end <= segments,
            "Invalid segment range for {id}"
        );
        let data = &segment_table[start * SEGMENT_SIZE..];
        let segment = Segment {
            offset: Le::read_u64(data),
            stored: Le::read_u32(&data[8..]),
            size: Le::read_u32(&data[12..]),
        };
        ensure!(
            segment.offset <= file_size && u64::from(segment.stored) <= file_size - segment.offset,
            "Invalid segment data for {id}"
        );
        // Raw media occupies one segment. Exclude multi-segment data such as voiceset metadata.
        if end == start + 1 {
            result.push((id, segment));
        }
    }
    result.sort_by_key(|(_, segment)| segment.offset);
    Ok(result)
}

pub(super) fn opus_members(info: &[u8]) -> Result<impl Iterator<Item = (u16, Member)> + '_> {
    ensure!(
        info.len() >= 20 && info.starts_with(b"SND "),
        "Unrecognized OpusInfo header"
    );
    let count = Le::read_u32(&info[12..]) as usize;
    let tables = &info[20..];
    ensure!(count <= tables.len() / 20, "Truncated OpusInfo index");
    Ok((0..count).filter_map(move |index| {
        let media_id = Le::read_u32(&tables[index * 4..]);
        let pack = Le::read_u16(&tables[count * 4 + index * 2..]);
        let offset = Le::read_u32(&tables[count * 6 + index * 4..]);
        let header = u32::from(Le::read_u16(&tables[count * 10 + index * 2..]));
        let length = Le::read_u32(&tables[count * 12 + index * 4..]);
        (length > header).then(|| {
            (
                pack,
                Member {
                    media_id,
                    start: u64::from(offset) + u64::from(header),
                    length: length - header,
                },
            )
        })
    }))
}

pub(super) fn bank_members(
    stream: &mut (impl Read + Seek),
    base: u64,
    size: u32,
) -> Result<Vec<Member>> {
    let mut header = [0; 8];
    let mut index = None;
    let mut data = None;
    let mut cursor = 0;
    while size - cursor >= 8 {
        read_at(stream, base + u64::from(cursor), &mut header)?;
        let length = Le::read_u32(&header[4..]);
        cursor += 8;
        ensure!(
            length <= size - cursor,
            "Soundbank contains a truncated section"
        );
        match &header[..4] {
            b"DIDX" => index = Some((cursor, length)),
            b"DATA" => data = Some((cursor, length)),
            _ => {}
        }
        cursor += length;
    }
    let (Some((index, index_len)), Some((data, data_len))) = (index, data) else {
        return Ok(Vec::new());
    };
    ensure!(
        index_len.is_multiple_of(WWISE_INDEX_ENTRY as u32) && index_len <= MAX_BLOCK,
        "Soundbank media index is malformed"
    );
    let mut records = vec![0; index_len as usize];
    read_at(stream, base + u64::from(index), &mut records)?;

    let mut result = Vec::new();
    let mut riff = [0; 12];
    for record in records.as_chunks::<WWISE_INDEX_ENTRY>().0 {
        let media_id = Le::read_u32(record);
        let offset = Le::read_u32(&record[4..]);
        let length = Le::read_u32(&record[8..]);
        if length < 12 || offset > data_len || length > data_len - offset {
            continue;
        }
        let start = u64::from(data) + u64::from(offset);
        read_at(stream, base + start, &mut riff)?;
        // Exclude prefetch prefixes for separately stored WEMs.
        if riff.starts_with(b"RIFF") && &riff[8..] == b"WAVE" {
            let media_length = u64::from(Le::read_u32(&riff[4..])) + 8;
            if (12..=u64::from(length)).contains(&media_length) {
                result.push(Member {
                    media_id,
                    start,
                    length: media_length as u32,
                });
            }
        }
    }
    Ok(result)
}

pub(super) fn audio_suffix(header: &[u8]) -> &'static str {
    if header.starts_with(b"RIFF") && header.get(8..12) == Some(b"WAVE") {
        ".wem"
    } else if header.starts_with(b"BKHD") {
        ".bnk"
    } else if header.starts_with(b"OggS") {
        ".ogg"
    } else if header.starts_with(b"SND ") {
        ".opusinfo"
    } else if header.starts_with(b"fLaC") {
        ".flac"
    } else {
        ""
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};

    #[test]
    fn archive_uses_single_segment_entries_and_validates_segment_bounds() -> Result<()> {
        let mut archive = tempfile::NamedTempFile::new()?;
        let mut file = vec![0u8; 20 + INDEX_HEADER + ENTRY_SIZE + SEGMENT_SIZE];
        file[..4].copy_from_slice(b"RDAR");
        Le::write_u32(&mut file[4..], 12);
        Le::write_u64(&mut file[8..], 20);
        Le::write_u32(
            &mut file[16..],
            (INDEX_HEADER + ENTRY_SIZE + SEGMENT_SIZE) as u32,
        );
        Le::write_u32(&mut file[20 + 16..], 1);
        Le::write_u32(&mut file[20 + 20..], 1);
        let start = 20 + INDEX_HEADER;
        Le::write_u64(&mut file[start..], 42);
        Le::write_u32(&mut file[start + 24..], 1);
        let end = start + ENTRY_SIZE;
        Le::write_u32(&mut file[end + 8..], 16);
        Le::write_u32(&mut file[end + 12..], 16);
        archive.write_all(&file)?;
        assert_eq!(
            archive_entries(archive.path())?,
            vec![(
                42,
                Segment {
                    offset: 0,
                    stored: 16,
                    size: 16
                }
            )]
        );

        Le::write_u64(&mut file[end..], u64::MAX);
        std::fs::write(archive.path(), file)?;
        assert!(archive_entries(archive.path()).is_err());
        Ok(())
    }

    #[test]
    fn opus_members_skip_the_pack_header() -> Result<()> {
        let mut info = [0; 40];
        info[..4].copy_from_slice(b"SND ");
        Le::write_u32(&mut info[12..], 1);
        Le::write_u32(&mut info[20..], 42);
        Le::write_u16(&mut info[24..], 2);
        Le::write_u32(&mut info[26..], 100);
        Le::write_u16(&mut info[30..], 12);
        Le::write_u32(&mut info[32..], 24);
        assert_eq!(
            opus_members(&info)?.collect::<Vec<_>>(),
            vec![(
                2,
                Member {
                    media_id: 42,
                    start: 112,
                    length: 12
                }
            )]
        );
        assert!(opus_members(&info[..39]).is_err());
        Ok(())
    }

    #[test]
    fn bank_members_exclude_prefetch_prefixes() -> Result<()> {
        let mut bank = b"DIDX\x18\0\0\0".to_vec();
        for (id, offset) in [(42u32, 0u32), (43, 12)] {
            bank.extend(id.to_le_bytes());
            bank.extend(offset.to_le_bytes());
            bank.extend(12u32.to_le_bytes());
        }
        bank.extend(b"DATA\x18\0\0\0RIFF\x04\0\0\0WAVERIFF\x40\0\0\0WAVE");
        assert_eq!(
            bank_members(&mut Cursor::new(&bank), 0, bank.len() as u32)?,
            vec![Member {
                media_id: 42,
                start: 40,
                length: 12
            }]
        );
        assert!(bank_members(&mut Cursor::new(&bank), 0, bank.len() as u32 - 1).is_err());
        Ok(())
    }
}
