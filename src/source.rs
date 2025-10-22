use std::{
    fs::File,
    io::{self, Read},
    path::PathBuf,
    str,
};

use anyhow::Context;
use memmap2::{Mmap, MmapOptions};

/// Where to load data from
pub enum Source {
    Stdin,
    File(PathBuf),
}

pub enum LoadedSource {
    Stdin(Vec<u8>),
    File(Mmap),
}

impl LoadedSource {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            LoadedSource::Stdin(bytes) => bytes,
            LoadedSource::File(mmap) => mmap,
        }
    }

    pub fn as_str(&self) -> anyhow::Result<&str> {
        str::from_utf8(self.as_bytes()).context("Invalid UTF-8 found")
    }
}

impl Source {
    pub fn load(self) -> anyhow::Result<LoadedSource> {
        match self {
            Source::Stdin => {
                let mut buf = Vec::with_capacity(1024);
                io::stdin().lock().read_to_end(&mut buf).unwrap();
                Ok(LoadedSource::Stdin(buf))
            }
            Source::File(path) => {
                let file = File::open(&path)
                    .with_context(|| format!("Failed to open {}", path.display()))?;
                let mmap = unsafe { MmapOptions::new().map(&file)? };

                Ok(LoadedSource::File(mmap))
            }
        }
    }
}
