use std::io::BufRead;
use std::{fs, io, path};

use super::result::DictError;

#[derive(Debug, Clone)]
pub struct Ifo {
    pub author: String,
    pub version: String,
    pub name: String,
    pub date: String,
    pub description: String,
    pub email: String,
    pub web_site: String,
    pub same_type_sequence: String,
    pub idx_file_size: isize,
    pub word_count: isize,
    pub syn_word_count: isize,
}

impl Ifo {
    pub fn open(file: path::PathBuf) -> Result<Ifo, DictError> {
        let mut it = Ifo {
            author: String::new(),
            version: String::new(),
            name: String::new(),
            date: String::new(),
            description: String::new(),
            email: String::new(),
            web_site: String::new(),
            same_type_sequence: String::new(),
            idx_file_size: 0,
            word_count: 0,
            syn_word_count: 0,
        };
        for line in io::BufReader::new(fs::File::open(file)?).lines() {
            let line = line?;
            if let Some(id) = line.find('=') {
                let key = &line[..id];
                let val = String::from(&line[id + 1..]);
                match key {
                    "author" => it.author = val,
                    "bookname" => it.name = val,
                    "version" => it.version = val,
                    "description" => it.description = val,
                    "date" => it.date = val,
                    "idxfilesize" => it.idx_file_size = val.parse()?,
                    "wordcount" => it.word_count = val.parse()?,
                    "website" => it.web_site = val,
                    "email" => it.email = val,
                    "sametypesequence" => it.same_type_sequence = val,
                    "synwordcount" => it.syn_word_count = val.parse()?,
                    _ => eprintln!("Ingnore line: {}", line),
                };
            }
        }
        Ok(it)
    }
}
