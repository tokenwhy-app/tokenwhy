// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use crate::metrics::summary::{summarize, SessionSummary};
use crate::model::{
    FileRecords, IdHash, ParseResume, ParsedFile, PriceTable, ProviderId, SourceFile, TimestampMs,
};
use crate::provider::{Change, Fingerprint, Provider};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Mutex};
use std::time::UNIX_EPOCH;

pub type SessionKey = (ProviderId, String);

const HEAD_BYTES: u64 = 4096;
const WORKERS: usize = 4;
pub const HOT_MS: i64 = 10 * 60_000;
const INDEX_MAGIC: [u8; 8] = *b"TWINDEX1";
const SUMMARY_MAGIC: [u8; 8] = *b"TWSUMRY4";
const SESSION_MAGIC: [u8; 8] = *b"TWSESSN1";
/// Bump when the encoding of the index, the summaries or the session files changes.
const SCHEMA: u32 = 9;
const MERGE_AT: usize = 4096;
const NO_OWNER: u32 = u32::MAX;

pub(crate) fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn salt(provider: &ProviderId) -> u64 {
    fnv1a(provider.0.as_bytes())
}

fn head_hash(path: &Path, len: u32) -> io::Result<u64> {
    let mut buf = vec![0u8; len as usize];
    std::fs::File::open(path)?.read_exact(&mut buf)?;
    Ok(fnv1a(&buf))
}

pub fn append_fingerprint(path: &Path) -> io::Result<crate::provider::Fingerprint> {
    let (size, mtime_ns) = stat(path).ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
    let head_len = size.min(HEAD_BYTES) as u32;
    Ok(crate::provider::Fingerprint {
        size,
        mtime_ns,
        head_len,
        head_hash: head_hash(path, head_len)?,
        change: crate::provider::Change::Append,
    })
}

fn stat(path: &Path) -> Option<(u64, u64)> {
    let m = std::fs::metadata(path).ok()?;
    if !m.is_file() {
        return None;
    }
    let mtime = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos() as u64);
    Some((m.len(), mtime))
}

