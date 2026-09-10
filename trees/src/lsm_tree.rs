use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use bloom_filter::hash::murmur3::Murmur3;
use bloom_filter::BloomFilter;

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


// 4. MemTable (In-Memory Write Buffer)

use std::collections::BTreeMap;

/// In-memory sorted write buffer.
///
/// Keys are stored in sorted lexicographical order.
/// Tracks approximate memory usage in bytes to decide when to trigger an SSTable flush.
/// We will not flush based on number of keys (since a key could be 4 bytes or 1MB)
/// Instead we will track approx  size which is key.len() + value.len() + node overhead(btree node overhead)
pub struct MemTable {
    /// Maps Key -> (Sequence Number, ValueType)
    entries: BTreeMap<Vec<u8>, (u64, ValueType)>,
    /// Tracked memory usage in bytes
    size_bytes: usize,
    /// Capacity threshold in bytes that triggers a flush
    capacity_bytes: usize,
}

// Approximate BTreeMap node allocation overhead per entry on 64-bit platforms
const ENTRY_OVERHEAD_BYTES: usize = 48;

impl MemTable {
    pub fn new(capacity_bytes: usize) -> Self {
        Self {
            entries: BTreeMap::new(),
            size_bytes: 0,
            capacity_bytes,
        }
    }

    /// Returns the approximate memory footprint of this MemTable in bytes.
    pub fn size_bytes(&self) -> usize {
        self.size_bytes
    }

    /// Checks if the MemTable has reached or exceeded its memory capacity.
    pub fn is_full(&self) -> bool {
        self.size_bytes >= self.capacity_bytes
    }

    /// Number of distinct keys currently held in the MemTable.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Inserts or updates a record in the MemTable.
    pub fn insert(&mut self, record: Record) {
        let key_len = record.key.len();
        let val_len = match &record.value {
            ValueType::Put(v) => v.len(),
            ValueType::Tombstone => 0,
        };
        // key_len + val_len + u64 seq_num + btree node overhead
        let added_bytes = key_len + val_len + 8 + ENTRY_OVERHEAD_BYTES;

        // If the key already existed, subtract its old memory contribution
        if let Some((_, old_val)) = self.entries.get(&record.key) {
            let old_val_len = match old_val {
                ValueType::Put(v) => v.len(),
                ValueType::Tombstone => 0,
            };
            self.size_bytes = self.size_bytes.saturating_sub(key_len + old_val_len + 8 + ENTRY_OVERHEAD_BYTES);
        }

        self.size_bytes += added_bytes;
        self.entries.insert(record.key, (record.seq_num, record.value));
    }

    /// Point lookup for a key in the MemTable.
    ///
    /// Returns:
    /// - `None`: Key was not modified in this MemTable (continue searching older SSTables).
    /// - `Some((seq, ValueType::Put(bytes)))`: Key found with active value.
    /// - `Some((seq, ValueType::Tombstone))`: Key was deleted (stop searching, return not found).
    pub fn get(&self, key: &[u8]) -> Option<(u64, &ValueType)> {
        self.entries.get(key).map(|(seq, val)| (*seq, val))
    }

    /// Iterates over all entries in sorted key order.
    pub fn iter(&self) -> impl Iterator<Item = (&[u8], u64, &ValueType)> {
        self.entries.iter().map(|(k, (seq, val))| (k.as_slice(), *seq, val))
    }

    /// Consumes the MemTable and returns sorted records ready to be written to an SSTable.
    pub fn into_records(self) -> Vec<Record> {
        self.entries
            .into_iter()
            .map(|(key, (seq_num, value))| Record {
                seq_num,
                key,
                value,
            })
            .collect()
    }
}


// 5. SSTable (Sorted String Table) On-Disk Format, Writer & Reader

pub const SSTABLE_MAGIC: u64 = 0x5353_5441_424C_4531; // "SSTABLE1"
pub const DEFAULT_BLOCK_SIZE: usize = 4096; // 4KB chunk target
pub const FOOTER_SIZE: u64 = 40; // 5 fields * 8 bytes

/// Sparse Index entry pointing to a contiguous, sorted Data Block on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    /// Highest key present in this block.
    pub last_key: Vec<u8>,
    /// Byte offset where this block starts in the .sst file.
    pub block_offset: u64,
    /// Length of the block data in bytes (excluding block CRC32).
    pub block_len: u64,
}

fn encode_record(record: &Record) -> Vec<u8> {
    let mut payload = Vec::with_capacity(8 + 1 + 4 + record.key.len() + 4 + 32);
    payload.extend_from_slice(&record.seq_num.to_be_bytes());

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
    payload
}

fn decode_block_records(data: &[u8]) -> io::Result<Vec<Record>> {
    let mut cursor = 0;
    let mut records = Vec::new();

    while cursor < data.len() {
        if cursor + 8 + 1 + 4 > data.len() {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Incomplete record header in block"));
        }

        let seq_num = u64::from_be_bytes(data[cursor..cursor + 8].try_into().unwrap());
        cursor += 8;

        let val_type_byte = data[cursor];
        cursor += 1;

        let key_len = u32::from_be_bytes(data[cursor..cursor + 4].try_into().unwrap()) as usize;
        cursor += 4;

        if cursor + key_len + 4 > data.len() {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Incomplete record key/val_len in block"));
        }

        let key = data[cursor..cursor + key_len].to_vec();
        cursor += key_len;

        let val_len = u32::from_be_bytes(data[cursor..cursor + 4].try_into().unwrap()) as usize;
        cursor += 4;

        if cursor + val_len > data.len() {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Incomplete record value bytes in block"));
        }

        let val_bytes = data[cursor..cursor + val_len].to_vec();
        cursor += val_len;

        let value = match val_type_byte {
            RECORD_TYPE_PUT => ValueType::Put(val_bytes),
            RECORD_TYPE_TOMBSTONE => ValueType::Tombstone,
            _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid record type byte in block")),
        };

        records.push(Record { seq_num, key, value });
    }

    Ok(records)
}

