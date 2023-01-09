use colored::*;
use dirs::home_dir;
use regex::Regex;
use semver::Version;
use std::{
    path::{Path, PathBuf},
    str::FromStr,
};
use structopt::StructOpt;
use terminal_magic::{
    git::{check_out_modules_with_key, check_out_modules_with_pw, update_modules},
    models::{GlobalConfig, PluginInfo, CONFIG_DIR},
    modules::{
        get_list_of_installed_modules, install::install, read_config, read_dir, remove,
        update::update, update_source_file,
    },
};

#[derive(StructOpt)]
#[structopt(
    name = "terminal-magic",
    author = "Systems Department @UbiqueCH",
    about = "Manage zsh stuff (aka Terminal Magic)"
)]
struct TerminalMagicArgs {
    #[structopt(
        help = "specify the git repo (on disk), where the plugins are found",
        short = "g",
        long = "git_repo"
    )]
    git_repo: Option<String>,
    #[structopt(
        help = "Clone modules from specified url (with username)",
        short = "c",
        long = "clone"
    )]
    clone: Option<String>,
    #[structopt(help = "Ssh key to clone repository", short = "s", long = "ssh_key")]
    ssh_key: Option<String>,
    #[structopt(subcommand)]
    subcommand: Option<TerminalMagicAction>,
}
#[derive(StructOpt)]
pub enum TerminalMagicAction {
    Install(InstallArgs),
    Remove(RemoveArgs),
    Update(UpdateArgs),
    List(ListArgs),
}

#[derive(StructOpt)]
#[structopt(about = "Install new extension. Use path from Git Repo as name")]
pub struct InstallArgs {
    input: String,
}

#[derive(StructOpt)]
#[structopt(about = "Update new extension.")]
pub struct UpdateArgs {
    input: String,
}

#[derive(StructOpt)]
#[structopt(about = "Remove extension. Use path from Git Repo as name")]
pub struct RemoveArgs {
    input: String,
}

#[derive(StructOpt)]
#[structopt(about = "List available modules")]
pub struct ListArgs {
    input: Option<String>,
}

