#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::process::{Command, Child, Stdio};
use std::sync::{Arc, Mutex};
use std::env;
use std::path::PathBuf;
use std::fs::File;
use tauri::{Manager, Window}; // Используем Window из Tauri
use winit::event::{DeviceEvent, Event, WindowEvent, ElementState, MouseButton};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::CursorGrabMode;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

// Структура для хранения информации о каждом процессе
#[derive(Clone)] // Добавляем возможность клонирования ProcessConfig
struct ProcessConfig {
    binary: String,
    args: Vec<String>,
    error_log: String,
}

#[tauri::command]
fn toggle_fullscreen(window: tauri::Window) {
    let is_fullscreen = window.is_fullscreen().unwrap_or(false);
    window.set_fullscreen(!is_fullscreen).unwrap();
}

#[tauri::command]
fn open_devtools(window: tauri::Window) {
    window.open_devtools();
}

fn create_processes(processes: &Arc<Mutex<Vec<Option<Child>>>>, in_debug: bool) {    // Массив с информацией о процессах

    let process_configs = vec![
        ProcessConfig {
            binary: "out/bin/node/node.exe".to_string(),
            args: vec!["./out/master_server.js".to_string(), "page.useGenWorkers=true".to_string()],
            error_log: "stderr_master.log".to_string(),
        },
        ProcessConfig {
            binary: "out/bin/node/node.exe".to_string(),
            args: vec!["./out/db_server.js".to_string()],
            error_log: "stderr_db.log".to_string(),
        },
        ProcessConfig {
            binary: "out/bin/node/node.exe".to_string(),
            args: vec!["./out/world_server.js".to_string(), "page.useGenWorkers=true".to_string()],
            error_log: "stderr_world.log".to_string(),
        },
    ];

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

        if !in_debug {
            // Установка флага для запуска без окна на Windows
            #[cfg(target_os = "windows")]
            {
                const DETACHED_PROCESS: u32 = 0x00000008;
                command.creation_flags(DETACHED_PROCESS);
            }
        }

        // Запуск процесса
        let child = command.spawn().expect("failed to start process");
        processes.lock().unwrap().push(Some(child)); // Добавляем процесс в вектор
    }
}

fn main() {

    // let processes: Arc<Mutex<Vec<Option<Child>>>> = Arc::new(Mutex::new(Vec::with_capacity(process_configs.len())));
    let processes: Arc<Mutex<Vec<Option<Child>>>> = Arc::new(Mutex::new(Vec::new()));

    tauri::Builder::default()
        .setup({
            let processes = Arc::clone(&processes);
            move |app| {

                let args: Vec<String> = env::args().collect();
                let in_debug = args.contains(&"--debug".to_string());

                create_processes(&processes, in_debug);

                Ok(())
            }
        })
        .on_window_event({
            let processes = Arc::clone(&processes);
            move |event| {

                // Захват курсора при активации окна
                // if let tauri::WindowEvent::Focused(focused) = event.event() {
                //     if *focused {
                //         let window = event.window();
                //         window.set_cursor_grab(true).unwrap();
                //         window.set_cursor_visible(false).unwrap();
                //         if let Ok(size) = window.inner_size() {
                //             let center_x = size.width as f64 / 2.0;
                //             let center_y = size.height as f64 / 2.0;
                //             window.set_cursor_position(tauri::PhysicalPosition::new(center_x, center_y))
                //                 .expect("Failed to set cursor position");
                //         }
                //     }
                // }

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
        .invoke_handler(tauri::generate_handler![toggle_fullscreen, open_devtools])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
