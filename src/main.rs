extern crate regex;

pub mod dict;
pub mod dictionary;
pub mod idx;
pub mod ifo;
pub mod reformat;
pub mod result;
pub mod syn;
//pub mod web;

use self::regex::bytes::Regex;
use std::cmp::Ordering;
use std::io::prelude::*;
use std::iter::Iterator;
use std::mem;
use std::net::TcpListener;
use std::net::TcpStream;
use std::{env, fs, path, str};
//use self::regex::Error;

/// StarDict contains all dictionary found within the specified file system directory.
pub struct StarDict {
    directories: Vec<dictionary::Dictionary>,
}

/// An iterator that merges several underlying iterators. try to dedup one duplicated
/// word from each iterator.
pub struct WordMergeIter<T: Iterator<Item = Vec<u8>>> {
    wordit: Vec<T>,
    cur: Vec<Option<Vec<u8>>>,
}
impl<'a, T: Iterator<Item = Vec<u8>>> Iterator for WordMergeIter<T> {
    type Item = Vec<u8>;
    fn next(&mut self) -> Option<Self::Item> {
        let l = self.cur.len();
        if l == 0 {
            return None;
        }

        let mut x = 0usize;
        let mut i = 1usize;
        while i < l {
            x = match (&self.cur[x], &self.cur[i]) {
                (None, _) => i,
                (_, None) => x,
                (Some(a), Some(b)) => match idx::Idx::dict_cmp(&a, &b, false) {
                    Ordering::Greater => i,
                    Ordering::Equal => {
                        self.cur[i] = self.wordit[i].next();
                        x
                    }
                    _ => x,
                },
            };
            i += 1;
        }
        mem::replace(&mut self.cur[x], self.wordit[x].next())
    }
}

