use std::{cell::{LazyCell, RefCell}, collections::HashMap, fmt::Write, rc::Rc};

use anyhow::anyhow;
use mlua::{prelude::*, Error, Lua, Table};
use reqwest::{blocking::{Client, Response}, header::HeaderMap};
use serde::{Deserialize, Serialize};
use serde_json::to_value;


pub fn create_htmlua_stdlib(l: &Lua, stdout: &Rc<RefCell<String>>) -> mlua::Result<Table> {
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

    t.set("http", create_http_lib(l)?)?;
    Ok(t)
}

fn create_http_lib(l: &Lua) -> mlua::Result<Table> {
    let t = l.create_table()?;
    let http_client: Rc<LazyCell<Client>> = Rc::new(LazyCell::new(|| Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .user_agent("htmlua/0.1.0")
            .build().unwrap()
        ));


    let client = http_client.clone();
    t.set(
        "get",
        l.create_function(move |_, url: String| {
            let res = client.get(url).send().map_err(|e| Error::RuntimeError(e.to_string()))?;
            let lua_res: LuaHttpResponse = TryFrom::try_from(res)?;
            Ok(lua_res)
        })?
    )?;

    let client = http_client.clone();
    t.set(
        "post",
        l.create_function(move |_, url: String| {
            let res = client.post(url).send().map_err(|e| Error::RuntimeError(e.to_string()))?;
            let lua_res: LuaHttpResponse = TryFrom::try_from(res)?;
            Ok(lua_res)
        })?
    )?;

    let client = http_client.clone();
    t.set(
        "get_with_data",
        l.create_function(move |_, (url, data): (String, HashMap<String, String>)| {
            let res = client.get(url).query(&data).send().map_err(|e| Error::RuntimeError(e.to_string()))?;
            let lua_res: LuaHttpResponse = TryFrom::try_from(res)?;
            Ok(lua_res)
        })?
    )?;

    let client = http_client.clone();
    t.set(
        "post_with_data_form",
        l.create_function(move |_, (url, data): (String, HashMap<String, String>)| {
            let res = client.post(url).form(&data).send().map_err(|e| Error::RuntimeError(e.to_string()))?;
            let lua_res: LuaHttpResponse = TryFrom::try_from(res)?;
            Ok(lua_res)
        })?
    )?;

    let client = http_client.clone();
    t.set(
        "post_with_data_json",
        l.create_function(move |_, (url, data): (String, HashMap<String, String>)| {
            let res = client.post(url).json(&data).send().map_err(|e| Error::RuntimeError(e.to_string()))?;
            let lua_res: LuaHttpResponse = TryFrom::try_from(res)?;
            Ok(lua_res)
        })?
    )?;
    //TODO: all http

    t.set(
        "decode_json",
        l.create_function(move |l, text: String| {
            let table: serde_json::Value = serde_json::from_str(&text).map_err(|e| Error::RuntimeError(e.to_string()))?;
            Ok(l.to_value(&table))
        })?
    )?;

    Ok(t)
}


#[derive(Serialize, Deserialize)]
struct LuaHttpResponse {
    headers: HashMap<String, String>,
    body: String,
    status: u16,
}

impl TryFrom<Response> for LuaHttpResponse {
    fn try_from(value: Response) -> Result<Self, Self::Error> {
        Ok(LuaHttpResponse { headers: headermap_to_hashmap(value.headers()), status: value.status().as_u16(), body: value.text().map_err(|e| Error::RuntimeError(e.to_string()))?, })
    }

    type Error = Error;
}

fn headermap_to_hashmap(headers: &HeaderMap) -> HashMap<String, String> {
    let mut map = HashMap::new();

    for (name, value) in headers {
        if let Ok(value_str) = value.to_str() {
            map.insert(name.as_str().to_string(), value_str.to_string());
        }
    }

    map
}

impl IntoLua for LuaHttpResponse {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table()?;

        table.set("status", self.status)?;
        table.set("body", self.body)?;

        let headers_table = lua.create_table()?;
        for (key, value) in self.headers {
            headers_table.set(key, value)?;
        }
        table.set("headers", headers_table)?;

        Ok(LuaValue::Table(table))
    }
}
