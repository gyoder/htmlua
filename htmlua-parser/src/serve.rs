use crate::{
    config::Config,
    helpers::read_doc_from_file,
    render::{execute_lua, expand_template, process_markdown, process_syntax_highlighting},
};
use anyhow::Result;
use std::{path::Path, sync::OnceLock};

static CONFIG: OnceLock<Config> = OnceLock::new();

pub fn get_config() -> &'static Config {
    CONFIG.get_or_init(|| {
        Config::load().unwrap_or_else(|e| {
            eprintln!("Warning: Failed to load config: {}", e);
            eprintln!("Using default configuration");
            Config::default()
        })
    })
}

pub fn serve_content(request_uri: &str) -> Result<String> {
    let config = get_config();
    let safe_path = Path::new(request_uri).strip_prefix("/")?;
    let page_path = config.paths.pages.join(safe_path);
    let doc = read_doc_from_file(page_path)?;
    let full_doc = expand_template(doc, &config.paths.components, None)?;
    let markdown_doc = process_markdown(full_doc)?;
    let highlighted_doc = process_syntax_highlighting(markdown_doc)?;
    let executed_doc = execute_lua(highlighted_doc)?;
    Ok(executed_doc.to_string())
}
