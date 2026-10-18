//! Quake 2 `.pak` archive support
use bevy::prelude::*;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use thiserror::Error;

const MAGIC: &[u8; 4] = b"PACK";
const HEADER_BYTES: usize = 12;
const ENTRY_BYTES: usize = 64;
const NAME_BYTES: usize = 56;

#[derive(Debug, Error)]
pub enum PakError {
    #[error("Failed to read PAK file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid PAK format: {0}")]
    InvalidFormat(String),
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    pos: u64,
    len: u64,
}

/// A single `.pak` archive
#[derive(Debug)]
pub struct Pak {
    path: PathBuf,
    entries: HashMap<String, Entry>,
}

impl Pak {
    pub fn open(path: &Path) -> Result<Pak, PakError> {
        let mut file = File::open(path)?;
        let file_len = file.metadata()?.len();

        let mut hdr = [0u8; HEADER_BYTES];
        file.read_exact(&mut hdr)
            .map_err(|_| PakError::InvalidFormat("Not enough bytes for header".to_string()))?;

        if &hdr[0..4] != MAGIC {
            return Err(PakError::InvalidFormat("Not a PACK file".to_string()));
        }

        let dir_ofs = i32::from_le_bytes(hdr[4..8].try_into().unwrap());
        let dir_len = i32::from_le_bytes(hdr[8..12].try_into().unwrap());
        let dir_ofs = u64::try_from(dir_ofs)
            .map_err(|_| PakError::InvalidFormat("Negative directory offset".to_string()))?;
        let dir_len = u64::try_from(dir_len)
            .map_err(|_| PakError::InvalidFormat("Negative directory length".to_string()))?;

        if dir_len % ENTRY_BYTES as u64 != 0 {
            return Err(PakError::InvalidFormat(
                "Directory length is not a multiple of the entry size".to_string(),
            ));
        }
        if dir_ofs
            .checked_add(dir_len)
            .is_none_or(|end| end > file_len)
        {
            return Err(PakError::InvalidFormat(
                "Directory extends past end of file".to_string(),
            ));
        }

        let mut dir = vec![0u8; dir_len as usize];
        file.seek(SeekFrom::Start(dir_ofs))?;
        file.read_exact(&mut dir)?;

        let mut entries = HashMap::new();
        for raw in dir.as_chunks::<ENTRY_BYTES>().0 {
            let name_end = raw[..NAME_BYTES]
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(NAME_BYTES);
            let name = String::from_utf8_lossy(&raw[..name_end]).to_lowercase();
            let pos = i32::from_le_bytes(raw[NAME_BYTES..NAME_BYTES + 4].try_into().unwrap());
            let len = i32::from_le_bytes(raw[NAME_BYTES + 4..].try_into().unwrap());

            let (Ok(pos), Ok(len)) = (u64::try_from(pos), u64::try_from(len)) else {
                return Err(PakError::InvalidFormat(format!(
                    "Negative position or length for '{name}'"
                )));
            };
            if pos.checked_add(len).is_none_or(|end| end > file_len) {
                return Err(PakError::InvalidFormat(format!(
                    "Entry '{name}' extends past end of file"
                )));
            }

            // keep the first entry when a name is duplicated
            entries.entry(name).or_insert(Entry { pos, len });
        }

        Ok(Pak {
            path: path.to_path_buf(),
            entries,
        })
    }

    pub fn contains(&self, name: &str) -> bool {
        self.entries.contains_key(&name.to_lowercase())
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    /// Read the bytes of an entry, `None` if there is no such entry
    pub fn read(&self, name: &str) -> Option<std::io::Result<Vec<u8>>> {
        let entry = self.entries.get(&name.to_lowercase())?;
        Some(self.read_entry(entry))
    }

    fn read_entry(&self, entry: &Entry) -> std::io::Result<Vec<u8>> {
        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(entry.pos))?;
        let mut bytes = vec![0u8; entry.len as usize];
        file.read_exact(&mut bytes)?;
        Ok(bytes)
    }
}

/// An ordered set of archives where later archives override earlier ones
#[derive(Debug, Default)]
pub struct PakSet {
    paks: Vec<Pak>,
}

impl PakSet {
    pub fn new(paks: Vec<Pak>) -> Self {
        PakSet { paks }
    }

