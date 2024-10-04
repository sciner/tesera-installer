#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::process::{Command, Child, Stdio};
use std::sync::{Arc, Mutex};
use std::env;
use std::path::PathBuf;
use std::fs::File;
use tauri::AppHandle;
use serde_json::Value; // Импортируем тип Value для работы с JSON
use std::fs;
use tauri::api::path::app_data_dir;
use tauri::Manager;
// use tauri::{Window}; // Используем Window из Tauri
// use winit::event::{DeviceEvent, Event, WindowEvent, ElementState, MouseButton};
// use winit::event_loop::{ControlFlow, EventLoop};
// use winit::window::CursorGrabMode;

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
fn toggle_fullscreen(window: tauri::Window, fullscreen: bool) {
    // Получаем Arc<Mutex<Option<Config>>>
    let config = window.state::<Arc<Mutex<Option<Config>>>>();
    // Блокируем доступ к Config и вызываем метод set_fullscreen
    let mut config_lock = config.lock().unwrap();
    if let Some(ref mut config_data) = *config_lock {
        config_data.set_fullscreen(fullscreen);
        config_data.save();
    }
    // Применяем полноэкранный режим
    window.set_fullscreen(fullscreen).unwrap();
}

#[tauri::command]
fn open_devtools(window: tauri::Window) {
    window.open_devtools();
}

#[tauri::command]
fn save_screenshot(_app_handle: AppHandle, file_name: &str, file_data: Vec<u8>) -> Result<String, String> {
    // Получаем путь к директории с исполняемым файлом
    let exe_path = std::env::current_exe().map_err(|e| format!("Ошибка при получении пути к исполняемому файлу: {}", e))?;
    // Определяем путь к папке screenshot
    let screenshot_dir = exe_path.parent().unwrap().join("screenshot");
    // Проверяем, существует ли папка, и если нет, создаем её
    if !screenshot_dir.exists() {
        fs::create_dir_all(&screenshot_dir).map_err(|e| format!("Не удалось создать папку screenshot: {}", e))?;
    }
    // Формируем полный путь для сохранения файла в папке screenshot
    let file_path: PathBuf = screenshot_dir.join(file_name);
    // Пытаемся сохранить файл
    fs::write(&file_path, file_data).map_err(|e| format!("Не удалось сохранить файл: {}", e))?;
    Ok(file_path.to_string_lossy().into_owned())
}

fn create_processes(processes: &Arc<Mutex<Vec<Option<Child>>>>, app_data_path: String, in_debug: bool) {    // Массив с информацией о процессах

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
        command.arg(format!("app_data_path=\"{}\"", app_data_path));

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

fn ensure_app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    // Получаем путь к директории данных приложения
    if let Some(app_dir) = app_data_dir(&app.config()) {
        // Пробуем создать директорию, если её нет
        if let Err(e) = fs::create_dir_all(&app_dir) {
            return Err(format!("Error creating directory: {}", e));
        }
        // Возвращаем путь к директории
        Ok(app_dir)
    } else {
        Err("Failed to get app data directory".to_string())
    }
}

struct Config {
    config_path: PathBuf,
    config_data: Option<Value>,
}

impl Config {
    fn new(config_path: PathBuf) -> Self {
        let mut config = Config {
            config_path,
            config_data: None,
        };
        config.load();
        config
    }
    
    // Метод для загрузки конфигурации из файла
    fn load(&mut self) {
        if let Ok(config_data) = fs::read_to_string(&self.config_path) {
            self.config_data = Some(serde_json::from_str(&config_data).expect("Не удалось распарсить JSON"));
        }
    }

    // Исправленный метод save
    fn save(&self) {
        if let Some(config_data) = &self.config_data {
            // Преобразование конфигурации в строку
            let config_string = serde_json::to_string_pretty(config_data)
                .expect("Не удалось сериализовать конфигурацию");
            // Запись строки в файл
            fs::write(&self.config_path, config_string)
                .expect("Не удалось записать файл конфигурации");
        } else {
            println!("Нет данных для сохранения");
        }
    }

    // Метод для получения значения конфигурации по ключу
    fn get_value(&self, key: &str) -> Option<&Value> {
        if let Some(config) = &self.config_data {
            config.get(key)
        } else {
            None
        }
    }

    // Метод для установки значения fullscreen в конфиге
    fn set_fullscreen(&mut self, fullscreen: bool) {
        if let Some(ref mut config_data) = self.config_data {
            config_data["fullscreen"] = serde_json::Value::Bool(fullscreen);
            let _ = &self.save();
        } else {
            println!("Конфигурация не загружена");
        }
    }

}

fn main() {

    // let processes: Arc<Mutex<Vec<Option<Child>>>> = Arc::new(Mutex::new(Vec::with_capacity(process_configs.len())));
    let processes: Arc<Mutex<Vec<Option<Child>>>> = Arc::new(Mutex::new(Vec::new()));
    let config: Arc<Mutex<Option<Config>>> = Arc::new(Mutex::new(None));

    tauri::Builder::default()
        .setup({
            let processes = Arc::clone(&processes);
            let config = Arc::clone(&config);
            move |app| {

                let args: Vec<String> = env::args().collect();
                let in_debug = args.contains(&"--debug".to_string());
                let app_data_path = ensure_app_data_dir(&app.handle())?.to_string_lossy().into_owned();
                let window = app.get_window("main").unwrap();

                create_processes(&processes, app_data_path, in_debug);

                // Создаем объект Config и передаем путь к файлу
                let config_path = PathBuf::from("config.json");
                let mut config_lock = config.lock().unwrap(); // Блокируем доступ к config и изменяем его
                *config_lock = Some(Config::new(config_path)); // Инициализируем config
                // Помещаем конфигурацию в окно
                window.manage(Arc::clone(&config));
                // Пример использования конфигурации
                if let Some(ref config) = *config_lock {
                    if let Some(fullscreen_value) = config.get_value("fullscreen") {
                        let fullscreen = fullscreen_value.as_bool().unwrap_or(false);
                        window.set_fullscreen(fullscreen).unwrap();
                    }
                }

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
        .invoke_handler(tauri::generate_handler![toggle_fullscreen, open_devtools, save_screenshot])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
