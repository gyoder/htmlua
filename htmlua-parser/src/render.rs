use std::{cell::RefCell, fmt::Write, path::PathBuf, rc::Rc};

use anyhow::{anyhow, Result};
use kuchikiki::{NodeRef, traits::TendrilSink};
use markup5ever::{LocalName, Namespace, QualName};
use mlua::{Lua, Table};
use pulldown_cmark::{Options, Parser, html};
use syntect::{
    easy::HighlightLines,
    highlighting::{Style, ThemeSet},
    html::{IncludeBackground, styled_line_to_highlighted_html},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};

use crate::{helpers::read_doc_from_file, serve::get_config};

fn create_htmlua_stdlib(l: &Lua, stdout: &Rc<RefCell<String>>) -> mlua::Result<Table> {
    let t = l.create_table()?;

    // This cannot be the best way to do this

    let stdout_println = stdout.clone();
    t.set(
        "println",
        l.create_function(move |_, text: String| {
            let mut stdout_ref = stdout_println.borrow_mut();
            writeln!(stdout_ref, "{text}").map_err(mlua::Error::external)
        })?,
    )?;

    let stdout_print = stdout.clone();
    t.set(
        "print",
        l.create_function(move |_, text: String| {
            let mut stdout_ref = stdout_print.borrow_mut();
            write!(stdout_ref, "{text}").map_err(mlua::Error::external)
        })?,
    )?;
    Ok(t)
}

pub fn execute_lua(document: NodeRef) -> Result<NodeRef> {
    let lua = Lua::new();
    let globals = lua.globals();

    let stdout: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));

    let htmlua_table =
        create_htmlua_stdlib(&lua, &stdout).map_err(|e| anyhow!("Failed to create Lua stdlib: {}", e))?;

    globals
        .set("htmlua", htmlua_table)
        .map_err(|e| anyhow!("Failed to set global: {}", e))?;

    let lua_elements: Vec<_> = match document.select("lua") {
        Ok(e) => e.collect(),
        Err(()) => return Err(anyhow!("Unable to find Lua")),
    };

    for node in lua_elements {
        if let Some(text_node) = node.as_node().first_child() {
            if let Some(lua_code) = text_node.as_text() {
                stdout.borrow_mut().clear();
                lua.load(lua_code.borrow().as_str())
                    .exec()
                    .map_err(|e| anyhow!("Failed to execute Lua: {}", e))?;
                node.as_node()
                    .insert_before(NodeRef::new_text(stdout.borrow().as_str()));
                node.as_node().detach();
            }
        }
    }

    Ok(document)
}

pub fn process_markdown(document: NodeRef) -> Result<NodeRef> {
    let markdown_elements: Vec<_> = match document.select("markdown") {
        Ok(e) => e.collect(),
        Err(()) => return Err(anyhow!("Unable to find markdown elements")),
    };
    for node in markdown_elements {
        if let Some(text_node) = node.as_node().first_child() {
            if let Some(markdown_text) = text_node.as_text() {
                let borrowed_text = markdown_text.borrow();
                let parser = Parser::new_ext(&borrowed_text, Options::all());
                let mut html_output = String::new();
                html::push_html(&mut html_output, parser);
                let html_fragment = kuchikiki::parse_html().one(html_output);
                for child in html_fragment.children() {
                    node.as_node().insert_before(child);
                }
                // Remove the original markdown node.
                node.as_node().detach();
            }
        }
    }
    Ok(document)
}

