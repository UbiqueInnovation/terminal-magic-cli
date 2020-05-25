#[macro_use]
extern crate clap;
use clap::App;

use toml;
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap};
use mustache::MapBuilder;
use prompts::{text::TextPrompt, confirm::ConfirmPrompt,Prompt};
use async_std::task;
use std::path::Path;
use dirs::home_dir;

static CONFIG_DIR : &str = ".terminal-magic";

fn main() {
    let yaml = load_yaml!("cli.yaml");
    let app = App::from_yaml(yaml);
    let matches = app.get_matches();
    let git_repo = matches.value_of("git_repo").unwrap();
    match matches.subcommand_name() {
        Some("install") => {
            if let Some(install_cmd) = matches.subcommand_matches("install") {
                if let Some(plugin_name) = install_cmd.value_of("INPUT") {
                    install(git_repo,plugin_name);
                } else {
                    eprintln!("{}", matches.usage());
                    std::process::exit(1);
                }
            }
            else {
                eprintln!("{}", matches.usage());
                std::process::exit(1);
            }
        },
        Some("update") => {
            if let Some(install_cmd) = matches.subcommand_matches("update") {
                if let Some(plugin_name) = install_cmd.value_of("INPUT") {
                    update(git_repo,plugin_name);
                } else {
                    eprintln!("{}", matches.usage());
                    std::process::exit(1);
                }
            }
            else {
                eprintln!("{}", matches.usage());
                std::process::exit(1);
            }
        },
        Some("remove") => {
            if let Some(install_cmd) = matches.subcommand_matches("remove") {
                if let Some(plugin_name) = install_cmd.value_of("INPUT") {
                    remove(plugin_name);
                } else {
                    eprintln!("{}", matches.usage());
                    std::process::exit(1);
                }
            }
            else {
                eprintln!("{}", matches.usage());
                std::process::exit(1);
            }
        },
        _ => {
            let mut out = std::io::stdout();
            eprintln!("{}", matches.usage());
            std::process::exit(1);
        }
    }
}

fn update(git_repo : &str, plugin_name : &str) {
    let home_path = home_dir().expect("Home dir not fount").join(CONFIG_DIR).join(plugin_name);
    if !home_path.exists() {
        eprintln!("module is not installed");
        std::process::exit(1);
    }
    let path_to_module = Path::new(git_repo).join(plugin_name);
    let mustache = mustache::compile_path(path_to_module.join("template.sh")).expect("Could not parse mustache template");

    let toml_str = std::fs::read_to_string(home_path.join("data.toml")).expect("Could not find config.toml");
    let mut toml : PluginInfo = toml::from_str(&toml_str).expect("Cannot parse TOML");
    if let Some(internal_deps) = toml.internal_dependencies.as_mut() {
        for dep in internal_deps {
            install(git_repo, &dep);
        }
    }
    let mut mustache_map_builder = MapBuilder::new();
    if let Some(placeholders) = toml.placeholders.as_mut() {
        for mut placeholder in placeholders.iter_mut() {
            mustache_map_builder = mustache_map_builder.insert(placeholder.0, &placeholder.1).expect("Could not parse object");
        }
    }
    let mustache_map = mustache_map_builder.build();
    render(toml, mustache, mustache_map, plugin_name);
}

fn remove(plugin_name : &str) {
    let home_path = home_dir().expect("Home dir not found").join(CONFIG_DIR).join(plugin_name);
    if !home_path.exists() {
        return;
    }
    std::fs::remove_dir_all(home_path).expect("Could not remove directory");
}