/// Builds an immutable SSTable file on disk from a sorted list of records.
pub struct SsTableWriter;

impl SsTableWriter {
    pub fn write_new(
        path: impl AsRef<Path>,
        records: &[Record],
        target_block_size: usize,
    ) -> io::Result<()> {
        if records.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Cannot create an empty SSTable",
            ));
        }

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path.as_ref())?;
        let mut writer = BufWriter::new(file);

        // 1. Build Bloom Filter over ALL keys (including tombstones!) (Using my own bloom filter implementation here 😋)
        // In real LSMs, tombstones MUST be in the bloom filter so deletions
        // mask older versions residing in deeper SSTables.
        let num_keys = records.len();
        let m = (num_keys * 10).max(64);
        let r = 1;
        let k = 7;
        let mut bloom = BloomFilter::new(m, k, r, Murmur3::new(0xbc9f_1d34));
        for record in records {
            bloom.insert(&record.key);
        }

        let min_key = records.first().unwrap().key.clone();
        let mut index_entries = Vec::new();

        let mut current_offset: u64 = 0;
        let mut block_buf = Vec::new();
        let mut last_key_in_block = Vec::new();

        // 2. Write Data Blocks
        for record in records {
            let encoded = encode_record(record);

            // Flush current block if it exceeds target size and is non-empty
            if block_buf.len() + encoded.len() > target_block_size && !block_buf.is_empty() {
                let block_len = block_buf.len() as u64;
                let crc = crc32(&block_buf);

                writer.write_all(&block_buf)?;
                writer.write_all(&crc.to_be_bytes())?;

                index_entries.push(IndexEntry {
                    last_key: last_key_in_block.clone(),
                    block_offset: current_offset,
                    block_len,
                });

                current_offset += block_len + 4;
                block_buf.clear();
            }

            block_buf.extend_from_slice(&encoded);
            last_key_in_block = record.key.clone();
        }

        // Flush trailing block
        if !block_buf.is_empty() {
            let block_len = block_buf.len() as u64;
            let crc = crc32(&block_buf);

            writer.write_all(&block_buf)?;
            writer.write_all(&crc.to_be_bytes())?;

            index_entries.push(IndexEntry {
                last_key: last_key_in_block.clone(),
                block_offset: current_offset,
                block_len,
            });

            current_offset += block_len + 4;
            block_buf.clear();
        }

        // 3. Write Filter Block
        let filter_offset = current_offset;
        let raw_words = bloom.raw_words();
        let mut filter_payload = Vec::new();
        filter_payload.extend_from_slice(&(m as u64).to_be_bytes());
        filter_payload.extend_from_slice(&(k as u64).to_be_bytes());
        filter_payload.extend_from_slice(&(r as u64).to_be_bytes());
        filter_payload.extend_from_slice(&(raw_words.len() as u64).to_be_bytes());
        for word in raw_words {
            filter_payload.extend_from_slice(&word.to_be_bytes());
        }
        let filter_crc = crc32(&filter_payload);
        writer.write_all(&filter_payload)?;
        writer.write_all(&filter_crc.to_be_bytes())?;
        let filter_len = (filter_payload.len() + 4) as u64;
        current_offset += filter_len;

        // 4. Write Index Block (Sparse Index)
        let index_offset = current_offset;
        let mut index_payload = Vec::new();
        index_payload.extend_from_slice(&(min_key.len() as u32).to_be_bytes());
        index_payload.extend_from_slice(&min_key);
        index_payload.extend_from_slice(&(index_entries.len() as u32).to_be_bytes());
        for entry in &index_entries {
            index_payload.extend_from_slice(&(entry.last_key.len() as u32).to_be_bytes());
            index_payload.extend_from_slice(&entry.last_key);
            index_payload.extend_from_slice(&entry.block_offset.to_be_bytes());
            index_payload.extend_from_slice(&entry.block_len.to_be_bytes());
        }
        let index_crc = crc32(&index_payload);
        writer.write_all(&index_payload)?;
        writer.write_all(&index_crc.to_be_bytes())?;
        let index_len = (index_payload.len() + 4) as u64;

        // 5. Write Fixed 40-byte Footer at EOF
        writer.write_all(&filter_offset.to_be_bytes())?;
        writer.write_all(&filter_len.to_be_bytes())?;
        writer.write_all(&index_offset.to_be_bytes())?;
        writer.write_all(&index_len.to_be_bytes())?;
        writer.write_all(&SSTABLE_MAGIC.to_be_bytes())?;

        writer.flush()?;
        writer.get_ref().sync_data()?;

        Ok(())
    }
}

struct CachedBlock {
    block_offset: u64,
    data: Vec<u8>,
}

/// Reader for an immutable SSTable file on disk.
/// Loads only the Sparse Index and Bloom Filter into memory.
/// Data blocks remain on disk and are fetched and cached on demand.
pub struct SsTableReader {
    file: File,
    path: PathBuf,
    pub min_key: Vec<u8>,
    pub max_key: Vec<u8>,
    pub bloom_filter: BloomFilter<Murmur3>,
    pub index: Vec<IndexEntry>,
    cached_block: Option<CachedBlock>,
}

