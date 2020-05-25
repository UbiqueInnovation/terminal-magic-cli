use toml;
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;
use mustache::MapBuilder;
use prompts::{text::TextPrompt, confirm::ConfirmPrompt,Prompt};
use async_std::task;

static CONFIG_DIR : &str = ".terminal-magic";
fn main() {
    let database_mustache = mustache::compile_str(&std::fs::read_to_string("database.sh").unwrap()).unwrap();
    let mut plugin_test : PluginInfo = toml::from_str(&std::fs::read_to_string("plugintest.toml").unwrap()).unwrap();
    let mut mustache_map = MapBuilder::new();
    for mut placeholder in plugin_test.placeholders.iter_mut() {
        read(&mut placeholder.1);
        mustache_map = mustache_map.insert(placeholder.0, &placeholder.1).expect("Could not parse object");
    }
    let map = mustache_map.build();
    database_mustache.render_data(&mut std::io::stdout(), &map).unwrap();
}

fn read(entry_type : &mut EntryType) {
    match entry_type {
        EntryType::Value(str ) => {
            read_value(str);
        },
        EntryType::Array(array) => {
            read_array(array);
        },
        EntryType::Object(obj) => {
            read_object(obj);
        }
    }
}

fn read_value(str : &mut String) {
    let mut prompt = TextPrompt::new(format!("{}? ", str));
        match task::block_on( async {prompt.run().await}) {
            Ok(Some(s)) => *str = s,
            _ => println!("Error reading")
    }
}

fn read_array(array : &mut Vec<EntryType>) {
    let proto_type : EntryType =  array.pop().expect("We need a prototype");
    loop {
        let mut object  = proto_type.clone();
        read(&mut object);
        array.push(object);
        let mut prompt = ConfirmPrompt::new(format!("Another one? "));
        match task::block_on( async {prompt.run().await}) {
            Ok(Some(true)) => continue,
            _ => break
        }
    }
}

fn read_object(obj : &mut BTreeMap<String, EntryType>) {
    for mut keys in obj.iter_mut() {
        read(&mut keys.1);
    }
}

#[derive(Deserialize, Serialize)]
struct PluginInfo {
    plugin_info : Package,
    internal_dependencies : Option<Vec<String>>,
    external_dependencies : Option<Vec<String>>,
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

#[derive(Deserialize, Serialize, Clone)]
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