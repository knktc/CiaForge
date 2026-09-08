import initWasm, { convert, preview_icon, preview_publisher, preview_title } from './pkg/ciaforge_web.js'

const input = document.querySelector<HTMLInputElement>('#file')!
const action = document.querySelector<HTMLButtonElement>('#action')!
const selected = document.querySelector<HTMLElement>('#selected')!
const status = document.querySelector<HTMLElement>('#status')!
const progress = document.querySelector<HTMLProgressElement>('#progress')!
const pickerArea = document.querySelector<HTMLElement>('.picker')!
const preview = document.querySelector<HTMLElement>('#preview')!
const icon = document.querySelector<HTMLCanvasElement>('#icon')!
const title = document.querySelector<HTMLElement>('#title')!
const publisher = document.querySelector<HTMLElement>('#publisher')!
const unsupported = document.querySelector<HTMLElement>('#unsupported')!
let file: File | null = null
let wasm: Promise<unknown> | null = null

function setStatus(message: string, state = '') { status.textContent = message; status.dataset.state = state }

function clearFile() {
  file = null
  input.value = ''
  pickerArea.hidden = false
  selected.textContent = ''
  preview.hidden = true
  progress.hidden = true
  progress.value = 0
  action.textContent = 'Convert to CIA'
  action.disabled = true
  setStatus('')
}

if (!('showSaveFilePicker' in window)) { unsupported.hidden = false; input.disabled = true; setStatus('Please download the desktop app to convert this file') }

input.addEventListener('change', async () => { file = input.files?.[0] ?? null; preview.hidden = true; selected.textContent = file ? `${file.name} · ${(file.size / 1024 / 1024).toFixed(1)} MB` : ''; action.disabled = !file; action.textContent = 'Convert to CIA'; setStatus(''); if (!file) return; try { wasm ??= initWasm(); await wasm; const [name, maker, pixels] = await Promise.all([preview_title({ file }, file.name, file.size), preview_publisher({ file }, file.name, file.size), preview_icon({ file }, file.name, file.size)]); title.textContent = name || file.name; publisher.textContent = maker; drawIcon(pixels); preview.hidden = false } catch { setStatus('Metadata preview is unavailable for encrypted or unsupported images') } })

function drawIcon(bytes: Uint8Array) { const data = icon.getContext('2d')!.createImageData(48, 48); for (let y = 0; y < 48; y++) for (let x = 0; x < 48; x++) { const tile = (Math.floor(y / 8) * 6 + Math.floor(x / 8)) * 64; const px = x & 7, py = y & 7; const morton = (px & 1) | ((py & 1) << 1) | ((px & 2) << 1) | ((py & 2) << 2) | ((px & 4) << 2) | ((py & 4) << 3); const value = bytes[(tile + morton) * 2] | bytes[(tile + morton) * 2 + 1] << 8; const index = (y * 48 + x) * 4; data.data[index] = (value >> 11 & 31) * 255 / 31; data.data[index + 1] = (value >> 5 & 63) * 255 / 63; data.data[index + 2] = (value & 31) * 255 / 31; data.data[index + 3] = 255 } icon.getContext('2d')!.putImageData(data, 0, 0) }
action.addEventListener('click', async () => {
  if (!file) return
  if (action.textContent === 'Clear file') { clearFile(); return }
  const picker = (window as Window & { showSaveFilePicker?: (options: unknown) => Promise<{ createWritable(): Promise<FileSystemWritableFileStream> }> }).showSaveFilePicker
  if (!picker) { setStatus('Use a browser with local file saving support, or download the desktop app'); return }
  try { const handle = await picker({ suggestedName: `${file.name.replace(/\.(cci|3ds)$/i, '')}.cia` }); const writable = await handle.createWritable(); pickerArea.hidden = true; action.disabled = true; progress.hidden = false; setStatus('Converting…'); wasm ??= initWasm(); await wasm; await convert({ file }, { writable }, file.name, file.size, (done: number, total: number) => { progress.value = total ? done / total * 100 : 0 }); status.innerHTML = '<strong>Conversion complete</strong><span>Your CIA file has been saved to the selected location</span>'; status.dataset.state = 'success'; action.textContent = 'Clear file'; action.disabled = false } catch (error) { pickerArea.hidden = false; setStatus(error instanceof Error ? error.message : String(error)); action.disabled = false }
})