fn search_in_block(data: &[u8], target_key: &[u8]) -> io::Result<Option<(u64, ValueType)>> {
    let mut cursor = 0;
    while cursor < data.len() {
        if cursor + 8 + 1 + 4 > data.len() {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Incomplete record header in block"));
        }

        let seq_num = u64::from_be_bytes(data[cursor..cursor + 8].try_into().unwrap());
        cursor += 8;

        let val_type_byte = data[cursor];
        cursor += 1;

        let key_len = u32::from_be_bytes(data[cursor..cursor + 4].try_into().unwrap()) as usize;
        cursor += 4;

        if cursor + key_len + 4 > data.len() {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Incomplete record key/val_len in block"));
        }

        let key_slice = &data[cursor..cursor + key_len];
        cursor += key_len;

        let val_len = u32::from_be_bytes(data[cursor..cursor + 4].try_into().unwrap()) as usize;
        cursor += 4;

        if cursor + val_len > data.len() {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Incomplete record value bytes in block"));
        }

        match key_slice.cmp(target_key) {
            std::cmp::Ordering::Equal => {
                let val_bytes = data[cursor..cursor + val_len].to_vec();
                let value = match val_type_byte {
                    RECORD_TYPE_PUT => ValueType::Put(val_bytes),
                    RECORD_TYPE_TOMBSTONE => ValueType::Tombstone,
                    _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid record type byte in block")),
                };
                return Ok(Some((seq_num, value)));
            }
            std::cmp::Ordering::Greater => {
                return Ok(None);
            }
            std::cmp::Ordering::Less => {
                cursor += val_len;
            }
        }
    }

    Ok(None)
}

impl SsTableReader {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let mut file = File::open(path.as_ref())?;
        let file_len = file.metadata()?.len();

        if file_len < FOOTER_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "File too short to be a valid SSTable",
            ));
        }

        // 1. Read Footer from EOF - 40
        file.seek(SeekFrom::Start(file_len - FOOTER_SIZE))?;
        let mut footer_buf = [0u8; 40];
        file.read_exact(&mut footer_buf)?;

        let filter_offset = u64::from_be_bytes(footer_buf[0..8].try_into().unwrap());
        let filter_len = u64::from_be_bytes(footer_buf[8..16].try_into().unwrap());
        let index_offset = u64::from_be_bytes(footer_buf[16..24].try_into().unwrap());
        let index_len = u64::from_be_bytes(footer_buf[24..32].try_into().unwrap());
        let magic = u64::from_be_bytes(footer_buf[32..40].try_into().unwrap());

        if magic != SSTABLE_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid SSTable magic: expected {:#x}, got {:#x}", SSTABLE_MAGIC, magic),
            ));
        }

        // 2. Read and verify Filter Block
        file.seek(SeekFrom::Start(filter_offset))?;
        let mut filter_bytes = vec![0u8; filter_len as usize];
        file.read_exact(&mut filter_bytes)?;

        let filter_payload = &filter_bytes[..filter_bytes.len() - 4];
        let filter_crc = u32::from_be_bytes(filter_bytes[filter_bytes.len() - 4..].try_into().unwrap());
        if crc32(filter_payload) != filter_crc {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Filter block CRC mismatch"));
        }

        let m = u64::from_be_bytes(filter_payload[0..8].try_into().unwrap()) as usize;
        let k = u64::from_be_bytes(filter_payload[8..16].try_into().unwrap()) as usize;
        let r = u64::from_be_bytes(filter_payload[16..24].try_into().unwrap()) as usize;
        let num_words = u64::from_be_bytes(filter_payload[24..32].try_into().unwrap()) as usize;

        let mut raw_words = Vec::with_capacity(num_words);
        let mut w_cursor = 32;
        for _ in 0..num_words {
            let word = u64::from_be_bytes(filter_payload[w_cursor..w_cursor + 8].try_into().unwrap());
            raw_words.push(word);
            w_cursor += 8;
        }

        let bloom_filter = BloomFilter::from_raw_words(m, k, r, Murmur3::new(0xbc9f_1d34), raw_words);

        // 3. Read and verify Index Block
        file.seek(SeekFrom::Start(index_offset))?;
        let mut index_bytes = vec![0u8; index_len as usize];
        file.read_exact(&mut index_bytes)?;

        let index_payload = &index_bytes[..index_bytes.len() - 4];
        let index_crc = u32::from_be_bytes(index_bytes[index_bytes.len() - 4..].try_into().unwrap());
        if crc32(index_payload) != index_crc {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Index block CRC mismatch"));
        }

        let mut idx_cursor = 0;
        let min_key_len = u32::from_be_bytes(index_payload[idx_cursor..idx_cursor + 4].try_into().unwrap()) as usize;
        idx_cursor += 4;
        let min_key = index_payload[idx_cursor..idx_cursor + min_key_len].to_vec();
        idx_cursor += min_key_len;

        let num_entries = u32::from_be_bytes(index_payload[idx_cursor..idx_cursor + 4].try_into().unwrap()) as usize;
        idx_cursor += 4;

        let mut index = Vec::with_capacity(num_entries);
        for _ in 0..num_entries {
            let last_key_len = u32::from_be_bytes(index_payload[idx_cursor..idx_cursor + 4].try_into().unwrap()) as usize;
            idx_cursor += 4;
            let last_key = index_payload[idx_cursor..idx_cursor + last_key_len].to_vec();
            idx_cursor += last_key_len;
            let block_offset = u64::from_be_bytes(index_payload[idx_cursor..idx_cursor + 8].try_into().unwrap());
            idx_cursor += 8;
            let block_len = u64::from_be_bytes(index_payload[idx_cursor..idx_cursor + 8].try_into().unwrap());
            idx_cursor += 8;

            index.push(IndexEntry {
                last_key,
                block_offset,
                block_len,
            });
        }

        let max_key = index.last().map(|e| e.last_key.clone()).unwrap_or_default();

        Ok(Self {
            file,
            path: path.as_ref().to_path_buf(),
            min_key,
            max_key,
            bloom_filter,
            index,
            cached_block: None,
        })
    }

    /// Fast membership check using the Bloom Filter.
    pub fn filter_contains(&self, key: &[u8]) -> bool {
        self.bloom_filter.contains(key)
    }

    /// Point lookup for a key inside this SSTable.
    ///
    /// Execution pipeline:
    /// 1. Key range check (`min_key` .. `max_key`): $O(1)$ fast exit if outside range.
    /// 2. Bloom Filter check: $O(1)$ fast exit if negative (ZERO data block reads).
    /// 3. Binary search on Sparse Index: finds target block in $O(\log B)$ time.
    /// 4. Block Cache check: if the target block is cached, skip disk I/O entirely!
    /// 5. In-block scan: zero-allocation slice search inside the cached block buffer.
    pub fn get(&mut self, key: &[u8]) -> io::Result<Option<(u64, ValueType)>> {
        // 1. Min/Max Range Check
        if key < self.min_key.as_slice() || key > self.max_key.as_slice() {
            return Ok(None);
        }

        // 2. Bloom Filter Check (Fast path: avoids touching disk blocks entirely!)
        if !self.bloom_filter.contains(key) {
            return Ok(None);
        }

        // 3. Binary search Sparse Index: find first block whose last_key >= key
        let block_idx = match self.index.binary_search_by(|entry| entry.last_key.as_slice().cmp(key)) {
            Ok(idx) => idx,
            Err(idx) => {
                if idx >= self.index.len() {
                    return Ok(None);
                }
                idx
            }
        };

        let target_entry = &self.index[block_idx];

        // 4. Block Cache: if block is already loaded, reuse it!
        let is_cached = self
            .cached_block
            .as_ref()
            .map_or(false, |c| c.block_offset == target_entry.block_offset);

        if !is_cached {
            self.file.seek(SeekFrom::Start(target_entry.block_offset))?;
            let mut block_data = vec![0u8; target_entry.block_len as usize];
            self.file.read_exact(&mut block_data)?;

            let mut crc_buf = [0u8; 4];
            self.file.read_exact(&mut crc_buf)?;
            let expected_crc = u32::from_be_bytes(crc_buf);
            if crc32(&block_data) != expected_crc {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Data block CRC mismatch"));
            }

            self.cached_block = Some(CachedBlock {
                block_offset: target_entry.block_offset,
                data: block_data,
            });
        }

        // 5. Zero-allocation scan in memory within target block
        let cached = self.cached_block.as_ref().unwrap();
        search_in_block(&cached.data, key)
    }

    /// Reads all records across all data blocks sequentially.
    /// Used for Range Scans and Compaction.
    pub fn read_all_records(&mut self) -> io::Result<Vec<Record>> {
        let mut all_records = Vec::new();

        for entry in &self.index {
            self.file.seek(SeekFrom::Start(entry.block_offset))?;
            let mut block_data = vec![0u8; entry.block_len as usize];
            self.file.read_exact(&mut block_data)?;

            let mut crc_buf = [0u8; 4];
            self.file.read_exact(&mut crc_buf)?;
            let expected_crc = u32::from_be_bytes(crc_buf);
            if crc32(&block_data) != expected_crc {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Data block CRC mismatch"));
            }

            let block_records = decode_block_records(&block_data)?;
            all_records.extend(block_records);
        }

        Ok(all_records)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}


