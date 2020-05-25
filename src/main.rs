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
use colored::*;

static CONFIG_DIR : &str = ".terminal-magic";

fn read_dir(dir : &Path, base : &str) -> std::io::Result<()> {
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let str_path = path.to_string_lossy();
            if str_path.contains(".git") {
                continue;
            }
            if path.is_dir() {
                let _ = read_dir(&path, base);
            }
            if str_path.contains("config.toml") {
                let module = dir.strip_prefix(base).unwrap();
                let mut module_str = "".yellow();
                let mut installed = "";
                let mut version = String::from("");
                let module_path = home_dir().expect("Home dir not found").join(CONFIG_DIR).join(module);
                if module_path.exists() {
                    module_str = module.to_string_lossy().blue();
                    let toml = read_config(&module_path.join("config.toml"))?;
                    version = toml.plugin_info.version.clone();
                    installed = "(installed)";
                } else {
                    module_str = module.to_string_lossy().green();
                }
                println!("{} {} {}", module_str, version.blue() , installed.blue());
            }
        }
    }
    Ok(())
}

fn read_config(config_path : &Path) -> Result<PluginInfo, std::io::Error>  {
    let toml_str = std::fs::read_to_string(config_path)?;
    if let Ok(pi) = toml::from_str(&toml_str) {
        return Ok(pi);
    }
    return Err(std::io::Error::from(std::io::ErrorKind::InvalidData));
}

fn main() {
    let yaml = load_yaml!("cli.yaml");
    let app = App::from_yaml(yaml);
    let matches = app.get_matches();
    let git_repo = matches.value_of("git_repo").unwrap();
    match matches.subcommand_name() {
        Some("list") => {
            if let Some(_) = matches.subcommand_matches("list") {
                let path_to_module = Path::new(git_repo);
                read_dir(path_to_module, git_repo);
            }
        }
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
    let home_path = home_dir().expect("Home dir not found").join(CONFIG_DIR).join(plugin_name);
    if !home_path.exists() {
        eprintln!("module is not installed");
        std::process::exit(1);
    }
    let path_to_module = Path::new(git_repo).join(plugin_name);
    let mustache = mustache::compile_path(path_to_module.join("template.sh")).expect("Could not parse mustache template");

    let mut toml = read_config(&home_path.join("data.toml")).expect("Cannot find TOML");


    let old_config = read_config(&home_path.join("config.toml")).expect("old config not found");
    let new_config = read_config(&path_to_module.join("config.toml")).expect("module config not found");

    if old_config != new_config {
        eprintln!("{}", "Cannot update since config changed. Need manual merge".yellow());
        eprintln!("");
        let old_config = toml::to_string(&old_config).unwrap();
        let new_config = toml::to_string(&new_config).unwrap();
        for diff in diff::lines(&old_config, &new_config) {
            match diff {
                diff::Result::Left(l)    => println!("-{}", l.red()),
                diff::Result::Both(l, _) => println!(" {}", l),
                diff::Result::Right(r)   => println!("+{}", r.green())
            }
        }
        std::process::exit(1);
    }

    if let Some(internal_deps) = toml.plugin_info.internal_dependencies.as_mut() {
        for dep in internal_deps {
            install(git_repo, &dep);
        }
    }
    if let Some(external_deps) = toml.plugin_info.external_dependencies.as_ref() {
        for dep in external_deps {
            println!("[{}] needs external dependency {}", plugin_name.yellow(), dep.yellow());
        }
    }

   

    let mut mustache_map_builder = MapBuilder::new();
    if let Some(placeholders) = toml.placeholders.as_mut() {
        for mut placeholder in placeholders.iter_mut() {
            mustache_map_builder = mustache_map_builder.insert(placeholder.0, &placeholder.1).expect("Could not parse object");
        }
    }
    let mustache_map = mustache_map_builder.build();
    render(toml, mustache, mustache_map, plugin_name, &path_to_module);
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

    let mut toml = read_config(&home_path.join("data.toml")).expect("Cannot find TOML");
    if let Some(internal_deps) = toml.plugin_info.internal_dependencies.as_mut() {
        for dep in internal_deps {
            install(git_repo, &dep);
        }
    }
    if let Some(external_deps) = toml.plugin_info.external_dependencies.as_ref() {
        for dep in external_deps {
            println!("[{}] needs external dependency {}", plugin_name.yellow(), dep.yellow());
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
    render(toml, mustache, mustache_map, plugin_name, &path_to_module);
}

fn render(mut toml : PluginInfo, mustache : mustache::Template, mustache_map : mustache::Data, plugin_name : &str, path_to_module : &Path) {
    let home_path = home_dir().expect("Home dir not fount").join(CONFIG_DIR).join(plugin_name);
   
    if let Ok(_) = std::fs::create_dir_all(&home_path) {
        println!("Created directory");
    }
    if let Ok(_) = std::fs::remove_file(home_path.join("script.sh")){
        println!("script.sh already existed");
    }

    if let Ok(_) = std::fs::copy(path_to_module.join("config.toml"), home_path.join("config.toml")) {

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

#[derive(Deserialize, Serialize,  Debug, PartialEq)]
struct PluginInfo {
    plugin_info : Package,
    placeholders : Option<BTreeMap<String,EntryType>>
}



#[derive(Deserialize, Serialize, Debug,PartialEq)]
struct Package {
    author : String,
    version : String,
    internal_dependencies : Option<Vec<String>>,
    external_dependencies : Option<Vec<String>>,
    plugin_type : PluginType 
}

#[derive(Deserialize, Serialize,Debug,PartialEq)]
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