pub fn expand_template(document: NodeRef, component_path: &PathBuf, include_from: Option<&NodeRef>) -> Result<NodeRef> {
    if let Some(from_node) = include_from {
        for i in document
            .select("includeelement")
            .map_err(|()| anyhow!("Error finding includeelement"))?
        {
            if let Some(name) = i.attributes.borrow().get("name") {
                let exported = from_node
                    .select_first(format!("exportelement.{name}").as_str())
                    .map_err(|()| anyhow!("Error finding exportelement"))?;
                exported
                    .as_node()
                    .children()
                    .rev()
                    .for_each(|c| i.as_node().insert_after(c));
            }
        }
        while let Ok(i) = document.select_first("includeelement") {
            i.as_node().detach();
        }
    }

    for i in document
        .select("include")
        .map_err(|()| anyhow!("Error finding include"))?
        .collect::<Vec<_>>()
    {
        let attrs = match i.as_node().as_element() {
            Some(e) => e.attributes.borrow(),
            None => continue,
        };
        if let Some(include_path) = attrs.get("path") {
            let mut item_path = component_path.clone();
            item_path.push(include_path);
            let new_node = read_doc_from_file(item_path)?;
            let replaced_node = expand_template(new_node, component_path, Some(i.as_node()))?;
            replaced_node
                .select_first("html")
                .map_err(|()| anyhow!("Error finding html"))?
                .as_node()
                .children()
                .rev()
                .for_each(|c| i.as_node().insert_after(c));
        }
    }
    while let Ok(i) = document.select_first("include") {
        i.as_node().detach();
    }
    Ok(document)
}

