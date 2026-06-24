use std::{
    fs::{self, File},
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
    Bytes(Vec<u8>),
    Mmap(Mmap),
}

impl LoadedSource {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            LoadedSource::Bytes(bytes) => bytes,
            LoadedSource::Mmap(mmap) => mmap,
        }
    }

    pub fn as_str(&self) -> anyhow::Result<&str> {
        str::from_utf8(self.as_bytes()).context("Invalid UTF-8 found")
    }
}

impl Source {
    pub fn load(self) -> anyhow::Result<LoadedSource> {
        self.load_with(true)
    }

    pub fn load_with(self, use_mmap: bool) -> anyhow::Result<LoadedSource> {
        match self {
            Source::Stdin => {
                let mut buf = Vec::with_capacity(1024);
                io::stdin().lock().read_to_end(&mut buf).unwrap();
                Ok(LoadedSource::Bytes(buf))
            }
            Source::File(path) => {
                if use_mmap {
                    let file = File::open(&path)
                        .with_context(|| format!("Failed to open {}", path.display()))?;
                    let mmap = unsafe { MmapOptions::new().map(&file)? };

                    Ok(LoadedSource::Mmap(mmap))
                } else {
                    fs::read(&path)
                        .with_context(|| format!("Failed to read {}", path.display()))
                        .map(LoadedSource::Bytes)
                }
            }
        }
    }
}
