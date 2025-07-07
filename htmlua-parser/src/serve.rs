use crate::helpers::read_doc_from_file;

use super::render::{execute_lua, expand_template, process_markdown, process_syntax_highlighting};
use anyhow::Result;
use std::path::{Path, PathBuf}
;

pub fn serve_content(request_uri: &str) -> Result<String> {
    // TODO: read from config
    let pages = PathBuf::from("/var/www/htmlua/pages");
    let components = PathBuf::from("/var/www/htmlua/components");
    let safe_path = Path::new(request_uri).strip_prefix("/")?;
    let page_path = pages.join(safe_path);

    let doc = read_doc_from_file(page_path)?;

    let full_doc = expand_template(doc, &components, None)?;
    let markdown_doc = process_markdown(full_doc)?;
    let highlighted_doc = process_syntax_highlighting(markdown_doc)?;
    let executed_doc = execute_lua(highlighted_doc)?;
    Ok(executed_doc.to_string())
}