pub fn process_syntax_highlighting(document: NodeRef) -> Result<NodeRef> {
    let config = get_config();
    let ps = SyntaxSet::load_defaults_newlines();
    let mut ts = ThemeSet::load_defaults();
    let syntax_elements: Vec<_> = match document.select("syntaxhighlight") {
        Ok(e) => e.collect(),
        Err(()) => return Err(anyhow!("Unable to find syntaxhighlight elements")),
    };
    if !syntax_elements.is_empty() && config.syntax_highlighting.load_custom_themes {
        let _ = ts.add_from_folder(&config.paths.themes);
    }
    for node in syntax_elements {
        let attrs = match node.as_node().as_element() {
            Some(e) => e.attributes.borrow(),
            None => continue,
        };
        let language = attrs.get("lang").unwrap_or("text");
        let theme_name = attrs.get("theme").unwrap_or(&config.syntax_highlighting.default_theme);
        if let Some(text_node) = node.as_node().first_child() {
            if let Some(code_text) = text_node.as_text() {
                let syntax = ps
                    .find_syntax_by_extension(language)
                    .or_else(|| ps.find_syntax_by_name(language))
                    .unwrap_or_else(|| ps.find_syntax_plain_text());
                let theme = &ts.themes[theme_name];
                let mut h = HighlightLines::new(syntax, theme);
                let mut html_output = String::new();
                write!(html_output, r#"<pre class="syntax-highlight" data-lang="{language}">"#)?;
                html_output.push_str("<code>");
                for line in LinesWithEndings::from(&code_text.borrow()) {
                    let ranges: Vec<(Style, &str)> = h.highlight_line(line, &ps)?;
                    let escaped = styled_line_to_highlighted_html(&ranges[..], IncludeBackground::No)?;
                    html_output.push_str(&escaped);
                }
                html_output.push_str("</code></pre>");
                let html_fragment = kuchikiki::parse_html().one(html_output);
                for child in html_fragment.children() {
                    node.as_node().insert_before(child);
                }
                // Remove the original syntaxhighlight node.
                node.as_node().detach();
            }
        }
    }
    Ok(document)
}

pub fn generate_footnotes(document: NodeRef) -> Result<NodeRef> {
    let Ok(footnote_container) = document.select_first("footnotecontainer") else { return Ok(document) };

    let ctx_name = QualName::new(None, Namespace::from("http://www.w3.org/1999/xhtml"), LocalName::from("div"));
    for (i, footnote) in document.select("footnote").map_err(|()| anyhow!("Failed to get footnote"))?.enumerate() {
        let i = i + 1;
        let fn_text = footnote.text_contents();
        let sup_tag = kuchikiki::parse_fragment(ctx_name.clone(), Vec::new()).one(
            format!("<a href=#ft-text-{i}><sup id=\"ft-sup-{i}\" title=\"{fn_text}\">{i}</sup></a>")
        ).select_first("a").map_err(|()| anyhow!("parse err"))?;
        footnote.as_node().insert_after(sup_tag.as_node().clone());
        let text_tag = kuchikiki::parse_fragment(ctx_name.clone(), Vec::new()).one(
            format!("<p id=\"ft-text-{i}\"><a href=#ft-sup-{i}>{i}:</a> {fn_text}</p>")
        ).select_first("p").map_err(|()| anyhow!("parse err"))?;
        footnote_container.as_node().insert_before(text_tag.as_node().clone());
    }
    while let Ok(i) = document.select_first("footnote") {
        i.as_node().detach();
    }
    footnote_container.as_node().detach();
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_lua() {
        let page = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <title>Basic HTML Page</title>
            </head>
            <body>
                <h1>Hello World</h1>
                <p>This is a paragraph.</p>
                <span><lua>
                    htmlua.println("Test from Lua!")
                </lua></span>
            </body>
            </html>"#;
        let document = kuchikiki::parse_html().one(page);
        let d = execute_lua(document).unwrap();
        let text = d.select_first("span").unwrap().as_node().text_contents();
        assert_eq!(text, "Test from Lua!\n");
        assert!(d.select_first("lua").is_err());
    }

    #[test]
    fn multiple_lua() {
        let page = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <title>Basic HTML Page</title>
            </head>
            <body>
                <h1>Hello World</h1>
                <p>This is a paragraph.</p>
                <span id="ta"><lua>
                    htmlua.println("Test from Lua!")
                </lua></span>
                <div>
                    <span id="tb"><lua>htmlua.println("test 2")</lua></span>
                </div>
            </body>
            </html>"#;
        let document = kuchikiki::parse_html().one(page);
        let d = execute_lua(document).unwrap();
        let text = d.select_first("#ta").unwrap().as_node().text_contents();
        assert_eq!(text, "Test from Lua!\n");
        let text = d.select_first("#tb").unwrap().as_node().text_contents();
        assert_eq!(text, "test 2\n");
        assert!(d.select_first("lua").is_err());
    }

    #[test]
    fn basic_include() {
        let page = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <title>Basic HTML Page</title>
            </head>
            <body>
                <h1>Hello World</h1>
                <p>This is a paragraph.</p>
                <include path="comp1.html" />
                <span><lua>
                    htmlua.println("Test from Lua!")
                </lua></span>
            </body>
            </html>"#;
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests/components");
        let document = kuchikiki::parse_html().one(page);
        let d = expand_template(document, &p, None).unwrap();
        let text = d.select_first("span").unwrap().as_node().text_contents();
        assert_eq!(text, "included1");
        assert!(d.select_first("include").is_err());
    }

    #[test]
    fn multiple_include() {
        let page = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <title>Basic HTML Page</title>
            </head>
            <body>
                <h1>Hello World</h1>
                <p>This is a paragraph.</p>
                <include path="multi1_1.html"></include>
                <include path="multi1_2.html"></include>
                <include path="multi1_3.html"></include>
            </body>
            </html>"#;
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests/components");
        let document = kuchikiki::parse_html().one(page);
        let d = expand_template(document, &p, None).unwrap();
        let text = d.select_first("#inc1").unwrap().as_node().text_contents();
        assert_eq!(text, "included1");
        let text = d.select_first("#inc2").unwrap().as_node().text_contents();
        assert_eq!(text, "included2");
        let text = d.select_first("#inc3").unwrap().as_node().text_contents();
        assert_eq!(text, "included3");
        assert!(d.select_first("include").is_err());
    }

    #[test]
    fn basic_lua_include() {
        let page = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <title>Basic HTML Page</title>
            </head>
            <body>
                <h1>Hello World</h1>
                <p>This is a paragraph.</p>
                <span><lua>
                    htmlua.println("Test from Lua!")
                </lua></span>
                <include path="comp1.html" />
            </body>
            </html>"#;
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests/components");
        let document = kuchikiki::parse_html().one(page);
        let d = execute_lua(expand_template(document, &p, None).unwrap()).unwrap();
        let text = d.select_first("span").unwrap().as_node().text_contents();
        println!("{}", d.to_string());
        assert_eq!(text, "Test from Lua!\n");
        assert!(d.select_first("lua").is_err());
        let text = d.select_first("#inc1").unwrap().as_node().text_contents();
        assert_eq!(text, "included1");
        assert!(d.select_first("include").is_err());
    }

    #[test]
    fn recursive_include() {
        let page = r#"
            <!DOCTYPE html>
            <html>
                <head>
                    <title>Basic HTML Page</title>
                </head>
            <body>
                <h1>Hello World</h1>
                <include path="comp2_level1.html" />
            </body>
            </html>"#;
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests/components");
        let document = kuchikiki::parse_html().one(page);
        let d = expand_template(document, &p, None).unwrap();
        let text = d.select_first("span").unwrap().as_node().text_contents();
        assert_eq!(text, "included_twice");
        assert!(d.select_first("include").is_err());
    }

    #[test]
    fn export_element_include() {
        let page = r#"
            <!DOCTYPE html>
            <html>
                <head>
                    <title>Basic HTML Page</title>
                </head>
            <body>
                <h1>Hello World</h1>
                <include path="with_include1.html">
                    <exportelement class="el1"><span id="ta">element 1</span></exportelement>
                    <exportelement class="el2"><span id="tb">element 2</span></exportelement>
                </include>
            </body>
            </html>"#;
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests/components");
        let document = kuchikiki::parse_html().one(page);
        let d = expand_template(document, &p, None).unwrap();
        let text = d.select_first("#ta").unwrap().as_node().text_contents();
        assert_eq!(text, "element 1");
        let text = d.select_first("#tb").unwrap().as_node().text_contents();
        assert_eq!(text, "element 2");
        assert!(d.select_first("exportelement").is_err());
        assert!(d.select_first("includeelement").is_err());
    }

    #[test]
    fn footnotes() {
        let page = r"
            <!DOCTYPE html>
            <html>
            <head>
                <title>Basic HTML Page</title>
            </head>
            <body>
                <div>
                    <p>asdf</p>
                    <p>asdf<footnote>um actually</footnote></p>
                    <p>asdf</p>
                    <p>asdf<footnote>no</footnote></p>
                </div>
                <div>
                <footnotecontainer></footnotecontainer>
                </div>
                </body>
                </html>";
        let document = kuchikiki::parse_html().one(page);
        let d = generate_footnotes(document).unwrap();
        let sup1 = d.select_first("#ft-sup-1").unwrap();
        assert_eq!(sup1.attributes.borrow().get("title").unwrap(), "um actually");
        assert_eq!(sup1.text_contents(), "1");

        let sup2 = d.select_first("#ft-sup-2").unwrap();
        assert_eq!(sup2.attributes.borrow().get("title").unwrap(), "no");
        assert_eq!(sup2.text_contents(), "2");

        let ft_text_1 = d.select_first("#ft-text-1").unwrap();
        assert!(ft_text_1.text_contents().contains("um actually"));

        let ft_text_2 = d.select_first("#ft-text-2").unwrap();
        assert!(ft_text_2.text_contents().contains("no"));

        assert!(d.select_first("footnote").is_err());
        assert!(d.select_first("footnotecontainer").is_err());
    }

    #[test]
    fn basic_markdown() {
        let page = r"
            <!DOCTYPE html>
            <html>
            <head>
                <title>Markdown Test</title>
            </head>
            <body>
                <h1>Hello World</h1>
                <div>
                    <markdown>
# This is a heading
This is a paragraph with **bold text** and *italic text*.

- Item 1
- Item 2
- Item 3
                    </markdown>
                </div>
            </body>
            </html>";
        let document = kuchikiki::parse_html().one(page);
        let d = process_markdown(document).unwrap();
        assert!(d.select_first("markdown").is_err());
        assert!(d.select_first("p").is_ok());
        assert!(d.select_first("ul").is_ok());
    }

    #[test]
    fn basic_syntax_highlighting() {
        let page = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <title>Syntax Highlighting Test</title>
            </head>
            <body>
                <h1>Code Example</h1>
                <syntaxhighlight lang="rust">
fn main() {
    println!("Hello, world!");
    let x = 42;
    let y = "test";
}
                </syntaxhighlight>
            </body>
            </html>"#;
        let document = kuchikiki::parse_html().one(page);
        let d = process_syntax_highlighting(document).unwrap();
        assert!(d.select_first("syntaxhighlight").is_err());
        assert!(d.select_first("pre").is_ok());
        assert!(d.select_first("code").is_ok());
        let pre = d.select_first("pre").unwrap();
        let attrs = pre.attributes.borrow();
        assert_eq!(attrs.get("data-lang"), Some("rust"));
        assert!(attrs.get("class").unwrap().contains("syntax-highlight"));
    }
}
