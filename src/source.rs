use memmap2::{Mmap, MmapOptions};
use std::io::{self, ErrorKind};
use std::{cmp::min, fs::File, ops::Range, path::PathBuf};

#[derive(Debug)]
pub struct Slice<'a> {
    pub data: &'a [u8],
    pub location: Range<u64>,
}

impl<'a> Slice<'a> {
    pub fn align_up(&self, align: u64) -> Slice<'a> {
        let misalignment = self.location.start % align;
        let offset = if misalignment > 0 {
            align - misalignment
        } else {
            0
        };

        let location = (self.location.start + offset).min(self.location.end)..self.location.end;
        let data = &self.data[(offset as usize).min(self.data.len())..];

        Slice { data, location }
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
            location: 0..self.buffer.len() as u64,
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
                location: range,
            }
        } else {
            Slice {
                data: &[],
                location: range,
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
