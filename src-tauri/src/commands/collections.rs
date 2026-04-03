//! Commands for creating and managing custom case collections.
//!
//! Collections allow users to group downloaded cases into logical units,
//! like "Fan-made Turnabouts" or "Justice For All Alternate Universe".
//! Collections are stored in a separate JSON file in the app data directory.

use tauri::State;

use crate::app_state::AppPaths;
use crate::collections as coll;
use crate::error::AppError;

/// List all defined collections.
///
/// Loads the collections configuration and returns the list of all collections.
#[tauri::command]
pub fn list_collections(paths: State<'_, AppPaths>) -> Result<Vec<coll::Collection>, AppError> {
    let data_dir = &paths.data_dir;
    let data = coll::load_collections(data_dir);
    Ok(data.collections)
}

/// Create a new collection.
///
/// # Arguments
///
/// * `title` - The display title for the new collection.
/// * `items` - Initial list of case items to include in the collection.
///
/// # Returns
///
/// The newly created `Collection` object with a generated unique ID.
#[tauri::command]
pub fn create_collection(
    paths: State<'_, AppPaths>,
    title: String,
    items: Vec<coll::CollectionItem>,
) -> Result<coll::Collection, AppError> {
    let data_dir = &paths.data_dir;
    let mut data = coll::load_collections(data_dir);
    let collection = coll::Collection {
        id: coll::generate_id(),
        title,
        items,
        created_date: coll::now_iso8601(),
    };
    data.collections.push(collection.clone());
    coll::save_collections(data_dir, &data)?;
    Ok(collection)
}

/// Update an existing collection's properties.
///
/// Only the provided optional fields will be modified.
///
/// # Arguments
///
/// * `id` - Unique identifier of the collection to update.
/// * `title` - (Optional) New title for the collection.
/// * `items` - (Optional) Entirely replaces the collection's items list.
///
/// # Returns
///
/// The updated `Collection` object.
#[tauri::command]
pub fn update_collection(
    paths: State<'_, AppPaths>,
    id: String,
    title: Option<String>,
    items: Option<Vec<coll::CollectionItem>>,
) -> Result<coll::Collection, AppError> {
    let data_dir = &paths.data_dir;
    let mut data = coll::load_collections(data_dir);
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
    coll::save_collections(data_dir, &data)?;
    Ok(result)
}

/// Delete a collection permanently by its ID.
///
/// Note that this only deletes the collection record; it does NOT delete
/// the actual downloaded case files from the disk.
///
/// # Errors
///
/// Returns an error if the collection with the specified ID is not found.
#[tauri::command]
pub fn delete_collection(
    paths: State<'_, AppPaths>,
    id: String,
) -> Result<(), AppError> {
    let data_dir = &paths.data_dir;
    let mut data = coll::load_collections(data_dir);
    let len_before = data.collections.len();
    data.collections.retain(|c| c.id != id);
    if data.collections.len() == len_before {
        return Err(format!("Collection {} not found", id).into());
    }
    coll::save_collections(data_dir, &data)?;
    Ok(())
}

/// Retrieve a single collection's data by its ID.
///
/// # Errors
///
/// Returns an error if the collection is not found.
#[tauri::command]
pub fn get_collection(
    paths: State<'_, AppPaths>,
    id: String,
) -> Result<coll::Collection, AppError> {
    let data_dir = &paths.data_dir;
    let data = coll::load_collections(data_dir);
    Ok(data.collections
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| format!("Collection {} not found", id))?)
}

/// Append additional items to an existing collection's list.
///
/// Unlike `update_collection`, which replaces the entire list, this
/// function adds new items to the end of the current list.
///
/// # Returns
///
/// The updated `Collection` object.
#[tauri::command]
pub fn add_to_collection(
    paths: State<'_, AppPaths>,
    id: String,
    items: Vec<coll::CollectionItem>,
) -> Result<coll::Collection, AppError> {
    let data_dir = &paths.data_dir;
    let mut data = coll::load_collections(data_dir);
    let coll = data
        .collections
        .iter_mut()
        .find(|c| c.id == id)
        .ok_or_else(|| format!("Collection {} not found", id))?;
    coll.items.extend(items);
    let result = coll.clone();
    coll::save_collections(data_dir, &data)?;
    Ok(result)
}
