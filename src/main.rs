use toml;
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn main() -> Result<(), std::io::Error>{
    let input : PluginInfo = toml::from_str(&std::fs::read_to_string("plugintest.toml")?).unwrap();
    if let Some(inner) = input.placeholders.get("DATABASES") {
        println!("Found databases is type : {}", inner.placeholder_type)
    }
   Ok(())
}

#[derive(Deserialize, Serialize)]
struct PluginInfo {
    plugin_info : Package,
    placeholders : BTreeMap<String, Placeholder>
}

#[derive(Deserialize, Serialize)]
struct Package {
    author : String,
    version : String,
    plugin_type : PluginType 
}

#[derive(Deserialize, Serialize)]
enum PluginType {
    Shell(String),
    Script(String)
}

#[derive(Deserialize, Serialize)]
struct Placeholder {
    placeholder_type : String
}