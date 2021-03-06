use std::io::BufRead;
use std::{fs, io, path};

use super::result::DictError;

/// corresponds to .ifo file.
#[derive(Debug)]
pub struct Ifo {
    pub author: String,
    //pub version: String,
    pub name: String,
    //pub date: String,
    pub description: String,
    //pub email: String,
    //pub web_site: String,
    pub same_type_sequence: String,
    pub dict_path: String,
    pub idx_file_size: usize,
    pub word_count: usize,
    pub syn_word_count: usize,
    pub idxoffsetbits: usize,
}

impl Ifo {
    /// open the .ifo file. construct a Ifo struct.
    pub fn open(file: &path::Path, base: &path::Path) -> Result<Ifo, DictError> {
        //println!("ifo file = {:?}", file);
        let mut it = Ifo {
            author: String::new(),
            //version: String::new(),
            name: String::new(),
            //date: String::new(),
            description: String::new(),
            //email: String::new(),
            //web_site: String::new(),
            same_type_sequence: String::new(),
            dict_path: file
                .strip_prefix(base)
                .ok()
                .and_then(|p| p.parent().and_then(|x| x.to_str()))
                .unwrap_or(";")
                .to_string(),
            idx_file_size: 0,
            word_count: 0,
            syn_word_count: 0,
            idxoffsetbits: 32,
        };
        for line in io::BufReader::new(fs::File::open(file)?).lines() {
            let line = line?;
            if let Some(id) = line.find('=') {
                let key = &line[..id];
                let val = String::from(&line[id + 1..]);
                match key {
                    "author" => it.author = val,
                    "bookname" => it.name = val,
                    //"version" => it.version = val,
                    "description" => it.description = val,
                    //"date" => it.date = val,
                    "idxfilesize" => it.idx_file_size = val.parse()?,
                    "wordcount" => it.word_count = val.parse()?,
                    //"website" => it.web_site = val,
                    //"email" => it.email = val,
                    "sametypesequence" => it.same_type_sequence = val,
                    "synwordcount" => it.syn_word_count = val.parse()?,
                    "idxoffsetbits" => it.idxoffsetbits = val.parse()?,
                    _ => (), //eprintln!("Ingnore line: {}", line),
                };
            }
        }
        Ok(it)
    }
}
