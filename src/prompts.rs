use async_std::task;
use indexmap::IndexMap;
use mustache::MapBuilder;
use prompts::{confirm::ConfirmPrompt, Prompt, text::TextPrompt};

use crate::models::EntryType;

// Copyright (c) 2022 Patrick Amrein <amrein@ubique.ch>
// 
// This software is released under the MIT License.
// https://opensource.org/licenses/MIT


pub fn boolean_prompt(prompt_string: &str) -> bool {
    let mut prompt = ConfirmPrompt::new(prompt_string);
    match task::block_on(async { prompt.run().await }) {
        Ok(Some(val)) => val,
        _ => std::process::exit(1),
    }
}
pub fn text_prompt(prompt_string: &str) -> Option<String> {
    let mut prompt = TextPrompt::new(prompt_string);
    match task::block_on(async { prompt.run().await }) {
        Ok(val) => val,
        _ => None,
    }
}
pub fn password_prompt(prompt_string: &str) -> Option<String> {
    let mut prompt = TextPrompt::new(prompt_string).with_style(prompts::text::Style::Password);
    match task::block_on(async { prompt.run().await }) {
        Ok(val) => val,
        _ => None,
    }
}


pub fn read(key: &str, entry_type: &EntryType, map_builder: MapBuilder) -> (MapBuilder, EntryType) {
    match entry_type {
        EntryType::Value(str) => (map_builder, read_value(key, str)),
        EntryType::Array(array) => read_array(key, &array[0], map_builder),
        EntryType::Object(obj) => read_object(obj, map_builder),
    }
}

pub fn read_value(key: &str, str: &str) -> EntryType {
    let mut prompt = TextPrompt::new(format!("{} [{}]? ", key, str));
    match task::block_on(async { prompt.run().await }) {
        Ok(Some(s)) => {
            if !s.is_empty() {
                EntryType::Value(shellexpand::tilde(&s).to_string())
            } else {
                EntryType::Value(shellexpand::tilde(&str).to_string())
            }
        }
        _ => std::process::exit(1),
    }
}

pub fn get_short_names(array: &[EntryType]) -> String {
    let mut short_names: Vec<String> = vec![];
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

pub fn read_array(
    key: &str,
    proto_type: &EntryType,
    mut map_builder: MapBuilder,
) -> (MapBuilder, EntryType) {
    let mut new_array = vec![];
    loop {
        let object = proto_type.clone();
        let (new_map_builder, object_to_insert) = read(key, &object, map_builder);
        map_builder = new_map_builder;
        new_array.push(object_to_insert);
        let mut prompt = ConfirmPrompt::new("Another one? ");
        match task::block_on(async { prompt.run().await }) {
            Ok(Some(true)) => continue,
            _ => break,
        }
    }
    let name = get_short_names(&new_array);
    map_builder = map_builder.insert_str(format!("{}_shortNames", key), name);
    (map_builder, EntryType::Array(new_array))
}

pub fn read_object(
    obj: &IndexMap<String, EntryType>,
    mut map_builder: MapBuilder,
) -> (MapBuilder, EntryType) {
    let mut new_obj = IndexMap::new();
    for keys in obj.iter() {
        let (new_map_builder, object) = read(keys.0, keys.1, map_builder);
        map_builder = new_map_builder;
        new_obj.insert(keys.0.to_string(), object);
    }
    (map_builder, EntryType::Object(new_obj))
}