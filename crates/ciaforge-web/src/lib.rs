use ciaforge_core::{
    ConversionError, ProgressSink, RandomAccessInput, RandomAccessOutput, convert_async,
    inspect_async, prepare_async,
};
use js_sys::{Function, Uint8Array};
use wasm_bindgen::{JsValue, prelude::*};

#[wasm_bindgen(module = "/wasm-host.js")]
extern "C" {
    #[wasm_bindgen(catch, js_name = readAt)]
    async fn read_at(host: &JsValue, offset: f64, length: u32) -> Result<Uint8Array, JsValue>;
    #[wasm_bindgen(catch, js_name = writeAt)]
    async fn write_at(host: &JsValue, offset: f64, bytes: Uint8Array) -> Result<(), JsValue>;
    #[wasm_bindgen(catch)]
    async fn finish(host: &JsValue) -> Result<(), JsValue>;
}

struct Input(JsValue);
impl RandomAccessInput for Input {
    async fn read_at(&mut self, offset: u64, buffer: &mut [u8]) -> std::io::Result<()> {
        let bytes = read_at(&self.0, offset as f64, buffer.len() as u32)
            .await
            .map_err(js_error)?;
        if bytes.length() as usize != buffer.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "short browser file read",
            ));
        }
        bytes.copy_to(buffer);
        Ok(())
    }
}

struct Output(JsValue);
impl RandomAccessOutput for Output {
    async fn write_at(&mut self, offset: u64, bytes: &[u8]) -> std::io::Result<()> {
        let view = Uint8Array::new_with_length(bytes.len() as u32);
        view.copy_from(bytes);
        write_at(&self.0, offset as f64, view)
            .await
            .map_err(js_error)
    }
    async fn finish(&mut self) -> std::io::Result<()> {
        finish(&self.0).await.map_err(js_error)
    }
}

struct Progress(Function);
impl ProgressSink for Progress {
    fn report(&mut self, completed: u64, total: u64) {
        let _ = self.0.call2(
            &JsValue::NULL,
            &JsValue::from_f64(completed as f64),
            &JsValue::from_f64(total as f64),
        );
    }
}

#[wasm_bindgen]
pub async fn convert(
    input: JsValue,
    output: JsValue,
    input_name: String,
    input_len: f64,
    on_progress: Function,
) -> Result<(), JsValue> {
    let mut input = Input(input);
    let mut output = Output(output);
    let mut progress = Progress(on_progress);
    convert_async(
        &mut input,
        &input_name,
        input_len as u64,
        &mut output,
        &mut progress,
    )
    .await
    .map_err(conversion_error)
}

#[wasm_bindgen]
pub async fn preview_title(
    input: JsValue,
    input_name: String,
    input_len: f64,
) -> Result<String, JsValue> {
    let game = load_preview(input, &input_name, input_len as u64).await?;
    Ok(smdh_text(&game.smdh, 0x08 + 0x200, 0x80))
}

#[wasm_bindgen]
pub async fn preview_publisher(
    input: JsValue,
    input_name: String,
    input_len: f64,
) -> Result<String, JsValue> {
    let game = load_preview(input, &input_name, input_len as u64).await?;
    Ok(smdh_text(&game.smdh, 0x08 + 0x200 + 0x180, 0x80))
}

#[wasm_bindgen]
pub async fn preview_icon(
    input: JsValue,
    input_name: String,
    input_len: f64,
) -> Result<Vec<u8>, JsValue> {
    let game = load_preview(input, &input_name, input_len as u64).await?;
    Ok(game.smdh[0x24c0..0x36c0].to_vec())
}

async fn load_preview(
    input: JsValue,
    input_name: &str,
    input_len: u64,
) -> Result<ciaforge_core::PreparedGame, JsValue> {
    let mut input = Input(input);
    let header = inspect_async(&mut input, input_name, input_len)
        .await
        .map_err(conversion_error)?;
    prepare_async(&mut input, header.game, header.encryption, input_name)
        .await
        .map_err(conversion_error)
}

fn smdh_text(smdh: &[u8], offset: usize, len: usize) -> String {
    let units = smdh[offset..offset + len]
        .chunks_exact(2)
        .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
        .take_while(|unit| *unit != 0)
        .collect::<Vec<_>>();
    String::from_utf16_lossy(&units)
}

fn js_error(value: JsValue) -> std::io::Error {
    std::io::Error::other(
        value
            .as_string()
            .unwrap_or_else(|| "browser I/O failed".into()),
    )
}
fn conversion_error(error: ConversionError) -> JsValue {
    JsValue::from_str(&error.to_string())
}
