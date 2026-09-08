use std::{
    fs::{self, File, OpenOptions},
    io::BufReader,
    path::Path,
};

use ciaforge_core::{ConversionError, ProgressSink, convert};

pub fn convert_file<P: ProgressSink>(
    input: &Path,
    output: &Path,
    progress: &mut P,
) -> Result<(), ConversionError> {
    if output.exists() {
        return Err(ConversionError::AlreadyExists {
            path: output.display().to_string(),
        });
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|e| ConversionError::io(parent.display(), e))?;
    }
    let file = File::open(input).map_err(|e| ConversionError::io(input.display(), e))?;
    let input_len = file
        .metadata()
        .map_err(|e| ConversionError::io(input.display(), e))?
        .len();
    let temporary = output.with_extension(format!(
        "{}.ciaforge-partial",
        output.extension().and_then(|v| v.to_str()).unwrap_or("cia")
    ));
    if temporary.exists() {
        let _ = fs::remove_file(&temporary);
    }
    let result = (|| {
        let mut reader = BufReader::new(file);
        let mut writer = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| ConversionError::io(temporary.display(), e))?;
        convert(
            &mut reader,
            &input.display().to_string(),
            input_len,
            &mut writer,
            progress,
        )
    })();
    if result.is_ok() {
        fs::rename(&temporary, output).map_err(|e| ConversionError::io(output.display(), e))?;
    } else {
        let _ = fs::remove_file(&temporary);
    }
    result
}