fn key_ts(r: &FileRecords) -> i64 {
    r.turns
        .iter()
        .map(|t| t.first_ts_ms)
        .min()
        .unwrap_or(i64::MAX)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileEntry {
    source: SourceFile,
    size: u64,
    mtime_ns: u64,
    head_len: u32,
    head_hash: u64,
    change: Change,
    key_ts: i64,
    shares_with: Vec<String>,
}

impl FileEntry {
    fn session(&self) -> SessionKey {
        (self.source.provider.clone(), self.source.session_id.clone())
    }
    fn order(&self) -> (i64, &Path) {
        (self.key_ts, &self.source.path)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct IdIndex {
    ids: Vec<u64>,
    owners: Vec<u32>,
    #[serde(skip)]
    pending: HashMap<u64, u32>,
}

impl IdIndex {
    fn get(&self, id: u64) -> Option<u32> {
        let o = match self.pending.get(&id) {
            Some(o) => *o,
            None => self
                .ids
                .binary_search(&id)
                .ok()
                .map_or(NO_OWNER, |i| self.owners[i]),
        };
        (o != NO_OWNER).then_some(o)
    }
    fn set(&mut self, id: u64, owner: u32) {
        match self.ids.binary_search(&id) {
            Ok(i) if !self.pending.contains_key(&id) => self.owners[i] = owner,
            _ => {
                self.pending.insert(id, owner);
                if self.pending.len() > MERGE_AT {
                    self.merge();
                }
            }
        }
    }
    fn remove(&mut self, id: u64) {
        self.set(id, NO_OWNER);
    }
    fn merge(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let mut p: Vec<(u64, u32)> = self.pending.drain().collect();
        p.sort_unstable();
        let n = self.ids.len() + p.len();
        let (mut ids, mut owners) = (Vec::with_capacity(n), Vec::with_capacity(n));
        let (mut i, mut j) = (0, 0);
        while i < self.ids.len() || j < p.len() {
            let take_old = j >= p.len() || (i < self.ids.len() && self.ids[i] < p[j].0);
            let (id, o) = if take_old {
                i += 1;
                (self.ids[i - 1], self.owners[i - 1])
            } else {
                if i < self.ids.len() && self.ids[i] == p[j].0 {
                    i += 1;
                }
                j += 1;
                p[j - 1]
            };
            if o != NO_OWNER {
                ids.push(id);
                owners.push(o);
            }
        }
        ids.shrink_to_fit();
        owners.shrink_to_fit();
        self.ids = ids;
        self.owners = owners;
        self.pending.shrink_to_fit();
    }
    fn bytes(&self) -> usize {
        self.ids.capacity() * 8 + self.owners.capacity() * 4 + self.pending.capacity() * 16
    }
}

struct Hot {
    files: HashMap<u32, ParsedFile>,
    last_write_ms: i64,
    disk_dirty: bool,
}

#[derive(Serialize, Deserialize)]
struct IndexDisk {
    magic: [u8; 8],
    schema: u32,
    generation: u64,
    prices: Vec<(String, u32, u64)>,
    files: Vec<Option<FileEntry>>,
    ids: IdIndex,
}

#[derive(Serialize, Deserialize)]
struct SummaryDisk {
    magic: [u8; 8],
    schema: u32,
    generation: u64,
    prices: Vec<(String, u32, u64)>,
    summaries: Vec<SessionSummary>,
}

#[derive(Serialize, Deserialize)]
struct SessionDisk {
    magic: [u8; 8],
    schema: u32,
    files: Vec<(PathBuf, FileRecords)>,
}

#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub files: usize,
    pub sessions: usize,
    pub hot_sessions: usize,
    pub index_bytes: usize,
    pub summary_bytes: usize,
    pub id_index_bytes: usize,
    pub ids: usize,
    pub hot_bytes: usize,
    pub summary_fields: Vec<(&'static str, usize)>,
}

pub struct Root<'a> {
    pub provider: &'a dyn Provider,
    pub path: PathBuf,
}

pub struct Corpus {
    files: Vec<Option<FileEntry>>,
    by_path: HashMap<PathBuf, u32>,
    ids: IdIndex,
    hot: HashMap<SessionKey, Hot>,
    summaries: HashMap<SessionKey, SessionSummary>,
    dir: Option<PathBuf>,
    prices: Vec<(String, u32, u64)>,
    /// Written into both the index and the summaries; a pair whose numbers differ was cut by a crash between the writes.
    generation: u64,
}

pub(crate) fn price_key(prices: &[PriceTable]) -> Vec<(String, u32, u64)> {
    let mut k: Vec<_> = prices
        .iter()
        .map(|p| {
            (
                p.provider.0.clone(),
                p.version,
                fnv1a(&postcard::to_allocvec(&p.rates).unwrap_or_default()),
            )
        })
        .collect();
    k.sort();
    k
}

use crate::store::write_atomic;

struct Job {
    no: Option<u32>,
    source: SourceFile,
    provider: usize,
    fp: Fingerprint,
    resume: Option<ParseResume>,
}

#[derive(Default)]
struct Pass {
    dirty: BTreeSet<SessionKey>,
    released: HashSet<u64>,
    candidates: BTreeSet<SessionKey>,
}

impl Corpus {
    /// A price change drops only the summaries of the providers whose table moved; they are rebuilt from the stored records, without reading transcripts.
    pub fn open(dir: Option<PathBuf>, prices: &[PriceTable]) -> Corpus {
        let key = price_key(prices);
        let mut c = Corpus {
            files: vec![],
            by_path: HashMap::new(),
            ids: IdIndex::default(),
            hot: HashMap::new(),
            summaries: HashMap::new(),
            dir,
            prices: key,
            generation: 0,
        };
        let Some(d) = c.dir.clone() else { return c };
        let index = std::fs::read(d.join("index.bin"))
            .ok()
            .and_then(|b| postcard::from_bytes::<IndexDisk>(&b).ok());
        let Some(index) = index.filter(|i| i.magic == INDEX_MAGIC && i.schema == SCHEMA) else {
            return c;
        };
        c.generation = index.generation;
        c.files = index.files;
        c.ids = index.ids;
        c.by_path = c
            .files
            .iter()
            .enumerate()
            .filter_map(|(i, e)| e.as_ref().map(|e| (e.source.path.clone(), i as u32)))
            .collect();
        let sums = std::fs::read(d.join("summaries.bin"))
            .ok()
            .and_then(|b| postcard::from_bytes::<SummaryDisk>(&b).ok());
        if let Some(s) = sums.filter(|s| {
            s.magic == SUMMARY_MAGIC && s.schema == SCHEMA && s.generation == c.generation
        }) {
            let of = |k: &[(String, u32, u64)], p: &ProviderId| -> Vec<(String, u32, u64)> {
                k.iter().filter(|e| e.0 == p.0).cloned().collect()
            };
            let (old, new) = (s.prices, c.prices.clone());
            c.summaries = s
                .summaries
                .into_iter()
                .filter(|s| {
                    let mine = of(&new, &s.provider);
                    !mine.is_empty() && of(&old, &s.provider) == mine
                })
                .map(|s| ((s.provider.clone(), s.session.clone()), s))
                .collect();
        }
        c
    }

    fn session_path(&self, key: &SessionKey) -> Option<PathBuf> {
        let d = self.dir.as_ref()?;
        Some(
            d.join("sessions")
                .join(&key.0 .0)
                .join(format!("{:016x}.bin", fnv1a(key.1.as_bytes()))),
        )
    }

    fn session_files(&self, key: &SessionKey) -> Vec<u32> {
        let mut v: Vec<u32> = self
            .files
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                e.as_ref()
                    .filter(|e| e.source.provider == key.0 && e.source.session_id == key.1)
                    .map(|_| i as u32)
            })
            .collect();
        v.sort_by(|a, b| self.entry(*a).source.path.cmp(&self.entry(*b).source.path));
        v
    }

    fn entry(&self, no: u32) -> &FileEntry {
        self.files[no as usize].as_ref().expect("live file number")
    }

    fn read_session_disk(&self, key: &SessionKey) -> HashMap<u32, ParsedFile> {
        let mut out = HashMap::new();
        let Some(p) = self.session_path(key) else {
            return out;
        };
        let Some(disk) = std::fs::read(p)
            .ok()
            .and_then(|b| postcard::from_bytes::<SessionDisk>(&b).ok())
        else {
            return out;
        };
        if disk.magic != SESSION_MAGIC || disk.schema != SCHEMA {
            return out;
        }
        for (path, records) in disk.files {
            if let Some(&no) = self.by_path.get(&path) {
                out.insert(
                    no,
                    ParsedFile {
                        source: self.entry(no).source.clone(),
                        records,
                    },
                );
            }
        }
        out
    }

    fn hot_mut(&mut self, key: &SessionKey) -> &mut Hot {
        if !self.hot.contains_key(key) {
            let files = self.read_session_disk(key);
            self.hot.insert(
                key.clone(),
                Hot {
                    files,
                    last_write_ms: 0,
                    disk_dirty: false,
                },
            );
        }
        self.hot.get_mut(key).unwrap()
    }

    fn write_session(&self, key: &SessionKey, files: &HashMap<u32, ParsedFile>) -> io::Result<()> {
        let Some(p) = self.session_path(key) else {
            return Ok(());
        };
        if files.is_empty() {
            let _ = std::fs::remove_file(&p);
            return Ok(());
        }
        #[derive(Serialize)]
        struct Out<'a> {
            magic: [u8; 8],
            schema: u32,
            files: Vec<(&'a Path, &'a FileRecords)>,
        }
        let mut v: Vec<(&Path, &FileRecords)> = files
            .values()
            .map(|f| (f.source.path.as_path(), &f.records))
            .collect();
        v.sort_by(|a, b| a.0.cmp(b.0));
        let bytes = postcard::to_allocvec(&Out {
            magic: SESSION_MAGIC,
            schema: SCHEMA,
            files: v,
        })
        .map_err(io::Error::other)?;
        write_atomic(&p, &bytes)
    }

    pub fn refresh_all(
        &mut self,
        roots: &[Root<'_>],
        prices: &[PriceTable],
        now_ms: TimestampMs,
        progress: &dyn Fn(f64),
    ) -> io::Result<BTreeSet<SessionKey>> {
        let mut found: Vec<(usize, SourceFile)> = vec![];
        let mut seen: HashSet<PathBuf> = HashSet::new();
        for (ri, root) in roots.iter().enumerate() {
            let Ok(list) = root.provider.discover(&root.path) else {
                continue;
            };
            for s in list {
                if seen.insert(s.path.clone()) {
                    found.push((ri, s));
                }
            }
        }
        let gone: Vec<PathBuf> = self
            .by_path
            .keys()
            .filter(|p| !seen.contains(*p))
            .cloned()
            .collect();
        self.apply(roots, found, gone, prices, now_ms, progress)
    }

    pub fn update(
        &mut self,
        roots: &[Root<'_>],
        dirty: &[PathBuf],
        prices: &[PriceTable],
        now_ms: TimestampMs,
    ) -> io::Result<BTreeSet<SessionKey>> {
        let mut found = vec![];
        let mut gone = vec![];
        for p in dirty {
            let src = roots.iter().enumerate().find_map(|(ri, r)| {
                if !p.starts_with(&r.path) {
                    return None;
                }
                let list = r.provider.sources_of(&r.path, p);
                (!list.is_empty()).then(|| list.into_iter().map(move |s| (ri, s)))
            });
            match src {
                Some(s) => found.extend(s),
                None if self.by_path.contains_key(p) => gone.push(p.clone()),
                None => {
                    let unit = format!("{}#", p.display());
                    gone.extend(
                        self.by_path
                            .keys()
                            .filter(|k| k.to_string_lossy().starts_with(&unit))
                            .cloned(),
                    );
                }
            }
        }
        self.apply(roots, found, gone, prices, now_ms, &|_| {})
    }

    fn apply(
        &mut self,
        roots: &[Root<'_>],
        found: Vec<(usize, SourceFile)>,
        gone: Vec<PathBuf>,
        prices: &[PriceTable],
        now_ms: TimestampMs,
        progress: &dyn Fn(f64),
    ) -> io::Result<BTreeSet<SessionKey>> {
        let mut pass = Pass::default();
        for p in gone {
            self.remove_file(&p, &mut pass);
        }

        let mut jobs: Vec<Job> = vec![];
        for (ri, source) in found {
            let no = self.by_path.get(&source.path).copied();
            let same_stat = no.and_then(|n| {
                let e = self.entry(n);
                (e.change == Change::Append)
                    .then(|| stat(&source.path) == Some((e.size, e.mtime_ns)))
            });
            let fp = match same_stat {
                Some(true) => None,
                _ => match roots[ri].provider.fingerprint(&source) {
                    Ok(fp) => Some(fp),
                    Err(_) => {
                        if no.is_some() {
                            self.remove_file(&source.path, &mut pass);
                        }
                        continue;
                    }
                },
            };
            if let Some(no) = no {
                let e = self.entry(no);
                let unchanged = fp.is_none_or(|fp| {
                    e.size == fp.size
                        && e.mtime_ns == fp.mtime_ns
                        && e.change == fp.change
                        && (fp.change == Change::Append || e.head_hash == fp.head_hash)
                });
                if unchanged {
                    if e.source != source {
                        let old = e.session();
                        let new = (source.provider.clone(), source.session_id.clone());
                        if old != new {
                            self.remove_file(&source.path, &mut pass);
                        } else {
                            self.files[no as usize].as_mut().unwrap().source = source.clone();
                            if let Some(pf) =
                                self.hot.get_mut(&old).and_then(|h| h.files.get_mut(&no))
                            {
                                pf.source = source;
                            }
                            pass.dirty.insert(old);
                            continue;
                        }
                    } else {
                        continue;
                    }
                }
            }
            let fp = match fp {
                Some(fp) => fp,
                None => match roots[ri].provider.fingerprint(&source) {
                    Ok(fp) => fp,
                    Err(_) => continue,
                },
            };
            let no = self.by_path.get(&source.path).copied();
            let mut resume = None;
            if let Some(no) = no.filter(|_| fp.change == Change::Append) {
                let e = self.entry(no).clone();
                let same_head = e.change == Change::Append
                    && if e.head_len == fp.head_len {
                        e.head_hash == fp.head_hash
                    } else {
                        head_hash(&source.path, e.head_len).ok() == Some(e.head_hash)
                    };
                if fp.size > e.size
                    && same_head
                    && e.session() == (source.provider.clone(), source.session_id.clone())
                {
                    if let Some(prev) = self.hot_mut(&e.session()).files.remove(&no) {
                        resume = Some(ParseResume {
                            offset: prev.records.offset_done,
                            previous: prev.records,
                        });
                    }
                }
            }
            jobs.push(Job {
                no,
                source,
                provider: ri,
                fp,
                resume,
            });
        }

        let mut parsed = parse_all(roots, jobs, progress);
        parsed.sort_by(|a, b| {
            (key_ts(&a.1), &a.0.source.path).cmp(&(key_ts(&b.1), &b.0.source.path))
        });
        for (job, rec) in parsed {
            self.store_file(job, rec, &mut pass);
        }
        self.reclaim(&mut pass);
        let missing: BTreeSet<SessionKey> = self
            .files
            .iter()
            .flatten()
            .map(|e| e.session())
            .filter(|k| !self.summaries.contains_key(k))
            .collect();
        pass.dirty.extend(missing);
        for key in &pass.dirty {
            self.rebuild(key, roots, prices, now_ms);
        }
        self.evict(now_ms)?;
        Ok(pass.dirty)
    }

    fn remove_file(&mut self, path: &Path, pass: &mut Pass) {
        let Some(&no) = self.by_path.get(path) else {
            return;
        };
        let key = self.entry(no).session();
        self.hot_mut(&key);
        self.by_path.remove(path);
        let e = self.files[no as usize].take().unwrap();
        let key = e.session();
        let h = self.hot_mut(&key);
        let recs = h.files.remove(&no);
        h.disk_dirty = true;
        if let Some(r) = recs {
            self.release(no, salt(&key.0), &r.records, &mut pass.released);
        }
        pass.candidates
            .extend(e.shares_with.iter().map(|s| (key.0.clone(), s.clone())));
        pass.dirty.insert(key);
    }

    fn release(&mut self, no: u32, salt: u64, rec: &FileRecords, released: &mut HashSet<u64>) {
        for t in &rec.turns {
            let id = t.msg_id ^ salt;
            if self.ids.get(id) == Some(no) {
                self.ids.remove(id);
                released.insert(id);
            }
        }
    }

    fn store_file(&mut self, job: Job, rec: FileRecords, pass: &mut Pass) {
        let new_key = key_ts(&rec);
        let no = match job.no {
            Some(no) => no,
            None => {
                self.files.push(None);
                let no = (self.files.len() - 1) as u32;
                self.by_path.insert(job.source.path.clone(), no);
                no
            }
        };
        if let Some(k) = self.files[no as usize].as_ref().map(|e| e.session()) {
            self.hot_mut(&k);
        }
        let old = self.files[no as usize].take();
        let mut shares: Vec<String> = old
            .as_ref()
            .map(|e| e.shares_with.clone())
            .unwrap_or_default();
        let s = salt(&job.source.provider);
        let mut new_ids: Vec<u64> = rec.turns.iter().map(|t| t.msg_id ^ s).collect();
        if let Some(e) = &old {
            let old_key = e.session();
            let prev: Option<FileRecords> = match job.resume {
                Some(r) => Some(r.previous),
                None => {
                    let h = self.hot_mut(&old_key);
                    h.disk_dirty = true;
                    h.files.remove(&no).map(|p| p.records)
                }
            };
            let appended = e.key_ts == new_key
                && e.session() == (job.source.provider.clone(), job.source.session_id.clone());
            match prev {
                Some(prev) if appended => {
                    let before: HashSet<u64> = prev.turns.iter().map(|t| t.msg_id ^ s).collect();
                    new_ids.retain(|id| !before.contains(id));
                    let now: HashSet<u64> = rec.turns.iter().map(|t| t.msg_id ^ s).collect();
                    for id in before.difference(&now) {
                        if self.ids.get(*id) == Some(no) {
                            self.ids.remove(*id);
                            pass.released.insert(*id);
                        }
                    }
                }
                Some(prev) => {
                    self.release(no, salt(&old_key.0), &prev, &mut pass.released);
                    pass.candidates
                        .extend(e.shares_with.iter().map(|s| (old_key.0.clone(), s.clone())));
                }
                None => {}
            }
            pass.dirty.insert(old_key);
        }
        let source = job.source;
        let key = (source.provider.clone(), source.session_id.clone());
        let me = (new_key, source.path.clone());
        for id in new_ids {
            match self.ids.get(id) {
                None => self.ids.set(id, no),
                Some(o) if o == no => {}
                Some(o) => {
                    let g = self.entry(o);
                    let g_session = g.source.session_id.clone();
                    if (me.0, me.1.as_path()) < g.order() {
                        let g_key = g.session();
                        let inherited = g.shares_with.clone();
                        self.ids.set(id, no);
                        pass.dirty.insert(g_key);
                        for s in inherited {
                            if !shares.contains(&s) {
                                shares.push(s);
                            }
                        }
                    }
                    if !shares.contains(&g_session) {
                        shares.push(g_session);
                    }
                    let ge = self.files[o as usize].as_mut().unwrap();
                    if !ge.shares_with.contains(&key.1) {
                        ge.shares_with.push(key.1.clone());
                    }
                }
            }
        }
        let mtime_ms = (job.fp.mtime_ns / 1_000_000) as i64;
        self.files[no as usize] = Some(FileEntry {
            source: source.clone(),
            size: job.fp.size,
            mtime_ns: job.fp.mtime_ns,
            head_len: job.fp.head_len,
            head_hash: job.fp.head_hash,
            change: job.fp.change,
            key_ts: new_key,
            shares_with: shares,
        });
        let h = self.hot_mut(&key);
        h.files.insert(
            no,
            ParsedFile {
                source,
                records: rec,
            },
        );
        h.disk_dirty = true;
        h.last_write_ms = h.last_write_ms.max(mtime_ms);
        pass.dirty.insert(key);
    }

    fn reclaim(&mut self, pass: &mut Pass) {
        if pass.released.is_empty() {
            return;
        }
        let mut holders: Vec<(u32, Vec<u64>)> = vec![];
        for key in &pass.candidates {
            let from_hot = self.hot.get(key).map(|h| {
                h.files
                    .iter()
                    .map(|(n, f)| (*n, ids_in(&f.records, salt(&key.0), &pass.released)))
                    .collect::<Vec<_>>()
            });
            let list = match from_hot {
                Some(l) => l,
                None => self
                    .read_session_disk(key)
                    .into_iter()
                    .map(|(n, f)| (n, ids_in(&f.records, salt(&key.0), &pass.released)))
                    .collect(),
            };
            holders.extend(
                list.into_iter()
                    .filter(|(n, _)| self.files[*n as usize].is_some()),
            );
        }
        holders.sort_by(|a, b| self.entry(a.0).order().cmp(&self.entry(b.0).order()));
        for (no, ids) in holders {
            for id in ids {
                if self.ids.get(id).is_none() {
                    self.ids.set(id, no);
                    pass.dirty.insert(self.entry(no).session());
                }
            }
        }
    }

    fn rebuild(
        &mut self,
        key: &SessionKey,
        roots: &[Root<'_>],
        prices: &[PriceTable],
        now_ms: TimestampMs,
    ) {
        let nos = self.session_files(key);
        if nos.is_empty() {
            self.summaries.remove(key);
            self.hot.remove(key);
            if let Some(p) = self.session_path(key) {
                let _ = std::fs::remove_file(p);
            }
            return;
        }
        let Some(provider) = roots
            .iter()
            .find(|r| r.provider.id() == key.0)
            .map(|r| r.provider)
        else {
            return;
        };
        let mut disk = if self.hot.contains_key(key) {
            HashMap::new()
        } else {
            self.read_session_disk(key)
        };
        let have = |c: &Corpus, d: &HashMap<u32, ParsedFile>, n: u32| {
            d.contains_key(&n) || c.hot.get(key).is_some_and(|h| h.files.contains_key(&n))
        };
        let lost: Vec<u32> = nos
            .iter()
            .copied()
            .filter(|n| !have(self, &disk, *n))
            .collect();
        if !lost.is_empty() {
            for (no, pf) in disk.drain() {
                self.hot_mut(key).files.insert(no, pf);
            }
            for no in lost {
                let src = self.entry(no).source.clone();
                if let Ok(records) = parse_guarded(provider, &src, None) {
                    let h = self.hot_mut(key);
                    h.files.insert(
                        no,
                        ParsedFile {
                            source: src,
                            records,
                        },
                    );
                    h.disk_dirty = true;
                }
            }
        }
        let summary = {
            let hot = self.hot.get(key);
            let ids = &self.ids;
            let s = salt(&key.0);
            let mut trimmed: Vec<ParsedFile> = vec![];
            let mut order: Vec<(u32, bool)> = vec![];
            for &me in &nos {
                let pf = match disk.remove(&me) {
                    Some(pf) => Cow::Owned(pf),
                    None => match hot.and_then(|h| h.files.get(&me)) {
                        Some(pf) => Cow::Borrowed(pf),
                        None => continue,
                    },
                };
                let owned = |id: IdHash| ids.get(id ^ s).is_none_or(|o| o == me);
                if pf.records.turns.iter().all(|t| owned(t.msg_id)) {
                    match pf {
                        Cow::Owned(pf) => {
                            trimmed.push(pf);
                            order.push((me, true));
                        }
                        Cow::Borrowed(_) => order.push((me, false)),
                    }
                    continue;
                }
                let mut pf = pf.into_owned();
                provider.drop_copies(&mut pf, &owned);
                pf.records.resegment();
                trimmed.push(pf);
                order.push((me, true));
            }
            let mut it = trimmed.iter();
            let refs: Vec<&ParsedFile> = order
                .iter()
                .filter_map(|(no, own)| {
                    if *own {
                        it.next()
                    } else {
                        hot.and_then(|h| h.files.get(no))
                    }
                })
                .collect();
            summarize(&refs, prices, now_ms).into_iter().next()
        };
        match summary {
            Some(s) => self.summaries.insert(key.clone(), s),
            None => self.summaries.remove(key),
        };
    }

    pub fn evict(&mut self, now_ms: TimestampMs) -> io::Result<()> {
        if self.dir.is_none() {
            return Ok(());
        }
        let cold: Vec<SessionKey> = self
            .hot
            .iter()
            .filter(|(_, h)| now_ms - h.last_write_ms > HOT_MS)
            .map(|(k, _)| k.clone())
            .collect();
        for k in cold {
            let h = self.hot.remove(&k).unwrap();
            if h.disk_dirty {
                self.write_session(&k, &h.files)?;
            }
        }
        Ok(())
    }

    pub fn next_eviction(&self) -> Option<TimestampMs> {
        self.dir.as_ref()?;
        self.hot
            .values()
            .map(|h| h.last_write_ms + HOT_MS + 1)
            .min()
    }

    pub fn save(&mut self) -> io::Result<()> {
        let Some(d) = self.dir.clone() else {
            return Ok(());
        };
        let dirty: Vec<SessionKey> = self
            .hot
            .iter()
            .filter(|(_, h)| h.disk_dirty)
            .map(|(k, _)| k.clone())
            .collect();
        for k in dirty {
            self.write_session(&k, &self.hot[&k].files)?;
            self.hot.get_mut(&k).unwrap().disk_dirty = false;
        }
        self.ids.merge();
        self.generation += 1;
        #[derive(Serialize)]
        struct Index<'a> {
            magic: [u8; 8],
            schema: u32,
            generation: u64,
            prices: &'a [(String, u32, u64)],
            files: &'a [Option<FileEntry>],
            ids: &'a IdIndex,
        }
        let index = Index {
            magic: INDEX_MAGIC,
            schema: SCHEMA,
            generation: self.generation,
            prices: &self.prices,
            files: &self.files,
            ids: &self.ids,
        };
        write_atomic(
            &d.join("index.bin"),
            &postcard::to_allocvec(&index).map_err(io::Error::other)?,
        )?;
        let mut sums: Vec<&SessionSummary> = self.summaries.values().collect();
        sums.sort_by(|a, b| (&a.provider, &a.session).cmp(&(&b.provider, &b.session)));
        #[derive(Serialize)]
        struct Sums<'a> {
            magic: [u8; 8],
            schema: u32,
            generation: u64,
            prices: &'a [(String, u32, u64)],
            summaries: Vec<&'a SessionSummary>,
        }
        let sums = Sums {
            magic: SUMMARY_MAGIC,
            schema: SCHEMA,
            generation: self.generation,
            prices: &self.prices,
            summaries: sums,
        };
        write_atomic(
            &d.join("summaries.bin"),
            &postcard::to_allocvec(&sums).map_err(io::Error::other)?,
        )?;
        // Schema 2's single-file cache is no longer read.
        let _ = std::fs::remove_file(d.join("records.bin"));
        Ok(())
    }

    pub fn summaries(&self) -> impl Iterator<Item = &SessionSummary> {
        self.summaries.values()
    }

    pub fn roll(
        &mut self,
        midnight: TimestampMs,
        roots: &[Root<'_>],
        prices: &[PriceTable],
        now_ms: TimestampMs,
    ) -> bool {
        let mut changed = false;
        let mut back = vec![];
        for (k, s) in self.summaries.iter_mut() {
            if s.files.iter().any(|f| f.live.midnight > midnight) {
                back.push(k.clone());
            } else if s.midnight() < midnight {
                s.roll(midnight);
                changed = true;
            }
        }
        for k in back {
            self.rebuild(&k, roots, prices, now_ms);
            changed = true;
        }
        changed
    }

    pub fn file_count(&self, provider: &ProviderId) -> usize {
        self.files
            .iter()
            .flatten()
            .filter(|e| &e.source.provider == provider)
            .count()
    }

    /// Loads every session: diagnostics and tests only.
    pub fn deduped_files(&self, roots: &[Root<'_>]) -> Vec<ParsedFile> {
        let keys: BTreeSet<SessionKey> = self.files.iter().flatten().map(|e| e.session()).collect();
        let mut out = vec![];
        for key in keys {
            let Some(provider) = roots
                .iter()
                .find(|r| r.provider.id() == key.0)
                .map(|r| r.provider)
            else {
                continue;
            };
            let mut recs = match self.hot.get(&key) {
                Some(h) => h.files.clone(),
                None => self.read_session_disk(&key),
            };
            for no in self.session_files(&key) {
                let Some(mut pf) = recs.remove(&no) else {
                    continue;
                };
                let (ids, s) = (&self.ids, salt(&key.0));
                provider.drop_copies(&mut pf, &|id| ids.get(id ^ s).is_none_or(|o| o == no));
                pf.records.resegment();
                out.push(pf);
            }
        }
        out.sort_by(|a, b| a.source.path.cmp(&b.source.path));
        out
    }

    pub fn stats(&self) -> Stats {
        let ser = |v: &dyn Fn() -> Result<Vec<u8>, postcard::Error>| v().map_or(0, |b| b.len());
        Stats {
            files: self.by_path.len(),
            sessions: self.summaries.len(),
            hot_sessions: self.hot.len(),
            index_bytes: self
                .files
                .iter()
                .flatten()
                .map(|e| ser(&|| postcard::to_allocvec(e)))
                .sum::<usize>()
                + self.by_path.capacity() * 40,
            summary_bytes: self
                .summaries
                .values()
                .map(|s| ser(&|| postcard::to_allocvec(s)))
                .sum(),
            id_index_bytes: self.ids.bytes(),
            ids: self.ids.ids.len() + self.ids.pending.len(),
            hot_bytes: self
                .hot
                .values()
                .flat_map(|h| h.files.values())
                .map(|f| ser(&|| postcard::to_allocvec(&f.records)))
                .sum(),
            summary_fields: crate::metrics::summary::size_breakdown(self.summaries.values()),
        }
    }
}

fn ids_in(r: &FileRecords, salt: u64, set: &HashSet<u64>) -> Vec<u64> {
    r.turns
        .iter()
        .map(|t| t.msg_id ^ salt)
        .filter(|id| set.contains(id))
        .collect()
}

fn parse_guarded<P: Provider + ?Sized>(
    provider: &P,
    source: &SourceFile,
    resume: Option<ParseResume>,
) -> io::Result<FileRecords> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        provider.parse_file(source, resume)
    }))
    .map(|r| {
        r.map(|mut rec| {
            rec.turns.retain(|t| t.usage.plausible());
            rec
        })
    })
    .unwrap_or_else(|_| {
        eprintln!(
            "tokenwhy: parser panicked on {}, file skipped",
            source.path.display()
        );
        Err(io::Error::other("parser panicked"))
    })
}

