use std::{path::{Path, PathBuf}, process::Command};

use colored::Colorize;
use indexmap::IndexMap;
use mustache::MapBuilder;

use crate::{modules::{update::update, read_config}, models::{GlobalConfig, PluginType, FileSystemEntry, PluginInfo}, prompts::read, template::{add_files_as_vars, render}};

// Copyright (c) 2022 Patrick Amrein <amrein@ubique.ch>
// 
// This software is released under the MIT License.
// https://opensource.org/licenses/MIT


pub fn install(global_config: &GlobalConfig, git_repo: &str, plugin_name: &str) {
    let home_path = global_config.home.join(plugin_name);
    if home_path.exists() {
        update(global_config, git_repo, plugin_name, false, true);
        return;
    }
    let path_to_module = Path::new(git_repo).join(plugin_name);
    if !path_to_module.exists() {
        eprintln!(
            "{}",
            "Could not find module in the git repo. Did you execute `git pull`?".red()
        );
        std::process::exit(1);
    }
    let mustache = mustache::compile_path(path_to_module.join("template.sh"))
        .expect("Could not parse mustache template");

    let mut toml = read_config(&path_to_module.join("config.toml")).expect("Cannot find TOML");
    if let Some(internal_deps) = toml.plugin_info.internal_dependencies.as_mut() {
        for dep in internal_deps {
            install(global_config, git_repo, dep);
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
            println!("Read {}", placeholder.0);
            let (new_mustache_map_builder, object) =
                read(placeholder.0, placeholder.1, mustache_map_builder);
            mustache_map_builder = new_mustache_map_builder;

            mustache_map_builder = mustache_map_builder
                .insert(placeholder.0, &object)
                .expect("Could not parse object");
            *placeholder.1 = object;
        }
    }
    let home_path = global_config.home.join(plugin_name);
    if std::fs::create_dir_all(&home_path).is_ok() {
        println!("Created Plugin directory");
    }
    println!("Copying supporting files");
    if let Some(files) = &toml.supporting_files {
        mustache_map_builder = add_files_as_vars(
            files,
            mustache_map_builder,
            &home_path,
            &path_to_module,
            &home_path,
            true,
        );
    }
    if let PluginType::RustPackage { path, git, tag } = &toml.plugin_info.plugin_type {
        let mut install_command = Command::new("cargo");
        if let Some(git) = git {
            install_command
                .env("CARGO_NET_GIT_FETCH_WITH_CLI", "true")
                .arg("install")
                .arg("--git")
                .arg(git);
            if let Some(tag) = tag {
                install_command.arg("--tag").arg(tag);
            }
        } else if let Some(path) = path {
            install_command
                .arg("install")
                .arg("--path")
                .arg(&path_to_module.join(path));
        } else {
            panic!("either path or git should be set");
        };
        match install_command
            .spawn()
            .expect("Could not install module")
            .wait_with_output()
        {
            Ok(_) => println!("Successfully installed rust-module"),
            Err(err) => panic!("{:?}", err),
        }
    }

    let mustache_map = mustache_map_builder.build();
    let script = render(mustache, mustache_map);
    write_file(global_config, toml, script, plugin_name, &path_to_module);
}


pub fn write_supporting_files(
    files: &IndexMap<String, FileSystemEntry>,
    home: &Path,
    path_to_module: &Path,
    cwd: &Path,
) {
    for (_, file) in files {
        match file {
            FileSystemEntry::File {
                version,
                path,
                destination,
            } => {
                let destination = if let Some(destination) = destination {
                    destination
                        .to_owned()
                        .parse()
                        .expect("Could not parse path")
                } else {
                    cwd.join(path)
                };
                if std::fs::remove_file(&destination).is_ok() {
                    println!(
                        "{:?} existed, overwriting with new version: {}",
                        destination, version
                    );
                }
                let source_relative = path.parse::<PathBuf>().expect("Source path is invalid");
                let source = path_to_module.join(source_relative);
                if let Err(err) = std::fs::copy(&source, &destination) {
                    panic!(
                        "Could not copy file from source {:?} to {:?}\n{:?}",
                        source, destination, err
                    );
                }
            }
            FileSystemEntry::Directory {
                version,
                path,
                destination,
                files,
            } => {
                let destination = if let Some(destination) = destination {
                    destination
                        .to_owned()
                        .parse()
                        .expect("Could not parse path")
                } else {
                    cwd.join(path)
                };

                if std::fs::create_dir_all(&destination).is_ok() {
                    println!("Created {:?} [{}]", destination, version);
                }
                write_supporting_files(files, home, path_to_module, &destination);
            }
        }
    }
}


pub fn write_file(global_config: &GlobalConfig, toml: PluginInfo, script: String, plugin_name: &str, path_to_module: &Path) {
    let home_path = global_config.home.join(plugin_name);
    if std::fs::create_dir_all(&home_path).is_ok() {
        println!("Created directory");
    }
    if std::fs::remove_file(home_path.join("script.sh")).is_ok() {
        println!("script.sh already existed");
    }
    if std::fs::copy(
        path_to_module.join("config.toml"),
        home_path.join("config.toml"),
    )
    .is_ok()
    {}

    // if let Some(files) = &toml.supporting_files {
    //     write_supporting_files(files, &home_path, path_to_module, &home_path);
    // }

    if std::fs::write(home_path.join("script.sh"), script).is_ok() {
        if std::fs::remove_file(home_path.join("data.toml")).is_ok() {
            println!("data.toml File existed");
        }
        if std::fs::write(
            home_path.join("data.toml"),
            toml::to_vec(&toml).expect("could not serialize data"),
        )
        .is_ok()
        {
            println!("Successfully wrote plugin {}!", plugin_name);
        }
    }
}