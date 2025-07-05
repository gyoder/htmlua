use std::env;
use std::fs;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use htmlua_parser::serve::serve_content;
use kuchikiki::{
    NodeRef,
    traits::TendrilSink,
};

fn main() {
    println!("Content-Type: text/html\n");

    let request_uri = env::var("PATH_INFO").unwrap_or_else(|_| "".to_string());

    let page = serve_content(request_uri.as_str()).unwrap();


    // Output the final expanded HTML
    println!("{page}");
}

// Simple HTML escape function
fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}
