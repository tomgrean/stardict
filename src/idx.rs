extern crate regex;

use std::cmp::Ordering;
use std::fs;
use std::io::Read;
use std::path;
use self::regex::Error;
use self::regex::Regex;

use super::result::DictError;

#[derive(Debug)]
pub struct IdxData {
    pub word: String, //utf-8 string, len() < 256
    pub offset: u32,  //32 or 64 for data bits, indicated from .ifo file idxoffsetbits=64.
    pub length: u32,  //u32 for data size.
}
#[derive(Debug)]
pub struct Idx {
    pub list: Vec<IdxData>,
}

pub struct IdxIter<'a> {
    cur: usize,
    data: &'a Idx,
    matcher: Regex,
}
#[derive(Debug)]
enum ParseState {
    Word,
    Offset(u8),
    Length(u8),
}
#[derive(Debug)]
struct Parser {
    offset: u32,
    length: u32,
    off_is_u64: bool,
    word: Vec<u8>,
    state: ParseState,
    result: Vec<IdxData>,
}
impl Parser {
    fn parse(&mut self, x: u8) {
        match self.state {
            ParseState::Word => {
                if x == 0 {
                    self.state = ParseState::Offset(0);
                } else {
                    self.word.push(x);
                }
            }
            ParseState::Offset(n) => {
                self.offset = (self.offset << 8) | (x as u32);
                let ck = if self.off_is_u64 { 7 } else { 3 };
                self.state = if n < ck {
                    ParseState::Offset(n + 1)
                } else {
                    ParseState::Length(0)
                };
            }
            ParseState::Length(n) => {
                self.length = (self.length << 8) | (x as u32);
                self.state = if n < 3 {
                    ParseState::Length(n + 1)
                } else {
                    self.result.push(IdxData {
                        word: String::from_utf8(self.word.clone()).unwrap(),
                        offset: self.offset,
                        length: self.length,
                    });
                    self.offset = 0;
                    self.length = 0;
                    self.word.clear();
                    ParseState::Word
                };
            }
        }
    }
}
impl Idx {
    pub fn open(file: &path::Path, count: usize, off_is_u64: bool) -> Result<Idx, DictError> {
        let mut file_con: Vec<u8>;
        {
            let mut idx_file = fs::File::open(file)?;
            file_con = Vec::with_capacity(idx_file.metadata()?.len() as usize);
            idx_file.read_to_end(&mut file_con)?;
        }
        let mut con = Parser {
            offset: 0,
            length: 0u32,
            off_is_u64,
            word: Vec::with_capacity(256),
            state: ParseState::Word,
            result: Vec::with_capacity(count),
            //result: Vec::new(),
        };
        file_con.iter().for_each(|x| con.parse(*x));
        //con.result.iter().for_each(|x| println!("word = {}",x.word));
        if count != con.result.len() {
            println!("warn!not equal! {} != {}", count, con.result.len());
        }
        Ok(Idx { list: con.result })
    }
    // the result Err(usize) is used for neighborhood hint.
    pub fn get(&self, word: &str) -> Result<usize, usize> {
        match self.list.binary_search_by(|e| Idx::dict_cmp(&(e.word), word, false)) {
            Err(_) => self.list.binary_search_by(|e| Idx::dict_cmp(&(e.word), word, true)),
            x => x,
        }
    }
    // search by regular expression
    pub fn search(&self, fuzzy: &str) -> Result<IdxIter, Error> {
        let reg = Regex::new(fuzzy)?;
        Ok(IdxIter {cur:0, data:self, matcher:reg})
    }
    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn dict_cmp(w1: &str, w2: &str, ignore_case: bool) -> Ordering {
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
        let mut case_eq: i32 = 0;
        let mut ci2 = w2.chars();
        for c1 in w1.chars() {
            let c22 = ci2.next();
            let c2: char;
            match c22 {
                None => return Ordering::Greater,
                Some(c) => c2 = c,
            }
            let l2 = c2.to_ascii_lowercase();
            let l1 = c1.to_ascii_lowercase();
            if l1 > l2 {
                return Ordering::Greater;
            } else if l1 < l2 {
                return Ordering::Less;
            }
            if case_eq == 0 {
                case_eq = c1 as i32 - c2 as i32;
            }
        }
        if w1len > w2len {
            Ordering::Greater
        } else if w1len < w2len {
            Ordering::Less
        } else if ignore_case {
            Ordering::Equal
        } else if case_eq > 0 {
            Ordering::Greater
        } else if case_eq < 0 {
            Ordering::Less
        } else {
            Ordering::Equal
        }
    }
}
impl<'a> Iterator for IdxIter<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        self.cur += 1;
        if self.cur < self.data.list.len() {
            for v in &(self.data.list[self.cur..]) {
                if self.matcher.is_match(v.word.as_str()) {
                    return Some(v.word.as_str());
                }
                self.cur += 1;
            }
        }
        None
    }
}
