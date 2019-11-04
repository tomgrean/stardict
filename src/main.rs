pub mod dict;
pub mod dictionary;
pub mod idx;
pub mod ifo;
pub mod result;

use std::{env, fs, path, str};

pub struct StarDict {
    directories: Vec<dictionary::Dictionary>,
}

impl StarDict {
    pub fn new(root: path::PathBuf) -> Result<StarDict, result::DictError> {
        let mut items = Vec::new();
        if root.is_dir() {
            for it in fs::read_dir(root)? {
                let it = it?.path();
                if it.is_dir() {
                    match dictionary::Dictionary::new(it) {
                        Ok(it) => {
                            items.push(it);
                        }
                        Err(e) => {
                            eprintln!("ignore reason: {:?}", e);
                        }
                    }
                }
            }
        }

        Ok(StarDict { directories: items })
    }

    pub fn info(&mut self) -> Vec<ifo::Ifo> {
        let mut items = Vec::new();
        for it in &mut self.directories {
            items.push(it.ifo.clone());
        }
        items
    }

    pub fn search(&mut self, word: &str) -> Vec<dictionary::Translation> {
        let mut items = Vec::new();
        for it in &mut self.directories {
            match it.search(word) {
                Ok(v) => items.push(dictionary::Translation {
                    info: it.ifo.clone(),
                    results: v,
                }),
                Err(e) => eprintln!("search {} in {} failed: {:?}", word, it.ifo.name, e),
            }
        }
        items
    }
}
fn main() {
    let difo = ifo::Ifo::open(path::PathBuf::from("c.ifo"));
    match difo {
        Ok(v) => println!("the ifo is {:?}", v),
        Err(e) => eprintln!("error! {}", e),
    }
    let didx = idx::Idx::open(path::PathBuf::from("c.idx"), false).unwrap();
    let mut ddict = dict::Dict::open(path::PathBuf::from("c.dict")).unwrap();
    //println!("idx= {:?}", didx.len());
    //let w = ddict.read(x.offset, x.length as usize).unwrap();
    //println!("the description={}", String::from_utf8(w).unwrap());
    for arg in env::args().skip(1) {
        let i = didx.get(&arg);
        match i {
            Ok(i) => {
                let x = &didx.list[i];
                println!("----get idx = {:?}", x);
                let w = ddict.read(x.offset, x.length as usize).unwrap();
                println!("the description={}", String::from_utf8(w).unwrap());
            }
            Err(e) => println!("error: {:?}", e),
        }
    }
}
