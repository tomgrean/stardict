use super::idx::Idx;
use super::result::DictError;
use std::cmp::Ordering;
use std::fs::File;
use std::io::{BufReader, Read};
use std::os::unix::prelude::FileExt;
use std::path;

// the bytes used for offset
const OFF_BYTES: usize = 4;

/// Syn corresponds to .syn file. It can be reguarded as a patch to .idx file.
/// As the document says, the .syn file may contain more than one same
/// word entry to different index of .idx, so do not suprise when `get()`
/// method does not return the same value as you've expected.
#[derive(Debug)]
pub struct Syn {
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
                self.state = if *n < 3 {
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
impl Syn {
    ///create Syn struct from file. with count as synword count from .ifo
    ///file. if the count is not correct, return Err(DictError).
    pub fn open(file: &path::Path, count: usize) -> Result<Syn, DictError> {
        let mut con = Parser {
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
            let syn_file = File::open(file)?;
            let buf_rd = BufReader::new(syn_file);
            buf_rd.bytes().for_each(|x| con.parse(x.unwrap()));
        }

        if count != con.result.len() {
            return Err(DictError::My(format!(
                "not equal! {} != {}",
                count,
                con.result.len()
            )));
        }
        Ok(Syn {
            filedesc: File::open(file)?,
            index: con.result,
            firstword: con.firstw,
            middleword: con.middlew,
            lastword: con.lastw,
        })
    }
    /// return syn word count.
    pub fn len(&self) -> usize {
        self.index.len()
    }
    /// return the word in the exact posision. Err(DictError) if not found.
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
        let start = self.index[i - 1] as usize + OFF_BYTES + 1;
        let end = self.index[i] as usize;
        //get data of [start, end)
        let mut word_result = vec![0u8; end - start];
        self.filedesc
            .read_exact_at(&mut word_result, start as u64)?;
        Ok(word_result)
    }
    /// return the index of Idx in the exact position. Err(i) if not found.
    pub fn get_offset(&self, i: usize) -> Result<usize, usize> {
        //check range first
        if i >= self.index.len() {
            return Err(i);
        }

        let start = self.index[i] as usize + 1;
        let mut buff = [0u8; 4];
        if let Ok(()) = self.filedesc.read_exact_at(&mut buff, start as u64) {
            let offset = u32::from_be_bytes(buff);
            Ok(offset as usize)
        } else {
            Err(i)
        }
    }
    /// search the word in Syn case-insensitively. return the index if found,
    /// return Err(usize) if not found. The Err result is used for
    /// neighborhood hint.
    pub fn get(&self, word: &[u8]) -> Result<usize, usize> {
        if Idx::dict_cmp(&self.get_word(0).unwrap(), word, true) == Ordering::Greater {
            return Err(0);
        }
        if Idx::dict_cmp(&self.get_word(self.index.len() - 1).unwrap(), word, true)
            == Ordering::Less
        {
            return Err(self.index.len());
        }

        //do not need 2 search like idx. as all neighbors will be collected by lookup().
        let mut size = self.index.len();
        let mut base = 0usize;
        while size > 1 {
            let half = size / 2;
            let mid = base + half;
            // mid is always in [0, size), that means mid is >= 0 and < size.
            // mid >= 0: by definition
            // mid < size: mid = size / 2 + size / 4 + size / 8 ...
            let cmp = Idx::dict_cmp(&self.get_word(mid).unwrap(), word, true);
            base = if cmp == Ordering::Greater { base } else { mid };
            size -= half;
        }
        // base is always in [0, size) because base <= mid.
        let cmp = Idx::dict_cmp(&self.get_word(base).unwrap(), word, true);
        if cmp == Ordering::Equal {
            Ok(base)
        } else {
            Err(base + (cmp == Ordering::Less) as usize)
        }
    }
}
