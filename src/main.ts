import { Channel, invoke } from '@tauri-apps/api/core';
import { LogicalSize } from '@tauri-apps/api/dpi';
import { open } from '@tauri-apps/plugin-dialog';
import { openUrl, revealItemInDir } from '@tauri-apps/plugin-opener';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import '@phosphor-icons/web/regular';
import './styles.css';

type State = 'ready' | 'converting' | 'completed' | 'failed';
type Job = { path: string; state: State; progress: number; detail: string; size?: number; outputPath?: string };
type ProgressEvent = { path: string; state: State; progress: number; detail: string; outputPath?: string };
type FileSize = { path: string; size: number };
type Language = 'en' | 'zh-CN';
type Settings = { language: Language; outputMode: 'source' | 'shared'; outputPath: string };

const supported = new Set(['.cci', '.3ds']);
const jobs: Job[] = [];
const queue = document.querySelector<HTMLDivElement>('#queue')!;
const summary = document.querySelector<HTMLElement>('#summary')!;
const convert = document.querySelector<HTMLButtonElement>('#convert')!;
const dropZone = document.querySelector<HTMLElement>('#drop-zone')!;
const appShell = document.querySelector<HTMLElement>('.app-shell')!;
const outputPath = document.querySelector<HTMLInputElement>('#output-path')!;
const chooseFolder = document.querySelector<HTMLButtonElement>('#choose-folder')!;
const settingsDialog = document.querySelector<HTMLDialogElement>('#settings-dialog')!;
const languageInput = document.querySelector<HTMLSelectElement>('#language')!;
const browserFileInput = document.querySelector<HTMLInputElement>('#browser-file-input')!;
const isTauri = '__TAURI_INTERNALS__' in window;
const defaultSettings: Settings = { language: 'en', outputMode: 'source', outputPath: '' };
const translations = {
  en: {
    dropFiles: 'Drop .cci or .3ds files here', dropHint: 'or click anywhere to browse', dropLabel: 'Choose CCI or 3DS files',
    selectedCount: (count: number) => `${count} file${count === 1 ? '' : 's'} selected`, addMoreFiles: 'Add more files', clearBatch: 'Clear batch',
    convert: (count: number) => `Convert ${count} file${count === 1 ? '' : 's'}`, convertingBatch: 'Converting…', remove: (name: string) => `Remove ${name}`,
    showInFinder: 'Show in Finder', ready: 'Ready', converting: (progress: number) => `Converting · ${progress}%`, completed: 'Completed', failed: 'Failed', waiting: 'Waiting to convert', checking: 'Checking CCI structure', writing: 'Writing CIA content',
    settings: 'Settings', settingsDescription: 'Choose where converted files are saved and your preferred language.', language: 'Language', output: 'Output location', sameFolder: 'Same folder as source', sameFolderDescription: 'Save each .cia next to its source file.', sharedFolder: 'One shared folder', chooseFolder: 'Choose…', chooseFolderPlaceholder: 'Choose a folder',
    about: 'About', email: 'Email', website: 'Website', cancel: 'Cancel', saveSettings: 'Save settings', openSettings: 'Open settings', closeSettings: 'Close settings', outputRequired: 'Choose a shared output folder in Settings before converting.',
  },
  'zh-CN': {
    dropFiles: '将 .cci 或 .3ds 文件拖到这里', dropHint: '或点击任意位置浏览文件', dropLabel: '选择 CCI 或 3DS 文件',
    selectedCount: (count: number) => `已选择 ${count} 个文件`, addMoreFiles: '添加更多文件', clearBatch: '清空批次',
    convert: (count: number) => `转换 ${count} 个文件`, convertingBatch: '正在转换…', remove: (name: string) => `移除 ${name}`,
    showInFinder: '在访达中显示', ready: '等待中', converting: (progress: number) => `正在转换 · ${progress}%`, completed: '已完成', failed: '失败', waiting: '等待转换', checking: '正在检查 CCI 结构', writing: '正在写入 CIA 内容',
    settings: '设置', settingsDescription: '选择转换文件的保存位置和偏好语言。', language: '语言', output: '输出位置', sameFolder: '与源文件放在同一文件夹', sameFolderDescription: '将每个 .cia 文件保存到对应源文件旁。', sharedFolder: '统一保存到一个文件夹', chooseFolder: '选择…', chooseFolderPlaceholder: '选择文件夹',
    about: '关于', email: '邮箱', website: '网站', cancel: '取消', saveSettings: '保存设置', openSettings: '打开设置', closeSettings: '关闭设置', outputRequired: '请先在设置中选择统一输出文件夹。',
  },
} as const;
let settings = loadSettings();
let isConversionRunning = false;

