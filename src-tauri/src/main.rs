#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::process::{Command, Child, Stdio};
use std::sync::{Arc, Mutex};
use std::env;
use std::path::PathBuf;
use std::fs::File;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

fn main() {
    let node_process: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));

    tauri::Builder::default()
        .setup({
            let node_process = Arc::clone(&node_process);
            move |_app| {

                // Определяем абсолютный путь к node.exe
                let exe_path = env::current_exe().expect("failed to get current exe directory");
                let exe_dir = exe_path.parent().expect("failed to get parent directory");
                let node_path: PathBuf = exe_dir.join("out/bin/node/node.exe");

                let mut command = Command::new(node_path);

                // Добавляем аргументы командной строки
                // let server_script = exe_dir.join("out/master_server.js");
                // command.arg(exe_dir.join("out/master_server.js"));

                command.arg("./out/master_server.js");
                command.arg("page.useGenWorkers=true");

                // Открываем файл для записи stdout
                // let stdout_file = File::create("stdout.log").expect("failed to create stdout log file");
                // command.stdout(Stdio::from(stdout_file));
                
                // Перенаправляем stderr в файл
                let stderr_file = File::create("stderr.log").expect("failed to create stderr log file");
                command.stderr(Stdio::from(stderr_file));

                // Установка флага для запуска без окна на Windows
                #[cfg(target_os = "windows")]
                {
                    const DETACHED_PROCESS: u32 = 0x00000008;
                    command.creation_flags(DETACHED_PROCESS);
                }

                let child = command.spawn().expect("failed to start node.js script");

                *node_process.lock().unwrap() = Some(child);
                Ok(())
            }
        })
        .on_window_event({
            let node_process = Arc::clone(&node_process);
            move |event| {
                // При закрытии окна выгружаем процесс node.js
                if let tauri::WindowEvent::CloseRequested { .. } = event.event() {
                    if let Some(mut child) = node_process.lock().unwrap().take() {
                        child.kill().expect("failed to kill node.js process");
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
