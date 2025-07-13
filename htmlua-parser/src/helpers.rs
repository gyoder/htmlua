use std::{
    fs::File,
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::PathBuf,
};

use anyhow::Result;
use kuchikiki::NodeRef;
use markup5ever::{LocalName, Namespace, QualName};
use tendril::TendrilSink;




pub fn read_doc_from_file(path: PathBuf) -> Result<NodeRef> {
    let mut file = File::open(path)?;
    let mut reader = BufReader::new(&mut file);

    let mut first_line = String::new();
    reader.read_line(&mut first_line)?;
    let first_line_trimmed = first_line.trim();

    let is_whole_doc = first_line_trimmed.eq_ignore_ascii_case("<!DOCTYPE html>");

    file.seek(SeekFrom::Start(0))?;
    let mut page_string = String::new();
    file.read_to_string(&mut page_string)?;

    if is_whole_doc {
        Ok(kuchikiki::parse_html().one(page_string))
    } else {
        let ctx_name = QualName::new(None, Namespace::from("http://www.w3.org/1999/xhtml"), LocalName::from("div"));
        Ok(kuchikiki::parse_fragment(ctx_name, Vec::new()).one(page_string))
    }
}