function loadSettings(): Settings {
  try {
    const saved = JSON.parse(localStorage.getItem('ciaforge.settings') ?? '{}');
    return { ...defaultSettings, ...saved, language: saved.language === 'zh-CN' ? 'zh-CN' : 'en', outputMode: saved.outputMode === 'shared' ? 'shared' : 'source', outputPath: typeof saved.outputPath === 'string' ? saved.outputPath : '' };
  } catch { return { ...defaultSettings }; }
}
function t() { return translations[settings.language]; }
function setText(selector: string, value: string) { const element = document.querySelector<HTMLElement>(selector); if (element) element.textContent = value; }
function basename(path: string) { return path.split('/').pop() ?? path; }
function isSupported(path: string) { return supported.has(path.slice(path.lastIndexOf('.')).toLowerCase()); }
function formatFileSize(size?: number) {
  if (size === undefined) return '—';
  if (size >= 1024 ** 3) return `${(size / 1024 ** 3).toFixed(2)} GB`;
  if (size >= 1024 ** 2) return `${Math.round(size / 1024 ** 2)} MB`;
  return `${Math.max(1, Math.round(size / 1024))} KB`;
}

function render() {
  const hasJobs = jobs.length > 0;
  appShell.dataset.mode = hasJobs ? 'batch' : 'idle';
  queue.replaceChildren(...jobs.map((job) => renderJob(job)));
  const ready = jobs.filter((job) => job.state === 'ready' || job.state === 'failed').length;
  summary.textContent = t().selectedCount(jobs.length);
  convert.disabled = isConversionRunning || ready === 0;
  convert.innerHTML = `<i class="ph ph-arrows-clockwise" aria-hidden="true"></i>${isConversionRunning ? t().convertingBatch : t().convert(ready)}`;
  void syncWindowHeight(hasJobs);
}

function renderJob(job: Job) {
  const row = document.createElement('article');
  row.className = `job ${job.state}`;
  const progress = job.state === 'converting' ? `<div class="progress"><i style="width:${job.progress}%"></i></div>` : '';
  const detailText = localizedDetail(job);
  const detail = job.state === 'completed'
    ? `<button class="show-output" type="button">${t().showInFinder}</button>`
    : job.state === 'ready' ? '' : `<small>${detailText}</small>`;
  row.innerHTML = `<span class="file"><i class="ph ph-file file-icon" aria-hidden="true"></i><span><b>${basename(job.path)}</b>${detail}</span></span><span class="file-size">${formatFileSize(job.size)}</span><span class="status"><b>${label(job)}</b>${progress}</span><button class="remove-job" type="button" aria-label="${t().remove(basename(job.path))}"><i class="ph ph-x" aria-hidden="true"></i></button>`;
  row.querySelector('b')?.setAttribute('title', basename(job.path));
  row.querySelector('small')?.setAttribute('title', detailText);
  row.querySelector<HTMLButtonElement>('.remove-job')!.onclick = (event) => {
    event.stopPropagation();
    jobs.splice(jobs.indexOf(job), 1);
    render();
  };
  row.querySelector<HTMLButtonElement>('.show-output')?.addEventListener('click', async (event) => {
    event.stopPropagation();
    if (!job.outputPath) return;
    try { await revealItemInDir(job.outputPath); }
    catch (error) { job.state = 'failed'; job.detail = String(error); render(); }
  });
  return row;
}

