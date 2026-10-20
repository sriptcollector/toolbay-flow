//! Local AI engines: model catalog, downloader, and the whisper.cpp / llama.cpp
//! servers that run on 127.0.0.1 only. Nothing here talks to anything except the
//! download hosts (Hugging Face, GitHub Releases) when you ask for a download.

use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use parking_lot::Mutex;
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[derive(Clone, Debug, Serialize)]
pub struct ModelInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: &'static str, // "speech" | "llm"
    pub file: &'static str,
    pub url: &'static str,
    pub size_mb: u32,
    pub english_only: bool,
    pub note: &'static str,
}

const HF_WHISPER: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/";

macro_rules! speech {
    ($id:expr, $label:expr, $file:expr, $mb:expr, $en:expr, $note:expr) => {
        ModelInfo { id: $id, label: $label, kind: "speech", file: $file, url: concat!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/", $file), size_mb: $mb, english_only: $en, note: $note }
    };
}

pub const MODELS: &[ModelInfo] = &[
    speech!("tiny.en", "Tiny (English)", "ggml-tiny.en.bin", 78, true, "Fastest, lowest accuracy"),
    speech!("tiny", "Tiny (multilingual)", "ggml-tiny.bin", 78, false, "Fastest, lowest accuracy"),
    speech!("base.en", "Base (English)", "ggml-base.en.bin", 148, true, "Fast on any CPU"),
    speech!("base", "Base (multilingual)", "ggml-base.bin", 148, false, "Fast on any CPU"),
    speech!("small.en", "Small (English)", "ggml-small.en.bin", 488, true, "Good accuracy, fine on modern CPUs"),
    speech!("small", "Small (multilingual)", "ggml-small.bin", 488, false, "Good accuracy, 99 languages"),
    speech!("medium.en", "Medium (English)", "ggml-medium.en-q5_0.bin", 539, true, "High accuracy, GPU recommended"),
    speech!("large-v3-turbo-q5", "Large v3 Turbo (quantized)", "ggml-large-v3-turbo-q5_0.bin", 574, false, "Best speed/accuracy with a GPU, 99 languages"),
    speech!("large-v3-turbo", "Large v3 Turbo", "ggml-large-v3-turbo.bin", 1625, false, "Best accuracy with a GPU, 99 languages"),
    ModelInfo {
        id: "qwen3-1.7b",
        label: "Qwen3 1.7B (fast)",
        kind: "llm",
        file: "Qwen3-1.7B-Q4_K_M.gguf",
        url: "https://huggingface.co/unsloth/Qwen3-1.7B-GGUF/resolve/main/Qwen3-1.7B-Q4_K_M.gguf",
        size_mb: 1107,
        english_only: false,
        note: "Light cleanup model for CPU-only machines",
    },
    ModelInfo {
        id: "qwen3-4b-instruct",
        label: "Qwen3 4B Instruct 2507 (recommended)",
        kind: "llm",
        file: "Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
        url: "https://huggingface.co/unsloth/Qwen3-4B-Instruct-2507-GGUF/resolve/main/Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
        size_mb: 2497,
        english_only: false,
        note: "Best local cleanup quality, fast with a GPU",
    },
];

#[allow(dead_code)]
const _HF: &str = HF_WHISPER;

pub fn model(id: &str) -> Option<&'static ModelInfo> {
    MODELS.iter().find(|m| m.id == id)
}

/// Runtime binaries. Windows uses official ggml-org builds; macOS whisper is built by
/// this repo's CI (Metal) and attached to our own GitHub release.
#[derive(Clone, Debug, Serialize)]
pub struct EngineInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub url: &'static str,
    pub size_mb: u32,
    pub exe: &'static str,
    pub gpu: bool,
}

#[cfg(windows)]
pub const ENGINES: &[EngineInfo] = &[
    EngineInfo { id: "whisper-cpu", label: "Speech engine (CPU)", url: "https://github.com/ggml-org/whisper.cpp/releases/download/b5454/whisper-bin-x64.zip", size_mb: 9, exe: "whisper-server.exe", gpu: false },
    EngineInfo { id: "whisper-cuda", label: "Speech engine (NVIDIA GPU)", url: "https://github.com/ggml-org/whisper.cpp/releases/download/b5454/whisper-bin-win-cuda-12.4.0-x64.zip", size_mb: 685, exe: "whisper-server.exe", gpu: true },
    EngineInfo { id: "llama-cpu", label: "Cleanup engine (CPU)", url: "https://github.com/ggml-org/llama.cpp/releases/download/b11748/llama-b11748-bin-win-cpu-x64.zip", size_mb: 24, exe: "llama-server.exe", gpu: false },
    EngineInfo { id: "llama-vulkan", label: "Cleanup engine (GPU, Vulkan)", url: "https://github.com/ggml-org/llama.cpp/releases/download/b11748/llama-b11748-bin-win-vulkan-x64.zip", size_mb: 37, exe: "llama-server.exe", gpu: true },
];

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub const ENGINES: &[EngineInfo] = &[
    EngineInfo { id: "whisper-metal", label: "Speech engine (Metal)", url: "https://github.com/sriptcollector/toolbay-flow/releases/download/engines-v1/whisper-server-macos-arm64.tar.gz", size_mb: 4, exe: "whisper-server", gpu: true },
    EngineInfo { id: "llama-metal", label: "Cleanup engine (Metal)", url: "https://github.com/ggml-org/llama.cpp/releases/download/b11748/llama-b11748-bin-macos-arm64.tar.gz", size_mb: 16, exe: "llama-server", gpu: true },
];

