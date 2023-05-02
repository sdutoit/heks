use memmap2::{Mmap, MmapOptions};
use std::io::{self, ErrorKind};
use std::{cmp::min, fs::File, ops::Range, path::PathBuf};

use crate::cursor::Cursor;

#[derive(Debug, Copy, Clone)]
pub struct Slice<'a> {
    pub data: &'a [u8],
    pub location_start: u64,
    pub location_end: u64,
}

impl<'a> Slice<'a> {
    pub fn align_up(&self, align: u64) -> Slice<'a> {
        let misalignment = self.location_start % align;
        let offset = if misalignment > 0 {
            align - misalignment
        } else {
            0
        };

        let location = (self.location_start + offset).min(self.location_end)..self.location_end;
        let data = &self.data[(offset as usize).min(self.data.len())..];

        Slice {
            data,
            location_start: location.start,
            location_end: location.end,
        }
    }

    pub fn fetch(&self, mut cursor: Cursor) -> Vec<u8> {
        cursor.clamp(self.location_start..self.location_end);
        let range = (cursor.start - self.location_start) as usize
            ..(cursor.end - self.location_start) as usize;

        self.data[range].to_vec()
    }
}

pub trait DataSource {
    fn name(&self) -> &str;
    fn fetch(&mut self, start: u64, end: u64) -> Slice;

    fn fraction(&self, index: u64) -> f64;
}

struct DebugSource {
    buffer: &'static [u8],
}

#[allow(dead_code)]
impl DebugSource {
    fn new() -> Self {
        DebugSource {
            buffer: b"\x09\x00\x06\x00hello\
                      \x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f\
                      \x10\x11\x12\x13\x14\x15\x16\x17\x18\x19\x1a\x1b\x1c\x1d\x1e\x1f\
                      \x7f\x80\x90\xa0\xb0\xc0\xd0\xe0\xf0\xf1\xf2\xf3\xf4\xf5\xf6\xf7\
                      \xf8\xf9\xfa\xfb\xfc\xfd\xfe\xff\
                      world01234567890",
        }
    }
}

impl DataSource for DebugSource {
    fn name(&self) -> &str {
        "debug"
    }

    fn fetch(&mut self, _offset: u64, _end: u64) -> Slice {
        Slice {
            data: self.buffer,
            location_start: 0,
            location_end: self.buffer.len() as u64,
        }
    }

    fn fraction(&self, index: u64) -> f64 {
        let max = (self.buffer.len() - 1) as u64;
        index.clamp(0, max) as f64 / max as f64
    }
}

pub struct FileSource {
    name: String,
    mmap: Mmap,
}

impl FileSource {
    pub fn new(filename: &PathBuf) -> Result<Self, io::Error> {
        let name = filename
            .to_str()
            .ok_or(io::Error::new(
                ErrorKind::Other,
                format!("Unable to parse filename {:?}", filename),
            ))?
            .to_string();
        let file = File::open(filename)?;
        let mmap = unsafe { MmapOptions::new().map(&file)? };

        Ok(FileSource { name, mmap })
    }
}

fn clamp(start: u64, end: u64, len: u64) -> Range<u64> {
    let size = min(end - start, len);

    let mut start = start;
    let mut end = end;

    if start >= len {
        start = len - size;
        end = start + size;
    } else if end >= len {
        end = len;
        start = end - size;
    };

    start..end
}

impl DataSource for FileSource {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn fetch(&mut self, start: u64, end: u64) -> Slice {
        let range = clamp(start, end, self.mmap.len() as u64);

        if !range.is_empty() {
            let range_usize = range.start as usize..range.end as usize;
            Slice {
                data: self.mmap.get(range_usize).unwrap(),
                location_start: range.start,
                location_end: range.end,
            }
        } else {
            Slice {
                data: &[],
                location_start: range.start,
                location_end: range.end,
            }
        }
    }

    fn fraction(&self, index: u64) -> f64 {
        let len = self.mmap.len() as u64;
        if len == 0 {
            0.5
        } else {
            index.clamp(0, len - 1) as f64 / (len - 1) as f64
        }
    }
}
