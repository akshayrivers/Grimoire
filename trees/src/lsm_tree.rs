use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

// 1. CORE DATA TYPES: ValueType & Record

/// In an LSM tree, writes are strictly append-only.
/// Deletions never modify existing data in place; instead, they append a "Tombstone" (a really cool concept if you ask me).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueType {
    Put(Vec<u8>),
    Tombstone,
}

/// A Record represents a versioned key-value mutation in the LSM tree.
///
/// In systems like LevelDB and RocksDB, every write receives a monotonically increasing Sequence Number (`seq_num`).
/// This enables:
/// 1. MVCC (Multi-Version Concurrency Control) / Snapshot isolation.
/// 2. Deterministic conflict resolution: when merging or compacting, the higher sequence number always supersedes the lower sequence number for the same key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub seq_num: u64,
    pub key: Vec<u8>,
    pub value: ValueType,
}

impl Record {
    pub fn put(seq_num: u64, key: impl Into<Vec<u8>>, value: impl Into<Vec<u8>>) -> Self {
        Self {
            seq_num,
            key: key.into(),
            value: ValueType::Put(value.into()),
        }
    }

    pub fn delete(seq_num: u64, key: impl Into<Vec<u8>>) -> Self {
        Self {
            seq_num,
            key: key.into(),
            value: ValueType::Tombstone,
        }
    }

    /// Convenience helper for string keys and values (useful for tests and CLI 😋).
    pub fn put_str(seq_num: u64, key: &str, value: &str) -> Self {
        Self::put(seq_num, key.as_bytes(), value.as_bytes())
    }

    pub fn delete_str(seq_num: u64, key: &str) -> Self {
        Self::delete(seq_num, key.as_bytes())
    }

    /// Convenience helper for integer keys.
    pub fn put_i32(seq_num: u64, key: i32, value: &str) -> Self {
        Self::put(seq_num, key.to_be_bytes(), value.as_bytes())
    }

    pub fn delete_i32(seq_num: u64, key: i32) -> Self {
        Self::delete(seq_num, key.to_be_bytes())
    }
}


// 2. CRC32 CHECKSUM (Self-contained, IEEE 802.3 polynomial)

/// Computes CRC32 checksum for validating data integrity in WAL records and SSTable blocks.
/// Detecting torn writes and disk corruption is critical for crash recovery.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}


// 3. WRITE-AHEAD LOG (WAL)

// WAL Record Type byte flags on disk:
const RECORD_TYPE_PUT: u8 = 1;
const RECORD_TYPE_TOMBSTONE: u8 = 2;

/// Binary Layout of each WAL record frame on disk:
/// +-----------------------------------------------------------------------------------------+
/// | crc32 (4B) | seq_num (8B) | type (1B) | key_len (4B) | key | val_len (4B) | value (opt) |
/// +-----------------------------------------------------------------------------------------+
///
/// If `type == RECORD_TYPE_TOMBSTONE`, `val_len` is 0 and no value bytes are stored.
/// The `crc32` is computed over: `[seq_num, type, key_len, key, val_len, value]`.
pub struct WalWriter {
    writer: BufWriter<File>,
    path: PathBuf,
}