fn main() {
    let cli_args = TerminalMagicArgs::from_args();

    let mut global_config = GlobalConfig::default();

    if let Some(git_repo) = cli_args.git_repo {
        global_config.git_repo = shellexpand::tilde(&git_repo).to_string();
        global_config.save().expect("Could not save global config");
    }

    let git_repo = global_config.git_repo.to_string();
    println!("Module Git Repo: {}", git_repo.green());
    println!();

    // save the config just generally everytime
    let _ = global_config.save();

    if let Some(clone_url) = &cli_args.clone {
        if let Some(ssh_key) = &cli_args.ssh_key {
            let ssh_key = Path::new(ssh_key);
            println!("{}{}", "Using key ".green(), ssh_key.to_string_lossy());
            match check_out_modules_with_key(&mut global_config, clone_url, ssh_key) {
                Ok(_) => {
                    // save the ssh key and if it needs a pw
                    let _ = global_config.save();
                    println!(
                        "{}{}{}",
                        "Clone repsitory from ".yellow(),
                        clone_url.blue(),
                        " successfully".yellow()
                    );
                }
                Err(e) => {
                    eprintln!("{}{:?}", "Could not clone module: ".red(), e);
                    std::process::exit(1);
                }
            }
        } else {
            match check_out_modules_with_pw(&mut global_config, clone_url) {
                Ok(_) => {
                    println!(
                        "{}{}{}",
                        "Clone repsitory from ".yellow(),
                        clone_url.blue(),
                        " successfully".yellow()
                    );
                }
                Err(e) => {
                    eprintln!("{}{:?}", "Could not clone module: ".red(), e);
                    std::process::exit(1);
                }
            }
        }
        std::process::exit(0);
    }
    let subcommand = if let Some(subcommand) = cli_args.subcommand {subcommand} else {
        std::process::exit(0);
    };

    match subcommand {
        TerminalMagicAction::List(list_args) => {
            if let Some(module) = list_args.input.as_ref() {
                let module_path = Path::new(module).join("script.sh");
                let base = global_config.home.join(module);

                if let Ok(installed_modules) = get_list_of_installed_modules(
                    &global_config.home,
                    &global_config.home.to_string_lossy(),
                ) {
                    let config: PluginInfo;
                    let mut updated_config: Option<PluginInfo> = None;
                    let mut installed = false;
                    if installed_modules.contains(&String::from(module_path.to_string_lossy())) {
                        config = read_config(&(base.join("config.toml")))
                            .expect("No config for module found");
                        updated_config = Some(
                            read_config(&Path::new(&git_repo).join(module).join("config.toml"))
                                .expect("Cannot find module"),
                        );
                        installed = true;
                    } else {
                        config =
                            read_config(&Path::new(&git_repo).join(module).join("config.toml"))
                                .expect("Cannot find module");
                    }
                    println!("Module {}", module.green());
                    println!("Author: {}", config.plugin_info.author.green());
                    if installed {
                        let new_version = updated_config.unwrap().plugin_info.version;
                        println!(
                            "Installed Version (Repo Version): {} ({}) ",
                            config.plugin_info.version.green(),
                            new_version.green()
                        );
                    }

                    println!();
                    if let Some(help) = config.plugin_info.help {
                        let re =
                            Regex::new(r"`(?P<color>[a-z]*)\s(?P<content>[\s\S]*?)\s*`").unwrap();
                        let mut cursor = 0;
                        for re_match in re.captures_iter(&help) {
                            print!("{}", &help[cursor..re_match.get(0).unwrap().start()]);
                            print!("{}", &re_match["content"].color(&re_match["color"]));
                            cursor = re_match.get(0).unwrap().end();
                        }
                        if cursor < help.len() - 1 {
                            print!("{}", &help[cursor..])
                        }
                        println!();
                    }

                    if let Some(internal_dependencies) = &config.plugin_info.internal_dependencies {
                        for dep in internal_dependencies {
                            let dep_path = Path::new(dep).join("script.sh");
                            if installed_modules.contains(&dep_path.to_string_lossy().to_string()) {
                                continue;
                            }
                            println!(
                                "{} {} {} {} {}",
                                "Module".yellow(),
                                dep.green(),
                                "not installed, but is listed as a dependency. Consider using"
                                    .yellow(),
                                "terminal-magic install".green().bold(),
                                dep.green()
                            );
                        }
                    }
                    println!();
                    if let Some(external_dependencies) = &config.plugin_info.external_dependencies {
                        for dep in external_dependencies {
                            println!("External Dependency {}", dep.green());
                        }
                    }
                    println!("Placeholders: ");
                    if let Some(placeholders) = &config.placeholders {
                        print!("{}", format!("{:?}", placeholders).green());
                    }
                    std::process::exit(0);
                }
            } else {
                update_modules(&mut global_config).expect("Could not update modules");
            }

            let path_to_module = Path::new(&git_repo);
            if read_dir(&global_config, path_to_module, &git_repo).is_err() {
                eprintln!("{}", "path not found".red());
            }
        }
        TerminalMagicAction::Install(install_args) => {
            install(&global_config, &git_repo, &install_args.input)
        }
        TerminalMagicAction::Update(update_args) => {
            let plugin_name = &update_args.input;
            if plugin_name == "all" {
                println!("{}\n\n", "Start updating all packages".green());
                let modules = if let Ok(modules) = get_list_of_installed_modules(
                    &global_config.home,
                    &global_config.home.to_string_lossy(),
                ) {
                    modules
                } else {
                    eprintln!("Could not get list of installed modules");
                    std::process::exit(1)
                };
                for module in modules {
                    let module = module.replace("/script.sh", "");
                    let base = global_config.home.join(&module);
                    let config = read_config(&(base.join("config.toml")))
                        .expect("No config for module found");
                    let new_config =
                        read_config(&Path::new(&git_repo).join(&module).join("config.toml"))
                            .expect("Cannot find module");
                    if let (Ok(old_version), Ok(new_version)) = (
                        Version::parse(&config.plugin_info.version),
                        Version::parse(&new_config.plugin_info.version),
                    ) {
                        if new_version > old_version
                            || config.placeholders != new_config.placeholders
                        {
                            println!(
                                "[{}] Try updating from {} to {}",
                                module.yellow(),
                                old_version,
                                new_version
                            );
                            update(&global_config, &git_repo, &module, false, true);
                        }
                    }
                }
                println!(
                    "{}",
                    "\n 🥳 All updateable packages are up to date.\n".green()
                );
            } else {
                update(&global_config, &git_repo, plugin_name, true, false);
            }
        }
        TerminalMagicAction::Remove(remove_args) => remove(&global_config, &remove_args.input),
    }

    if update_source_file(&global_config).is_err() {
        eprintln!("{}", "Could not update globals source file".red());
    } else {
        let command = format!("source ~/{}/env", CONFIG_DIR);
        let alternative_command = format!(
            "source {}",
            &global_config.home.join("env").to_string_lossy()
        );
        let zshrc_file = &home_dir()
            .unwrap_or_else(|| PathBuf::from_str("~").unwrap())
            .join(".zshrc");
        if let Ok(lines) = std::fs::read_to_string(zshrc_file) {
            if lines.contains(&command) || lines.contains(&alternative_command) {
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
