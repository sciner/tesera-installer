#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::process::{Command, Child, Stdio};
use std::sync::{Arc, Mutex};
use std::env;
use std::path::PathBuf;
use std::fs::File;

// #[cfg(target_os = "windows")]
// use std::os::windows::process::CommandExt;

// Структура для хранения информации о каждом процессе
#[derive(Clone)] // Добавляем возможность клонирования ProcessConfig
struct ProcessConfig {
    binary: String,
    args: Vec<String>,
    error_log: String,
}

fn main() {
    // Массив с информацией о процессах
    let process_configs = vec![
        ProcessConfig {
            binary: "out/bin/node/node.exe".to_string(),
            args: vec!["./out/master_server.js".to_string(), "page.useGenWorkers=true".to_string()],
            error_log: "stderr_master_server.log".to_string(),
        },
        ProcessConfig {
            binary: "out/bin/node/node.exe".to_string(),
            args: vec!["./out/world_server.js".to_string(), "page.useGenWorkers=true".to_string()],
            error_log: "stderr_world_server.log".to_string(),
        },
    ];

    // Мьютекс для хранения всех дочерних процессов, инициализируем как None, без клонирования
    let processes: Arc<Mutex<Vec<Option<Child>>>> = Arc::new(Mutex::new(Vec::with_capacity(process_configs.len())));

    tauri::Builder::default()
        .setup({
            let processes = Arc::clone(&processes);
            move |_app| {
                for config in process_configs.iter() {
                    let exe_path = env::current_exe().expect("failed to get current exe directory");
                    let exe_dir = exe_path.parent().expect("failed to get parent directory");
                    let binary_path: PathBuf = exe_dir.join(&config.binary);

                    let mut command = Command::new(binary_path);

                    // Добавляем аргументы командной строки
                    for arg in &config.args {
                        command.arg(arg);
                    }

                    // Перенаправляем stderr в лог файл
                    let stderr_file = File::create(&config.error_log).expect("failed to create stderr log file");
                    command.stderr(Stdio::from(stderr_file));

                    // Установка флага для запуска без окна на Windows
                    // #[cfg(target_os = "windows")]
                    // {
                    //     const DETACHED_PROCESS: u32 = 0x00000008;
                    //     command.creation_flags(DETACHED_PROCESS);
                    // }

                    // Запуск процесса
                    let child = command.spawn().expect("failed to start process");
                    processes.lock().unwrap().push(Some(child)); // Добавляем процесс в вектор
                }

                Ok(())
            }
        })
        .on_window_event({
            let processes = Arc::clone(&processes);
            move |event| {
                // При закрытии окна выгружаем все процессы
                if let tauri::WindowEvent::CloseRequested { .. } = event.event() {
                    let mut processes = processes.lock().unwrap();
                    for child_option in processes.iter_mut() {
                        if let Some(mut child) = child_option.take() {
                            child.kill().expect("failed to kill process");
                        }
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
