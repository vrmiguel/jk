use std::{
    collections::hash_map::RandomState,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    hash::{BuildHasher, Hash, Hasher},
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use jsax::Parser;

use crate::{source::Source, utils::should_use_colors};

pub fn run(source: Source, write: bool) -> anyhow::Result<()> {
    if write {
        let Source::File(path) = source else {
            unreachable!("CLI argparsing should have rejected `fmt --write` without a file path")
        };
        format_file_in_place(&path)?;
        return Ok(());
    }

    let source = source.load()?;

    let stdout = io::stdout();
    let mut writer = BufWriter::new(stdout.lock());
    format_to_writer(source.as_str()?, should_use_colors(), &mut writer)?;
    writer.flush()?;

    Ok(())
}

fn format_to_writer<W: Write>(
    source: &str,
    use_colors: bool,
    writer: &mut W,
) -> anyhow::Result<()> {
    if use_colors {
        jk::fmt::Formatter::new_colored(Parser::new(source)).format_to(writer)?;
    } else {
        jk::fmt::Formatter::new_plain(Parser::new(source)).format_to(writer)?;
    }

    Ok(())
}

fn format_file_in_place(path: &Path) -> anyhow::Result<()> {
    let source = Source::File(path.to_path_buf()).load_with(false)?;
    let mut output = Vec::with_capacity(source.as_bytes().len() + source.as_bytes().len() / 2);
    format_to_writer(source.as_str()?, false, &mut output)?;

    let metadata = fs::metadata(path)
        .with_context(|| format!("Failed to read metadata for {}", path.display()))?;
    let (temp_path, mut temp_file) = create_temp_file(path)?;

    let write_result = (|| -> anyhow::Result<()> {
        fs::set_permissions(&temp_path, metadata.permissions())
            .with_context(|| format!("Failed to set permissions on {}", temp_path.display()))?;
        temp_file
            .write_all(&output)
            .with_context(|| format!("Failed to write {}", temp_path.display()))?;
        temp_file
            .sync_all()
            .with_context(|| format!("Failed to sync {}", temp_path.display()))?;
        fs::rename(&temp_path, path).with_context(|| {
            format!(
                "Failed to replace {} with {}",
                path.display(),
                temp_path.display()
            )
        })?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    write_result
}

fn create_temp_file(path: &Path) -> anyhow::Result<(PathBuf, File)> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path.file_name().context("Somehow no file name found")?;

    let mut temp_name = OsString::from(".");
    temp_name.push(file_name);
    temp_name.push(".jk-fmt.");
    let suffix = temp_file_suffix(path);
    let suffix = std::str::from_utf8(&suffix).expect("hex digits are valid UTF-8");
    temp_name.push(suffix);
    let temp_path = parent.join(temp_name);

    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .with_context(|| format!("Failed to create {}", temp_path.display()))?;

    Ok((temp_path, file))
}

fn temp_file_suffix(path: &Path) -> [u8; 16] {
    fn hex_u64(value: u64) -> [u8; 16] {
        let mut buf = [0; 16];
        for (idx, byte) in buf.iter_mut().enumerate() {
            let digit = ((value >> ((15 - idx) * 4)) & 0xf) as u8;
            *byte = match digit {
                0..=9 => b'0' + digit,
                _ => b'a' + digit - 10,
            };
        }
        buf
    }

    let mut hasher = RandomState::new().build_hasher();
    path.hash(&mut hasher);
    process::id().hash(&mut hasher);
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
        .hash(&mut hasher);

    hex_u64(hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_json_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("jk-{name}-{}-{nonce}.json", process::id()))
    }

    #[test]
    fn format_file_in_place_writes_pretty_json() {
        let path = temp_json_path("fmt-write");
        fs::write(&path, br#"{"b":2,"a":[1,2]}"#).unwrap();

        format_file_in_place(&path).unwrap();

        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "{\n  \"b\": 2,\n  \"a\": [\n    1,\n    2\n  ]\n}\n"
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn format_file_in_place_leaves_invalid_json_untouched() {
        let path = temp_json_path("fmt-write-invalid");
        fs::write(&path, br#"{"a":1,}"#).unwrap();

        assert!(format_file_in_place(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), r#"{"a":1,}"#);
        let _ = fs::remove_file(path);
    }
}
