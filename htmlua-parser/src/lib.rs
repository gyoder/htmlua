#![warn(clippy::pedantic)]

use std::cell::RefCell;
use std::fmt::Write;
use std::rc::Rc;

use anyhow::Result;
use anyhow::anyhow;
use kuchikiki::{
    NodeRef,
    traits::TendrilSink,
};
use mlua::{Lua, Table};
use tendril::fmt;
use tendril::{Atomicity, Tendril};

fn create_luahtml_stdlib(l: &Lua, stdout: &Rc<RefCell<String>>) -> mlua::Result<Table> {
    let t = l.create_table()?;

    // This cannot be the best way to do this

    let stdout_println = stdout.clone();
    t.set(
        "println",
        l.create_function(move |_, text: String| {
            let mut stdout_ref = stdout_println.borrow_mut();
            writeln!(stdout_ref, "{text}").map_err(mlua::Error::external)?;
            Ok(())
        })?,
    )?;

    let stdout_print = stdout.clone();
    t.set(
        "print",
        l.create_function(move |_, text: String| {
            let mut stdout_ref = stdout_print.borrow_mut();
            write!(stdout_ref, "{text}").map_err(mlua::Error::external)?;
            Ok(())
        })?,
    )?;
    Ok(t)
}

pub fn parse<F: fmt::Format, A: Atomicity, T: Into<Tendril<F, A>>>(to_parse: T) -> Result<NodeRef>
where
    tendril::Tendril<tendril::fmt::UTF8>: std::convert::From<tendril::Tendril<F, A>>,
{
    let document = kuchikiki::parse_html().one(to_parse.into());
    let lua = Lua::new();
    let globals = lua.globals();

    let stdout: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));

    let htmlua_table = create_luahtml_stdlib(&lua, &stdout)
        .map_err(|e| anyhow::anyhow!("Failed to create Lua stdlib: {}", e))?;

    globals
        .set("htmlua", htmlua_table)
        .map_err(|e| anyhow::anyhow!("Failed to set global: {}", e))?;

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
                    .map_err(|e| anyhow::anyhow!("Failed to execute Lua: {}", e))?;
                node.as_node()
                    .insert_before(NodeRef::new_text(stdout.borrow().as_str()));
                node.as_node().detach();
            }
        }
    }

    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base() -> Result<()> {
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
        let d = parse(page)?;
        let text = d.select_first("span").unwrap().as_node().text_contents();
        assert_eq!(text, "Test from Lua!\n");
        assert!(d.select_first("lua").is_err());
        Ok(())
    }
}
