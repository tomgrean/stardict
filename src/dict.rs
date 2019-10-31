use std::path;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use super::result::DictError;

pub struct Dict {
    dictf: File,
}

impl Dict {
    pub fn open(file: path::PathBuf) -> Result<Dict, DictError> {
        let f = File::open(file)?;
        Ok(Dict{dictf:f })
    }
    pub fn read(&mut self, start: u64, length: usize) -> Result<Vec<u8>, DictError> {
        self.dictf.seek(SeekFrom::Start(start))?;
        let mut result = vec!(0u8; length);
        self.dictf.read_exact(&mut *result)?;
        Ok(result)
    }
}
