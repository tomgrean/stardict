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

    pub fn search(&mut self, word: &str) -> Result<Vec<TranslationItem>, DictError> {
        //self.idx.get(word)?;
        Err(DictError::My(format!("not implemented")))
    }
}

#[derive(Debug, Clone)]
pub struct Translation {
    pub info: Ifo,
    pub results: Vec<TranslationItem>,
}

#[derive(Debug, Clone)]
pub struct TranslationItem {
    pub mode: char,
    pub body: String,
}
