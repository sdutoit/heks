use memmap2::{Mmap, MmapOptions};
use std::io::{self, ErrorKind};
use std::{cmp::min, fs::File, ops::Range, path::PathBuf};

pub trait DataSource {
    fn name(&self) -> &str;
    fn fetch(&mut self, offset: u64, size: u32) -> &[u8];
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
    fn fetch(&mut self, offset: u64, size: u32) -> &[u8] {
        &self.buffer[clamp(offset, size, self.buffer.len())]
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

fn clamp(offset: u64, size: u32, len: usize) -> Range<usize> {
    let begin: usize = min(offset as usize, len);
    let end: usize = min(offset as usize + size as usize, len);

    begin..end
}

impl DataSource for FileSource {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn fetch(&mut self, offset: u64, size: u32) -> &[u8] {
        let range = clamp(offset, size, self.mmap.len());

        if !range.is_empty() {
            self.mmap.get(range).unwrap()
        } else {
            &[]
        }
    }
}
