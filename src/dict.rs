use std::fs::File;
use std::os::unix::prelude::FileExt;
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
    pub fn read(&self, start: u64, length: usize) -> Result<Vec<u8>, DictError> {
        let mut result = vec![0u8; length];
        self.dictf.read_exact_at(&mut *result, start)?;
        Ok(result)
    }
}