function label(job: Job) {
  if (job.state === 'converting') return t().converting(Math.round(job.progress));
  return t()[job.state];
}
function localizedDetail(job: Job) {
  if (job.detail === 'Checking CCI structure') return t().checking;
  if (job.detail === 'Writing CIA content') return t().writing;
  return job.detail;
}

async function readFileSizes(paths: string[]) {
  if (!isTauri) return new Map<string, number>();
  try {
    const sizes = await invoke<FileSize[]>('file_sizes', { paths });
    return new Map(sizes.map(({ path, size }) => [path, size]));
  } catch { return new Map<string, number>(); }
}
async function addPaths(paths: string[], knownSizes = new Map<string, number>()) {
  const existing = new Set(jobs.map((job) => job.path));
  const accepted = paths.filter((path) => isSupported(path) && !existing.has(path));
  const fileSizes = knownSizes.size ? knownSizes : await readFileSizes(accepted);
  for (const path of accepted) jobs.push({ path, size: fileSizes.get(path), state: 'ready', progress: 0, detail: t().waiting });
  render();
}

async function startConversion() {
  if (isConversionRunning) return;
  const pending = jobs.filter((job) => job.state === 'ready' || job.state === 'failed');
  if (settings.outputMode === 'shared' && !settings.outputPath.trim()) { settingsDialog.showModal(); return; }
  if (!pending.length) return;
  isConversionRunning = true;
  render();
  const channel = new Channel<ProgressEvent>();
  channel.onmessage = (event) => {
    const job = jobs.find((candidate) => candidate.path === event.path);
    if (job) Object.assign(job, event);
    render();
  };
  try {
    await invoke('start_conversion', { requests: pending.map((job) => job.path), outputMode: settings.outputMode, outputPath: settings.outputPath || null, channel });
  } catch (error) {
    for (const job of pending) { job.state = 'failed'; job.detail = String(error); }
    render();
  } finally {
    isConversionRunning = false;
    render();
  }
}
async function chooseFiles() {
  if (!isTauri) { browserFileInput.click(); return; }
  const selected = await open({ multiple: true, filters: [{ name: 'Nintendo 3DS images', extensions: ['cci', '3ds'] }] });
  if (selected) await addPaths(Array.isArray(selected) ? selected : [selected]);
}
async function chooseOutputFolder() {
  const selected = await open({ directory: true, multiple: false });
  if (typeof selected === 'string') {
    settings = { ...settings, outputMode: 'shared', outputPath: selected };
    persistSettings();
    updateSettingsForm();
  }
}
async function syncWindowHeight(hasJobs: boolean) {
  if (!isTauri) return;
  // The queue itself stops growing at 480px and scrolls after that. Size the
  // native window from that capped queue height so the footer is always fully
  // visible, without leaving a large blank area below it.
  const queueHeight = Math.min(480, jobs.length * 76);
  const height = settingsDialog.open ? 650 : hasJobs ? Math.max(360, 280 + queueHeight) : 360;
  try { await getCurrentWebviewWindow().setSize(new LogicalSize(900, height)); } catch { /* Window sizing is cosmetic. */ }
}
function updateSettingsForm() {
  languageInput.value = settings.language;
  outputPath.value = settings.outputPath;
  document.querySelector<HTMLInputElement>(`input[name="output"][value="${settings.outputMode}"]`)!.checked = true;
  updateFolderPicker();
}
function updateFolderPicker() {
  const isShared = document.querySelector<HTMLInputElement>('input[name="output"]:checked')!.value === 'shared';
  outputPath.disabled = !isShared;
  chooseFolder.disabled = !isShared;
}
function persistSettings() { localStorage.setItem('ciaforge.settings', JSON.stringify(settings)); }
async function openExternalUrl(url: string) {
  if (isTauri) { await openUrl(url); return; }
  window.open(url, '_blank', 'noopener,noreferrer');
}
async function setOutputMode(mode: Settings['outputMode']) {
  if (mode === 'shared' && !settings.outputPath) {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected !== 'string') { updateSettingsForm(); return; }
    settings = { ...settings, outputMode: 'shared', outputPath: selected };
  } else {
    settings = { ...settings, outputMode: mode };
  }
  persistSettings();
  updateSettingsForm();
}
function applyTranslations() {
  const copy = t();
  document.documentElement.lang = settings.language;
  document.title = 'CiaForge';
  setText('#drop-zone strong', copy.dropFiles); setText('#drop-hint', copy.dropHint); dropZone.setAttribute('aria-label', copy.dropLabel);
  document.querySelector('#add-more')!.innerHTML = `<i class="ph ph-plus-circle" aria-hidden="true"></i>${copy.addMoreFiles}`;
  setText('#clear-batch', copy.clearBatch);
  setText('#settings-title', copy.settings); setText('.settings-heading p', copy.settingsDescription);
  setText('label[for="language"]', copy.language); setText('.settings-section legend', copy.output);
  const outputLabels = document.querySelectorAll<HTMLElement>('fieldset label'); outputLabels[0]!.lastChild!.textContent = ` ${copy.sameFolder}`; outputLabels[1]!.lastChild!.textContent = ` ${copy.sharedFolder}`;
  setText('fieldset p', copy.sameFolderDescription); outputPath.placeholder = copy.chooseFolderPlaceholder; setText('#choose-folder', copy.chooseFolder);
  setText('#about-title', copy.about); setText('#about-email-label', copy.email); setText('#about-website-label', copy.website);
  document.querySelector('#open-settings')!.setAttribute('aria-label', copy.openSettings); document.querySelector('#close-settings')!.setAttribute('aria-label', copy.closeSettings);
  render();
}

