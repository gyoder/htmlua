use std::cell::RefCell;
use std::fmt::Write;
use std::fs::read_to_string;
use std::path::PathBuf;
use std::rc::Rc;

use anyhow::Result;
use anyhow::anyhow;
use kuchikiki::{NodeRef, traits::TendrilSink};
use mlua::{Lua, Table};

fn create_luahtml_stdlib(l: &Lua, stdout: &Rc<RefCell<String>>) -> mlua::Result<Table> {
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
        create_luahtml_stdlib(&lua, &stdout).map_err(|e| anyhow!("Failed to create Lua stdlib: {}", e))?;

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
                exported.as_node().children().for_each(|c| i.as_node().insert_after(c));
            }
        }
        while let Ok(i) = document.select_first("includeelement") {
            i.as_node().detach();
        }
    }
    for i in document
        .select("include")
        .map_err(|()| anyhow!("Error finding include"))?
    {
        let attrs = match i.as_node().as_element() {
            Some(e) => e.attributes.borrow(),
            None => continue,
        };
        if let Some(include_path) = attrs.get("path") {
            let mut item_path = component_path.clone();
            item_path.push(include_path);
            let component_text = read_to_string(item_path)?;
            let new_node = kuchikiki::parse_html().one(component_text);
            let replaced_node = expand_template(new_node, component_path, Some(i.as_node()))?;
            i.as_node().insert_before(replaced_node);
            i.as_node().detach();
        }
    }
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
}
