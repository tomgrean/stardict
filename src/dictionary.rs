use std::path;

use super::dict::Dict;
use super::idx::Idx;
use super::ifo::Ifo;
use super::result::DictError;

pub struct Dictionary {
    pub idx: Idx,
    pub ifo: Ifo,
    pub dict: Dict,
}

impl Dictionary {
    pub fn new(root: path::PathBuf) -> Result<Dictionary, DictError> {
        if let Some(name) = root.file_name() {
            if let Some(name) = name.to_str() {
                return Ok(Dictionary {
                    idx: Idx::open(root.join(format!("{}.idx", name)), false)?,
                    ifo: Ifo::open(root.join(format!("{}.ifo", name)))?,
                    dict: Dict::open(root.join(format!("{}.dict", name)))?,
                });
            }
        }
        Err(DictError::My(format!(
            "bad dictionary directory {}",
            root.display()
        )))
    }

    pub fn neighbors(&self, word: &str, off: i32, length: usize) -> Vec<&str> {
        let ret = match self.idx.get(word) {
            Ok(i) => i,
            Err(i) => i,
        } as i32;
        let start = (ret + off) as usize;

        let mut end = start + length;
        if end > self.idx.len() {
            end = self.idx.len();
        }
        //(start..end).map(|x| self.idx.list[x].word.as_str()).collect()
        let mut ret: Vec<&str> = Vec::with_capacity(length);
        for x in start..end {
            ret.push(&self.idx.list[x].word);
        }
        ret
    }

    pub fn lookup(&mut self, word: &str) -> Result<Vec<u8>, DictError> {
        match self.idx.get(word) {
            Ok(i) => {
                let e = &(self.idx.list[i]);
                self.dict.read(e.offset, e.length as usize)
            },
            _ => Err(DictError::My(format!("not found"))),
        }
    }
}

