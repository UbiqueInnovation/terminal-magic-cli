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
use semver::Version;
use serde_derive::{Deserialize, Serialize};
use std::{path::{Path, PathBuf}, str::FromStr};
use toml;
use std::sync::Mutex;
use indexmap::IndexMap;
use regex::Regex;

static CONFIG_DIR: &str = ".terminal-magic";

lazy_static!{
    static ref HOME : PathBuf = home_dir().expect("Could not find HOME").join(CONFIG_DIR);
    static ref GLOBAL_CONFIG : Mutex<GlobalConfig> = { 
        let config_dir = home_dir().expect("Could not find HOME").join(CONFIG_DIR).join("global_config.toml");
        let res : GlobalConfig;
        if config_dir.exists() {
            res = toml::from_str(&std::fs::read_to_string(config_dir).expect("Could not find global config")).expect("cannot parse config");
        } else {
            if !HOME.join(CONFIG_DIR).exists() {
                std::fs::create_dir_all(HOME.join(CONFIG_DIR)).expect("Could not create config dir");
            }
            res = GlobalConfig {
                config_path: config_dir,
                git_repo: String::from(""),
                ssh_key : None,
                key_needs_pw : false
            };
            if res.save().is_err() {
                eprintln!("{}", "Could not write config".red());
            }
        }
        Mutex::new(res)
    };
}

