pub mod conversion;

use conversion::{ProgressSink, convert_unencrypted};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::ipc::Channel;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent {
    path: String,
    state: String,
    progress: f32,
    detail: String,
    output_path: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileSize {
    path: String,
    size: u64,
}

#[tauri::command]
fn file_sizes(paths: Vec<String>) -> Vec<FileSize> {
    paths
        .into_iter()
        .filter_map(|path| {
            std::fs::metadata(&path).ok().map(|metadata| FileSize {
                path,
                size: metadata.len(),
            })
        })
        .collect()
}

#[tauri::command]
async fn start_conversion(
    requests: Vec<String>,
    output_mode: String,
    output_path: Option<String>,
    channel: Channel<ProgressEvent>,
) -> Result<(), String> {
    for path in requests {
        let output = output_for(Path::new(&path), &output_mode, output_path.as_deref())?;
        channel
            .send(event(&path, "converting", 2.0, "Checking CCI structure"))
            .map_err(|error| error.to_string())?;
        let mut progress = ChannelProgress {
            channel: channel.clone(),
            path: path.clone(),
        };
        match convert_unencrypted(Path::new(&path), &output, &mut progress) {
            Ok(()) => channel.send(ProgressEvent {
                path: path.clone(),
                state: "completed".into(),
                progress: 100.0,
                detail: "Completed".into(),
                output_path: Some(output.display().to_string()),
            }),
            Err(error) => channel.send(event(&path, "failed", 0.0, &error.to_string())),
        }
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

struct ChannelProgress {
    channel: Channel<ProgressEvent>,
    path: String,
}

impl ProgressSink for ChannelProgress {
    fn report(&mut self, completed: u64, total: u64) {
        let ratio = if total == 0 {
            0.0
        } else {
            completed as f32 / total as f32
        };
        let _ = self.channel.send(event(
            &self.path,
            "converting",
            5.0 + ratio * 90.0,
            "Writing CIA content",
        ));
    }
}

fn event(path: &str, state: &str, progress: f32, detail: &str) -> ProgressEvent {
    ProgressEvent {
        path: path.into(),
        state: state.into(),
        progress,
        detail: detail.into(),
        output_path: None,
    }
}

fn output_for(input: &Path, mode: &str, shared_folder: Option<&str>) -> Result<PathBuf, String> {
    let stem = input
        .file_stem()
        .ok_or_else(|| format!("{} has no file name", input.display()))?;
    let folder = match mode {
        "source" | "same" => input
            .parent()
            .ok_or_else(|| format!("{} has no parent folder", input.display()))?
            .to_path_buf(),
        "shared" => PathBuf::from(
            shared_folder
                .filter(|value| !value.trim().is_empty())
                .ok_or("Choose a shared output folder")?,
        ),
        _ => return Err("Unknown output mode".into()),
    };
    let output = folder.join(stem).with_extension("cia");
    if !output.exists() {
        return Ok(output);
    }

    for suffix in 1u32.. {
        let mut name = stem.to_os_string();
        name.push(format!("_{suffix}"));
        let candidate = folder.join(name).with_extension("cia");
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    unreachable!("the output suffix counter is unbounded")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![file_sizes, start_conversion])
        .run(tauri::generate_context!())
        .expect("error while running CiaForge");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn resolves_the_source_folder_mode_used_by_the_ui() {
        assert_eq!(
            output_for(Path::new("/games/Example.3ds"), "source", None).unwrap(),
            PathBuf::from("/games/Example.cia"),
        );
    }

    #[test]
    fn resolves_a_shared_folder() {
        assert_eq!(
            output_for(
                Path::new("/games/Example.3ds"),
                "shared",
                Some("/converted")
            )
            .unwrap(),
            PathBuf::from("/converted/Example.cia"),
        );
    }

    #[test]
    fn adds_an_incrementing_suffix_when_the_target_already_exists() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("ciaforge-output-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("Example.cia"), []).unwrap();
        fs::write(root.join("Example_1.cia"), []).unwrap();

        assert_eq!(
            output_for(Path::new("/games/Example.3ds"), "shared", root.to_str()).unwrap(),
            root.join("Example_2.cia"),
        );

        fs::remove_dir_all(root).unwrap();
    }
}