dropZone.addEventListener('click', chooseFiles);
dropZone.addEventListener('keydown', (event) => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); void chooseFiles(); } });
document.querySelector('#add-more')!.addEventListener('click', chooseFiles);
document.querySelector('#clear-batch')!.addEventListener('click', () => { jobs.splice(0); render(); });
chooseFolder.addEventListener('click', chooseOutputFolder);
browserFileInput.addEventListener('change', () => {
  const files = Array.from(browserFileInput.files ?? []);
  if (files.length) void addPaths(files.map((file) => `/Browser preview/${file.name}`), new Map(files.map((file) => [`/Browser preview/${file.name}`, file.size])));
  browserFileInput.value = '';
});
document.querySelector('#open-settings')!.addEventListener('click', () => { updateSettingsForm(); settingsDialog.showModal(); void syncWindowHeight(jobs.length > 0); });
document.querySelector('#close-settings')!.addEventListener('click', () => settingsDialog.close());
settingsDialog.addEventListener('close', () => { void syncWindowHeight(jobs.length > 0); });
languageInput.addEventListener('change', () => { settings = { ...settings, language: languageInput.value as Language }; persistSettings(); applyTranslations(); });
document.querySelectorAll<HTMLInputElement>('input[name="output"]').forEach((input) => input.addEventListener('change', () => { void setOutputMode(input.value as Settings['outputMode']); }));
document.querySelector('#about-email')!.addEventListener('click', () => { void openExternalUrl('mailto:me@knktc.com'); });
document.querySelector('#about-website')!.addEventListener('click', () => { void openExternalUrl('https://knktc.com'); });
convert.addEventListener('click', startConversion);
if (isTauri) getCurrentWebviewWindow().onDragDropEvent((event) => { if (event.payload.type === 'drop') void addPaths(event.payload.paths); });
applyTranslations();
