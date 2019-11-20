extern crate regex;

pub mod dict;
pub mod dictionary;
pub mod idx;
pub mod ifo;
pub mod result;
pub mod reformat;
//pub mod web;

use std::{env, fs, path, str};
use std::iter::{Iterator};
use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;
use std::cmp::Ordering;
use self::regex::bytes::Regex;
//use self::regex::Error;

pub struct StarDict {
    directories: Vec<dictionary::Dictionary>,
}
pub struct LookupResult<'a> {
    dictionary: &'a ifo::Ifo,
    result: Vec<u8>,
}

pub struct WordMergeIter<'a, T: Iterator<Item=&'a [u8]>> {
    wordit: Vec<T>,
    cur: Vec<Option<&'a [u8]>>,
}
impl<'a, T: Iterator<Item=&'a [u8]>> Iterator for WordMergeIter<'a, T> {
    type Item = &'a [u8];
    fn next(&mut self) -> Option<Self::Item> {
        let l = self.cur.len();

        let mut x = 0usize;
        let mut i = 1usize;
        while i < l {
            x = match (self.cur[x], self.cur[i]) {
                (None, _) => i,
                (_, None) => x,
                (Some(a), Some(b)) => {
                    match idx::Idx::dict_cmp(a, b, false) {
                        Ordering::Greater => i,
                        Ordering::Equal => {
                            self.cur[i] = self.wordit[i].next();
                            x
                        },
                        _ => x,
                    }
                },
            };
            i += 1;
        }
        let ret = self.cur[x];
        self.cur[x] = self.wordit[x].next();
        ret
    }
}

