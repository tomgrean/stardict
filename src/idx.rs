use std::cmp::Ordering;
use std::fs;
use std::io::Read;
use std::path;
use super::result::DictError;

const OFF_LEN_BYTES: usize = 8;
/// An .idx file representor.
/// file format:
/// 1.utf8-coded string with '\0' ending.
/// 2.offset in dict file u32 or u64(not used for now)
/// 3.length in dict file u32
#[derive(Debug)]
pub struct Idx {
    content: Vec<u8>,//file content
    index: Vec<u32>,//end of each word
    //off_len_bytes: u32,
}

enum ParseState {
    Word,
    OffsetLength(u8),
}
struct Parser {
    off_len_bytes_m1: u8,
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
                self.state = if n < self.off_len_bytes_m1 {
                    ParseState::OffsetLength(n + 1)
                } else {
                    ParseState::Word
                };
            }
        }
        self.off_word += 1;
    }
}
impl Idx {
    /// create Idx struct from a .idx file, with `filesize`, word `count` and some other arguments.
    pub fn open(file: &path::Path, filesize: usize, count: usize, off_len_bytes: u8) -> Result<Idx, DictError> {
        let mut file_con: Vec<u8>;
        {
            let mut idx_file = fs::File::open(file)?;
            file_con = Vec::with_capacity(filesize + 1);//read to end may realloc...
            idx_file.read_to_end(&mut file_con)?;
        }
        let mut con = Parser {
            off_len_bytes_m1: off_len_bytes - 1,
            state: ParseState::Word,
            off_word: 0,
            result: Vec::with_capacity(count),
        };
        file_con.iter().for_each(|x| con.parse(*x));

        if count != con.result.len() {
            return Err(DictError::My(format!("not equal! {} != {}", count, con.result.len())));
        }
        //println!("content: {} {}, {}", file_con.len(), file_con.capacity(), filesize);
        Ok(Idx { content:file_con, index: con.result })
    }
    /// return the Idx word count.
    pub fn len(&self) -> usize {
        self.index.len()
    }
    /// retuen the word of Idx in the specified position.
    /// Err(DictError) if not found.
    pub fn get_word(&self, i: usize) -> Result<&[u8], DictError> {
        //check range first
        if i >= self.index.len() {
            return Err(DictError::NotFound(i));
        }

        let start = if i == 0 { 0usize } else { self.index[i - 1] as usize + OFF_LEN_BYTES + 1 };
        let end = self.index[i] as usize;
        Ok(&self.content[start..end])
    }

    /// return the offset and length in .dict file. by the specified position of Idx.
    pub fn get_offset_length(&self, i: usize) -> Result<(u32, u32), DictError> {
        //check range first
        if i >= self.index.len() {
            return Err(DictError::NotFound(i));
        }

        let mut start = self.index[i] as usize + 1;
        let mut buff = [0u8; 4];
        buff.copy_from_slice(&self.content[start..start + 4]);
        let offset = u32::from_be_bytes(buff);
        start += 4;
        buff.copy_from_slice(&self.content[start..start + 4]);
        let length = u32::from_be_bytes(buff);
        Ok((offset, length))
    }
    /// get the position in the Idx. if not found, return Err(usize).
    /// it will first try to do case-sensitive find, it no result,
    /// try again with case-insensitive find.
    /// the result Err(usize) is used for neighborhood hint.
    pub fn get(&self, word: &[u8]) -> Result<usize, usize> {
        if Idx::dict_cmp(self.get_word(0).unwrap(), word, true) == Ordering::Greater {
            return Err(0);
        }
        if Idx::dict_cmp(self.get_word(self.index.len() - 1).unwrap(), word, true) == Ordering::Less {
            return Err(self.index.len());
        }
        self.binary_search(word, false).or_else(|_|self.binary_search(word, true))
    }
    fn binary_search(&self, word: &[u8], ignore_case: bool) -> Result<usize, usize> {
        let mut size = self.index.len();
        let mut base = 0usize;
        while size > 1 {
            let half = size / 2;
            let mid = base + half;
            // mid is always in [0, size), that means mid is >= 0 and < size.
            // mid >= 0: by definition
            // mid < size: mid = size / 2 + size / 4 + size / 8 ...
            let cmp = Idx::dict_cmp(self.get_word(mid).unwrap(), word, ignore_case);
            base = if cmp == Ordering::Greater { base } else { mid };
            size -= half;
        }
        // base is always in [0, size) because base <= mid.
        let cmp = Idx::dict_cmp(self.get_word(base).unwrap(), word, ignore_case);
        if cmp == Ordering::Equal { Ok(base) } else { Err(base + (cmp == Ordering::Less) as usize) }
    }
    pub fn dict_cmp(w1: &[u8], w2: &[u8], ignore_case: bool) -> Ordering {
        let w1len = w1.len();
        let w2len = w2.len();

        if w1len == 0 || w2len == 0 {
            return if w1len > 0 {
                Ordering::Greater
            } else if w2len > 0 {
                Ordering::Less
            } else {
                Ordering::Equal
            };
        }
        #[inline]
        fn i32_to_order(x: i32) -> Ordering {
            if x > 0 {
                Ordering::Greater
            } else if x < 0 {
                Ordering::Less
            } else {
                Ordering::Equal
            }
        }
        let mut case_eq: i32 = 0;
        let mut ci2 = w2.iter();
        for c1 in w1.iter() {
            let c2: u8;
            match ci2.next() {
                None => return Ordering::Greater,
                Some(c) => c2 = *c,
            }
            let l2 = c2.to_ascii_lowercase();
            let l1 = c1.to_ascii_lowercase();
            if l1 > l2 {
                return Ordering::Greater;
            } else if l1 < l2 {
                return Ordering::Less;
            }
            if case_eq == 0 {
                case_eq = *c1 as i32 - c2 as i32;
                // do NOT return early.
            }
        }
        if w1len > w2len {
            Ordering::Greater
        } else if w1len < w2len {
            Ordering::Less
        } else if ignore_case {
            Ordering::Equal
        } else {
            i32_to_order(case_eq)
        }
    }
}

