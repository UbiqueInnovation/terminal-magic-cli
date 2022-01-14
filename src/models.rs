use std::path::PathBuf;

use colored::Colorize;
use dirs::home_dir;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

// Copyright (c) 2022 Patrick Amrein <amrein@ubique.ch>
//
// This software is released under the MIT License.
// https://opensource.org/licenses/MIT

pub enum ModuleState {
    NotInstalled,
    UpToDate,
    NeedsUpdate(UpdateReason),
}
#[derive(Debug, Clone, Copy)]
pub enum UpdateReason {
    TomlChanged,
    TemplateChanged,
    NewVersion,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct PluginInfo {
    pub plugin_info: Package,
    pub placeholders: Option<IndexMap<String, EntryType>>,
    // interpreter: Option<Interpreter>,
    pub supporting_files: Option<IndexMap<String, FileSystemEntry>>,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
#[serde(untagged)]
pub enum FileSystemEntry {
    Directory {
        version: String,
        path: String,
        destination: Option<String>,
        files: IndexMap<String, FileSystemEntry>,
    },
    File {
        version: String,
        path: String,
        destination: Option<String>,
    },
}

// #[derive(Deserialize,Serialize,Debug,PartialEq)]
// struct Interpreter {
//     name : String,
//     install_path: String
// }

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct Package {
    pub author: String,
    pub version: String,
    pub help: Option<String>,
    pub internal_dependencies: Option<Vec<String>>,
    pub external_dependencies: Option<Vec<String>>,
    pub plugin_type: PluginType,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
#[serde(untagged)]
pub enum PluginType {
    Shell(String),
    Script(String),
    RustPackage {
        path: Option<String>,
        git: Option<String>,
        tag: Option<String>,
    },
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum EntryType {
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
pub struct GlobalConfig {
    pub config_path: PathBuf,
    pub git_repo: String,
    pub ssh_key: Option<String>,
    #[serde(default)]
    pub key_needs_pw: bool,
    #[serde(default = "default_home")]
    pub home: PathBuf,
}

fn default_home() -> PathBuf {
    home_dir().expect("Could not find HOME").join(CONFIG_DIR)
}

pub static CONFIG_DIR: &str = ".terminal-magic";

impl GlobalConfig {
    pub fn new() -> Self {
        let home = home_dir().expect("Could not find HOME").join(CONFIG_DIR);
        let config_dir = home.join("global_config.toml");
        let res: GlobalConfig;
        if config_dir.exists() {
            res = toml::from_str(
                &std::fs::read_to_string(&config_dir).expect("Could not find global config"),
            )
            .expect("cannot parse config");
        } else {
            if !config_dir.exists() {
                std::fs::create_dir_all(&config_dir).expect("Could not create config dir");
            }
            res = Self {
                config_path: config_dir,
                git_repo: String::from(""),
                ssh_key: None,
                key_needs_pw: false,
                home,
            };
            if res.save().is_err() {
                eprintln!("{}", "Could not write config".red());
            }
        }
        res
    }
    pub fn save(&self) -> std::io::Result<()> {
        std::fs::write(self.config_path.as_path(), toml::to_string(self).unwrap())?;
        Ok(())
    }
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self::new()
    }
}
