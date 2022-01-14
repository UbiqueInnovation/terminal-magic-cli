// Copyright (c) 2022 Patrick Amrein <amrein@ubique.ch>
//
// This software is released under the MIT License.
// https://opensource.org/licenses/MIT

use std::path::{Path, PathBuf};

use colored::Colorize;
//GIT Section
use git2::{Commit, Cred, Error, ObjectType, RemoteCallbacks, Repository};

use crate::{
    models::GlobalConfig,
    prompts::{boolean_prompt, password_prompt, text_prompt},
};

pub fn get_callbacks(
    global_config: &mut GlobalConfig,
    ssh_key: Option<PathBuf>,
    key_needs_pw: bool,
) -> RemoteCallbacks {
    update_git_repo_path(global_config).expect("Could not create git repo directory");
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(move |_url, username_from_url, allowed_types| {
        let mut username = String::from("");
        if let Some(name_from_url) = username_from_url {
            username = String::from(name_from_url);
        } else if let Some(user) = text_prompt("Git Username: ") {
            username = user;
        }

        if allowed_types.is_user_pass_plaintext() {
            if let Some(password) = password_prompt("Git Password: ") {
                Cred::userpass_plaintext(&username, &password)
            } else {
                Cred::default()
            }
        } else if allowed_types.is_ssh_key() {
            if let Some(ssh_key) = &ssh_key {
                if key_needs_pw {
                    if let Some(key_pw) = password_prompt("SSH key password: ") {
                        Cred::ssh_key(&username, None, ssh_key, Some(&key_pw))
                    } else {
                        Cred::default()
                    }
                } else {
                    Cred::ssh_key(&username, None, ssh_key, None)
                }
            } else if let Some(key_path) = text_prompt("SSH Key Path: ") {
                let key_path = shellexpand::tilde(&key_path).to_string();
                global_config.ssh_key = Some(key_path.clone());
                let key_path = Path::new(&key_path);
                let key_needs_pw = boolean_prompt("Is the key password protected? ");
                global_config.key_needs_pw = key_needs_pw;
                if key_needs_pw {
                    if let Some(key_pw) = password_prompt("SSH key password: ") {
                        Cred::ssh_key(&username, None, key_path, Some(&key_pw))
                    } else {
                        Cred::default()
                    }
                } else {
                    Cred::ssh_key(&username, None, key_path, None)
                }
            } else {
                Cred::default()
            }
        } else {
            Cred::default()
        }
    });
    callbacks
}

pub fn check_out_modules_with_key(
    global_config: &mut GlobalConfig,
    remote: &str,
    ssh_key: &Path,
) -> Result<(), Error> {
    let key_needs_pw = boolean_prompt("Does key need password? ");
    global_config.key_needs_pw = key_needs_pw;
    global_config.ssh_key = Some(String::from(ssh_key.to_string_lossy()));
    let git_modules = global_config.home.join("git_modules");
    let callbacks = get_callbacks(global_config, Some(ssh_key.into()), false);
    check_out(git_modules, remote, callbacks)?;
    Ok(())
}

pub fn check_out_modules_with_pw(
    global_config: &mut GlobalConfig,
    remote: &str,
) -> Result<(), Error> {
    let git_modules = global_config.home.join("git_modules");
    let callbacks = get_callbacks(global_config, None, false);
    check_out(git_modules, remote, callbacks)?;
    Ok(())
}

pub fn update_git_repo_path(global_config: &mut GlobalConfig) -> Result<(), Error> {
    if Path::new(&global_config.git_repo).exists() {
        return Ok(());
    }
    if !global_config.home.join("git_modules").exists() {
        match std::fs::create_dir_all(global_config.home.join("git_modules")) {
            Ok(_) => {}
            Err(_) => {
                eprintln!("Could not create git_modules");
                std::process::exit(1);
            }
        }
    }
    global_config.git_repo = global_config
        .home
        .join("git_modules")
        .to_string_lossy()
        .to_string();
    if global_config.save().is_err() {
        eprintln!("{}", "Could not write config".red());
    }
    Ok(())
}

pub fn check_out<P: AsRef<Path>>(
    git_modules: P,
    remote: &str,
    callbacks: RemoteCallbacks,
) -> Result<(), Error> {
    let mut fo = git2::FetchOptions::new();
    fo.remote_callbacks(callbacks);

    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fo);

    builder.clone(remote, git_modules.as_ref())?;

    Ok(())
}

pub fn update_modules(global_config: &mut GlobalConfig) -> Result<(), Error> {
    let mut fo = git2::FetchOptions::new();
    let mut ssh_key: Option<PathBuf> = None;
    if let Some(key) = global_config.ssh_key.clone() {
        ssh_key = Some(Path::new(&key).into());
    }
    let git_repo = global_config.git_repo.clone();
    let callbacks = get_callbacks(global_config, ssh_key, global_config.key_needs_pw);
    fo.remote_callbacks(callbacks);
    match Repository::open(shellexpand::tilde(&git_repo).to_string()) {
        Ok(repo) => {
            fetch_origin_master(&repo, fo)?;
            fast_forward(&repo)?;
            println!("{}", "Updated repo to newest revision".green());
        }
        Err(e) => {
            eprintln!("Could not update repo, manual update needed: {:?}", e);
        }
    }
    Ok(())
}

pub fn fetch_origin_master(
    repo: &git2::Repository,
    mut opts: git2::FetchOptions,
) -> Result<(), git2::Error> {
    repo.find_remote("origin")?
        .fetch(&["master"], Some(&mut opts), None)
}

pub fn fast_forward(repo: &Repository) -> Result<(), Error> {
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
