use std::env;

use htmlua_parser::serve::serve_content;

fn main() {
    println!("Content-Type: text/html\n");

    let request_uri = env::var("PATH_INFO").unwrap_or_else(|_| "".to_string());
    let page = serve_content(request_uri.as_str()).unwrap();

    println!("{page}");
}