// 6. LSM Tree Storage Engine (THE GREAT ORCHESTRATOR)

/// Configuration parameters for tuning the LSM Tree.
#[derive(Debug, Clone)]
pub struct LsmConfig {
    /// Maximum memory (in bytes) of the active MemTable before triggering an SSTable flush.
    pub memtable_capacity_bytes: usize,
    /// Target chunk size (in bytes) for data blocks inside an SSTable (~4KB is typical).
    pub sstable_block_size: usize,
    /// Whether to call fsync on every single mutation.
    /// Default is false for high-throughput buffered WAL (matches LevelDB/RocksDB defaults).
    pub sync_wal: bool,
}

impl Default for LsmConfig {
    fn default() -> Self {
        Self {
            memtable_capacity_bytes: 4 * 1024 * 1024, // 4MB
            sstable_block_size: DEFAULT_BLOCK_SIZE,   // 4KB
            sync_wal: false,
        }
    }
}

/// A complete, high-performance/Educational Log-Structured Merge (LSM) Tree engine.
///
/// Features:
/// 1. Append-only WAL for crash-durability (fsync on commit).
/// 2. In-memory sorted MemTable for sub-microsecond writes.
/// 3. Immutable SSTables with Bloom Filters for 0-disk-read skips on cold keys.
/// 4. Sparse indexes for binary-search block lookups.
/// 5. Append-only tombstone deletions.
/// 6. Crash recovery replaying un-flushed WAL logs.
/// 7. Multi-way merge Range Scans.
/// 8. Compaction to reclaim space, prune dead tombstones, and reduce read amplification.
pub struct LsmTree {
    dir: PathBuf,
    config: LsmConfig,
    active_memtable: MemTable,
    active_wal: WalWriter,
    /// SSTables ordered from NEWEST (index 0) to OLDEST (index n-1).
    sstables: Vec<SsTableReader>,
    next_seq_num: u64,
    next_sst_id: u64,
}

impl LsmTree {
    /// Opens or creates an LSM Tree with default settings in the specified directory.
    pub fn open(dir: impl AsRef<Path>) -> io::Result<Self> {
        Self::open_with_config(dir, LsmConfig::default())
    }