impl StarDict {
    /// Create a StarDict struct from a system path. in the path,
    /// there should be some directories. each directory contains
    /// the dict files, like .ifo, .idx, .dict, etc.
    /// The dictionary will be sorted by its directory name.
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
            match dictionary::Dictionary::new(&**it, root) {
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
    /// Get the Ifo struct, which is parsed from the .ifo file.
    pub fn info(&self) -> Vec<&ifo::Ifo> {
        let mut items = Vec::with_capacity(self.directories.len());
        for it in &self.directories {
            items.push(&it.ifo);
        }
        items
    }
    /// List the following neighbor words of `word`, from `off`.
    /// If `off` is a negative number, list from before `-off`.
    pub fn neighbors(&self, word: &[u8], off: i32) -> WordMergeIter<dictionary::DictNeighborIter> {
        let mut wordit = Vec::with_capacity(2 * self.directories.len());
        let mut cur = Vec::with_capacity(2 * self.directories.len());
        for d in self.directories.iter() {
            let mut x = d.neighbors(word, off);
            let mut s = d.neighbors_syn(word, off);
            cur.push(x.next());
            cur.push(s.next());
            wordit.push(x);
            wordit.push(s);
        }

        WordMergeIter { wordit, cur }
    }
    /// Search from all dictionaries. using the specified regular expression.
    /// to match the beginning of a word, use `^`, the ending of a word, use `$`.
    pub fn search<'a>(&'a self, reg: &'a Regex) -> WordMergeIter<dictionary::IdxIter> {
        let mut wordit = Vec::with_capacity(2 * self.directories.len());
        let mut cur = Vec::with_capacity(2 * self.directories.len());
        for d in self.directories.iter() {
            //println!("in for {}", d.ifo.name.as_str());
            let mut x = d.search_regex(reg);
            let mut s = d.search_syn(reg);
            //println!("created inner iter");
            cur.push(x.next());
            cur.push(s.next());
            //println!("created 1st value");
            wordit.push(x);
            wordit.push(s);
        }

        WordMergeIter { wordit, cur }
    }
    /// Lookup the word. Find in the Idx case-sensitively, if not found then try to do
    /// case-insensitive search. Also find all case-insensitive matching words in Syn.
    pub fn lookup(&self, word: &[u8]) -> Result<Vec<dictionary::LookupResult>, result::DictError> {
        let mut ret: Vec<dictionary::LookupResult> = Vec::with_capacity(self.directories.len());
        for d in self.directories.iter() {
            if let Ok(x) = d.lookup(word) {
                ret.extend(x);
            }
        }
        Ok(ret)
    }
}
struct StardictUrl {
    path: [u8; 4usize],
    word: Vec<u8>,
    offset: i32, // args for offset and length, may use BTreeMap, but it cost too much.
    length: usize,
}
impl StardictUrl {
    fn new() -> StardictUrl {
        StardictUrl {
            path: [0; 4],
            word: Vec::with_capacity(16),
            offset: 0,
            length: 0,
        }
    }
    fn byte_to_u8(b: u8) -> u8 {
        match b {
            b'0'..=b'9' => b - b'0',
            b'A'..=b'F' => b - (b'A' - 10),
            b'a'..=b'f' => b - (b'a' - 10),
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
    fn add_arg_offset(&mut self, c: i32) {
        self.offset = self.offset * 10 + c;
    }
    fn add_arg_length(&mut self, c: usize) {
        self.length = self.length * 10 + c;
    }
}
fn main() {
    let mut host = String::from("0.0.0.0:8888");
    //let mut host = String::from("[::]:8888");
    let mut dictdir = String::from("/usr/share/stardict/dic");
    let dict;
    {
        let mut _daemon = false;
        let mut pendarg = 0u8;

        for arg in env::args().skip(1) {
            //parse options.
            println!("cmd args: {}", &arg);
            let a = arg.as_bytes();
            match pendarg {
                b'h' => {
                    host.clear();
                    host.push_str(&arg);
                    pendarg = 0;
                }
                b'd' => {
                    _daemon = true;
                    pendarg = 0;
                }
                b'r' => {
                    dictdir.clear();
                    dictdir.push_str(&arg);
                    pendarg = 0;
                }
                0 => (),
                _ => {
                    println!("parameter: [-d] [-h host:port] [-r dict-root-dir]");
                    return;
                }
            }
            if a[0] == b'-' {
                pendarg = a[1];
            }
        }
        //println!("get arg host={}, daemon={}", host, daemon);
        //if daemon {
        //}

        dict = StarDict::new(&path::PathBuf::from(&dictdir)).unwrap();
    }
    println!("dict size={}", dict.directories.len());
    //for d in dict.info().iter() {
    //    println!("dict: wordcount:{} {}", d.word_count, d.name);
    //}
    //webs
    let listener = TcpListener::bind(&host).expect("Bind Socket failed!");
    //let pool = web::ThreadPool::new(4);
    let cr = {
        let mut fmtp = path::PathBuf::from(&dictdir);
        fmtp.push("rformat.conf");
        reformat::ContentReformat::from_config_file(&fmtp)
    };

    for stream in listener.incoming() {
        let stream = stream.expect("accept TCP failed!");

        //pool.execute(
        if let Err(_) = handle_connection(stream, &dict, &cr, &dictdir) {
            println!("communication failed!");
        }

        //);
    }

    println!("Shutting down.");
}
fn handle_connection(
    mut stream: TcpStream,
    dict: &StarDict,
    cr: &reformat::ContentReformat,
    dictdir: &str,
) -> std::io::Result<()> {
    //stream.set_nonblocking(false)?;
    //stream.set_nodelay(false)?;
    let mut buffer = vec![0u8; 512];
    {
        let mut sz = 0usize;
        while let Ok(bn) = stream.read(&mut buffer[sz..]) {
            sz = sz + bn;

            let mut stateheader = 0u32;
            for c in buffer[..sz].iter().rev().take(4) {
                if *c == b'\n' {
                    stateheader = stateheader + 1;
                } else if *c == b'\r' {
                    stateheader = stateheader + 10;
                } else {
                    stateheader = 10000;
                    break;
                }
                if stateheader == 2 || stateheader == 22 {
                    stateheader = 2;
                    break;
                }
            }
            if stateheader == 2 {
                buffer.resize(sz, 0);
                break;
            }
            if sz > 4096 {
                stream.write(b"HTTP/1.0 414 Request URI Too Long URI\r\n\r\nnot found")?;
                return Ok(());
            }

            if sz >= buffer.len() {
                buffer.resize(buffer.len() * 2, 0);
            }
        }
    }

    let get = b"GET /";

    //("HTTP/1.0 200 OK\r\nConnection: close\r\n", "index.html");
    let mut content: Vec<u8> = Vec::new();
    let mut surl = StardictUrl::new();

    if buffer.starts_with(get) {
        let mut state = 0i16; //>=0 path, -1 w, -2 p0w, -3 p1w, -4 argKey, -5 argVal
        let mut w = 0u8;
        buffer[5..]
            .iter()
            .take_while(|c| **c != b' ')
            .for_each(|c| {
                if state < 0 {
                    if *c == b'%' {
                        state = -2;
                    } else if *c == b'?' {
                        // parse args.
                        state = -4;
                    } else {
                        if state == -2 {
                            w = StardictUrl::byte_to_u8(*c) << 4;
                            state = -3;
                        } else if state == -3 {
                            w |= StardictUrl::byte_to_u8(*c);
                            surl.add_byte(w);
                            state = -1;
                        } else if state == -4 {
                            if *c == b'=' {
                                state = -5;
                            } else {
                                w = *c;
                            }
                        } else if state == -5 {
                            match *c {
                                b'&' => {
                                    state = -4;
                                }
                                b'-' => {
                                    if w == b'o' {
                                        w = b'O';
                                    } else {
                                        state = -32768;
                                    }
                                }
                                b'0'..=b'9' => {
                                    let v: i32 = (*c - b'0') as i32;
                                    if w == b'o' {
                                        surl.add_arg_offset(v);
                                    } else if w == b'O' {
                                        // negative offset
                                        surl.add_arg_offset(-v);
                                    } else if w == b'l' {
                                        // length
                                        surl.add_arg_length(v as usize);
                                    }
                                }
                                _ => {
                                    state = -32768;
                                }
                            }
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

        //println!("get from url path={}, word={}, off={}, len={}", str::from_utf8(&surl.path).unwrap(), str::from_utf8(&surl.word).unwrap(), surl.offset, surl.length);
        if surl.length == 0 {
            surl.length = 10;
        }
        if surl.word.len() > 0 {
            if surl.path[0] == b'W' {
                //word lookup
                match dict.lookup(&surl.word) {
                    Ok(x) => {
                        content.extend(b"<ol>");
                        for (i, e) in x.iter().enumerate() {
                            content.extend(b"<li><a href='#word_");
                            content.extend(i.to_string().as_bytes());
                            content.extend(b"'>");
                            content.extend(&e.word);
                            content.extend(b" : ");
                            content.extend(e.dictionary.name.as_bytes());
                            content.extend(b"</a></li>");
                        }
                        content.extend(b"</ol>\n");

                        for (i, e) in x.iter().enumerate() {
                            content.extend(b"<div id='word_");
                            content.extend(i.to_string().as_bytes());
                            content.extend(b"' class='res_word'>");
                            content.extend(e.dictionary.name.as_bytes());
                            content.extend(b" (");
                            content.extend(&e.word);
                            content.extend(b") </div><div class='res_definition'>".iter());
                            for (a, b) in e
                                .dictionary
                                .same_type_sequence
                                .as_bytes()
                                .iter()
                                .zip(e.result.split(|c| *c == 0))
                            {
                                content.extend(&cr.replace_all(
                                    *a,
                                    e.dictionary.dict_path.as_bytes(),
                                    b,
                                ));
                            }
                            content.extend(b"</div>\n");
                        }
                    }
                    Err(e) => println!("err: {:?}", e),
                }
            } else if surl.path[0] == b'n' {
                //neighbor words reference
                for s in dict.neighbors(&surl.word, surl.offset).take(surl.length) {
                    content.extend(s);
                    content.extend(b"\n");
                }
            } else if surl.path[0] == b's' {
                //search with regex
                match str::from_utf8(&surl.word) {
                    Ok(x) => match Regex::new(x) {
                        Ok(v) => {
                            content.extend(b"/~/:<ol>");
                            dict.search(&v).take(surl.length).for_each(|e| {
                                content.extend(b"<li><a>");
                                content.extend(e);
                                content.extend(b"</a></li>\n");
                            });
                            content.extend(b"</ol>");
                        }
                        Err(e) => println!("err: {:?}", e),
                    },
                    Err(e) => println!("err: {:?}", e),
                }
            } else if surl.path[0] == b'r' {
                //html js css page etc.
                if let Ok(fname) = str::from_utf8(&surl.word) {
                    let mut pfile = path::PathBuf::from(dictdir);
                    pfile.push(fname);
                    if let Ok(mut f) = fs::File::open(pfile) {
                        if f.read_to_end(&mut content).is_err() {
                            content.clear();
                        }
                    }
                }
            } else if surl.path[0] == b'w' {
                content.extend(HOME_PAGE.as_bytes());
            }
        } else {
            content.extend(HOME_PAGE.as_bytes());
        }
    }

    fn map_by_file(f: &[u8]) -> &'static [u8] {
        if let Some(s) = f.rsplit(|c| *c == b'.').next() {
            match s {
                b"js" => return b"application/javascript",
                b"css" => return b"text/css",
                b"jpg" => return b"image/jpeg",
                b"png" => return b"image/png",
                _ => (),
            }
        }
        b"text/html"
    }
    if content.len() > 0 {
        //let mut cg = 0;
        //content.iter_mut().for_each(|x|{ *x = if cg % 10 == 0 {b'\n'} else {b'a'}; cg = cg + 1;});
        stream.write(b"HTTP/1.0 200 OK\r\nContent-Type: ")?;
        if surl.path[0] == b'n' {
            stream.write(b"text/plain")?;
        } else if surl.path[0] == b'r' {
            stream.write(map_by_file(&surl.word))?;
        } else {
            stream.write(b"text/html")?;
        }
        stream.write(b"\r\nContent-Length: ")?;
        stream.write(content.len().to_string().as_bytes())?;
        stream.write(b"\r\nConnection: close\r\n\r\n")?;
        //stream.write(b"\r\n\r\n")?;
        /*
        for blk in content.chunks(1024) {
            stream.write(blk)?;
        }
        */
        stream.write(&content)?;
    } else {
        stream.write(b"HTTP/1.0 404 NOT FOUND\r\n\r\nnot found")?;
    }
    stream.flush()?;
    //stream.shutdown(std::net::Shutdown::Both)?;
    Ok(())
}
const HOME_PAGE: &'static str = r"<html><head>
<meta http-equiv='Content-Type' content='text/html; charset=UTF-8' />
<title>Star Dictionary</title>
<style>
.res_definition{
 table-layout: fixed;
 border-left: thin dashed black;
 border-right: thin dashed black;
 padding: 5px;
}
.res_word{
 table-layout: fixed;
 border: thin solid black;
 padding: 5px;
}
.numi{
 width:5em;
}
span{
 color:green;
}
a{
 color:blue;
 text-decoration:underline;
 cursor:pointer;
}
blockquote{
 margin:0em 0em 0em 1em;
 padding:0em 0em 0em 0em;
}
</style>
<link href='/r/rhtm/jquery-ui.css' rel='stylesheet'>
<script src='/r/rhtm/jquery.js'></script>
<script src='/r/rhtm/jquery-ui.js'></script>
<script src='/r/rhtm/autohint.js'></script>
</head><body>
<form id='qwFORM' action='/' method='GET'>
<input id='qwt' type='text' name='w' class='ui-autocomplete-input' placeholder='input word' required='required' value=''/>/<input id='chkreg' type='checkbox'/>/
<input type='submit' value='='/> &nbsp;<input type='button' id='backwardbtn' value='<'/> <input type='button' id='forwardbtn' value='>'/>
(<input type='number' class='numi' id='hint_offset' value='0' disabled/>, <input type='number' class='numi' id='result_length' value='10'/>)
</form><hr/>
<div id='dict_content'></div></body></html>";
