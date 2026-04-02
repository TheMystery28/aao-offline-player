use std::sync::Mutex;
use tauri::State;

use crate::app_state::{AppState, AppStateLock};
use crate::collections as coll;
use crate::error::AppError;

/// List all collections.
#[tauri::command]
pub fn list_collections(state: State<'_, Mutex<AppState>>) -> Result<Vec<coll::Collection>, AppError> {
    let data_dir = state.data_dir()?;
    let data = coll::load_collections(&data_dir);
    Ok(data.collections)
}

/// Create a new collection with the given title and initial items.
#[tauri::command]
pub fn create_collection(
    state: State<'_, Mutex<AppState>>,
    title: String,
    items: Vec<coll::CollectionItem>,
) -> Result<coll::Collection, AppError> {
    let data_dir = state.data_dir()?;
    let mut data = coll::load_collections(&data_dir);
    let collection = coll::Collection {
        id: coll::generate_id(),
        title,
        items,
        created_date: coll::now_iso8601(),
    };
    data.collections.push(collection.clone());
    coll::save_collections(&data_dir, &data)?;
    Ok(collection)
}

/// Update a collection's title and/or items. Only provided fields are changed.
#[tauri::command]
pub fn update_collection(
    state: State<'_, Mutex<AppState>>,
    id: String,
    title: Option<String>,
    items: Option<Vec<coll::CollectionItem>>,
) -> Result<coll::Collection, AppError> {
    let data_dir = state.data_dir()?;
    let mut data = coll::load_collections(&data_dir);
    let coll = data
        .collections
        .iter_mut()
        .find(|c| c.id == id)
        .ok_or_else(|| format!("Collection {} not found", id))?;
    if let Some(t) = title {
        coll.title = t;
    }
    if let Some(i) = items {
        coll.items = i;
    }
    let result = coll.clone();
    coll::save_collections(&data_dir, &data)?;
    Ok(result)
}

/// Delete a collection by ID. Does not delete the underlying cases.
#[tauri::command]
pub fn delete_collection(
    state: State<'_, Mutex<AppState>>,
    id: String,
) -> Result<(), AppError> {
    let data_dir = state.data_dir()?;
    let mut data = coll::load_collections(&data_dir);
    let len_before = data.collections.len();
    data.collections.retain(|c| c.id != id);
    if data.collections.len() == len_before {
        return Err(format!("Collection {} not found", id).into());
    }
    coll::save_collections(&data_dir, &data)?;
    Ok(())
}

/// Get a single collection by ID.
#[tauri::command]
pub fn get_collection(
    state: State<'_, Mutex<AppState>>,
    id: String,
) -> Result<coll::Collection, AppError> {
    let data_dir = state.data_dir()?;
    let data = coll::load_collections(&data_dir);
    Ok(data.collections
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| format!("Collection {} not found", id))?)
}

/// Append items to an existing collection.
#[tauri::command]
pub fn add_to_collection(
    state: State<'_, Mutex<AppState>>,
    id: String,
    items: Vec<coll::CollectionItem>,
) -> Result<coll::Collection, AppError> {
    let data_dir = state.data_dir()?;
    let mut data = coll::load_collections(&data_dir);
    let coll = data
        .collections
        .iter_mut()
        .find(|c| c.id == id)
        .ok_or_else(|| format!("Collection {} not found", id))?;
    coll.items.extend(items);
    let result = coll.clone();
    coll::save_collections(&data_dir, &data)?;
    Ok(result)
}