#[cfg(all(target_os = "macos", not(target_arch = "aarch64")))]
pub const ENGINES: &[EngineInfo] = &[
    EngineInfo { id: "whisper-mac-intel", label: "Speech engine", url: "https://github.com/sriptcollector/toolbay-flow/releases/download/engines-v1/whisper-server-macos-x86_64.tar.gz", size_mb: 4, exe: "whisper-server", gpu: true },
    EngineInfo { id: "llama-mac-intel", label: "Cleanup engine", url: "https://github.com/ggml-org/llama.cpp/releases/download/b11748/llama-b11748-bin-macos-x64.tar.gz", size_mb: 12, exe: "llama-server", gpu: true },
];

#[cfg(not(any(windows, target_os = "macos")))]
pub const ENGINES: &[EngineInfo] = &[];

pub fn has_nvidia() -> bool {
    #[cfg(windows)]
    {
        std::path::Path::new(r"C:\Windows\System32\nvcuda.dll").exists()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

pub fn pick_engine(kind: &str, use_gpu: bool) -> Option<&'static EngineInfo> {
    let cands: Vec<&EngineInfo> = ENGINES.iter().filter(|e| e.id.starts_with(kind)).collect();
    if use_gpu {
        let gpu = cands.iter().find(|e| e.gpu && (e.id != "whisper-cuda" || has_nvidia()));
        if let Some(g) = gpu {
            return Some(g);
        }
    }
    cands.into_iter().find(|e| !e.gpu).or_else(|| ENGINES.iter().find(|e| e.id.starts_with(kind)))
}

pub struct Paths {
    pub models: PathBuf,
    pub engines: PathBuf,
}

impl Paths {
    pub fn new(data: &Path) -> Self {
        let p = Paths { models: data.join("models"), engines: data.join("engines") };
        let _ = std::fs::create_dir_all(&p.models);
        let _ = std::fs::create_dir_all(&p.engines);
        p
    }
    pub fn model_path(&self, m: &ModelInfo) -> PathBuf {
        self.models.join(m.file)
    }
    /// Versioned by the package URL, so a new engine build never mixes with an old one.
    pub fn engine_dir(&self, e: &EngineInfo) -> PathBuf {
        let pkg = e.url.rsplit('/').take(2).collect::<Vec<_>>().join("-");
        let pkg: String = pkg.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' }).collect();
        self.engines.join(e.id).join(pkg)
    }
    pub fn engine_exe(&self, e: &EngineInfo) -> Option<PathBuf> {
        find_file(&self.engine_dir(e), e.exe)
    }
}

fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    let rd = std::fs::read_dir(dir).ok()?;
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if let Some(f) = find_file(&p, name) {
                return Some(f);
            }
        } else if p.file_name().map(|n| n.to_string_lossy().eq_ignore_ascii_case(name)).unwrap_or(false) {
            return Some(p);
        }
    }
    None
}

#[derive(Clone, Serialize)]
struct Progress<'a> {
    id: &'a str,
    done: u64,
    total: u64,
    stage: &'a str,
}

/// Streams a URL to `dest` with progress events, via a .part file.
pub async fn download(app: &AppHandle, id: &str, url: &str, dest: &Path) -> Result<()> {
    let client = reqwest::Client::builder().user_agent("ToolbayFlow/0.1").build()?;
    let resp = client.get(url).send().await?.error_for_status().with_context(|| format!("download {url}"))?;
    let total = resp.content_length().unwrap_or(0);
    let part = dest.with_extension("part");
    let mut f = std::fs::File::create(&part)?;
    let mut done = 0u64;
    let mut last = std::time::Instant::now();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        f.write_all(&chunk)?;
        done += chunk.len() as u64;
        if last.elapsed() > Duration::from_millis(150) {
            last = std::time::Instant::now();
            let _ = app.emit("download-progress", Progress { id, done, total, stage: "downloading" });
        }
    }
    f.flush()?;
    drop(f);
    if total > 0 && done != total {
        return Err(anyhow!("download incomplete ({done}/{total} bytes)"));
    }
    std::fs::rename(&part, dest)?;
    let _ = app.emit("download-progress", Progress { id, done, total, stage: "done" });
    Ok(())
}

