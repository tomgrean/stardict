extern crate regex;

use std::{fs, path, str};

use super::dict::Dict;
use super::idx::Idx;
use super::ifo::Ifo;
use super::result::DictError;
use self::regex::bytes::Regex;
use self::regex::Error;

pub struct Dictionary {
    pub ifo: Ifo,
    pub idx: Idx,
    pub dict: Dict,
}

pub struct IdxIter<'a> {
    cur: usize,
    idx: &'a Idx,
    matcher: Result<Regex, &'a Regex>,
}
pub struct DictNeighborIter<'a> {
    cur: usize,
    idx: &'a Idx,
}
impl Dictionary {
    pub fn new(root: &path::Path) -> Result<Dictionary, DictError> {
        for it in fs::read_dir(root)? {
            let it = it?.path();
            if it.is_file() {
                if let Some(ext) = it.extension() {
                    if let Some("ifo") = ext.to_str() {
                        let ifo = Ifo::open(&it)?;
                        let mut file = it.to_path_buf();
                        file.set_extension("idx");
                        let idx = Idx::open(&file, ifo.idx_file_size, ifo.word_count, (ifo.idxoffsetbits / 8 + 4) as u8)?;
                        file.set_extension("dict");
                        let dict = Dict::open(&file)?;
                        return Ok(Dictionary { ifo, idx, dict });
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
                Ok(IdxIter {cur:0, idx:&self.idx, matcher:Ok(reg)})
            },
            _ => Err(Error::Syntax(String::from("bad utf8"))),
        }
    }
    pub fn search_regex<'a>(&'a self, reg: &'a Regex) -> IdxIter<'a> {
        IdxIter {cur: 0, idx: &self.idx, matcher: Err(reg)}
    }

    pub fn lookup(&mut self, word: &[u8]) -> Result<Vec<u8>, DictError> {
        match self.idx.get(word) {
            Ok(i) => {
                let (eoffset, elength) = self.idx.get_offset_length(i)?;
                self.dict.read(eoffset as u64, elength as usize)
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
                if self.matcher.as_ref().unwrap_or_else(|v|*v).is_match(e) {
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
