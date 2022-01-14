use std::path::Path;

use colored::{ColoredString, Colorize};
use semver::Version;

use crate::models::{GlobalConfig, PluginInfo, ModuleState, UpdateReason};

// Copyright (c) 2022 Patrick Amrein <amrein@ubique.ch>
//
// This software is released under the MIT License.
// https://opensource.org/licenses/MIT

pub mod install;
pub mod update;

pub fn read_dir(global_config: &GlobalConfig, dir: &Path, base: &str) -> std::io::Result<()> {
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            let str_path = path.to_string_lossy();
            if str_path.contains(".git") {
                continue;
            }
            if path.is_dir() {
                let _ = read_dir(global_config, &path, base);
            }
            if path.ends_with("config.toml") {
                let module = dir.strip_prefix(base).unwrap();
                let module_str: ColoredString;
                let mut installed = "";
                let mut version = String::from("");
                let module_path = global_config.home.join(module);
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
                println!(
                    "{} {}{} {}",
                    module_str,
                    version.blue(),
                    repo_version.yellow(),
                    installed.blue()
                );
            }
        }
    }
    Ok(())
}

pub fn read_config(config_path: &Path) -> Result<PluginInfo, std::io::Error> {
    let toml_str = std::fs::read_to_string(config_path)?;
    if let Ok(pi) = toml::from_str(&toml_str) {
        return Ok(pi);
    }
    Err(std::io::Error::from(std::io::ErrorKind::InvalidData))
}

pub fn get_list_of_installed_modules(path: &Path, base: &str) -> std::io::Result<Vec<String>> {
    let mut out_result: Vec<String> = vec![];
    if path.is_dir() {
        for entry in path.read_dir()? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if let Ok(mut list) = get_list_of_installed_modules(&path, base) {
                    out_result.append(&mut list);
                }
            } else if path.to_string_lossy().contains("script.sh") {
                out_result.push(
                    path.strip_prefix(base)
                        .unwrap()
                        .to_string_lossy()
                        .to_string(),
                );
            }
        }
    }
    Ok(out_result)
}

pub fn update_source_file(global_config: &GlobalConfig) -> std::io::Result<()> {
    let base = &global_config.home;
    let modules = get_list_of_installed_modules(base, &base.to_string_lossy())?;
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



pub fn print_diff(left: &str, right: &str) {
    for diff in diff::lines(left, right) {
        match diff {
            diff::Result::Left(l) => println!("-{}", l.red()),
            diff::Result::Both(_, _) => {}
            diff::Result::Right(r) => println!("+{}", r.green()),
        }
    }
}

pub fn remove(global_config: &GlobalConfig, plugin_name: &str) {
    let home_path = global_config.home.join(plugin_name);
    if !home_path.exists() {
        eprintln!(
            "{}{}",
            "Could not find installed module ".red(),
            plugin_name
        );
        std::process::exit(1);
    }
    std::fs::remove_dir_all(home_path).expect("Could not remove directory");
}

pub fn check_module_state(global_config: &GlobalConfig, git_repo: &str, plugin_name: &str) -> ModuleState {
    let home_path = global_config.home.join(plugin_name);
    if !home_path.exists() {
        return ModuleState::NotInstalled;
    }
    let config = read_config(&(home_path.join("config.toml"))).expect("No config for module found");
    let new_config = read_config(&Path::new(&git_repo).join(plugin_name).join("config.toml"))
        .expect("Cannot find module");
    if let (Ok(old_version), Ok(new_version)) = (
        Version::parse(&config.plugin_info.version),
        Version::parse(&new_config.plugin_info.version),
    ) {
        if new_version > old_version {
            return ModuleState::NeedsUpdate(UpdateReason::NewVersion);
        }
    }
    if config != new_config {
        return ModuleState::NeedsUpdate(UpdateReason::TomlChanged);
    }
    let old_script = home_path.join("script.sh");
    let new_script = Path::new(&git_repo).join(plugin_name).join("template.sh");
    if (!old_script.exists() && !new_script.exists())
        || (old_script.exists() && new_script.exists())
    {
        ModuleState::UpToDate
    } else {
        ModuleState::NeedsUpdate(UpdateReason::TemplateChanged)
    }
}

pub fn get_old_script(global_config: &GlobalConfig, plugin_name: &str) -> String {
    let home_path = global_config.home.join(plugin_name);
    std::fs::read_to_string(home_path.join("script.sh")).expect("Old script was not existent")
}