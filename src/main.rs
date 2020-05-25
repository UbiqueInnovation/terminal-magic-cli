#[macro_use]
extern crate clap;
#[macro_use]
extern crate lazy_static;
use clap::App;

use async_std::task;
use colored::*;
use dirs::home_dir;
use mustache::MapBuilder;
use prompts::{confirm::ConfirmPrompt, text::TextPrompt, Prompt};
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use toml;
use std::sync::Mutex;

static CONFIG_DIR: &str = ".terminal-magic";

lazy_static!{
    static ref HOME : PathBuf = { home_dir().expect("Could not find HOME").join(CONFIG_DIR) };
    static ref GLOBAL_CONFIG : Mutex<GlobalConfig> = { 
        let config_dir = home_dir().expect("Could not find HOME").join(CONFIG_DIR).join("global_config.toml");
        let res : GlobalConfig;
        if config_dir.exists() {
            res = toml::from_str(&std::fs::read_to_string(config_dir).expect("Could not find global config")).expect("cannot parse config");
        } else {
            res = GlobalConfig {
                config_path: config_dir,
                git_repo: String::from("")
            };
        }
        Mutex::new(res)
    };
}

fn main() {
    let yaml = load_yaml!("cli.yaml");
    let app = App::from_yaml(yaml);
    let matches = app.get_matches();
    let git_repo : &str;
   
    if matches.is_present("git_repo") {
        git_repo = matches.value_of("git_repo").unwrap();
        let mut glob_conf = GLOBAL_CONFIG.lock().unwrap();
        glob_conf.git_repo = String::from(git_repo);
        glob_conf.save().expect("Could not save global config");
    } else {
        git_repo = "";
    }
   
    match matches.subcommand_name() {
        Some("list") => {
            if let Some(sub_matches) = matches.subcommand_matches("list") {
                if sub_matches.is_present("INPUT") {
                    let module = sub_matches.value_of("INPUT").unwrap();
                    let module_path = Path::new(module).join("script.sh");
                    let base = HOME.join(module);

                    if let Ok(installed_modules) = get_list_of_installed_modules(&HOME, &HOME.to_string_lossy()) {
                        if installed_modules.contains(&String::from(module_path.to_string_lossy())) {
                            let config = read_config(&(base.join("config.toml"))).expect("No config for module found");
                            println!("Module {}", module.green());
                            println!("Author: {}", config.plugin_info.author.green());
                            println!("Installed Version: {}", config.plugin_info.version.green());
                            println!("");
                            if let Some(internal_dependencies) = &config.plugin_info.internal_dependencies {
                                for dep in internal_dependencies {
                                    let dep_path = Path::new(dep).join("script.sh");
                                    if installed_modules.contains(&dep_path.to_string_lossy().to_string()) { continue }
                                    println!("{} {} {} {} {}","Module".yellow(), dep.green(),"not installed, but is listed as a dependency. Consider using".yellow() ,"terminal-magic install".green(), dep.green());
                                }
                            }
                            println!("");
                            if let Some(external_dependencies) = &config.plugin_info.external_dependencies {
                                for dep in external_dependencies {
                                    println!("External Dependency {}", dep.green());
                                }
                            }
                            println!("Placeholders: ");
                            if let Some(placeholders) = &config.placeholders {
                                print!("{}", format!("{:?}",placeholders).green());
                            }
                           
                        }
                    }
                    std::process::exit(0);
                }
                let path_to_module = Path::new(git_repo);
                if read_dir(path_to_module, git_repo).is_err() {
                    eprintln!("{}", "path not found".red());
                }
            }
        }
        Some("install") => {
            if let Some(install_cmd) = matches.subcommand_matches("install") {
                if let Some(plugin_name) = install_cmd.value_of("INPUT") {
                    install(git_repo, plugin_name);
                } else {
                    eprintln!("{}", matches.usage());
                    std::process::exit(1);
                }
            } else {
                eprintln!("{}", matches.usage());
                std::process::exit(1);
            }
        }
        Some("update") => {
            if let Some(install_cmd) = matches.subcommand_matches("update") {
                if let Some(plugin_name) = install_cmd.value_of("INPUT") {
                    update(git_repo, plugin_name);
                } else {
                    eprintln!("{}", matches.usage());
                    std::process::exit(1);
                }
            } else {
                eprintln!("{}", matches.usage());
                std::process::exit(1);
            }
        }
        Some("remove") => {
            if let Some(install_cmd) = matches.subcommand_matches("remove") {
                if let Some(plugin_name) = install_cmd.value_of("INPUT") {
                    remove(plugin_name);
                } else {
                    eprintln!("{}", matches.usage());
                    std::process::exit(1);
                }
            } else {
                eprintln!("{}", matches.usage());
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("{}", matches.usage());
            std::process::exit(1);
        }
    }
   
    if update_source_file().is_err() {
        eprintln!("{}", "Could not update globals source file".red());
    } else {
        println!(
            "Make sure to include {}{}{} in your ~/.zshrc",
            "source ~/".green(),
            CONFIG_DIR.green(),
            "/env".green()
        )
    }
}

fn read_dir(dir: &Path, base: &str) -> std::io::Result<()> {
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
                let module_str: ColoredString;
                let mut installed = "";
                let mut version = String::from("");
                let module_path = HOME.join(module);
                if module_path.exists() {
                    module_str = module.to_string_lossy().blue();
                    let toml = read_config(&module_path.join("config.toml"))?;
                    version = toml.plugin_info.version.clone();
                    installed = "(installed)";
                } else {
                    module_str = module.to_string_lossy().green();
                }
                println!("{} {} {}", module_str, version.blue(), installed.blue());
            }
        }
    }
    Ok(())
}