fn install(git_repo : &str, plugin_name : &str) {
    let home_path = home_dir().expect("Home dir not fount").join(CONFIG_DIR).join(plugin_name);
    if home_path.exists() {
        return;
    }
    let path_to_module = Path::new(git_repo).join(plugin_name);
    let mustache = mustache::compile_path(path_to_module.join("template.sh")).expect("Could not parse mustache template");
    let toml_str = std::fs::read_to_string(path_to_module.join("config.toml")).expect("Could not find config.toml");
    let mut toml : PluginInfo = toml::from_str(&toml_str).expect("Cannot parse TOML");
    if let Some(internal_deps) = toml.internal_dependencies.as_mut() {
        for dep in internal_deps {
            install(git_repo, &dep);
        }
    }
    let mut mustache_map_builder = MapBuilder::new();
    if let Some(mut placeholders) = toml.placeholders.as_mut() {
        for mut placeholder in placeholders.iter_mut() {
            println!("Read {}", placeholder.0);
            read(&placeholder.0,&mut placeholder.1);
            mustache_map_builder = mustache_map_builder.insert(placeholder.0, &placeholder.1).expect("Could not parse object");
        }
    }
    let mustache_map = mustache_map_builder.build();
    render(toml, mustache, mustache_map, plugin_name);
}

fn render(mut toml : PluginInfo, mustache : mustache::Template, mustache_map : mustache::Data, plugin_name : &str) {
    let home_path = home_dir().expect("Home dir not fount").join(CONFIG_DIR).join(plugin_name);
   
    if let Ok(_) = std::fs::create_dir_all(&home_path) {
        println!("Created directory");
    }
    if let Ok(_) = std::fs::remove_file(home_path.join("script.sh")){
        println!("script.sh already existed");
    }
    let mut script_file = std::fs::File::create(home_path.join("script.sh")).expect("Could not create file");
    if let Ok(_) = mustache.render_data(&mut script_file, &mustache_map) {
        if let Ok(_) = std::fs::remove_file(home_path.join("data.toml")) {
            println!("data.toml File existed");
        }
        if let Ok(_) = std::fs::write(home_path.join("data.toml"),toml::to_vec(&toml).expect("could not serialize data")){
            println!("Successfully wrote plugin {}!", plugin_name);
        }
    }   
}

fn read(key : &str, entry_type : &mut EntryType) {
    match entry_type {
        EntryType::Value(str ) => {
            read_value(key, str);
        },
        EntryType::Array(array) => {
            read_array(key, array);
        },
        EntryType::Object(obj) => {
            read_object(key, obj);
        }
    }
}

fn read_value(key : &str, str : &mut String) {
    let mut prompt = TextPrompt::new(format!("{} [{}]? ", key, str));
    match task::block_on( async {prompt.run().await}) {
        Ok(Some(s)) => {
            if !s.is_empty() {
                *str = s
            }
        },
        _ => std::process::exit(1)
    }
}

fn read_array(key : &str, array : &mut Vec<EntryType>) {
    let proto_type : EntryType =  array.pop().expect("We need a prototype");
    loop {
        let mut object  = proto_type.clone();
        read(key, &mut object);
        array.push(object);
        let mut prompt = ConfirmPrompt::new(format!("Another one? "));
        match task::block_on( async {prompt.run().await}) {
            Ok(Some(true)) => continue,
            _ => break
        }
    }
}

fn read_object(key : &str, obj : &mut BTreeMap<String, EntryType>) {
    for mut keys in obj.iter_mut() {
        read(keys.0, &mut keys.1);
    }
}

#[derive(Deserialize, Serialize,  Debug)]
struct PluginInfo {
    plugin_info : Package,
    internal_dependencies : Option<Vec<String>>,
    external_dependencies : Option<Vec<String>>,
    placeholders : Option<BTreeMap<String,EntryType>>
}



#[derive(Deserialize, Serialize, Debug)]
struct Package {
    author : String,
    version : String,
    plugin_type : PluginType 
}

#[derive(Deserialize, Serialize,Debug)]
#[serde(untagged)]
enum PluginType {
    Shell(String),
    Script(String)
}

#[derive(Deserialize, Serialize, Clone,Debug, PartialEq, PartialOrd)]
#[serde(untagged)]
enum EntryType {
    Value(String),
    Object(BTreeMap<String, EntryType>),
    Array(Vec<EntryType>)
}

impl std::fmt::Display for EntryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
       if let EntryType::Value(val) = self {
            return f.write_str(val);
       }
       f.write_str("")
    }
    
}
#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Blub {
    test : Test
}
#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Test {
    actions : Actions
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
enum Actions {
    Wait(usize),
    Move { x: usize, y: usize },
}