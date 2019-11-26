use std::cmp::Ordering;
use std::fs;
use std::io::Read;
use std::path;
use super::result::DictError;
use super::idx::Idx;

const OFF_BYTES : usize = 4;
#[derive(Debug)]
pub struct Syn {
    content: Vec<u8>,//file content
    index: Vec<u32>,//end of each word
    //off_len_bytes: u32,
}

enum ParseState {
    Word,
    OffsetLength(u8),
}
struct Parser {
    state: ParseState,

    off_word: u32,
    result: Vec<u32>,
}
impl Parser {
    fn parse(&mut self, x: u8) {
        match self.state {
            ParseState::Word => {
                if x == 0 {
                    self.result.push(self.off_word);
                    self.state = ParseState::OffsetLength(0);
                }
            }
            ParseState::OffsetLength(n) => {
                self.state = if n < 3 {
                    ParseState::OffsetLength(n + 1)
                } else {
                    ParseState::Word
                };
            }
        }
        self.off_word += 1;
    }
}
impl Syn {
    pub fn open(file: &path::Path, count: usize) -> Result<Syn, DictError> {
        let mut file_con: Vec<u8>;
        {
            let mut syn_file = fs::File::open(file)?;
            let file_len = syn_file.metadata()?.len();
            file_con = Vec::with_capacity(file_len as usize + 1);//read to end may realloc...
            syn_file.read_to_end(&mut file_con)?;
        }
        let mut con = Parser {
            state: ParseState::Word,
            off_word: 0,
            result: Vec::with_capacity(count),
        };
        file_con.iter().for_each(|x| con.parse(*x));

        if count != con.result.len() {
            return Err(DictError::My(format!("not equal! {} != {}", count, con.result.len())));
        }
        //println!("content: {} {}, {}", file_con.len(), file_con.capacity(), filesize);
        Ok(Syn { content:file_con, index: con.result })
    }
    pub fn len(&self) -> usize {
        self.index.len()
    }
    pub fn get_word(&self, i: usize) -> Result<&[u8], DictError> {
        //check range first
        if i >= self.index.len() {
            return Err(DictError::NotFound(i));
        }

        let start = if i == 0 { 0usize } else { self.index[i - 1] as usize + OFF_BYTES + 1 };
        let end = self.index[i] as usize;
        Ok(&self.content[start..end])
    }

    pub fn get_offset(&self, i: usize) -> Result<usize, usize> {
        //check range first
        if i >= self.index.len() {
            return Err(i);
        }

        let start = self.index[i] as usize + 1;
        let mut buff = [0u8; 4];
        buff.copy_from_slice(&self.content[start..start + 4]);
        let offset = u32::from_be_bytes(buff);
        Ok(offset as usize)
    }
    // the result Err(usize) is used for neighborhood hint.
    pub fn get(&self, word: &[u8]) -> Result<usize, usize> {
        if Idx::dict_cmp(self.get_word(0).unwrap(), word, true) == Ordering::Greater {
            return Err(0);
        }
        if Idx::dict_cmp(self.get_word(self.index.len() - 1).unwrap(), word, true) == Ordering::Less {
            return Err(self.index.len());
        }

        let mut size = self.index.len();
        let mut base = 0usize;
        while size > 1 {
            let half = size / 2;
            let mid = base + half;
            // mid is always in [0, size), that means mid is >= 0 and < size.
            // mid >= 0: by definition
            // mid < size: mid = size / 2 + size / 4 + size / 8 ...
            let cmp = Idx::dict_cmp(self.get_word(mid).unwrap(), word, true);
            base = if cmp == Ordering::Greater { base } else { mid };
            size -= half;
        }
        // base is always in [0, size) because base <= mid.
        let cmp = Idx::dict_cmp(self.get_word(base).unwrap(), word, true);
        if cmp == Ordering::Equal { Ok(base) } else { Err(base + (cmp == Ordering::Less) as usize) }
    }
}