    /// Load every `pak*.pak` file in a directory, sorted by file name
    pub fn load_dir(dpath: &Path) -> PakSet {
        let mut fpaths: Vec<PathBuf> = std::fs::read_dir(dpath)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                let name = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default()
                    .to_lowercase();
                name.starts_with("pak") && name.ends_with(".pak")
            })
            .collect();
        fpaths.sort();

        let mut paks = Vec::new();
        for fpath in fpaths {
            match Pak::open(&fpath) {
                Ok(pak) => {
                    info!("loaded {} ({} entries)", fpath.display(), pak.entries.len());
                    paks.push(pak);
                }
                Err(err) => warn!("skipping {}: {err}", fpath.display()),
            }
        }

        PakSet { paks }
    }

    pub fn is_empty(&self) -> bool {
        self.paks.is_empty()
    }

    pub fn read(&self, name: &str) -> Option<std::io::Result<Vec<u8>>> {
        self.paks.iter().rev().find_map(|pak| pak.read(name))
    }

    /// All distinct entry names, sorted
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .paks
            .iter()
            .flat_map(|p| p.names())
            .map(str::to_string)
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// Entries ending in `ext` (e.g. `.md2`), sorted
    pub fn find_ext(&self, ext: &str) -> Vec<String> {
        self.names()
            .into_iter()
            .filter(|n| n.ends_with(ext))
            .collect()
    }

    /// Entries directly inside `dir` (e.g. `models/monsters/berserk`) ending in one of `exts`
    pub fn find_in_dir(&self, dir: &str, exts: &[&str]) -> Vec<String> {
        let dir = dir.to_lowercase();
        self.names()
            .into_iter()
            .filter(|n| {
                n.rsplit_once('/').is_some_and(|(parent, _)| parent == dir)
                    && exts.iter().any(|ext| n.ends_with(ext))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Build PAK bytes from (name, data) pairs
    fn build_pak(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut data = Vec::new();
        let mut dir = Vec::new();
        for (name, bytes) in files {
            let pos = (HEADER_BYTES + data.len()) as i32;
            data.extend_from_slice(bytes);
            let mut raw = [0u8; NAME_BYTES];
            raw[..name.len()].copy_from_slice(name.as_bytes());
            dir.extend_from_slice(&raw);
            dir.extend_from_slice(&pos.to_le_bytes());
            dir.extend_from_slice(&(bytes.len() as i32).to_le_bytes());
        }
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&((HEADER_BYTES + data.len()) as i32).to_le_bytes());
        out.extend_from_slice(&(dir.len() as i32).to_le_bytes());
        out.extend_from_slice(&data);
        out.extend_from_slice(&dir);
        out
    }

    fn write_pak(dpath: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let fpath = dpath.join(name);
        File::create(&fpath).unwrap().write_all(bytes).unwrap();
        fpath
    }

    #[test]
    fn parses_and_reads_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let dpath = tmp.path();
        let bytes = build_pak(&[
            ("models/Foo/tris.md2", b"model"),
            ("models/foo/skin.pcx", b"skin"),
        ]);
        let pak = Pak::open(&write_pak(dpath, "pak0.pak", &bytes)).unwrap();

        assert!(pak.contains("models/foo/tris.md2"));
        assert_eq!(pak.read("MODELS/FOO/TRIS.MD2").unwrap().unwrap(), b"model");
        assert_eq!(pak.read("models/foo/skin.pcx").unwrap().unwrap(), b"skin");
        assert!(pak.read("missing").is_none());
    }

    #[test]
    fn rejects_bad_archives() {
        let tmp = tempfile::tempdir().unwrap();
        let dpath = tmp.path();

        let mut bad_magic = build_pak(&[("a", b"x")]);
        bad_magic[0] = b'X';
        assert!(Pak::open(&write_pak(dpath, "magic.pak", &bad_magic)).is_err());

        let truncated = build_pak(&[("a", b"xxxx")]);
        let truncated = &truncated[..truncated.len() - 10];
        assert!(Pak::open(&write_pak(dpath, "trunc.pak", truncated)).is_err());

        assert!(Pak::open(&write_pak(dpath, "short.pak", b"PACK")).is_err());
    }

    #[test]
    fn later_paks_override_earlier() {
        let tmp = tempfile::tempdir().unwrap();
        let dpath = tmp.path();
        write_pak(
            dpath,
            "pak0.pak",
            &build_pak(&[("a.md2", b"old"), ("b.md2", b"b")]),
        );
        write_pak(dpath, "pak1.pak", &build_pak(&[("a.md2", b"new")]));
        write_pak(dpath, "other.pak", &build_pak(&[("c.md2", b"c")]));

        let set = PakSet::load_dir(dpath);
        assert_eq!(set.read("a.md2").unwrap().unwrap(), b"new");
        assert_eq!(set.find_ext(".md2"), vec!["a.md2", "b.md2"]);
    }

    #[test]
    fn finds_entries_in_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let dpath = tmp.path();
        write_pak(
            dpath,
            "pak0.pak",
            &build_pak(&[
                ("models/foo/tris.md2", b"1"),
                ("models/foo/skin.pcx", b"2"),
                ("models/foo/sub/skin.pcx", b"3"),
                ("models/bar/skin.pcx", b"4"),
            ]),
        );

        let set = PakSet::load_dir(dpath);
        assert_eq!(
            set.find_in_dir("models/foo", &[".pcx"]),
            vec!["models/foo/skin.pcx"]
        );
    }
}