impl WalWriter {
    /// Creates a new WAL file at the given path, truncating any existing file.
    pub fn create(path: impl AsRef<Path>) -> io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path.as_ref())?;

        Ok(Self {
            writer: BufWriter::new(file),
            path: path.as_ref().to_path_buf(),
        })
    }

    /// Opens an existing WAL file in append mode.
    pub fn open_append(path: impl AsRef<Path>) -> io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(path.as_ref())?;

        Ok(Self {
            writer: BufWriter::new(file),
            path: path.as_ref().to_path_buf(),
        })
    }

    /// Appends a single record to the WAL buffer.
    pub fn append(&mut self, record: &Record) -> io::Result<()> {
        let mut payload = Vec::with_capacity(8 + 1 + 4 + record.key.len() + 4 + 32);

        // 1. SeqNum (8B)
        payload.extend_from_slice(&record.seq_num.to_be_bytes());

        // 2. Record Type (1B) & Value (0B or 4B len + bytes)
        match &record.value {
            ValueType::Put(val) => {
                payload.push(RECORD_TYPE_PUT);
                payload.extend_from_slice(&(record.key.len() as u32).to_be_bytes());
                payload.extend_from_slice(&record.key);
                payload.extend_from_slice(&(val.len() as u32).to_be_bytes());
                payload.extend_from_slice(val);
            }
            ValueType::Tombstone => {
                payload.push(RECORD_TYPE_TOMBSTONE);
                payload.extend_from_slice(&(record.key.len() as u32).to_be_bytes());
                payload.extend_from_slice(&record.key);
                payload.extend_from_slice(&0u32.to_be_bytes());
            }
        }

        // 3. Checksum over payload
        let checksum = crc32(&payload);

        // 4. Write CRC32 + payload
        self.writer.write_all(&checksum.to_be_bytes())?;
        self.writer.write_all(&payload)?;

        Ok(())
    }

    /// Flushes user-space buffers and syncs OS disk cache to guarantee durability (fsync).
    pub fn sync(&mut self) -> io::Result<()> {
        self.writer.flush()?;
        self.writer.get_ref().sync_data()?;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// WAL Reader for Crash Recovery.
/// Replays records sequentially from a WAL file.
/// If a crash occurred midway through writing a record (torn write / truncated file),
/// the reader stops at the last valid record and reports the committed state without panic.
pub struct WalReader;

impl WalReader {
    pub fn read_all(path: impl AsRef<Path>) -> io::Result<Vec<Record>> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut records = Vec::new();

        loop {
            // Read CRC32 (4 bytes). If EOF here, we reached the end of the log cleanly.
            let mut crc_buf = [0u8; 4];
            match reader.read_exact(&mut crc_buf) {
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    // Clean EOF between records
                    break;
                }
                Err(e) => return Err(e),
            }
            let expected_crc = u32::from_be_bytes(crc_buf);

            // Read SeqNum (8 bytes)
            let mut seq_buf = [0u8; 8];
            if let Err(e) = reader.read_exact(&mut seq_buf) {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    // Torn write detected: partial record header at EOF
                    eprintln!("WAL recovery: torn write detected at header, stopping recovery here.");
                    break;
                }
                return Err(e);
            }
            let seq_num = u64::from_be_bytes(seq_buf);

            // Read Record Type (1 byte)
            let mut type_buf = [0u8; 1];
            if let Err(e) = reader.read_exact(&mut type_buf) {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    eprintln!("WAL recovery: torn write detected at type, stopping recovery here.");
                    break;
                }
                return Err(e);
            }
            let rec_type = type_buf[0];

            // Read Key Length (4 bytes)
            let mut key_len_buf = [0u8; 4];
            if let Err(e) = reader.read_exact(&mut key_len_buf) {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    eprintln!("WAL recovery: torn write detected at key_len, stopping recovery here.");
                    break;
                }
                return Err(e);
            }
            let key_len = u32::from_be_bytes(key_len_buf) as usize;

            // Read Key bytes
            let mut key = vec![0u8; key_len];
            if let Err(e) = reader.read_exact(&mut key) {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    eprintln!("WAL recovery: torn write detected in key data, stopping recovery here.");
                    break;
                }
                return Err(e);
            }

            // Read Value Length (4 bytes)
            let mut val_len_buf = [0u8; 4];
            if let Err(e) = reader.read_exact(&mut val_len_buf) {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    eprintln!("WAL recovery: torn write detected at val_len, stopping recovery here.");
                    break;
                }
                return Err(e);
            }
            let val_len = u32::from_be_bytes(val_len_buf) as usize;

            // Read Value bytes
            let mut val_bytes = vec![0u8; val_len];
            if let Err(e) = reader.read_exact(&mut val_bytes) {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    eprintln!("WAL recovery: torn write detected in value data, stopping recovery here.");
                    break;
                }
                return Err(e);
            }

            // Reconstruct payload to verify CRC
            let mut payload = Vec::with_capacity(8 + 1 + 4 + key_len + 4 + val_len);
            payload.extend_from_slice(&seq_buf);
            payload.push(rec_type);
            payload.extend_from_slice(&key_len_buf);
            payload.extend_from_slice(&key);
            payload.extend_from_slice(&val_len_buf);
            payload.extend_from_slice(&val_bytes);

            let computed_crc = crc32(&payload);
            if computed_crc != expected_crc {
                eprintln!(
                    "WAL recovery: CRC mismatch (expected {:#x}, got {:#x}) - stopping at last committed record.",
                    expected_crc, computed_crc
                );
                break;
            }

            let value = match rec_type {
                RECORD_TYPE_PUT => ValueType::Put(val_bytes),
                RECORD_TYPE_TOMBSTONE => ValueType::Tombstone,
                unknown => {
                    eprintln!("WAL recovery: unknown record type {}, stopping.", unknown);
                    break;
                }
            };

            records.push(Record {
                seq_num,
                key,
                value,
            });
        }

        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // Unit Tests for Step 1 : Write Ahead log 
    #[test]
    fn test_crc32_deterministic() {
        let data1 = b"hello lsm tree";
        let data2 = b"hello lsm tree";
        let data3 = b"hello lsm tree 2";

        assert_eq!(crc32(data1), crc32(data2));
        assert_ne!(crc32(data1), crc32(data3));
    }

    #[test]
    fn test_wal_write_and_recover() {
        let temp_dir = std::env::temp_dir().join("lsm_wal_test_1");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();
        let wal_path = temp_dir.join("000001.wal");

        // 1. Write some records
        {
            let mut writer = WalWriter::create(&wal_path).unwrap();
            writer.append(&Record::put_str(1, "user:100", "Alice")).unwrap();
            writer.append(&Record::put_str(2, "user:200", "Bob")).unwrap();
            writer.append(&Record::delete_str(3, "user:100")).unwrap();
            writer.append(&Record::put_i32(4, 42, "meaning_of_life")).unwrap();
            writer.sync().unwrap();
        }

        // 2. Read back & verify
        let recovered = WalReader::read_all(&wal_path).unwrap();
        assert_eq!(recovered.len(), 4);

        assert_eq!(recovered[0], Record::put_str(1, "user:100", "Alice"));
        assert_eq!(recovered[1], Record::put_str(2, "user:200", "Bob"));
        assert_eq!(recovered[2], Record::delete_str(3, "user:100"));
        assert_eq!(recovered[3], Record::put_i32(4, 42, "meaning_of_life"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_wal_torn_write_recovery() {
        let temp_dir = std::env::temp_dir().join("lsm_wal_torn_write");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();
        let wal_path = temp_dir.join("000002.wal");

        // 1. Write 2 complete records
        {
            let mut writer = WalWriter::create(&wal_path).unwrap();
            writer.append(&Record::put_str(1, "k1", "v1")).unwrap();
            writer.append(&Record::put_str(2, "k2", "v2")).unwrap();
            writer.sync().unwrap();
        }

        // 2. Simulate crash midway through third record (corrupt partial bytes appended to file)
        {
            let mut file = OpenOptions::new().append(true).open(&wal_path).unwrap();
            file.write_all(&[0x12, 0x34, 0x56, 0x78, 0x00, 0x00]).unwrap(); // truncated junk
            file.sync_data().unwrap();
        }

        // 3. Reader should recover the 2 valid records cleanly without crashing
        let recovered = WalReader::read_all(&wal_path).unwrap();
        assert_eq!(recovered.len(), 2);
        assert_eq!(recovered[0].key, b"k1");
        assert_eq!(recovered[1].key, b"k2");

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
