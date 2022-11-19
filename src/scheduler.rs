use std::collections::HashMap;
use std::path::Path;
use std::thread;

use std::time::{Duration, Instant};

use clokwerk::{ScheduleHandle, Scheduler, TimeUnits};
use log::info;

use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;

use crate::resource_reader::RemoteResource;
use crate::resource_store::ResourceStore;
use crate::{filesystem_client, ResourceReader};

/// Schedules the cache indexer at every day at midnight
pub fn schedule_indexer(
    resource_reader: ResourceReader,
    resource_store: ResourceStore,
) -> ScheduleHandle {
    let mut scheduler = Scheduler::new();

    let resource_reader_clone = resource_reader.clone();
    let resource_store_clone = resource_store.clone();

    // Fetch resources at midnight
    scheduler
        .every(1.day())
        .at("00:00")
        .run(move || index_resources(resource_reader.clone(), resource_store.clone()));

    // For the first time on application start
    thread::spawn(move || {
        index_resources(resource_reader_clone, resource_store_clone);
    });

    // Check the thread every minute
    scheduler.watch_thread(Duration::from_secs(60))
}

/// Fetches the resources from the configures paths and writes them to the resource store
pub fn index_resources(resource_reader: ResourceReader, resource_store: ResourceStore) {
    let s = Instant::now();
    info!("Begin resource indexing");

    info!("Purging resources store");
    resource_store.clear_resources();

    info!("Indexing resources, this may take some time depending on the amount of resources...");
    let resources: Vec<RemoteResource> = resource_reader
        .local_resource_paths
        .par_iter()
        .map(|path_str| Path::new(path_str.as_str()))
        .flat_map(filesystem_client::read_files_recursive)
        .map(|resource| filesystem_client::augment_with_exif_data(&resource))
        .collect();

    // TODO: read samba resources
    // TODO: directly insert resources when iterating filesystem

    info!("Found {} resources", resources.len());
    let map: HashMap<String, String> = resources
        .iter()
        .map(|resource| {
            (
                resource.id.clone(),
                serde_json::to_string(resource).unwrap(),
            )
        })
        .collect();

    info!("Cleanup cache");
    resource_store.clear_data_cache();

    info!("Inserting new resources");
    resource_store.add_resources(map);

    info!("Job done in {}s!", s.elapsed().as_secs());
}
