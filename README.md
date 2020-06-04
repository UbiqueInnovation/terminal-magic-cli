# terminal-magic-cli
Organize scripts and shell extensions.

## Installation

We use `cargo` to manage the binary. So install [rustup](https://rustup.rs/#) and then continue.

Use `cargo install terminal-magic` to install the binary, or build it by yourselves with `cargo build` or `cargo install --path .` from inside the repository.

## Usage

### First time

You need to initialize the `terminal-magic` with a git repository containing modules. For that proceed as following:

- `terminal-magic --clone <git_repo_url_with_user_name> [--ssh_key <key>]` (notice that 1) if you are using a username and pw to clone you cannot use a ssh_key to update later on and 2) we currently don't support password protected ssh_key files)

- `terminal-magic list`

After that it is important to add a `source ~/.terminal-magic/env` statement to your `~/.zshrc` in order to load the terminal-magic commands.

### Configuration

All configuration (e.g. the git_repo path and the ssh_key) are saved in the `~/.terminal-magic/global_config.toml` file. You can adjust the properties at your will, as the config is read each time the CLI is run.

The default path for the git repo clone is `~/.terminal-magic/git_modules`.

### Listing modules

In order to see all available modules use the `list` command without an argument `terminal-magic list`. This will also try to update the git repo. Currently only fast-forward updates can be performed automatically.

To show the help page for a module use `terminal-magic list zsh/test`. This will show some metadata, as well as a help string, the used dependencies and the placeholders defined.

### Installing modules

To install a module you can use the `install` command. The CLI just uses the path relative to the root of the repo to find "modules". 

`terminal-magic install zsh/test`

If there are any placeholders defined in the script, the CLI asks for entries. If there is an array placeholder, the CLI adds the first element and then asks if you want to proceed adding entries.

The original config file, the script and the data are placed in the `~/.terminal-magic/zsh/test` folder (following the same path structure as in the repository).

### Updating modules

Currently updating only works via the CLI if the placeholders don't change. Use the `update` command to update a module `cargo update zsh/test`. The script will show a diff of the config, and of the expanded script which you have to acknowledge.

The update command can also be used to add new elements to an array placeholder. Though, any more advanced updates should be performed in the `data.toml` in the respective folder under `~/.terminal-magic`. This is also the place, where to perform the update manually.

