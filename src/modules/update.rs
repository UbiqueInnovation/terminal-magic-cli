use std::{path::Path, process::Command};

use colored::Colorize;
use indexmap::IndexMap;
use mustache::MapBuilder;

use crate::{models::{GlobalConfig, ModuleState, EntryType, PluginType}, prompts::{boolean_prompt, read, read_array, get_short_names}, modules::print_diff, template::{add_files_as_vars, render}};

use super::{read_config, check_module_state, install::{install, write_file}, get_old_script};

// Copyright (c) 2022 Patrick Amrein <amrein@ubique.ch>
//
// This software is released under the MIT License.
// https://opensource.org/licenses/MIT

pub fn update(
    global_config: &GlobalConfig,
    git_repo: &str,
    plugin_name: &str,
    fail_on_error: bool,
    silent: bool,
) {
    let home_path = global_config.home.join(plugin_name);
    if !home_path.exists() {
        eprintln!("module is not installed");
        if fail_on_error {
            std::process::exit(1);
        }
        return;
    }
    let path_to_module = Path::new(git_repo).join(plugin_name);
    if !path_to_module.exists() {
        eprintln!(
            "{}",
            "Could not find module in the git repo. Did you execute `git pull`?".red()
        );
        if fail_on_error {
            std::process::exit(1);
        }
        return;
    }
    let mustache = mustache::compile_path(path_to_module.join("template.sh"))
        .expect("Could not parse mustache template");

    let mut toml = read_config(&home_path.join("data.toml")).expect("Cannot find TOML");
    let old_toml = toml.clone();
    let old_config = read_config(&home_path.join("config.toml"))
        .expect("Cannot find old config (maybe you did update terminal-magic)");
    let new_config =
        read_config(&path_to_module.join("config.toml")).expect("module config not found");
    let mut mustache_map_builder = MapBuilder::new();
    if old_config != new_config {
        println!("{}", "Config changed check the changes".yellow());

        let old_config_str = toml::to_string(&old_config).unwrap();
        let new_config_str = toml::to_string(&new_config).unwrap();
        print_diff(&old_config_str, &new_config_str);
        if old_config.placeholders != new_config.placeholders {
            let mut update_map = IndexMap::new();
            if let Some(new_placeholders) = &new_config.placeholders {
                for (key, entry) in new_placeholders {
                    if let Some(v) = toml.placeholders.as_ref().and_then(|a| a.get(key)) {
                        update_map.insert(key.to_owned(), v.to_owned());
                    } else {
                        // prompt new value
                        let (new_mustache_map_builder, object) =
                            read(key, entry, mustache_map_builder);
                        mustache_map_builder = new_mustache_map_builder;
                        update_map.insert(key.to_owned(), object);
                    }
                }
                toml.placeholders = Some(update_map);
            } else {
                toml.placeholders = None;
            }
        }
    }
    toml.plugin_info = new_config.plugin_info.clone();

    if let Some(internal_deps) = toml.plugin_info.internal_dependencies.as_mut() {
        for dep in internal_deps {
            match check_module_state(global_config, git_repo, dep) {
                ModuleState::NotInstalled => install(global_config, git_repo, dep),
                ModuleState::UpToDate => {}
                ModuleState::NeedsUpdate(reason) => {
                    println!("Update since: {:?}", reason);
                    update(global_config, git_repo, dep, false, true)
                }
            }
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

    if let Some(placeholders) = toml.placeholders.as_mut() {
        for placeholder in placeholders.iter_mut() {
            if old_toml
                .placeholders
                .as_ref()
                .and_then(|a| a.get(placeholder.0))
                .is_some()
            {
                if let EntryType::Array(arr) = placeholder.1 {
                    if !silent && boolean_prompt(&format!("Add new elements [{}]? ", placeholder.0))
                    {
                        if old_config
                            .placeholders
                            .as_ref()
                            .unwrap()
                            .get(placeholder.0)
                            .is_some()
                        {
                            let (new_mustache_map_builder, object) =
                                read_array(placeholder.0, &arr[0], mustache_map_builder);
                            mustache_map_builder = new_mustache_map_builder;
                            arr.extend(if let EntryType::Array(a) = object {
                                a
                            } else {
                                unreachable!("read_array MUST always return an EntryType::Array")
                            });
                        }
                    } else {
                        let name = get_short_names(arr);
                        mustache_map_builder = mustache_map_builder
                            .insert_str(format!("{}_shortNames", placeholder.0), name);
                    }
                }
            }
            mustache_map_builder = mustache_map_builder
                .insert(placeholder.0, &placeholder.1)
                .expect("Could not parse object");
        }
    }
    let should_overwrite = boolean_prompt("Update supporting files?");

    if let Some(files) = &new_config.supporting_files {
        let home_path = global_config.home.join(plugin_name);
        mustache_map_builder = add_files_as_vars(
            files,
            mustache_map_builder,
            &home_path,
            &path_to_module,
            &home_path,
            should_overwrite,
        );
    }

    let mustache_map = mustache_map_builder.build();
    let script = render(mustache, mustache_map);
    let old_script = get_old_script(global_config, plugin_name);

    print_diff(&old_script, &script);

    if !boolean_prompt("Update?") {
        return;
    }

    if let PluginType::RustPackage { path, git, tag } = new_config.plugin_info.plugin_type {
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
    write_file(global_config, toml, script, plugin_name, &path_to_module);
}
