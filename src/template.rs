use std::path::Path;

use indexmap::IndexMap;
use mustache::MapBuilder;

use crate::{models::FileSystemEntry, modules::install::write_supporting_files};

// Copyright (c) 2022 Patrick Amrein <amrein@ubique.ch>
// 
// This software is released under the MIT License.
// https://opensource.org/licenses/MIT



pub fn add_files_as_vars(
    files: &IndexMap<String, FileSystemEntry>,
    mut mustache_map_builder: MapBuilder,
    home: &Path,
    path_to_module: &Path,
    cwd: &Path,
    should_overwrite: bool,
) -> MapBuilder {
    if should_overwrite {
        write_supporting_files(files, home, path_to_module, cwd);
    }
    for (place_holder, entry) in files.iter() {
        match entry {
            FileSystemEntry::File {
                version: _version,
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

                mustache_map_builder = mustache_map_builder
                    .insert(place_holder, &destination.to_string_lossy())
                    .expect("Error inserting file placeholder");
            }
            FileSystemEntry::Directory {
                version: _version,
                destination,
                path,
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

                mustache_map_builder = mustache_map_builder
                    .insert(place_holder, &destination.to_string_lossy())
                    .expect("Error inserting file placeholder");
                mustache_map_builder = add_files_as_vars(
                    files,
                    mustache_map_builder,
                    home,
                    path_to_module,
                    cwd,
                    should_overwrite,
                );
            }
        }
    }
    mustache_map_builder
}


pub fn render(mustache: mustache::Template, mustache_map: mustache::Data) -> String {
    mustache
        .render_data_to_string(&mustache_map)
        .expect("Could not render mustache template")
}