fn parse_all(
    roots: &[Root<'_>],
    mut jobs: Vec<Job>,
    progress: &dyn Fn(f64),
) -> Vec<(Job, FileRecords)> {
    jobs.sort_by(|a, b| b.fp.size.cmp(&a.fp.size));
    let sizes: Vec<u64> = jobs.iter().map(|j| j.fp.size.max(1)).collect();
    let total: u64 = sizes.iter().sum();
    progress(if jobs.is_empty() { 1.0 } else { 0.0 });
    let mut out = Vec::with_capacity(jobs.len());
    if jobs.len() <= 2 {
        for mut job in jobs {
            if let Ok(r) =
                parse_guarded(roots[job.provider].provider, &job.source, job.resume.take())
            {
                out.push((job, r));
            }
        }
        progress(1.0);
        return out;
    }
    let slots: Vec<Mutex<Option<Job>>> = jobs.into_iter().map(|j| Mutex::new(Some(j))).collect();
    let next = AtomicUsize::new(0);
    let (tx, rx) = mpsc::channel::<(usize, Job, io::Result<FileRecords>)>();
    std::thread::scope(|s| {
        for _ in 0..WORKERS.min(slots.len()) {
            let tx = tx.clone();
            let (slots, next) = (&slots, &next);
            s.spawn(move || loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(slot) = slots.get(i) else { break };
                let Some(mut job) = slot.lock().unwrap().take() else {
                    continue;
                };
                let res =
                    parse_guarded(roots[job.provider].provider, &job.source, job.resume.take());
                if tx.send((i, job, res)).is_err() {
                    break;
                }
            });
        }
        drop(tx);
        let mut done = 0u64;
        for (i, job, res) in rx {
            done += sizes[i];
            if let Ok(r) = res {
                out.push((job, r));
            }
            progress(done as f64 / total as f64);
        }
    });
    progress(1.0);
    out
}

pub fn dedup_all<P: Provider + ?Sized>(provider: &P, files: &mut [ParsedFile]) {
    let mut order: Vec<usize> = (0..files.len()).collect();
    order.sort_by(|&a, &b| {
        (key_ts(&files[a].records), &files[a].source.path)
            .cmp(&(key_ts(&files[b].records), &files[b].source.path))
    });
    let mut owner: HashMap<IdHash, usize> = HashMap::new();
    for &i in &order {
        for t in &files[i].records.turns {
            owner.entry(t.msg_id).or_insert(i);
        }
    }
    for (i, f) in files.iter_mut().enumerate() {
        provider.drop_copies(f, &|id| owner.get(&id).is_none_or(|o| *o == i));
    }
}
