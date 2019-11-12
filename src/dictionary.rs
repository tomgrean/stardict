use std::fs;
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
    pub fn new(root: &path::Path) -> Result<Dictionary, DictError> {
        for it in fs::read_dir(root)? {
            let it = it?.path();
            if it.is_file() {
                if let Some(ext) = it.extension() {
                    if let Some("ifo") = ext.to_str() {
                        let ifo = Ifo::open(&it)?;
                        let mut file = it.to_path_buf();
                        file.set_extension("idx");
                        let idx = Idx::open(&file, ifo.idxoffsetbits == 64)?;
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

    pub fn neighbors(&self, word: &str, off: i32, length: usize) -> Option<Vec<&str>> {
        let ret = match self.idx.get(word) {
            Ok(i) => i,
            Err(i) => i,
        } as i32;
        let start = (ret + off) as usize;

        if start < self.idx.list.len() {
            Some(
                self.idx.list[start..]
                    .iter()
                    .take(length)
                    .map(|x| x.word.as_str())
                    .collect(),
            )
        } else {
            None
        }
    }

    pub fn lookup(&mut self, word: &str) -> Result<Vec<u8>, DictError> {
        match self.idx.get(word) {
            Ok(i) => {
                let e = &(self.idx.list[i]);
                self.dict.read(e.offset, e.length as usize)
            }
            _ => Err(DictError::My(format!("not found"))),
        }
    }
}
