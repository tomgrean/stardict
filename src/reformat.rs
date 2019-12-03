extern crate aho_corasick;
extern crate regex;

use std::collections::HashMap;
use std::io::BufRead;
use std::borrow::Cow;
use std::{path, fs, io};

use self::aho_corasick::AhoCorasick;
use self::regex::bytes::{Regex, NoExpand};

/// Used to replace strings in the lookup result.
/// see ContentReformat.
pub struct Replacer {
    line: Vec<u8>,
    op_idx: usize,
}

/// Used to replace strings in the lookup result.
/// read a configuration file which lists everything to replace to.
/// can replace plain text or regular expression with replacement.
/// Detailed configuration file format, see `from_config_file()`.
pub struct ContentReformat {
    repl: HashMap<u8, Vec<Replacer>>,
    regex_cache: HashMap<(u8,usize), Regex>,
}

impl ContentReformat {
    fn from_escape(c: u8) -> u8 {
        match c {
            b't' => b'\t',
            b'n' => b'\n',
            b'r' => b'\r',
            _ => c
        }
    }
    /// load a configuration file. create a `ContentReformat` struct.
    /// format of the configuration file:<br>
    /// The file is split into lines. each line makes up a single config. There are several types of line:
    ///1. Comment. it must start with '#'.
    ///2. Dictionary type specifier. it must start with ':', following a single char that is the same as "sametypesequence" in the .ifo file.
    ///3. Plain string replace: x=y replaces all x to y. note there is no space in between.
    ///4. Plain string replace with variable replacement: x@y replaces all x to y. in y all @p will
    ///   be replaced with dictionary path.
    ///5. Regular expression replace: x~y replaces any text that matches x, with y as Regex replacement string.
    pub fn from_config_file(config: &path::Path) -> ContentReformat {
        let file;
        match fs::File::open(config) {
            Ok(f) => file = f,
            Err(e) => {
                println!("open config failed:{:?}", e);
                return ContentReformat { repl: HashMap::new(), regex_cache: HashMap::new() };
            },
        }
        let mut repl: HashMap<u8, Vec<Replacer>> = HashMap::new();
        let mut dict_format = 0u8;
        let mut regex_cache = HashMap::new();
        io::BufReader::new(file).split(b'\n').filter(|x| match x {
            Ok(v) => {
                if v.len() > 0 && v[0] != b'#' {
                    true
                } else {
                    false
                }
            },
            _ => false,
        }).for_each(|x|{ if let Ok(v) = x {
            if v.len() > 1 && v[0] == b':' {
                dict_format = v[1];
            } else if dict_format != 0u8 {
                let mut op_idx = 0usize;
                let new_vec = if v.contains(& b'\\') {
                    let mut nv = Vec::with_capacity(v.len() - 1);
                    let mut i = 0usize;
                    let mut esc = false;
                    while i < v.len() {
                        if op_idx == 0 && (v[i] == b'=' || v[i] == b'~' || v[i] == b'@') && !esc {
                            op_idx = nv.len();
                        }
                        if !esc && v[i] == b'\\' {
                            esc = true;
                        } else {
                            if esc {
                                nv.push(ContentReformat::from_escape(v[i]));
                            } else {
                                nv.push(v[i]);
                            }
                            esc = false;
                        }

                        i += 1;
                    }
                    nv
                } else {
                    op_idx = v.iter().position(|&a| a == b'=' || a == b'~' || a == b'@').unwrap_or(0);
                    v
                };

                if op_idx > 0 {
                    let obj = Replacer { line: new_vec, op_idx };
                    let reg_cache = if obj.line[op_idx] == b'~' {
                        //do regex cache.
                        Some(Regex::new(std::str::from_utf8(&obj.line[..obj.op_idx]).unwrap()).unwrap())
                    } else { None };
                    match repl.get_mut(&dict_format) {
                        Some(r) => {
                            if let Some(c) = reg_cache {
                                regex_cache.insert((dict_format, r.len()), c);
                            }
                            r.push(obj);
                        },
                        None => {
                            if let Some(c) = reg_cache {
                                regex_cache.insert((dict_format, 0), c);
                            }
                            repl.insert(dict_format, vec![obj]);
                        },
                    }
                }
            }
        }});
        ContentReformat { repl, regex_cache }
    }
    /// find all text in `haystack`, according to `dict_format` and `dict_path`, to
    /// the replacement in `self`, using `AhoCorasick` to make the text replacement.
    pub fn replace_all(&self, dict_format: u8, dict_path: &[u8], haystack: &[u8]) -> Vec<u8> {
        let mut from = Vec::new();
        let mut to = Vec::new();

        let mut hay = Cow::Borrowed(haystack);
        if let Some(x) = self.repl.get(&dict_format) {
            from.reserve(x.len());
            to.reserve(x.len());
            for (hi, v) in x.iter().enumerate() {
                if v.line[v.op_idx] == b'=' {
                    from.push(&v.line[..v.op_idx]);
                    to.push(Cow::Borrowed(&v.line[(v.op_idx+1)..]));
                } else if v.line[v.op_idx] == b'@' {
                    from.push(&v.line[..v.op_idx]);
                    let mut not_first = false;
                    let mut bufe = Vec::new();

                    for s in v.line[(v.op_idx+1)..].split(|x|*x == b'@') {
                        if not_first {
                            if s.len() > 0 {
                                match s[0] {
                                    b'p' => bufe.extend(dict_path),
                                    // add other variables.
                                    _ => (),
                                }
                                //println!("dict path={} p={} {}", std::str::from_utf8(dict_path).unwrap(), s[0], b'p');
                                bufe.extend(&s[1..]);
                            }
                        } else {
                            not_first = true;
                            bufe.extend(s);
                        }
                    }
                    to.push(Cow::Owned(bufe));
                } else if v.line[v.op_idx] == b'~' {
                    /*
                    let re: &Regex = match self.regex_cache.get(&(dict_format, hi)) {
                        Some(r) => &r,
                        _ => {
                            let re = Regex::new(std::str::from_utf8(&v.line[..v.op_idx]).unwrap()).unwrap();
                            self.regex_cache.insert((dict_format, hi), re);
                            &self.regex_cache.get(&(dict_format, hi)).unwrap()
                        },
                    };
                    */
                    if let Some(re) = self.regex_cache.get(&(dict_format, hi)) {
                        match re.replace_all(&hay, NoExpand(&v.line[(v.op_idx+1)..])) {
                            Cow::Owned(o) => hay = Cow::Owned(o),
                            _ => (),
                        }
                    }
                }
            }
        }
        AhoCorasick::new(&from).replace_all_bytes(&hay, &to)
    }
}

