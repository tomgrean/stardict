pub mod dict;
pub mod dictionary;
pub mod idx;
pub mod ifo;
pub mod result;
//pub mod web;

use std::{env, fs, path, str};
use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;
use std::cmp::Ordering;

pub struct StarDict {
    directories: Vec<dictionary::Dictionary>,
}
pub struct LookupResult<'a> {
    dictionary: &'a str,
    result: Vec<u8>,
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
    fn merger(e: &str, ret: &Vec<&str>, ridx: &mut usize) -> Ordering {
        if ret.len() <= *ridx {
            return Ordering::Greater;
        }
        match idx::Idx::dict_cmp(e, ret[*ridx], false) {
            Ordering::Greater => {
                *ridx += 1;
                StarDict::merger(e, ret, ridx)
            },
            x => x,
        }
    }
    pub fn neighbors(&self, word: &str, off: i32, length: usize) -> Vec<&str> {
        let mut ret: Vec<&str> = Vec::new();
        for d in self.directories.iter() {
            if let Some(n) = d.neighbors(word, off, length) {
                if ret.len() > 0 {
                    let mut ridx = 0usize;
                    for e in n.iter() {
                        match StarDict::merger(e, &mut ret, &mut ridx) {
                            Ordering::Less => ret.insert(ridx, e),
                            Ordering::Greater => ret.push(e),
                            _ => (),
                        }
                    }
                } else if n.len() > 0 {
                    ret.extend(n);
                }
            }
        }
        ret.truncate(length);
        ret
    }
    pub fn search(&self, fuzzy: &str, length: usize) -> Vec<&str> {
        let mut ret: Vec<&str> = Vec::new();
        for d in self.directories.iter() {
            if let Ok(n) = d.idx.search(fuzzy) {
                if ret.len() > 0 {
                    let mut ridx = 0usize;
                    for e in n {
                        match StarDict::merger(e, &mut ret, &mut ridx) {
                            Ordering::Less => ret.insert(ridx, e),
                            Ordering::Greater => ret.push(e),
                            _ => (),
                        }
                        if ret.len() >= length {
                            break;
                        }
                    }
                } else {
                    ret.extend(n);
                }
                if ret.len() >= length {
                    break;
                }
            }
        }
        ret
    }
    pub fn lookup(&mut self, word: &str) -> Result<Vec<LookupResult>, result::DictError> {
        let mut ret: Vec<LookupResult> = Vec::new();
        for d in self.directories.iter_mut() {
            match d.lookup(word) {
                Ok(result) => ret.push(LookupResult {
                    dictionary: d.ifo.name.as_str(),
                    result,
                }),
                Err(x) => println!("dict {} look err:{:?}", d.ifo.name, x),
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
    //let mut dict = StarDict::new(&path::PathBuf::from("/media/blank/code/stardict")).unwrap();
    println!("dict size={}", dict.directories.len());
    for d in dict.info().iter() {
        println!("dict: wordcount:{} {}", d.word_count, d.name);
    }
    //webs
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    //let pool = web::ThreadPool::new(4);

    for stream in listener.incoming()/*.take(1)*/ {
        let stream = stream.unwrap();

        //pool.execute(
            handle_connection(stream, &mut dict);
        //);
    }

    println!("Shutting down.");
}
fn handle_connection(mut stream: TcpStream, dict: &mut StarDict) {
    let mut buffer = [0u8; 512];
    stream.read(&mut buffer).unwrap();

    let get = b"GET /";

    //("HTTP/1.0 200 OK\r\nConnection: close\r\n", "index.html");
    let mut content:Vec<u8> = Vec::new();

    if buffer.starts_with(get) {
        let mut surl = StardictUrl::new();
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
        let word = match str::from_utf8(&surl.word) {
            Ok(w) => w,
            _ => "",
        };
        if word.len() > 0 {
            if surl.path[0] == b'w' {//word lookup
                match dict.lookup(word) {
                    Ok(x) => x.iter().for_each(|e| {
                        content.extend(b"<li>");
                        content.extend(e.dictionary.as_bytes());
                        content.extend(b"</li>");
                        content.extend(&e.result);
                    }),
                    Err(e) => println!("err: {:?}", e),
                }
            } else if surl.path[0] == b'n' {//neighbor words reference
                for s in dict.neighbors(word, 0, 10).iter() {
                    content.extend(b"<li>");
                    content.extend(s.as_bytes());
                    content.extend(b"</li>");
                }
            } else if surl.path[0] == b's' {//search with regex
                for s in dict.search(word, 20).iter() {
                    content.extend(b"<li>");
                    content.extend(s.as_bytes());
                    content.extend(b"</li>");
                }
            }
        }
    }

    if content.len() > 0 {
        stream.write(b"HTTP/1.0 200 OK\r\nConnection: close\r\nContent-Type: text/html\r\nContent-Length: ").unwrap();
        stream.write(content.len().to_string().as_bytes()).unwrap();
        stream.write(b"\r\n\r\n").unwrap();
        stream.write(&content).unwrap();
        stream.flush().unwrap();
    } else {
        stream.write(b"HTTP/1.1 404 NOT FOUND\r\n\r\nnot found").unwrap();
        stream.flush().unwrap();
    }
}