    /// Opens or creates an LSM Tree with custom configuration.
    pub fn open_with_config(dir: impl AsRef<Path>, config: LsmConfig) -> io::Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)?;

        // 1. Discover all .sst files in directory
        let mut sst_files = Vec::new();
        let mut max_id = 0u64;

        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("sst") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(id) = stem.parse::<u64>() {
                        max_id = max_id.max(id);
                        sst_files.push((id, path));
                    }
                }
            }
        }

        // Sort descending: highest id = newest SSTable
        sst_files.sort_by(|a, b| b.0.cmp(&a.0));
        let mut sstables = Vec::new();
        for (_, sst_path) in sst_files {
            sstables.push(SsTableReader::open(sst_path)?);
        }

        // 2. Discover .wal files and replay any uncommitted records (Crash Recovery)
        let mut wal_files = Vec::new();
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("wal") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(id) = stem.parse::<u64>() {
                        max_id = max_id.max(id);
                        wal_files.push((id, path));
                    }
                }
            }
        }

        // Sort ascending: oldest WAL first for correct replay ordering
        wal_files.sort_by(|a, b| a.0.cmp(&b.0));

        let mut memtable = MemTable::new(config.memtable_capacity_bytes);
        let mut max_seq_num = 0u64;

        for (_, wal_path) in &wal_files {
            let recovered_records = WalReader::read_all(wal_path)?;
            for record in recovered_records {
                max_seq_num = max_seq_num.max(record.seq_num);
                memtable.insert(record);
            }
        }

        // 3. Initialize fresh active WAL and preserve recovered un-flushed records
        let active_wal_path = dir.join(format!("{:06}.wal", max_id + 1));
        let mut active_wal = WalWriter::create(&active_wal_path)?;

        // Re-persist any recovered in-memory records into the active WAL
        for (k, seq, val) in memtable.iter() {
            let record = match val {
                ValueType::Put(v) => Record::put(seq, k, v.clone()),
                ValueType::Tombstone => Record::delete(seq, k),
            };
            active_wal.append(&record)?;
        }
        active_wal.sync()?;

        // Clean up old replayed WAL files that have been merged into the new active WAL
        for (_, wal_path) in wal_files {
            if wal_path != active_wal_path {
                let _ = fs::remove_file(wal_path);
            }
        }

        Ok(Self {
            dir,
            config,
            active_memtable: memtable,
            active_wal,
            sstables,
            next_seq_num: max_seq_num + 1,
            next_sst_id: max_id + 1,
        })
    }

    /// Inserts or updates a key-value pair.
    pub fn put(&mut self, key: impl Into<Vec<u8>>, value: impl Into<Vec<u8>>) -> io::Result<()> {
        let key = key.into();
        let value = value.into();
        let seq_num = self.next_seq_num;
        self.next_seq_num += 1;

        let record = Record::put(seq_num, key, value);

        // 1. Write to WAL first (durability)
        self.active_wal.append(&record)?;
        if self.config.sync_wal {
            self.active_wal.sync()?;
        }

        // 2. Write to in-memory MemTable
        self.active_memtable.insert(record);

        // 3. Trigger flush if MemTable capacity reached
        if self.active_memtable.is_full() {
            self.flush()?;
        }

        Ok(())
    }

    /// Deletes a key by appending a Tombstone record.
    pub fn delete(&mut self, key: impl Into<Vec<u8>>) -> io::Result<()> {
        let key = key.into();
        let seq_num = self.next_seq_num;
        self.next_seq_num += 1;

        let record = Record::delete(seq_num, key);

        // 1. Write Tombstone to WAL
        self.active_wal.append(&record)?;
        if self.config.sync_wal {
            self.active_wal.sync()?;
        }

        // 2. Write Tombstone to MemTable
        self.active_memtable.insert(record);

        // 3. Trigger flush if full
        if self.active_memtable.is_full() {
            self.flush()?;
        }

        Ok(())
    }

    /// Explicitly flushes the WAL buffer and syncs OS disk cache (fsync).
    pub fn sync(&mut self) -> io::Result<()> {
        self.active_wal.sync()
    }

    /// Point lookup for a key across MemTable and all SSTables.
    /// Returns `Ok(Some(bytes))` if found, or `Ok(None)` if deleted or missing.
    pub fn get(&mut self, key: &[u8]) -> io::Result<Option<Vec<u8>>> {
        // Step 1: Check active MemTable
        if let Some((_seq, val)) = self.active_memtable.get(key) {
            return match val {
                ValueType::Put(v) => Ok(Some(v.clone())),
                ValueType::Tombstone => Ok(None), // Tombstone in MemTable masks everything older!
            };
        }

        // Step 2: Check SSTables in reverse chronological order (newest first)
        for sstable in &mut self.sstables {
            if let Some((_seq, val)) = sstable.get(key)? {
                return match val {
                    ValueType::Put(v) => Ok(Some(v)),
                    ValueType::Tombstone => Ok(None), // Tombstone in newer SSTable masks older SSTables!
                };
            }
        }

        // Not found in MemTable or any SSTable
        Ok(None)
    }

    /// Flushes the active MemTable to a new SSTable on disk.
    pub fn flush(&mut self) -> io::Result<()> {
        if self.active_memtable.is_empty() {
            return Ok(());
        }

        let sst_id = self.next_sst_id;
        self.next_sst_id += 1;
        let sst_path = self.dir.join(format!("{:06}.sst", sst_id));

        // 1. Extract sorted records from current MemTable and reset it
        let old_memtable = std::mem::replace(
            &mut self.active_memtable,
            MemTable::new(self.config.memtable_capacity_bytes),
        );
        let records = old_memtable.into_records();

        // 2. Write new immutable SSTable to disk
        SsTableWriter::write_new(&sst_path, &records, self.config.sstable_block_size)?;

        // 3. Prepend newly flushed SSTable to the FRONT of our active list (newest first)
        let reader = SsTableReader::open(&sst_path)?;
        self.sstables.insert(0, reader);

        // 4. Safely rotate WAL: delete old WAL only AFTER SSTable is committed to disk
        let old_wal_path = self.active_wal.path().to_path_buf();
        let new_wal_path = self.dir.join(format!("{:06}.wal", sst_id + 1));
        self.active_wal = WalWriter::create(&new_wal_path)?;
        let _ = fs::remove_file(old_wal_path);

        Ok(())
    }

    /// Range scan returning all active (non-deleted) key-value pairs in `[start, end]`.
    /// Merges MemTable and all SSTables, resolving version conflicts via sequence numbers.
    pub fn scan(&mut self, start: &[u8], end: &[u8]) -> io::Result<Vec<(Vec<u8>, Vec<u8>)>> {
        if start > end {
            return Ok(Vec::new());
        }

        // Map: Key -> (highest_seq_num, ValueType)
        let mut merged: BTreeMap<Vec<u8>, (u64, ValueType)> = BTreeMap::new();

        // 1. Merge all records from all SSTables
        for sstable in &mut self.sstables {
            let records = sstable.read_all_records()?;
            for record in records {
                if record.key.as_slice() >= start && record.key.as_slice() <= end {
                    match merged.get(&record.key) {
                        Some((existing_seq, _)) if *existing_seq >= record.seq_num => {}
                        _ => {
                            merged.insert(record.key, (record.seq_num, record.value));
                        }
                    }
                }
            }
        }

        // 2. Merge active MemTable entries (newest live writes)
        for (k, seq, val) in self.active_memtable.iter() {
            if k >= start && k <= end {
                match merged.get(k) {
                    Some((existing_seq, _)) if *existing_seq >= seq => {}
                    _ => {
                        merged.insert(k.to_vec(), (seq, val.clone()));
                    }
                }
            }
        }

        // 3. Collect active Put entries (skipping dead tombstones)
        let mut results = Vec::new();
        for (key, (_seq, val)) in merged {
            if let ValueType::Put(v) = val {
                results.push((key, v));
            }
        }

        Ok(results)
    }

    /// Merges all SSTables into a single consolidated SSTable.
    ///
    /// Compaction achieves three critical LSM optimizations:
    /// 1. Drops superseded older versions of overwritten keys.
    /// 2. Purges dead tombstones (at the bottom-most level, no older version can exist below).
    /// 3. Merges N SSTables into 1, reducing read amplification back to 1 Bloom check.
    pub fn compact(&mut self) -> io::Result<()> {
        if self.sstables.len() <= 1 {
            return Ok(());
        }

        // 1. Collect all records from all SSTables and resolve conflicts by highest sequence number
        let mut merged: BTreeMap<Vec<u8>, (u64, ValueType)> = BTreeMap::new();
        for sstable in &mut self.sstables {
            let records = sstable.read_all_records()?;
            for record in records {
                match merged.get(&record.key) {
                    Some((existing_seq, _)) if *existing_seq >= record.seq_num => {}
                    _ => {
                        merged.insert(record.key, (record.seq_num, record.value));
                    }
                }
            }
        }

        // 2. Bottom-level tombstone elimination:
        // Since we are compacting all SSTables, any Tombstone has superseded all prior versions.
        // We can safely purge all Tombstones!
        let mut compacted_records = Vec::new();
        for (key, (seq_num, val)) in merged {
            if let ValueType::Put(v) = val {
                compacted_records.push(Record {
                    seq_num,
                    key,
                    value: ValueType::Put(v),
                });
            }
        }

        // 3. Write new compacted SSTable
        let new_sst_id = self.next_sst_id;
        self.next_sst_id += 1;
        let new_sst_path = self.dir.join(format!("{:06}.sst", new_sst_id));

        if !compacted_records.is_empty() {
            SsTableWriter::write_new(&new_sst_path, &compacted_records, self.config.sstable_block_size)?;
            let new_reader = SsTableReader::open(&new_sst_path)?;

            // 4. Delete old SSTable files from disk
            for old_sstable in &self.sstables {
                let _ = fs::remove_file(old_sstable.path());
            }

            self.sstables = vec![new_reader];
        } else {
            // All entries were tombstones! Purge all SSTables.
            for old_sstable in &self.sstables {
                let _ = fs::remove_file(old_sstable.path());
            }
            self.sstables.clear();
        }

        Ok(())
    }

    /// Number of active SSTables on disk.
    pub fn sstable_count(&self) -> usize {
        self.sstables.len()
    }

    /// Number of distinct entries in the active MemTable.
    pub fn memtable_len(&self) -> usize {
        self.active_memtable.len()
    }

    // Convenience Helpers for String Keys & Values

    pub fn put_str(&mut self, key: &str, value: &str) -> io::Result<()> {
        self.put(key.as_bytes(), value.as_bytes())
    }

    pub fn delete_str(&mut self, key: &str) -> io::Result<()> {
        self.delete(key.as_bytes())
    }

    pub fn get_str(&mut self, key: &str) -> io::Result<Option<String>> {
        self.get(key.as_bytes()).map(|opt| opt.and_then(|bytes| String::from_utf8(bytes).ok()))
    }

    pub fn scan_str(&mut self, start: &str, end: &str) -> io::Result<Vec<(String, String)>> {
        let raw = self.scan(start.as_bytes(), end.as_bytes())?;
        let res = raw
            .into_iter()
            .filter_map(|(k, v)| {
                let key_str = String::from_utf8(k).ok()?;
                let val_str = String::from_utf8(v).ok()?;
                Some((key_str, val_str))
            })
            .collect();
        Ok(res)
    }

    // Convenience Helpers for Integer Keys

    pub fn put_i32(&mut self, key: i32, value: &str) -> io::Result<()> {
        self.put(key.to_be_bytes(), value.as_bytes())
    }

    pub fn delete_i32(&mut self, key: i32) -> io::Result<()> {
        self.delete(key.to_be_bytes())
    }

    pub fn get_i32(&mut self, key: i32) -> io::Result<Option<String>> {
        self.get(&key.to_be_bytes()).map(|opt| opt.and_then(|bytes| String::from_utf8(bytes).ok()))
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

    #[test]
    fn test_memtable_basic_operations() {
        let mut memtable = MemTable::new(1024);

        memtable.insert(Record::put_str(1, "user:1", "Alice"));
        memtable.insert(Record::put_str(2, "user:2", "Bob"));

        // Verify gets
        let res1 = memtable.get(b"user:1");
        assert!(res1.is_some());
        let (seq, val) = res1.unwrap();
        assert_eq!(seq, 1);
        assert_eq!(val, &ValueType::Put(b"Alice".to_vec()));

        // Update user:1 with newer seq_num
        memtable.insert(Record::put_str(3, "user:1", "Alice Updated"));
        let (seq2, val2) = memtable.get(b"user:1").unwrap();
        assert_eq!(seq2, 3);
        assert_eq!(val2, &ValueType::Put(b"Alice Updated".to_vec()));

        // Delete user:2 (Tombstone)
        memtable.insert(Record::delete_str(4, "user:2"));
        let (seq3, val3) = memtable.get(b"user:2").unwrap();
        assert_eq!(seq3, 4);
        assert_eq!(val3, &ValueType::Tombstone);

        // Non-existent key
        assert!(memtable.get(b"user:3").is_none());
    }

    #[test]
    fn test_memtable_capacity_and_sorted_order() {
        // Small capacity to test flush trigger
        let mut memtable = MemTable::new(180);
        assert!(!memtable.is_full());

        memtable.insert(Record::put_str(1, "cherry", "red"));
        memtable.insert(Record::put_str(2, "apple", "green"));
        memtable.insert(Record::put_str(3, "banana", "yellow"));

        assert!(memtable.is_full());

        // Verify sorted order on iteration: apple -> banana -> cherry
        let keys: Vec<String> = memtable
            .iter()
            .map(|(k, _, _)| String::from_utf8(k.to_vec()).unwrap())
            .collect();
        assert_eq!(keys, vec!["apple", "banana", "cherry"]);

        let records = memtable.into_records();
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].key, b"apple");
        assert_eq!(records[1].key, b"banana");
        assert_eq!(records[2].key, b"cherry");
    }

    #[test]
    fn test_sstable_write_and_read() {
        let temp_dir = std::env::temp_dir().join("lsm_sstable_test_1");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();
        let sst_path = temp_dir.join("000001.sst");

        // Prepare sorted records (simulating flushed MemTable)
        let records = vec![
            Record::put_str(1, "k:01", "val_1"),
            Record::put_str(2, "k:02", "val_2"),
            Record::delete_str(3, "k:03"), // Tombstone!
            Record::put_str(4, "k:04", "val_4"),
            Record::put_str(5, "k:05", "val_5"),
            Record::put_str(6, "k:06", "val_6"),
            Record::put_str(7, "k:07", "val_7"),
            Record::put_str(8, "k:08", "val_8"),
        ];

        // Write with tiny block size (64 bytes) to force multiple data blocks and a multi-entry sparse index
        SsTableWriter::write_new(&sst_path, &records, 64).unwrap();

        // Open and read SSTable
        let mut reader = SsTableReader::open(&sst_path).unwrap();

        // Sparse index should have multiple blocks
        assert!(reader.index.len() > 1, "Expected multiple blocks, got {}", reader.index.len());
        assert_eq!(reader.min_key, b"k:01");
        assert_eq!(reader.max_key, b"k:08");

        // 1. Point lookups for existing active keys
        let r1 = reader.get(b"k:01").unwrap();
        assert_eq!(r1, Some((1, ValueType::Put(b"val_1".to_vec()))));

        let r6 = reader.get(b"k:06").unwrap();
        assert_eq!(r6, Some((6, ValueType::Put(b"val_6".to_vec()))));

        // 2. Point lookup for deleted key (Tombstone must be found!)
        let r3 = reader.get(b"k:03").unwrap();
        assert_eq!(r3, Some((3, ValueType::Tombstone)));

        // 3. Point lookup for keys outside range
        assert_eq!(reader.get(b"k:00").unwrap(), None);
        assert_eq!(reader.get(b"k:99").unwrap(), None);

        // 4. Sequential full scan
        let all = reader.read_all_records().unwrap();
        assert_eq!(all, records);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_sstable_bloom_filter_rejection() {
        let temp_dir = std::env::temp_dir().join("lsm_sstable_bloom");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();
        let sst_path = temp_dir.join("000002.sst");

        let records = vec![
            Record::put_str(1, "apple", "1"),
            Record::put_str(2, "banana", "2"),
            Record::put_str(3, "cherry", "3"),
            Record::put_str(4, "date", "4"),
            Record::put_str(5, "elderberry", "5"),
        ];

        SsTableWriter::write_new(&sst_path, &records, 128).unwrap();
        let reader = SsTableReader::open(&sst_path).unwrap();

        // Bloom filter must contain all inserted keys
        assert!(reader.filter_contains(b"apple"));
        assert!(reader.filter_contains(b"banana"));
        assert!(reader.filter_contains(b"cherry"));
        assert!(reader.filter_contains(b"date"));
        assert!(reader.filter_contains(b"elderberry"));

        // Bloom filter should not contain definitely absent keys within the range
        assert!(!reader.filter_contains(b"blueberry"));
        assert!(!reader.filter_contains(b"cantaloupe"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_lsm_tree_basic_ops() {
        let temp_dir = std::env::temp_dir().join("lsm_engine_basic");
        let _ = fs::remove_dir_all(&temp_dir);

        let mut tree = LsmTree::open(&temp_dir).unwrap();

        tree.put_str("user:101", "Alice").unwrap();
        tree.put_str("user:102", "Bob").unwrap();

        assert_eq!(tree.get_str("user:101").unwrap(), Some("Alice".to_string()));
        assert_eq!(tree.get_str("user:102").unwrap(), Some("Bob".to_string()));
        assert_eq!(tree.get_str("user:999").unwrap(), None);

        // Update
        tree.put_str("user:101", "Alice Updated").unwrap();
        assert_eq!(tree.get_str("user:101").unwrap(), Some("Alice Updated".to_string()));

        // Delete with Tombstone
        tree.delete_str("user:101").unwrap();
        assert_eq!(tree.get_str("user:101").unwrap(), None);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_lsm_tree_auto_flush_and_sstable_reads() {
        let temp_dir = std::env::temp_dir().join("lsm_engine_flush");
        let _ = fs::remove_dir_all(&temp_dir);

        // Configure tiny memtable (200 bytes) to force flushes
        let config = LsmConfig {
            memtable_capacity_bytes: 200,
            sstable_block_size: 128,
            sync_wal: false,
        };
        let mut tree = LsmTree::open_with_config(&temp_dir, config).unwrap();

        for i in 0..20 {
            tree.put_str(&format!("key:{:03}", i), &format!("val:{:03}", i)).unwrap();
        }

        // Multiple SSTables should have been flushed
        assert!(tree.sstable_count() > 0, "Expected flushes, got {} SSTables", tree.sstable_count());

        // Verify all keys can still be retrieved across MemTable and SSTables
        for i in 0..20 {
            let key = format!("key:{:03}", i);
            let expected = format!("val:{:03}", i);
            assert_eq!(tree.get_str(&key).unwrap(), Some(expected));
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_lsm_tree_crash_recovery() {
        let temp_dir = std::env::temp_dir().join("lsm_engine_recovery");
        let _ = fs::remove_dir_all(&temp_dir);

        // 1. Open tree, insert keys without flushing (live only in WAL + MemTable)
        {
            let mut tree = LsmTree::open(&temp_dir).unwrap();
            tree.put_str("tx:1", "pending").unwrap();
            tree.put_str("tx:2", "committed").unwrap();
            tree.put_str("tx:3", "rollback").unwrap();
            tree.delete_str("tx:1").unwrap();
            tree.sync().unwrap();
            // Drop tree (simulating shutdown/crash without flushing to SSTable)
        }

        // 2. Reopen from same directory: should replay WAL
        {
            let mut recovered_tree = LsmTree::open(&temp_dir).unwrap();
            assert_eq!(recovered_tree.get_str("tx:1").unwrap(), None); // was deleted before crash
            assert_eq!(recovered_tree.get_str("tx:2").unwrap(), Some("committed".to_string()));
            assert_eq!(recovered_tree.get_str("tx:3").unwrap(), Some("rollback".to_string()));

            // Write new records after recovery
            recovered_tree.put_str("tx:4", "new_tx").unwrap();
            assert_eq!(recovered_tree.get_str("tx:4").unwrap(), Some("new_tx".to_string()));
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_lsm_tree_range_scan() {
        let temp_dir = std::env::temp_dir().join("lsm_engine_scan");
        let _ = fs::remove_dir_all(&temp_dir);

        let config = LsmConfig {
            memtable_capacity_bytes: 200,
            sstable_block_size: 128,
            sync_wal: false,
        };
        let mut tree = LsmTree::open_with_config(&temp_dir, config).unwrap();

        // Populate entries across multiple SSTables
        for i in (10..=60).step_by(10) {
            tree.put_str(&format!("k:{:02}", i), &format!("v:{}", i)).unwrap();
        }

        // Update k:30, delete k:40
        tree.put_str("k:30", "v:30_updated").unwrap();
        tree.delete_str("k:40").unwrap();

        // Scan range k:20 ..= k:50
        let results = tree.scan_str("k:20", "k:50").unwrap();
        let expected = vec![
            ("k:20".to_string(), "v:20".to_string()),
            ("k:30".to_string(), "v:30_updated".to_string()),
            // k:40 was deleted (Tombstone), so must NOT appear!
            ("k:50".to_string(), "v:50".to_string()),
        ];

        assert_eq!(results, expected);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_lsm_tree_compaction() {
        let temp_dir = std::env::temp_dir().join("lsm_engine_compaction");
        let _ = fs::remove_dir_all(&temp_dir);

        let config = LsmConfig {
            memtable_capacity_bytes: 150,
            sstable_block_size: 128,
            sync_wal: false,
        };
        let mut tree = LsmTree::open_with_config(&temp_dir, config).unwrap();

        // Generate multiple SSTables with overwrites and deletions
        for epoch in 0..5 {
            tree.put_str("user:alice", &format!("status_{}", epoch)).unwrap();
            tree.put_str("user:bob", "active").unwrap();
            tree.put_str(&format!("temp:{}", epoch), "garbage").unwrap();
            tree.flush().unwrap();
        }

        // Delete Bob and all temp keys
        tree.delete_str("user:bob").unwrap();
        for epoch in 0..5 {
            tree.delete_str(&format!("temp:{}", epoch)).unwrap();
        }
        tree.flush().unwrap();

        let sst_count_before = tree.sstable_count();
        assert!(sst_count_before >= 5, "Expected at least 5 SSTables before compaction, got {}", sst_count_before);

        // Run Compaction: collapses all SSTables into 1, purges dead tombstones & overwritten versions
        tree.compact().unwrap();

        assert_eq!(tree.sstable_count(), 1, "Expected exactly 1 consolidated SSTable after compaction");

        // Latest Alice status is preserved
        assert_eq!(tree.get_str("user:alice").unwrap(), Some("status_4".to_string()));

        // Bob and temp keys are deleted
        assert_eq!(tree.get_str("user:bob").unwrap(), None);
        for epoch in 0..5 {
            assert_eq!(tree.get_str(&format!("temp:{}", epoch)).unwrap(), None);
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }
}