impl StarDict {
    pub fn new(root: &path::Path) -> Result<StarDict, result::DictError> {
        let mut sort_dirs = Vec::new();
        let mut items = Vec::new();

        if root.is_dir() {
            for it in fs::read_dir(root)? {
                //println!("push direc: {:?}", it);
                let it = it?.path();
                if it.is_dir() {
                    sort_dirs.push(it.into_boxed_path());
                }
            }
        }

        sort_dirs.sort();
        for it in sort_dirs.iter() {
            match dictionary::Dictionary::new(&**it) {
                Ok(d) => {
                    items.push(d);
                }
                Err(e) => {
                    eprintln!("ignore reason: {:?}", e);
                }
            }
        }
        Ok(StarDict { directories: items })
    }
    pub fn info(&self) -> Vec<&ifo::Ifo> {
        let mut items = Vec::with_capacity(self.directories.len());
        for it in &self.directories {
            items.push(&it.ifo);
        }
        items
    }
    pub fn neighbors(&self, word: &[u8], off: i32) -> WordMergeIter<dictionary::DictNeighborIter> {
        let mut wordit = Vec::with_capacity(self.directories.len());
        let mut cur = Vec::with_capacity(self.directories.len());
        for d in self.directories.iter() {
            let mut x = d.neighbors(word, off);
            cur.push(x.next());
            wordit.push(x);
        }

        WordMergeIter { wordit, cur }
    }
    pub fn search<'a>(&'a self, reg: &'a Regex) -> WordMergeIter<'a, dictionary::IdxIter> {
        let mut wordit = Vec::with_capacity(self.directories.len());
        let mut cur = Vec::with_capacity(self.directories.len());
        for d in self.directories.iter() {
            println!("in for {}", d.ifo.name.as_str());
            let mut x = d.search_regex(reg);
            println!("created inner iter");
            cur.push(x.next());
            println!("created 1st value");
            wordit.push(x);
        }

        WordMergeIter { wordit, cur }
    }
    pub fn lookup(&mut self, word: &[u8]) -> Result<Vec<LookupResult>, result::DictError> {
        let mut ret: Vec<LookupResult> = Vec::new();
        for d in self.directories.iter_mut() {
            match d.lookup(word) {
                Ok(result) => ret.push(LookupResult {
                    dictionary: &d.ifo,
                    result,
                }),
                //Err(x) => println!("dict {} look err:{:?}", d.ifo.name, x),
                _ => (),
            }
        }
        Ok(ret)
    }
}
struct StardictUrl {
    path: [u8;4],
    word: Vec<u8>,
}
impl StardictUrl {
    fn new() -> StardictUrl {
        StardictUrl {
            path: [0u8;4],
            word: Vec::with_capacity(16),
        }
    }
    fn byte_to_u8(b: u8) -> u8 {
        match b {
            b'0' ..= b'9' => b - b'0',
            b'A' ..= b'F' => b + 10 - b'A',
            b'a' ..= b'f' => b + 10 - b'a',
            _ => b,
        }
    }
    fn add_path(&mut self, c: u8, idx: usize) {
        if idx < self.path.len() {
            self.path[idx] = c;
        }
    }
    fn add_byte(&mut self, c: u8) {
        self.word.push(c);
    }
}
fn main() {
    for arg in env::args().skip(1) {
        //parse options.
        println!("cmd args: {}", &arg);
    }

    let mut dict = StarDict::new(&path::PathBuf::from("/usr/share/stardict/dic")).unwrap();
    println!("dict size={}", dict.directories.len());
    for d in dict.info().iter() {
        println!("dict: wordcount:{} {}", d.word_count, d.name);
    }
    //webs
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    //let pool = web::ThreadPool::new(4);

    let cr = reformat::ContentReformat::load_from_file(&path::PathBuf::from("/usr/share/stardict/dic/format.conf"));

    for stream in listener.incoming()/*.take(1)*/ {
        let stream = stream.unwrap();

        //pool.execute(
            handle_connection(stream, &mut dict, &cr);
        //);
    }

    println!("Shutting down.");
}
fn handle_connection(mut stream: TcpStream, dict: &mut StarDict, cr: &reformat::ContentReformat) {
    let mut buffer = [0u8; 512];
    stream.read(&mut buffer).unwrap();

    let get = b"GET /";

    //("HTTP/1.0 200 OK\r\nConnection: close\r\n", "index.html");
    let mut content:Vec<u8> = Vec::new();
    let mut surl = StardictUrl::new();

    if buffer.starts_with(get) {
        let mut state = 0i16;//>=0 path, -1 w, -2 p0w, -3 p1w
        let mut w = 0u8;
        buffer[5..].iter().take_while(|c| **c != b' ').for_each(|c|{
            if state < 0 {
                if *c == b'%' {
                    state = -2;
                } else {
                    if state == -2 {
                        w = StardictUrl::byte_to_u8(*c) << 4;
                        state = -3;
                    } else if state == -3 {
                        w |= StardictUrl::byte_to_u8(*c);
                        surl.add_byte(w);
                        state = -1;
                    } else {
                        surl.add_byte(*c);
                    }
                }
            } else if *c == b'/' {
                state = -1;
            } else {
                surl.add_path(*c, state as usize);
                state += 1;
            }
        });

        println!("get from url path={}, word={}", str::from_utf8(&surl.path[..]).unwrap(), str::from_utf8(&surl.word).unwrap());
        //let contents = fs::read_to_string(filename).unwrap();
        //let response = format!("{}\r\n{}", status_line, contents);
    //let content = cr.replace_all(&content, );
        if surl.word.len() > 0 {
            if surl.path[0] == b'w' {//word lookup
                match dict.lookup(&surl.word) {
                    Ok(x) => x.iter().for_each(|e| {
                        content.extend(b"<li>");
                        content.extend(e.dictionary.name.as_bytes());
                        content.extend(b"</li>");
                        for (a, b) in e.dictionary.same_type_sequence.as_bytes().iter().zip(e.result.split(|c| *c == 0)) {
                            content.extend(&cr.replace_all(*a, b));
                        }
                    }),
                    Err(e) => println!("err: {:?}", e),
                }
            } else if surl.path[0] == b'n' {//neighbor words reference
                for s in dict.neighbors(&surl.word, 0).take(10) {
                    content.extend(s);
                    content.extend(b"\n");
                }
            } else if surl.path[0] == b's' {//search with regex
                match str::from_utf8(&surl.word) {
                    Ok(x) => {
                        match Regex::new(x) {
                            Ok(v) => dict.search(&v).for_each(|e| {
                                content.extend(e);
                                content.extend(b"\n");
                            }),
                            Err(e) => println!("err: {:?}", e),
                        }
                    },
                    Err(e) => println!("err: {:?}", e),
                }
            }
        }
    }

    if content.len() > 0 {
        stream.write(b"HTTP/1.0 200 OK\r\nContent-Type: ").unwrap();
        if surl.path[0] == b'w' {
            stream.write(b"text/html").unwrap();
        } else {
            stream.write(b"text/plain").unwrap();
        }
        stream.write(b"\r\nContent-Length: ").unwrap();
        stream.write(content.len().to_string().as_bytes()).unwrap();
        stream.write(b"\r\nConnection: close\r\n\r\n").unwrap();
        stream.write(&content).unwrap();
        stream.flush().unwrap();
    } else {
        stream.write(b"HTTP/1.0 404 NOT FOUND\r\n\r\nnot found").unwrap();
        stream.flush().unwrap();
    }
}