fn read_config(config_path: &Path) -> Result<PluginInfo, std::io::Error> {
    let toml_str = std::fs::read_to_string(config_path)?;
    if let Ok(pi) = toml::from_str(&toml_str) {
        return Ok(pi);
    }
    return Err(std::io::Error::from(std::io::ErrorKind::InvalidData));
}

fn get_list_of_installed_modules(path: &Path, base: &str) -> std::io::Result<Vec<String>> {
    let mut out_result: Vec<String> = vec![];
    if path.is_dir() {
        for entry in path.read_dir()? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if let Ok(mut list) = get_list_of_installed_modules(&path, base) {
                    out_result.append(&mut list);
                }
            } else {
                if path.to_string_lossy().contains("script.sh") {
                    out_result.push(
                        path.strip_prefix(base)
                            .unwrap()
                            .to_string_lossy()
                            .to_string(),
                    );
                }
            }
        }
    }
    Ok(out_result)
}

fn update_source_file() -> std::io::Result<()> {
    let base = &HOME;
    let modules = get_list_of_installed_modules(&base, &base.to_string_lossy())?;
    let env_path = base.join("env");
    if env_path.exists() {
        std::fs::remove_file(&env_path).expect("Cannot delete file");
    }
    let mapped_values: String = modules
        .into_iter()
        .map(|val| format!("source {}", base.join(val).to_string_lossy()))
        .collect::<Vec<String>>()
        .join("\n");
    std::fs::write(env_path, mapped_values)?;
    Ok(())
}

fn update(git_repo: &str, plugin_name: &str) {
    let home_path = HOME.join(plugin_name);
    if !home_path.exists() {
        eprintln!("module is not installed");
        std::process::exit(1);
    }
    let path_to_module = Path::new(git_repo).join(plugin_name);
    let mustache = mustache::compile_path(path_to_module.join("template.sh"))
        .expect("Could not parse mustache template");

    let mut toml = read_config(&home_path.join("data.toml")).expect("Cannot find TOML");

    if let Ok(old_config) = read_config(&home_path.join("config.toml")) {
        let new_config =
            read_config(&path_to_module.join("config.toml")).expect("module config not found");

        if old_config != new_config {
            eprintln!(
                "{}",
                "Cannot update since config changed. Need manual merge".yellow()
            );
            eprintln!("");
            let old_config = toml::to_string(&old_config).unwrap();
            let new_config = toml::to_string(&new_config).unwrap();
            for diff in diff::lines(&old_config, &new_config) {
                match diff {
                    diff::Result::Left(l) => println!("-{}", l.red()),
                    diff::Result::Both(l, _) => println!(" {}", l),
                    diff::Result::Right(r) => println!("+{}", r.green()),
                }
            }
            std::process::exit(1);
        }
    }

    if let Some(internal_deps) = toml.plugin_info.internal_dependencies.as_mut() {
        for dep in internal_deps {
            install(git_repo, &dep);
        }
    }
    if let Some(external_deps) = toml.plugin_info.external_dependencies.as_ref() {
        for dep in external_deps {
            println!(
                "[{}] needs external dependency {}",
                plugin_name.yellow(),
                dep.yellow()
            );
        }
    }

    let mut mustache_map_builder = MapBuilder::new();
    if let Some(placeholders) = toml.placeholders.as_mut() {
        for placeholder in placeholders.iter_mut() {
            mustache_map_builder = mustache_map_builder
                .insert(placeholder.0, &placeholder.1)
                .expect("Could not parse object");
        }
    }
    let mustache_map = mustache_map_builder.build();
    render(toml, mustache, mustache_map, plugin_name, &path_to_module);
}

fn remove(plugin_name: &str) {
    let home_path = HOME.join(plugin_name);
    if !home_path.exists() {
        return;
    }
    std::fs::remove_dir_all(home_path).expect("Could not remove directory");
}

