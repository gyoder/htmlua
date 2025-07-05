use super::render::execute_lua;
use super::render::expand_template;
use anyhow::Result;
use kuchikiki::traits::TendrilSink;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

pub fn serve_content(request_uri: &str) -> Result<String> {
    // TODO: read from config
    let pages = PathBuf::from("/var/www/htmlua/pages");
    let components = PathBuf::from("/var/www/htmlua/components");

    let safe_path = Path::new(request_uri).strip_prefix("/")?;
    let page_path = pages.join(safe_path);

    let page_string = read_to_string(&page_path)?;

    let doc = kuchikiki::parse_html().one(page_string);
    let full_doc = expand_template(doc, &components, None)?;

    let executed_doc = execute_lua(full_doc)?;

    Ok(executed_doc.to_string())
}
