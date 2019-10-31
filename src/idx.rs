use std::path;
use std::fs;
use std::str;
use std::io::Read;

use super::result::DictError;

#[derive(Debug)]
pub struct IdxData {
    pub word: String,   //utf-8 string, len() < 256
    pub offset: u64, //32 or 64 for data bits, indicated from .ifo file idxoffsetbits=64.
    pub length: u32,   //u32 for data size.
}
#[derive(Debug)]
pub struct Idx {
    pub list: Vec<IdxData>
}

#[derive(Debug)]
enum ParseState {
    Word,
    Offset(u8),
    Length(u8),
}
#[derive(Debug)]
struct Parser {
    offset: u64,
    length: u32,
    off_is_u64: bool,
    word: Vec<u8>,
    state: ParseState,
    result: Vec<IdxData>,
}
impl Parser {
    fn parse(&mut self, x: u8) -> &mut Self {
        match self.state {
            ParseState::Word => {
                if x == 0 {
                    self.state = ParseState::Offset(0);
                } else {
                    self.word.push(x);
                }
            },
            ParseState::Offset(n) => {
                self.offset = (self.offset << 8) | (x as u64);
                let ck = if self.off_is_u64 { 7 } else { 3 };
                self.state = if n < ck {ParseState::Offset(n + 1)} else {ParseState::Length(0)};
            },
            ParseState::Length(n) => {
                self.length = (self.length << 8) | (x as u32);
                self.state = if n < 3 {ParseState::Length(n + 1)} else {
                    self.result.push(IdxData{
                        word: str::from_utf8(&self.word).unwrap().to_string(),
                        offset: self.offset,
                        length: self.length,
                    });
                    self.offset = 0;
                    self.length = 0;
                    self.word.clear();
                    ParseState::Word
                };
            },
        }
        self
    }
}
impl Idx {
    pub fn open(file: path::PathBuf, off_is_u64: bool) -> Result<Idx, DictError> {
        let mut file_con: Vec<u8> = Vec::new();
        {
            fs::File::open(file)?.read_to_end(&mut file_con)?;
        }
        let mut con = Parser {
            offset: 0u64,
            length: 0u32,
            off_is_u64,
            word: Vec::with_capacity(256),
            state: ParseState::Word,
            result: Vec::new(),
        };
        file_con.iter().fold(&mut con, |acc, x| acc.parse(*x));
        println!("the second one = {:?}", con.result[0]);
        Ok(Idx {list: con.result})
    }
    //pub fn get(word: &str) -> Result<&str, DictError> {
    //}
    pub fn len(&self) -> usize {
        self.list.len()
    }
}
