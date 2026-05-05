use crate::config::PackageConfig;
use engine::AnimationDef;
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct CreateConfig {
    pub fps: u32,
}

/// Creates a FairyFlow package (a standalone package that allows to play a series of scenes)
///
/// A package is a zipped archive with the following structure:
/// ffpackage.json - A package config with information about scenes (PackageConfig)
/// scenes/<scenes>.json - Scenes in JSON (deserializible into AnimationDef)
/// images/ - Images used in scenes
pub fn create_package(
    project_path: &Path,
    scenes: &[&Path],
    create_config: CreateConfig,
    output: &Path,
) -> anyhow::Result<()> {
    let mut scene_archive_paths: Vec<String> = Vec::with_capacity(scenes.len());

    // Create the zip archive.
    let file = std::fs::File::create(output)
        .map_err(|e| anyhow::anyhow!("creating {}: {}", output.display(), e))?;
    let mut archive = zip::ZipWriter::new(file);

    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let mut image_tmp_paths: HashSet<Arc<String>> = HashSet::new();
    let mut image_map = HashMap::new();
    let mut image_paths = HashMap::new();
    let mut counter = 0;
    for (i, scene_path) in scenes.iter().enumerate() {
        let scene = std::fs::read_to_string(scene_path)?;
        let archive_path = format!("scenes/s{}.json", i);
        archive.start_file(&archive_path, options)?;
        archive.write_all(scene.as_bytes())?;
        scene_archive_paths.push(archive_path);
        let anim = AnimationDef::from_str(&scene)?;
        image_tmp_paths.clear();
        anim.collect_images(&mut image_tmp_paths);
        for image_path in &image_tmp_paths {
            let path = project_path.join(image_path.as_str());
            let archive_path = image_paths.entry(path).or_insert_with(|| {
                let image_id = counter;
                counter += 1;
                if let Some(ext) = PathBuf::from(image_path.as_str())
                    .extension()
                    .and_then(|e| e.to_str())
                {
                    format!("images/{image_id}.{ext}")
                } else {
                    format!("images/{image_id}")
                }
            });
            image_map.insert(image_path.as_str().to_string(), archive_path.clone());
        }
    }

    // Write images.
    for (source_path, archive_path) in image_paths.into_iter() {
        archive.start_file(&archive_path, options)?;
        let bytes = std::fs::read(&source_path)
            .map_err(|e| anyhow::anyhow!("reading image {}: {}", source_path.display(), e))?;
        archive.write_all(&bytes)?;
    }

    // Write ffpackage.json.
    let config = PackageConfig {
        scenes: scene_archive_paths,
        fps: create_config.fps,
        image_map,
    };
    archive.start_file("ffpackage.json", options)?;
    let config_bytes = serde_json::to_vec(&config)?;
    archive.write_all(&config_bytes)?;
    archive.finish()?;
    Ok(())
}