pub async fn ensure_model(app: &AppHandle, paths: &Paths, id: &str) -> Result<PathBuf> {
    let m = model(id).ok_or_else(|| anyhow!("unknown model {id}"))?;
    let p = paths.model_path(m);
    if !p.exists() {
        download(app, m.id, m.url, &p).await?;
    }
    Ok(p)
}

pub async fn ensure_engine(app: &AppHandle, paths: &Paths, e: &EngineInfo) -> Result<PathBuf> {
    if let Some(exe) = paths.engine_exe(e) {
        return Ok(exe);
    }
    let dir = paths.engine_dir(e);
    std::fs::create_dir_all(&dir)?;
    let archive = dir.join(if e.url.ends_with(".zip") { "pkg.zip" } else { "pkg.tar.gz" });
    download(app, e.id, e.url, &archive).await?;
    let _ = app.emit("download-progress", Progress { id: e.id, done: 0, total: 0, stage: "extracting" });
    let a2 = archive.clone();
    let d2 = dir.clone();
    tokio::task::spawn_blocking(move || extract(&a2, &d2)).await??;
    let _ = std::fs::remove_file(&archive);
    let _ = app.emit("download-progress", Progress { id: e.id, done: 1, total: 1, stage: "done" });
    let exe = paths.engine_exe(e).ok_or_else(|| anyhow!("{} not found in engine package", e.exe))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755));
    }
    Ok(exe)
}

fn extract(archive: &Path, dir: &Path) -> Result<()> {
    if archive.extension().map(|e| e == "zip").unwrap_or(false) {
        let f = std::fs::File::open(archive)?;
        let mut z = zip::ZipArchive::new(f)?;
        for i in 0..z.len() {
            let mut entry = z.by_index(i)?;
            let Some(rel) = entry.enclosed_name() else { continue };
            let out = dir.join(rel);
            if entry.is_dir() {
                std::fs::create_dir_all(&out)?;
            } else {
                if let Some(p) = out.parent() {
                    std::fs::create_dir_all(p)?;
                }
                let mut o = std::fs::File::create(&out)?;
                std::io::copy(&mut entry, &mut o)?;
            }
        }
        Ok(())
    } else {
        let st = Command::new("tar").arg("-xzf").arg(archive).arg("-C").arg(dir).status()?;
        if !st.success() {
            return Err(anyhow!("tar failed"));
        }
        Ok(())
    }
}

/// A running local server process (whisper-server or llama-server).
pub struct Server {
    child: Child,
    pub port: u16,
    pub key: String,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).map(|a| a.port()).unwrap_or(18_765)
}

#[cfg(windows)]
mod job {
    //! Puts child servers in a kill-on-close job so they never outlive Flow.
    use once_cell::sync::OnceCell;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::JobObjects::*;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE};

    struct Job(HANDLE);
    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}
    static JOB: OnceCell<Option<Job>> = OnceCell::new();

    pub fn assign(pid: u32) {
        let job = JOB.get_or_init(|| unsafe {
            let h = CreateJobObjectW(None, None).ok()?;
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                h,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .ok()?;
            Some(Job(h))
        });
        if let Some(Job(h)) = job {
            unsafe {
                if let Ok(p) = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, false, pid) {
                    let _ = AssignProcessToJobObject(*h, p);
                }
            }
        }
    }
}

pub fn spawn_server(exe: &Path, args: &[String], key: String, log_path: &Path) -> Result<Server> {
    let port = free_port();
    let mut cmd = Command::new(exe);
    cmd.args(args).arg("--host").arg("127.0.0.1").arg("--port").arg(port.to_string());
    if let Some(dir) = exe.parent() {
        cmd.current_dir(dir);
    }
    let log = std::fs::File::create(log_path)?;
    cmd.stdout(Stdio::from(log.try_clone()?)).stderr(Stdio::from(log)).stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let child = cmd.spawn().with_context(|| format!("start {}", exe.display()))?;
    #[cfg(windows)]
    job::assign(child.id());
    Ok(Server { child, port, key })
}

impl Server {
    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
    pub fn base(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

pub async fn wait_ready(base: &str, path: &str, timeout: Duration) -> Result<()> {
    let client = reqwest::Client::new();
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if let Ok(r) = client.get(format!("{base}{path}")).timeout(Duration::from_secs(2)).send().await {
            if r.status().is_success() {
                return Ok(());
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Err(anyhow!("engine did not become ready"))
}

/// Holds the two local servers; restarts them when the chosen model changes.
#[derive(Default)]
pub struct Engines {
    pub whisper: Mutex<Option<Server>>,
    pub llama: Mutex<Option<Server>>,
}

pub fn threads() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(2, 8)
}
