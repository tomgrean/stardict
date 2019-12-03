extern crate regex;

use std::cmp::Ordering;
use std::{borrow::Cow, fs, path, str};

use self::regex::bytes::Regex;
use self::regex::Error;
use super::dict::Dict;
use super::idx::Idx;
use super::syn::Syn;
use super::ifo::Ifo;
use super::result::DictError;

/// used to make Syn and Idx iterator work together.
pub enum IdxRef<'a> {
    Ref(&'a Idx),
    SynRef(&'a Option<Syn>),
}
/// a Dictionary contains Ifo, Idx, Dict and Syn(optionally).
pub struct Dictionary {
    pub ifo: Ifo,
    pub idx: Idx,
    pub syn: Option<Syn>,
    pub dict: Dict,
}
/// the successful result a lookup would return.
pub struct LookupResult<'a> {
    pub dictionary: &'a Ifo,
    pub word: &'a [u8],
    pub result: Vec<u8>,
}
/// a regular expression search iterator.
pub struct IdxIter<'a> {
    cur: usize,
    idx: IdxRef<'a>,
    matcher: Cow<'a, Regex>,
}
/// a neighborhood list iterator.
pub struct DictNeighborIter<'a> {
    cur: usize,
    idx: IdxRef<'a>,
}
impl Dictionary {
    /// create a Dictionary from the dictionary directory.
    pub fn new(root: &path::Path, base: &path::Path) -> Result<Dictionary, DictError> {
        for it in fs::read_dir(root)? {
            let it = it?.path();
            if it.is_file() {
                if let Some(ext) = it.extension() {
                    if let Some("ifo") = ext.to_str() {
                        let ifo = Ifo::open(&it, base)?;
                        let mut file = it.to_path_buf();
                        file.set_extension("idx");
                        let idx = Idx::open(
                            &file,
                            ifo.idx_file_size,
                            ifo.word_count,
                            (ifo.idxoffsetbits / 8 + 4) as u8,
                        )?;
                        file.set_extension("dict");
                        let dict = Dict::open(&file)?;
                        file.set_extension("syn");
                        let syn = Syn::open(&file, ifo.syn_word_count).ok();
                        return Ok(Dictionary {
                            ifo,
                            idx,
                            dict,
                            syn,
                        });
                    }
                }
            }
        }
        Err(DictError::My(format!(
            "bad dictionary directory {}",
            root.display()
        )))
    }
    /// get the following neighbor words from Idx after `word` from `off`.
    /// if `off` is negative, list from before `-off`.
    pub fn neighbors(&self, word: &[u8], off: i32) -> DictNeighborIter {
        let ret = match self.idx.get(word) {
            Ok(i) => i,
            Err(i) => i,
        } as i32;
        let istart = ret + off;
        let start: usize = if istart < 0 { 0 } else { istart as usize };

        DictNeighborIter {
            cur: start,
            idx: IdxRef::Ref(&self.idx),
        }
    }
    /// get the following neighbor words from Syn after `word` from `off`.
    /// if `off` is negative, list from before `-off`.
    pub fn neighbors_syn(&self, word: &[u8], off: i32) -> DictNeighborIter {
        let mut start: usize = usize::max_value();
        if let Some(s) = &self.syn {
            let ret = match s.get(word) {
                Ok(i) => i,
                Err(i) => i,
            } as i32;
            let istart = ret + off;
            start = if istart < 0 { 0 } else { istart as usize };
        }
        DictNeighborIter {
            cur: start,
            idx: IdxRef::SynRef(&self.syn),
        }
    }

    /// search Idx by regular expression
    pub fn search(&self, expr: &[u8]) -> Result<IdxIter, Error> {
        match str::from_utf8(expr) {
            Ok(e) => {
                let reg = Regex::new(e)?;
                Ok(IdxIter {
                    cur: 0,
                    idx: IdxRef::Ref(&self.idx),
                    matcher: Cow::Owned(reg),
                })
            }
            _ => Err(Error::Syntax(String::from("bad utf8"))),
        }
    }
    /// search Syn by pre-created regular expression object.
    pub fn search_syn<'a>(&'a self, reg: &'a Regex) -> IdxIter {
        IdxIter {
            cur: 0,
            idx: IdxRef::SynRef(&self.syn),
            matcher: Cow::Borrowed(reg),
        }
    }
    /// search Idx by pre-created regular expression object.
    pub fn search_regex<'a>(&'a self, reg: &'a Regex) -> IdxIter {
        IdxIter {
            cur: 0,
            idx: IdxRef::Ref(&self.idx),
            matcher: Cow::Borrowed(reg),
        }
    }
    /// lookup `word` in Dictionary. find from Idx, and also find all matches from Syn.
    pub fn lookup(&mut self, word: &[u8]) -> Result<Vec<LookupResult>, DictError> {
        let mut possible = Vec::with_capacity(4);
        possible.push(self.idx.get(word));

        if let Some(s) = &self.syn {
            if let Ok(i) = s.get(word) {
                possible.push(s.get_offset(i));
                //check neighbors
                let mut c = i - 1;
                //left neighbors
                while let Ok(w) = s.get_word(c) {
                    if Idx::dict_cmp(word, w, true) == Ordering::Equal {
                        possible.push(s.get_offset(c));
                    } else {
                        break;
                    }
                    c -= 1;
                }
                //right neighbors
                c = i + 1;
                while let Ok(w) = s.get_word(c) {
                    if Idx::dict_cmp(word, w, true) == Ordering::Equal {
                        possible.push(s.get_offset(c));
                    } else {
                        break;
                    }
                    c += 1;
                }
            }
        }

        let mut ret = Vec::new();
        for v in possible.iter() {
            if let Ok(i) = v {
                let (eoffset, elength) = self.idx.get_offset_length(*i)?;
                ret.push(LookupResult {
                    dictionary: &self.ifo,
                    word: self.idx.get_word(*i)?,
                    result: self.dict.read(eoffset as u64, elength as usize)?,
                });
            }
        }
        if ret.len() > 0 {
            Ok(ret)
        } else {
            Err(DictError::NotFound(0))
        }
    }
}
impl<'a> Iterator for IdxIter<'a> {
    type Item = &'a [u8];
    fn next(&mut self) -> Option<Self::Item> {
        match &self.idx {
            IdxRef::Ref(r) => {
                while self.cur < r.len() {
                    let v = r.get_word(self.cur);
                    self.cur += 1;
                    if let Ok(e) = v {
                        if self.matcher.is_match(e) {
                            return Some(e);
                        }
                    }
                }
            }
            IdxRef::SynRef(r) => {
                if r.is_none() {
                    return None;
                }
                let s = r.as_ref().unwrap();
                while self.cur < s.len() {
                    let v = s.get_word(self.cur);
                    self.cur += 1;
                    if let Ok(e) = v {
                        if self.matcher.is_match(e) {
                            return Some(e);
                        }
                    }
                }
            }
        }
        None
    }
}
impl<'a> Iterator for DictNeighborIter<'a> {
    type Item = &'a [u8];
    fn next(&mut self) -> Option<Self::Item> {
        match &self.idx {
            IdxRef::Ref(r) => {
                if self.cur < r.len() {
                    let v = r.get_word(self.cur);
                    self.cur += 1;
                    if let Ok(e) = v {
                        return Some(e);
                    }
                }
            }
            IdxRef::SynRef(r) => {
                if r.is_none() {
                    return None;
                }
                let s = r.as_ref().unwrap();
                if self.cur < s.len() {
                    let v = s.get_word(self.cur);
                    self.cur += 1;
                    return v.ok();
                }
            }
        }
        None
    }
}
