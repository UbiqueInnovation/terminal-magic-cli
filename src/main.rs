use toml;
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn main() -> Result<(), std::io::Error>{
    let input : PluginInfo = toml::from_str(&std::fs::read_to_string("plugintest.toml")?).unwrap();
    if let Some(inner) = input.placeholders.get("DATABASES") {
        if let EntryType::Array(array) = inner {
            for entry in array {
                if let EntryType::Object(inner) = entry {
                    println!("Found databases path is : {}, name is {}", inner.get("dbPath").unwrap(), inner.get("dbName").unwrap());
                }
            }
        }
    }
   Ok(())
}

#[derive(Deserialize, Serialize)]
struct PluginInfo {
    plugin_info : Package,
    placeholders : BTreeMap<String, EntryType>
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
#[serde(untagged)]
enum EntryType {
    Value(String),
    Array(Vec<EntryType>),
    Object(BTreeMap<String, EntryType>)
}

impl std::fmt::Display for EntryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
       if let EntryType::Value(val) = self {
            return f.write_str(val);
       }
       f.write_str("")
    }
    
}