fn install(git_repo: &str, plugin_name: &str) {
    let home_path = HOME.join(plugin_name);
    if home_path.exists() {
        return;
    }
    let path_to_module = Path::new(git_repo).join(plugin_name);
    let mustache = mustache::compile_path(path_to_module.join("template.sh"))
        .expect("Could not parse mustache template");

    let mut toml = read_config(&path_to_module.join("config.toml")).expect("Cannot find TOML");
    if let Some(internal_deps) = toml.plugin_info.internal_dependencies.as_mut() {
        for dep in internal_deps {
            install(git_repo, &dep);
        }
    }
    if let Some(external_deps) = toml.plugin_info.external_dependencies.as_ref() {
        for dep in external_deps {
            println!(
                "[{}] needs external dependency {}",
                plugin_name.yellow(),
                dep.yellow()
            );
        }
    }
    let mut mustache_map_builder = MapBuilder::new();
    if let Some(placeholders) = toml.placeholders.as_mut() {
        for mut placeholder in placeholders.iter_mut() {
            println!("Read {}", placeholder.0);
            read(&placeholder.0, &mut placeholder.1);
            mustache_map_builder = mustache_map_builder
                .insert(placeholder.0, &placeholder.1)
                .expect("Could not parse object");
        }
    }
    let mustache_map = mustache_map_builder.build();
    render(toml, mustache, mustache_map, plugin_name, &path_to_module);
}

fn render(
    toml: PluginInfo,
    mustache: mustache::Template,
    mustache_map: mustache::Data,
    plugin_name: &str,
    path_to_module: &Path,
) {
    let home_path = HOME.join(plugin_name);
    if let Ok(_) = std::fs::create_dir_all(&home_path) {
        println!("Created directory");
    }
    if let Ok(_) = std::fs::remove_file(home_path.join("script.sh")) {
        println!("script.sh already existed");
    }

    if let Ok(_) = std::fs::copy(
        path_to_module.join("config.toml"),
        home_path.join("config.toml"),
    ) {}
    let mut script_file =
        std::fs::File::create(home_path.join("script.sh")).expect("Could not create file");
    if let Ok(_) = mustache.render_data(&mut script_file, &mustache_map) {
        if let Ok(_) = std::fs::remove_file(home_path.join("data.toml")) {
            println!("data.toml File existed");
        }
        if let Ok(_) = std::fs::write(
            home_path.join("data.toml"),
            toml::to_vec(&toml).expect("could not serialize data"),
        ) {
            println!("Successfully wrote plugin {}!", plugin_name);
        }
    }
}

fn read(key: &str, entry_type: &mut EntryType) {
    match entry_type {
        EntryType::Value(str) => {
            read_value(key, str);
        }
        EntryType::Array(array) => {
            read_array(key, array);
        }
        EntryType::Object(obj) => {
            read_object(obj);
        }
    }
}

fn read_value(key: &str, str: &mut String) {
    let mut prompt = TextPrompt::new(format!("{} [{}]? ", key, str));
    match task::block_on(async { prompt.run().await }) {
        Ok(Some(s)) => {
            if !s.is_empty() {
                *str = s
            }
        }
        _ => std::process::exit(1),
    }
}

fn read_array(key: &str, array: &mut Vec<EntryType>) {
    let proto_type: EntryType = array.pop().expect("We need a prototype");
    loop {
        let mut object = proto_type.clone();
        read(key, &mut object);
        array.push(object);
        let mut prompt = ConfirmPrompt::new(format!("Another one? "));
        match task::block_on(async { prompt.run().await }) {
            Ok(Some(true)) => continue,
            _ => break,
        }
    }
}

fn read_object(obj: &mut BTreeMap<String, EntryType>) {
    for mut keys in obj.iter_mut() {
        read(keys.0, &mut keys.1);
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
struct PluginInfo {
    plugin_info: Package,
    placeholders: Option<BTreeMap<String, EntryType>>,
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
struct Package {
    author: String,
    version: String,
    internal_dependencies: Option<Vec<String>>,
    external_dependencies: Option<Vec<String>>,
    plugin_type: PluginType,
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
#[serde(untagged)]
enum PluginType {
    Shell(String),
    Script(String),
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, PartialOrd)]
#[serde(untagged)]
enum EntryType {
    Value(String),
    Object(BTreeMap<String, EntryType>),
    Array(Vec<EntryType>),
}

impl std::fmt::Display for EntryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let EntryType::Value(val) = self {
            return f.write_str(val);
        }
        f.write_str("")
    }
}
#[derive(Deserialize, Serialize, Debug, PartialEq)]
struct GlobalConfig {
    config_path : PathBuf,
    git_repo : String
}

impl GlobalConfig {
    fn save(&self) -> std::io::Result<()>{
        std::fs::write(self.config_path.as_path(), toml::to_string(self).unwrap())?;
        Ok(())
    }
}