use super::result::DictError;
use std::cmp::Ordering;
use std::fs::File;
use std::io::{BufReader, Read};
use std::os::unix::prelude::FileExt;
use std::path;

// total bytes used for offset and length
const OFF_LEN_BYTES: usize = 8;

/// An .idx file representor.
/// file format:
/// 1. utf8-coded string with '\0' ending.
/// 2. offset in dict file u32 or u64(not used for now)
/// 3. length in dict file u32
#[derive(Debug)]
pub struct Idx {
    filedesc: File,  //file descriptor
    index: Vec<u32>, //end of each word

    firstword: Vec<u8>, //first word
    middleword: Vec<u8>,//middle word
    lastword: Vec<u8>,  //last word
                        //cache:
}

enum WordPosition {
    FirstWordPos,
    MiddleWordPos,
    LastWordPos,
    OtherPos
}
enum ParseState {
    Word(WordPosition),
    OffsetLength(u8),
}
struct Parser {
    off_len_bytes_m1: u8,
    state: ParseState,
    count_m1: usize,
    count_half: usize,
    off_word: u32,
    result: Vec<u32>,
    firstw: Vec<u8>,
    middlew: Vec<u8>,
    lastw: Vec<u8>,
}
impl Parser {
    fn parse(&mut self, x: u8) {
        match &self.state {
            ParseState::Word(pos) => {
                if x == 0 {
                    self.result.push(self.off_word);
                    self.state = ParseState::OffsetLength(0);
                } else {
                    match pos {
                        WordPosition::FirstWordPos => self.firstw.push(x),
                        WordPosition::MiddleWordPos => self.middlew.push(x),
                        WordPosition::LastWordPos => self.lastw.push(x),
                        _ => (),
                    }
                }
            }
            ParseState::OffsetLength(n) => {
                self.state = if *n < self.off_len_bytes_m1 {
                    ParseState::OffsetLength(*n + 1)
                } else {
                    if self.result.len() == self.count_m1 {
                        ParseState::Word(WordPosition::LastWordPos)
                    } else if self.result.len() == self.count_half {
                        ParseState::Word(WordPosition::MiddleWordPos)
                    } else {
                        ParseState::Word(WordPosition::OtherPos)
                    }
                };
            }
        }
        self.off_word += 1;
    }
}
impl Idx {
    /// create Idx struct from a .idx file, with `filesize`, word `count` and some other arguments.
    pub fn open(
        file: &path::Path,
        _filesize: usize,
        count: usize,
        off_len_bytes: u8,
    ) -> Result<Idx, DictError> {
        let mut con = Parser {
            off_len_bytes_m1: off_len_bytes - 1,
            state: ParseState::Word(WordPosition::FirstWordPos),
            count_m1: count - 1,
            count_half: count / 2,
            off_word: 0,
            result: Vec::with_capacity(count),
            firstw: Vec::new(),
            middlew: Vec::new(),
            lastw: Vec::new(),
        };
        {
            let idx_file = File::open(file)?;
            let buf_rd = BufReader::new(idx_file);
            buf_rd.bytes().for_each(|x| con.parse(x.unwrap()));
        }

        if count != con.result.len() {
            return Err(DictError::My(format!(
                "not equal! {} != {}",
                count,
                con.result.len()
            )));
        }
        Ok(Idx {
            filedesc: File::open(file)?,
            index: con.result,
            firstword: con.firstw,
            middleword: con.middlew,
            lastword: con.lastw,
        })
    }
    /// return the Idx word count.
    pub fn len(&self) -> usize {
        self.index.len()
    }
    /// return the word of Idx in the specified position.
    /// Err(DictError) if not found.
    pub fn get_word(&self, i: usize) -> Result<Vec<u8>, DictError> {
        //check range first
        if i >= self.index.len() {
            return Err(DictError::NotFound(i));
        }

        if i == 0 {
            return Ok(self.firstword.clone());
        } else if i == self.index.len() - 1 {
            return Ok(self.lastword.clone());
        } else if i == self.index.len() / 2 {
            return Ok(self.middleword.clone());
        }

        // no i==0 case here.
        let start = self.index[i - 1] as usize + OFF_LEN_BYTES + 1;
        let end = self.index[i] as usize;
        //get data of [start, end)
        let mut word_result = vec![0u8; end - start];
        self.filedesc
            .read_exact_at(&mut word_result, start as u64)?;
        Ok(word_result)
    }

    /// return the offset and length in .dict file. by the specified position of Idx.
    pub fn get_offset_length(&self, i: usize) -> Result<(u32, u32), DictError> {
        //check range first
        if i >= self.index.len() {
            return Err(DictError::NotFound(i));
        }

        let start = self.index[i] as usize + 1;
        let mut buff = [0u8; 8];
        self.filedesc.read_exact_at(&mut buff, start as u64)?;
        let offset = u32::from_be_bytes([buff[0], buff[1], buff[2], buff[3]]);
        let length = u32::from_be_bytes([buff[4], buff[5], buff[6], buff[7]]);
        Ok((offset, length))
    }
    /// get the position in the Idx. if not found, return Err(usize).
    /// it will first try to do case-sensitive find, it no result,
    /// try again with case-insensitive find.
    /// the result Err(usize) is used for neighborhood hint.
    pub fn get(&self, word: &[u8]) -> Result<usize, usize> {
        if Idx::dict_cmp(&self.firstword, word, true) == Ordering::Greater {
            return Err(0);
        }
        if Idx::dict_cmp(&self.lastword, word, true) == Ordering::Less {
            return Err(self.index.len());
        }
        self.binary_search(word, false)
            .or_else(|_| self.binary_search(word, true))
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
            let cmp = Idx::dict_cmp(&self.get_word(mid).unwrap(), word, ignore_case);
            base = if cmp == Ordering::Greater { base } else { mid };
            size -= half;
        }
        // base is always in [0, size) because base <= mid.
        let cmp = Idx::dict_cmp(&self.get_word(base).unwrap(), word, ignore_case);
        if cmp == Ordering::Equal {
            Ok(base)
        } else {
            Err(base + (cmp == Ordering::Less) as usize)
        }
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