fn main() {
    let yaml = load_yaml!("cli.yaml");
    let app = App::from_yaml(yaml);
    let matches = app.get_matches();
    let git_repo : String;
    {
        let mut glob_conf = GLOBAL_CONFIG.lock().unwrap();
        if matches.is_present("git_repo") {
            git_repo = String::from(matches.value_of("git_repo").unwrap());
        
            glob_conf.git_repo = shellexpand::tilde(&git_repo.clone()).to_string();
            glob_conf.save().expect("Could not save global config");
        } else {
            git_repo = shellexpand::tilde(&glob_conf.git_repo.clone()).to_string();
        }
        println!("Module Git Repo: {}", git_repo.green());
        println!("");
    }
    if matches.is_present("clone") {
        let clone_url = matches.value_of("clone").unwrap();
        if matches.is_present("ssh_key") {
            let ssh_key = Path::new(matches.value_of("ssh_key").unwrap());
            println!("{}{}", "Using key ".green(), ssh_key.to_string_lossy());
            match check_out_modules_with_key(clone_url, &ssh_key) {
                Ok(_) => {
                    println!("{}{}{}", "Clone repsitory from ".yellow(), clone_url.blue(), " successfully".yellow());
                },
                Err(e) => {
                    eprintln!("{}{:?}", "Could not clone module: ".red(), e);
                    std::process::exit(1);
                }
            }
        } else {
            match check_out_modules_with_pw(clone_url) {
                Ok(_) => {
                    println!("{}{}{}", "Clone repsitory from ".yellow(), clone_url.blue(), " successfully".yellow());
                },
                Err(_) => {
                    eprintln!("{}", "Could not clone module".red());
                    std::process::exit(1);
                }
            }
        }
        std::process::exit(0);
    }
   
    match matches.subcommand_name() {
        Some("list") => {
            if let Some(sub_matches) = matches.subcommand_matches("list") {
                if sub_matches.is_present("INPUT") {
                    let module = sub_matches.value_of("INPUT").unwrap();
                    let module_path = Path::new(module).join("script.sh");
                    let base = HOME.join(module);

                    if let Ok(installed_modules) = get_list_of_installed_modules(&HOME, &HOME.to_string_lossy()) {
                        let config : PluginInfo;
                        let mut updated_config : Option<PluginInfo> = None;
                        let mut installed = false;
                        if installed_modules.contains(&String::from(module_path.to_string_lossy())) {
                            config = read_config(&(base.join("config.toml"))).expect("No config for module found");
                            updated_config = Some(read_config(&Path::new(&git_repo).join(module).join("config.toml")).expect("Cannot find module"));
                            installed = true;
                        } else {
                            config = read_config(&Path::new(&git_repo).join(module).join("config.toml")).expect("Cannot find module");
                        }
                            println!("Module {}", module.green());
                            println!("Author: {}", config.plugin_info.author.green());
                            if installed {
                                let new_version = updated_config.unwrap().plugin_info.version;
                                println!("Installed Version (Repo Version): {} ({}) ", config.plugin_info.version.green(), new_version.green());
                            }
                        
                            println!("");
                            if let Some(help) = config.plugin_info.help {
                                let re = Regex::new(r"`(?P<color>[a-z]*)\s(?P<content>[\s\S]*?)\s*`").unwrap();
                                let mut cursor = 0;
                                for re_match in re.captures_iter(&help) {
                                    print!("{}", &help[cursor..re_match.get(0).unwrap().start()]);
                                    print!("{}", &re_match["content"].color(&re_match["color"]));
                                    cursor = re_match.get(0).unwrap().end();
                                }
                                if cursor <  help.len()-1 {
                                    print!("{}", &help[cursor..])
                                }
                                println!("");
                            }
                            
                            if let Some(internal_dependencies) = &config.plugin_info.internal_dependencies {
                                for dep in internal_dependencies {
                                    let dep_path = Path::new(dep).join("script.sh");
                                    if installed_modules.contains(&dep_path.to_string_lossy().to_string()) { continue }
                                    println!("{} {} {} {} {}","Module".yellow(), dep.green(),"not installed, but is listed as a dependency. Consider using".yellow() ,"terminal-magic install".green().bold(), dep.green());
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
                    std::process::exit(0);
                } else {
                    match update_modules() {
                        Err(e) => {
                            eprintln!("{}{:?}", "Could not update repo".red(), e);
                        },
                        _ => {}
                    }
                }
                let path_to_module = Path::new(&git_repo);
                if read_dir(path_to_module, &git_repo).is_err() {
                    eprintln!("{}", "path not found".red());
                }
            }
        }
        Some("install") => {
            if let Some(install_cmd) = matches.subcommand_matches("install") {
                if let Some(plugin_name) = install_cmd.value_of("INPUT") {
                    install(&git_repo, plugin_name);
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
                    if plugin_name == "all" {
                        println!("{}\n\n", "Start updating all packages".green());
                        let modules = if let Ok(modules) = get_list_of_installed_modules(&HOME, &HOME.to_string_lossy()) {modules} else {
                            eprintln!("Could not get list of installed modules");
                            std::process::exit(1)
                        };
                        for module in modules {
                            let module = module.replace("/script.sh", "");
                            let base = HOME.join(&module);
                            let config = read_config(&(base.join("config.toml"))).expect("No config for module found");
                            let new_config = read_config(&Path::new(&git_repo).join(&module).join("config.toml")).expect("Cannot find module");
                            if let (Ok(old_version), Ok(new_version)) = (Version::parse(&config.plugin_info.version), Version::parse(&new_config.plugin_info.version)) {
                                if new_version > old_version {
                                    if config.placeholders == new_config.placeholders {
                                        println!("[{}] Try updating from {} to {}",module.yellow(), old_version, new_version);
                                        update(&git_repo, &module,false);
                                    }
                                }
                            }
                        }
                        println!("{}", "\n 🥳 All updateable packages are up to date.\n".green());
                    } else {
                        update(&git_repo, plugin_name, true);
                    }
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
        let command = format!("source ~/{}/env", CONFIG_DIR);
        let alternative_command = format!("source {}", &HOME.join("env").to_string_lossy());
        let zshrc_file = &home_dir().unwrap_or(PathBuf::from_str("~").unwrap()).join(".zshrc");
        if let Ok(lines) = std::fs::read_to_string(zshrc_file) {
            if lines.contains(&command)
            || lines.contains(&alternative_command) {
                std::process::exit(0);
            }
        }
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
            if path.ends_with("config.toml"){
                let module = dir.strip_prefix(base).unwrap();
                let module_str: ColoredString;
                let mut installed = "";
                let mut version = String::from("");
                let module_path = HOME.join(module);
                let mut repo_version = String::from("");
                if module_path.exists() {
                    module_str = module.to_string_lossy().blue();
                    let toml = read_config(&module_path.join("config.toml"))?;
                    let new_toml = read_config(&dir.join("config.toml")).unwrap();
                    version = toml.plugin_info.version.clone();
                    installed = "(installed)";
                    if version != new_toml.plugin_info.version {
                        repo_version = format!(" ({}) ", new_toml.plugin_info.version);
                    }
                } else {
                    module_str = module.to_string_lossy().green();
                }
                println!("{} {}{} {}", module_str, version.blue(), repo_version.yellow(), installed.blue());
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

fn boolean_prompt(prompt_string : &str) -> bool {
    let mut prompt = ConfirmPrompt::new(format!("{}",prompt_string));
    match task::block_on(async { prompt.run().await }) {
        Ok(Some(val)) => val,
        _ =>  std::process::exit(1)
    }
}
fn text_prompt(prompt_string : &str) -> Option<String>{
    let mut prompt = TextPrompt::new(format!("{}", prompt_string));
    match task::block_on(async  {prompt.run().await}) {
        Ok(val) => { 
            val
        },
        _ => { 
            None
        }
    }
}
fn password_prompt(prompt_string : &str) -> Option<String>{
    let mut prompt = TextPrompt::new(format!("{}", prompt_string)).with_style(prompts::text::Style::Password);
    match task::block_on(async  {prompt.run().await}) {
        Ok(val) => { 
            val
        },
        _ => { 
            None
        }
    }
}

fn update(git_repo: &str, plugin_name: &str, fail_on_error: bool) {
    let home_path = HOME.join(plugin_name);
    if !home_path.exists() {
        eprintln!("module is not installed");
        if fail_on_error {
            std::process::exit(1);
        }
    }
    let path_to_module = Path::new(git_repo).join(plugin_name);
    if !path_to_module.exists() {
        eprintln!("{}", "Could not find module in the git repo. Did you execute `git pull`?".red());
         if fail_on_error {
            std::process::exit(1);
        }
    }
    let mustache = mustache::compile_path(path_to_module.join("template.sh"))
        .expect("Could not parse mustache template");

    let mut toml = read_config(&home_path.join("data.toml")).expect("Cannot find TOML");
    let old_config = read_config(&home_path.join("config.toml")).expect("Cannot find old config (maybe you did update terminal-magic)");
        let new_config =
            read_config(&path_to_module.join("config.toml")).expect("module config not found");

    if old_config != new_config {
        println!(
            "{}",
            "Config changed check the changes".yellow()
            
        );

        let old_config_str = toml::to_string(&old_config).unwrap();
        let new_config_str = toml::to_string(&new_config).unwrap();
        print_diff(&old_config_str, &new_config_str);
        if old_config.placeholders != new_config.placeholders {
            eprintln!("{} {}", "Placeholders are different, cannot merge yet data files.".red(), if fail_on_error { "Abbort!".red() } else { "Continue!".red()});
            if fail_on_error {
            std::process::exit(1);
        }
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
            if let EntryType::Array(arr) = placeholder.1 {
                if boolean_prompt(&format!("Add new elements [{}]? ", placeholder.0)) {
                    if let EntryType::Array(element)= old_config.placeholders.as_ref().unwrap().get(placeholder.0).unwrap() {
                        let element = element.first().unwrap().clone();
                        arr.insert(0, element);
                        mustache_map_builder = read_array(placeholder.0, arr, mustache_map_builder);
                    }
                } else {
                    let name = get_short_names(arr);
                    mustache_map_builder = mustache_map_builder.insert_str(format!("{}_shortNames", placeholder.0), name);
                }
            }
            mustache_map_builder = mustache_map_builder
                .insert(placeholder.0, &placeholder.1)
                .expect("Could not parse object");
        }
    }
    let should_overwrite = boolean_prompt("Update supporting files?");
    
    if let Some(files) = &new_config.supporting_files {
        let home_path = HOME.join(plugin_name);
        mustache_map_builder = add_files_as_vars(files, mustache_map_builder, &home_path, &path_to_module, &home_path, should_overwrite);
    }
    

    let mustache_map = mustache_map_builder.build();
    let script = render(mustache, mustache_map);
    let old_script = get_old_script(plugin_name);

    print_diff(&old_script, &script);

    if !boolean_prompt("Update?") {
        std::process::exit(1);
    }
    write_file(toml, script, plugin_name, &path_to_module);
}

fn print_diff(left : &str, right : &str){
    for diff in diff::lines(&left, &right) {
        match diff {
            diff::Result::Left(l) => println!("-{}", l.red()),
            diff::Result::Both(_, _) => {},
            diff::Result::Right(r) => println!("+{}", r.green()),
        }
    }
}

fn remove(plugin_name: &str) {
    let home_path = HOME.join(plugin_name);
    if !home_path.exists() {
        eprintln!("{}{}", "Could not find installed module ".red(), plugin_name);
        std::process::exit(1);
    }
    std::fs::remove_dir_all(home_path).expect("Could not remove directory");
}

fn install(git_repo: &str, plugin_name: &str) {
    let home_path = HOME.join(plugin_name);
    if home_path.exists() {
        return;
    }
    let path_to_module = Path::new(git_repo).join(plugin_name);
    if !path_to_module.exists() {
        eprintln!("{}", "Could not find module in the git repo. Did you execute `git pull`?".red());
        std::process::exit(1);
    }
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
            mustache_map_builder = read(&placeholder.0, &mut placeholder.1, mustache_map_builder);
            mustache_map_builder = mustache_map_builder
                .insert(placeholder.0, &placeholder.1)
                .expect("Could not parse object");
        }
    }
    let home_path = HOME.join(plugin_name);
    if let Ok(_) = std::fs::create_dir_all(&home_path) {
        println!("Created Plugin directory");
    }
    println!("Copying supporting files");
    if let Some(files) = &toml.supporting_files {
        mustache_map_builder = add_files_as_vars(files, mustache_map_builder, &home_path, &path_to_module, &home_path, true);
    }
    let mustache_map = mustache_map_builder.build();
    let script = render(mustache, mustache_map);
    write_file(toml, script, plugin_name, &path_to_module);
}

fn add_files_as_vars(files: &IndexMap<String, FileSystemEntry>, mut mustache_map_builder: MapBuilder, home: &PathBuf, path_to_module: &Path, cwd: &Path, should_overwrite: bool) -> MapBuilder {
    if should_overwrite {
        write_supporting_files(files, home, path_to_module, cwd);
    }
    for (place_holder, entry) in files.iter() {
        match entry {
            FileSystemEntry::File {version, path, destination} => {
                let destination = if let Some(destination) = destination {destination.to_owned().parse().expect("Could not parse path")} else { cwd.join(path)};
                
                mustache_map_builder = mustache_map_builder.insert(place_holder, &destination.to_string_lossy()).expect("Error inserting file placeholder");
            }
            FileSystemEntry::Directory { version, destination, path, files } => {
                let destination = if let Some(destination) = destination {destination.to_owned().parse().expect("Could not parse path")} else { cwd.join(path)};

                mustache_map_builder = mustache_map_builder.insert(place_holder, &destination.to_string_lossy()).expect("Error inserting file placeholder");
                mustache_map_builder = add_files_as_vars(files, mustache_map_builder, home, path_to_module, cwd, should_overwrite);
            }
        }
    }
    mustache_map_builder
}

fn write_supporting_files(files: &IndexMap<String, FileSystemEntry>, home: &PathBuf, path_to_module: &Path, cwd: &Path){
    for (entry, file) in files {
        match file {
            FileSystemEntry::File { version, path, destination } => {
                let destination = if let Some(destination) = destination {destination.to_owned().parse().expect("Could not parse path")} else { cwd.join(path)};
                if let Ok(_) = std::fs::remove_file(&destination) {
                    println!("{:?} existed, overwriting with new version: {}", destination, version);
                }
                let source_relative = path.parse::<PathBuf>().expect("Source path is invalid");
                let source = path_to_module.join(source_relative);
                if let Err(err) = std::fs::copy(
                    &source,
                    &destination
                ){
                    panic!("Could not copy file from source {:?} to {:?}\n{:?}", source, destination, err);
                }
            }
            FileSystemEntry::Directory { version, path, destination, files } => {
                let destination = if let Some(destination) = destination {destination.to_owned().parse().expect("Could not parse path")} else { cwd.join(path)};

                if let Ok(_) = std::fs::create_dir_all(&destination) {
                    println!("Created {:?} [{}]", destination, version);
                }
                write_supporting_files(files, home, path_to_module, &destination);
            }
        }
    }
}

fn get_old_script(plugin_name : &str) -> String {
    let home_path = HOME.join(plugin_name);
    std::fs::read_to_string(home_path.join("script.sh")).expect("Old script was not existent")
}

fn write_file(toml: PluginInfo,script : String, plugin_name : &str, path_to_module : &Path) {
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
    
    // if let Some(files) = &toml.supporting_files {
    //     write_supporting_files(files, &home_path, path_to_module, &home_path);
    // }
   
    if let Ok(_) =  std::fs::write(home_path.join("script.sh"), script){
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

fn render(
    mustache: mustache::Template,
    mustache_map: mustache::Data
) -> String {
    mustache.render_data_to_string(&mustache_map).expect("Could not render mustache template")
}

fn read(key: &str, entry_type: &mut EntryType, mut map_builder : MapBuilder) -> MapBuilder{
    match entry_type {
        EntryType::Value(str) => {
            read_value(key, str);
        }
        EntryType::Array(array) => {
            map_builder = read_array(key, array, map_builder);
        }
        EntryType::Object(obj) => {
            map_builder = read_object(obj, map_builder);
        }
    }
    map_builder
}

fn read_value(key: &str, str: &mut String){
    let mut prompt = TextPrompt::new(format!("{} [{}]? ", key, str));
    match task::block_on(async { prompt.run().await }) {
        Ok(Some(s)) => {
            if !s.is_empty() {
                *str = shellexpand::tilde(&s).to_string();
            } else {
                *str = shellexpand::tilde(&str).to_string();
            }
        }
        _ => std::process::exit(1),
    }
}

fn get_short_names(array : &Vec<EntryType>) -> String {
    let mut short_names : Vec<String> = vec![];
    for entry in array {
        if let EntryType::Object(obj) = entry {
            if obj.contains_key("shortName") {
                if let Some(EntryType::Value(short_name)) = obj.get("shortName") {
                    short_names.push(short_name.clone());
                }
                
            }
        }
    }
    short_names.join(" ")
}

fn read_array(key: &str, array: &mut Vec<EntryType>, mut map_builder : MapBuilder) -> MapBuilder {
    let proto_type: EntryType = array.remove(0);
    loop {
        let mut object = proto_type.clone();
        map_builder = read(key, &mut object, map_builder);
        
        array.push(object);
        let mut prompt = ConfirmPrompt::new(format!("Another one? "));
        match task::block_on(async { prompt.run().await }) {
            Ok(Some(true)) => continue,
            _ => break,
        }
    }
    let name = get_short_names(array);
    map_builder = map_builder.insert_str(format!("{}_shortNames", key), name);
    map_builder
}

fn read_object(obj: &mut IndexMap<String, EntryType>, mut map_builder : MapBuilder) -> MapBuilder {
    for mut keys in obj.iter_mut() {
        map_builder = read(keys.0, &mut keys.1, map_builder);
    }
    map_builder
}


//GIT Section
use git2::{Repository, Error, Cred, RemoteCallbacks,Commit, ObjectType};

fn get_callbacks<'a>(ssh_key : Option<PathBuf>, key_needs_pw : bool) -> RemoteCallbacks<'a>{
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(move |_url , username_from_url, allowed_types| {
        let mut username = String::from("");
        if let Some(name_from_url) = username_from_url {
            username = String::from(name_from_url);
        } else {
            if let Some(user) = text_prompt("Git Username: ") {
                username = user;
            }
        }
        if allowed_types.is_user_pass_plaintext() {
            if let Some(password) = password_prompt("Git Password: ") {
                Cred::userpass_plaintext(&username, &password)
            }
            else {
                Cred::default()
            }
        } else if allowed_types.is_ssh_key() {
            if let Some(ssh_key) = &ssh_key {
                if key_needs_pw {
                    if let Some(key_pw) = password_prompt("SSH key password: ") {
                        Cred::ssh_key(&username, None, &ssh_key, Some(&key_pw))
                    } else {
                        Cred::default()
                    }
                } else {
                    Cred::ssh_key(&username, None,&ssh_key, None)
                }
            } else {
                if let Some(key_path) = text_prompt("SSH Key Path: ") {
                    let key_path = shellexpand::tilde( &key_path).to_string();
                    GLOBAL_CONFIG.lock().unwrap().ssh_key = Some(key_path.clone());
                    let key_path = Path::new(&key_path);
                    let key_needs_pw = boolean_prompt("Is the key password protected? ");
                    GLOBAL_CONFIG.lock().unwrap().key_needs_pw = key_needs_pw;
                    if key_needs_pw {
                        if let Some(key_pw) = password_prompt("SSH key password: ") {
                            Cred::ssh_key(&username, None, key_path, Some(&key_pw))
                        } else {
                            Cred::default()
                        }
                    } else {
                        Cred::ssh_key(&username, None,key_path, None)
                    }
                } else {
                    Cred::default()
                }
            }
        }
        else {
            Cred::default()
        }
    });
    callbacks
}


fn check_out_modules_with_key(remote : &str, ssh_key : &Path) -> Result<(), Error> {
    let key_needs_pw = boolean_prompt("Does key need password? ");
    GLOBAL_CONFIG.lock().unwrap().key_needs_pw = key_needs_pw;
    GLOBAL_CONFIG.lock().unwrap().ssh_key = Some(String::from(ssh_key.to_string_lossy()));
    let callbacks = get_callbacks( Some(ssh_key.into()), false);
    check_out(remote, callbacks)?;
    Ok(())
}

fn check_out_modules_with_pw(remote : &str) -> Result<(), Error> {
    let callbacks = get_callbacks( None, false);
    check_out(remote, callbacks)?;
    Ok(())
}

fn check_out(remote : &str, callbacks : RemoteCallbacks) -> Result<(), Error> {
    let mut fo = git2::FetchOptions::new();
    fo.remote_callbacks(callbacks);

    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fo);

    if !HOME.join("git_modules").exists() {
        match std::fs::create_dir_all(HOME.join("git_modules")) {
            Ok(_) => {},
            Err(_) => {
                eprintln!("Could not create git_modules");
                std::process::exit(1);
            }
        }
    }
    builder.clone(remote, &HOME.join("git_modules"))?;
    let mut config = GLOBAL_CONFIG.lock().unwrap();
    config.git_repo = HOME.join("git_modules").to_string_lossy().to_string();
    if config.save().is_err() {
        eprintln!("{}","Could not write config".red());
    }
    Ok(())
}

fn update_modules() -> Result<(), Error> {
    let config = GLOBAL_CONFIG.lock().unwrap();
    let mut fo = git2::FetchOptions::new();
    let mut ssh_key : Option<PathBuf> = None;
    if let Some(key) = config.ssh_key.clone() {
       ssh_key = Some(Path::new(&key).into());
    } 

    let callbacks = get_callbacks(ssh_key, config.key_needs_pw);
    fo.remote_callbacks(callbacks);
    match Repository::open(shellexpand::tilde(&config.git_repo).to_string()) {
        Ok(repo) => {
            fetch_origin_master(&repo, fo)?;
            fast_forward(&repo)?;
            println!("{}", "Updated repo to newest revision".green());
        },
        Err(e) => {
            eprintln!("Could not update repo, manual update needed: {:?}", e);
        }
    }
    Ok(())
}

fn fetch_origin_master(repo: &git2::Repository, mut opts: git2::FetchOptions) -> Result<(), git2::Error> {
    repo.find_remote("origin")?.fetch(&["master"], Some(&mut opts), None)
}

fn fast_forward(repo : &Repository) -> Result<(), Error> {
    let fetch_head = repo.find_reference("FETCH_HEAD")?;
    let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)?;
    let analysis = repo.merge_analysis(&[&fetch_commit])?;
    if analysis.0.is_up_to_date() {
        Ok(())
    } else if analysis.0.is_fast_forward() {
        let refname = format!("refs/heads/{}", "master");
        let mut reference = repo.find_reference(&refname)?;
        reference.set_target(fetch_commit.id(), "Fast-Forward")?;
        repo.set_head(&refname)?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
    } else {
        Err(Error::from_str("Fast-forward only!"))
    }
}
pub fn find_last_commit(repo: &Repository) -> Result<Commit, git2::Error> {
    let obj = repo.head()?.resolve()?.peel(ObjectType::Commit)?;
    match obj.into_commit() {
        Ok(c) => Ok(c),
        Err(_) => Err(Error::from_str("commit error")),
    }
} 

#[derive(Deserialize, Serialize, Debug, PartialEq)]
struct PluginInfo {
    plugin_info: Package,
    placeholders: Option<IndexMap<String, EntryType>>,
    // interpreter: Option<Interpreter>,
    supporting_files: Option<IndexMap<String, FileSystemEntry>>
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
#[serde(untagged)]
enum FileSystemEntry {
    File { version:String, path: String, destination: Option<String>},
    Directory {version: String, path: String, destination: Option<String>, files : IndexMap<String, FileSystemEntry> }
}

// #[derive(Deserialize,Serialize,Debug,PartialEq)]
// struct Interpreter {
//     name : String,
//     install_path: String
// }

#[derive(Deserialize, Serialize, Debug, PartialEq)]
struct Package {
    author: String,
    version: String,
    help: Option<String>,
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

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
enum EntryType {
    Value(String),
    Object(IndexMap<String, EntryType>),
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
    git_repo : String,
    ssh_key : Option<String>,
    #[serde(default)]
    key_needs_pw : bool
}

impl GlobalConfig {
    fn save(&self) -> std::io::Result<()>{
        std::fs::write(self.config_path.as_path(), toml::to_string(self).unwrap())?;
        Ok(())
    }
}