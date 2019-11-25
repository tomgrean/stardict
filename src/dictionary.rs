extern crate regex;

use std::{fs, path, str, borrow::Cow};

use super::dict::Dict;
use super::idx::Idx;
use super::syn::Syn;
use super::ifo::Ifo;
use super::result::DictError;
use self::regex::bytes::Regex;
use self::regex::Error;

pub struct Dictionary {
    pub ifo: Ifo,
    pub idx: Idx,
    pub syn: Option<Syn>,
    pub dict: Dict,
}
pub struct LookupResult<'a> {
    pub dictionary: &'a Ifo,
    pub word: &'a [u8],
    pub result: Vec<u8>,
}

pub struct IdxIter<'a> {
    cur: usize,
    idx: &'a Idx,
    matcher: Cow<'a, Regex>,
}
pub struct DictNeighborIter<'a> {
    cur: usize,
    idx: &'a Idx,
}
impl Dictionary {
    pub fn new(root: &path::Path, base: &path::Path) -> Result<Dictionary, DictError> {
        for it in fs::read_dir(root)? {
            let it = it?.path();
            if it.is_file() {
                if let Some(ext) = it.extension() {
                    if let Some("ifo") = ext.to_str() {
                        let ifo = Ifo::open(&it, base)?;
                        let mut file = it.to_path_buf();
                        file.set_extension("idx");
                        let idx = Idx::open(&file, ifo.idx_file_size, ifo.word_count, (ifo.idxoffsetbits / 8 + 4) as u8)?;
                        file.set_extension("dict");
                        let dict = Dict::open(&file)?;
                        file.set_extension("syn");
                        let syn = Syn::open(&file, ifo.syn_word_count).ok();
                        return Ok(Dictionary { ifo, idx, dict, syn });
                    }
                }
            }
        }
        Err(DictError::My(format!(
            "bad dictionary directory {}",
            root.display()
        )))
    }

    pub fn neighbors(&self, word: &[u8], off: i32) -> DictNeighborIter {
        let ret = match self.idx.get(word) {
            Ok(i) => i,
            Err(i) => i,
        } as i32;
        let istart = ret + off;
        let start: usize = if istart < 0 { 0 } else { istart as usize };

        DictNeighborIter { cur: start, idx: &self.idx }
    }
    // search by regular expression
    pub fn search(&self, expr: &[u8]) -> Result<IdxIter, Error> {
        match str::from_utf8(expr) {
            Ok(e) => {
                let reg = Regex::new(e)?;
                Ok(IdxIter {cur:0, idx:&self.idx, matcher:Cow::Owned(reg)})
            },
            _ => Err(Error::Syntax(String::from("bad utf8"))),
        }
    }
    pub fn search_regex<'a>(&'a self, reg: &'a Regex) -> IdxIter<'a> {
        IdxIter {cur: 0, idx: &self.idx, matcher: Cow::Borrowed(reg)}
    }

    pub fn lookup(&mut self, word: &[u8]) -> Result<LookupResult, DictError> {
        let mut index = Err(DictError::NotFound);
        if let Some(s) = &self.syn {
            if let Ok(i) = s.get(word) {
                index = s.get_offset(i);
            }
        }
        match index.or_else(|_|self.idx.get(word)) {
            Ok(i) => {
                let (eoffset, elength) = self.idx.get_offset_length(i)?;
                Ok(LookupResult {
                    dictionary: &self.ifo,
                    word: self.idx.get_word(i)?,
                    result: self.dict.read(eoffset as u64, elength as usize)?
                })
            }
            _ => Err(DictError::NotFound),
        }
    }
}
impl<'a> Iterator for IdxIter<'a> {
    type Item = &'a [u8];
    fn next(&mut self) -> Option<Self::Item> {
        while self.cur < self.idx.len() {
            let v = self.idx.get_word(self.cur);
            self.cur += 1;
            if let Ok(e) = v {
                if self.matcher.is_match(e) {
                    return Some(e);
                }
            }
        }
        None
    }
}
impl<'a> Iterator for DictNeighborIter<'a> {
    type Item = &'a [u8];
    fn next(&mut self) -> Option<Self::Item> {
        if self.cur < self.idx.len() {
            let v = self.idx.get_word(self.cur);
            self.cur += 1;
            if let Ok(e) = v {
                return Some(e);
            }
        }
        None
    }
}
