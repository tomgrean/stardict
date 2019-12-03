use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path;

use super::result::DictError;

/// the .dict file reader.
pub struct Dict {
    dictf: File,
}

impl Dict {
    /// test and open the .dict file.
    pub fn open(file: &path::Path) -> Result<Dict, DictError> {
        let f = File::open(file)?;
        Ok(Dict { dictf: f })
    }
    /// read `length` from `start`
    pub fn read(&mut self, start: u64, length: usize) -> Result<Vec<u8>, DictError> {
        self.dictf.seek(SeekFrom::Start(start))?;
        let mut result = vec![0u8; length];
        self.dictf.read_exact(&mut *result)?;
        Ok(result)
    }
}
