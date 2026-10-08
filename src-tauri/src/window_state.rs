//! メインウィンドウの位置と大きさを記録し、次の起動時に戻す（ADR 0007、ADR 0014）。

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewWindow, WindowEvent,
};

use crate::environment::Environment;

const FILENAME: &str = "window-state.json";

/// 画面上の矩形。座標と大きさは物理ピクセル。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    fn right(&self) -> i64 {
        self.x as i64 + self.width as i64
    }

    fn bottom(&self) -> i64 {
        self.y as i64 + self.height as i64
    }

    fn contains_point(&self, x: f64, y: f64) -> bool {
        (self.x as f64) <= x
            && x < self.right() as f64
            && (self.y as f64) <= y
            && y < self.bottom() as f64
    }
}

/// 記録するウィンドウの状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowState {
    /// 枠を含むウィンドウの位置と大きさ。画面に収まるかの判定に使う
    pub outer: Rect,
    /// 枠を除く内側の大きさ。復元するときに設定する
    pub inner_width: u32,
    pub inner_height: u32,
}

/// `window` の全体が、いずれかのディスプレイの上にあるかを返す。
/// 複数のディスプレイにまたがっていてもよいが、どのディスプレイにも載らない部分があれば偽とする。
pub fn fits_on_monitors(window: &Rect, monitors: &[Rect]) -> bool {
    if window.width == 0 || window.height == 0 {
        return false;
    }

    // ウィンドウとディスプレイの辺で、ウィンドウを格子に区切る。
    // 各区画は、中心がいずれかのディスプレイに入っていれば、全体がそのディスプレイに入る。
    let mut xs = vec![window.x as i64, window.right()];
    let mut ys = vec![window.y as i64, window.bottom()];
    for m in monitors {
        xs.extend([m.x as i64, m.right()]);
        ys.extend([m.y as i64, m.bottom()]);
    }
    let clip = |v: Vec<i64>, lo: i64, hi: i64| {
        let mut v: Vec<i64> = v.into_iter().filter(|&p| lo <= p && p <= hi).collect();
        v.sort_unstable();
        v.dedup();
        v
    };
    let xs = clip(xs, window.x as i64, window.right());
    let ys = clip(ys, window.y as i64, window.bottom());

    xs.windows(2).all(|xw| {
        ys.windows(2).all(|yw| {
            let cx = (xw[0] + xw[1]) as f64 / 2.0;
            let cy = (yw[0] + yw[1]) as f64 / 2.0;
            monitors.iter().any(|m| m.contains_point(cx, cy))
        })
    })
}

/// 現在のウィンドウの状態を記録しておく場所。終了時にファイルへ書き出す。
/// ファイルは環境ごとに分ける（ADR 0014）
pub struct WindowStateStore {
    env: Environment,
    state: Mutex<Option<WindowState>>,
}

impl WindowStateStore {
    pub fn new(env: Environment) -> Self {
        Self {
            env,
            state: Mutex::new(None),
        }
    }
}

fn state_path<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    let env = app.state::<WindowStateStore>().env;
    app.path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join(env.dir_name()).join(FILENAME))
}

fn load<R: Runtime>(app: &AppHandle<R>) -> Option<WindowState> {
    let text = fs::read_to_string(state_path(app)?).ok()?;
    serde_json::from_str(&text).ok()
}

/// 記録した状態をファイルに書き出す。
pub fn save<R: Runtime>(app: &AppHandle<R>) {
    let Some(state) = *app.state::<WindowStateStore>().state.lock().unwrap() else {
        return;
    };
    let Some(path) = state_path(app) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string_pretty(&state) {
        let _ = fs::write(path, text);
    }
}

fn current_state<R: Runtime>(window: &WebviewWindow<R>) -> Option<WindowState> {
    // 最大化、最小化、全画面のときの位置と大きさは記録しない
    if window.is_maximized().ok()? || window.is_minimized().ok()? || window.is_fullscreen().ok()? {
        return None;
    }
    let position = window.outer_position().ok()?;
    let outer = window.outer_size().ok()?;
    let inner = window.inner_size().ok()?;
    Some(WindowState {
        outer: Rect {
            x: position.x,
            y: position.y,
            width: outer.width,
            height: outer.height,
        },
        inner_width: inner.width,
        inner_height: inner.height,
    })
}

fn record<R: Runtime>(window: &WebviewWindow<R>) {
    if let Some(state) = current_state(window) {
        *window.state::<WindowStateStore>().state.lock().unwrap() = Some(state);
    }
}

/// 記録した位置と大きさを `window` に戻し、以降の移動と大きさの変更を記録する。
/// 記録した位置にウィンドウが収まらなければ、設定ファイルにある既定の位置と大きさのままにする。
pub fn restore_and_track<R: Runtime>(window: &WebviewWindow<R>) -> tauri::Result<()> {
    let app = window.app_handle();
    if let Some(state) = load(app) {
        let monitors: Vec<Rect> = window
            .available_monitors()?
            .iter()
            .map(|m| Rect {
                x: m.position().x,
                y: m.position().y,
                width: m.size().width,
                height: m.size().height,
            })
            .collect();
        if fits_on_monitors(&state.outer, &monitors) {
            // 先に移すことで、大きさを移動先のディスプレイの上で設定する
            window.set_position(PhysicalPosition::new(state.outer.x, state.outer.y))?;
            window.set_size(PhysicalSize::new(state.inner_width, state.inner_height))?;
        }
    }
    record(window);

    let tracked = window.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::Moved(_) | WindowEvent::Resized(_) => record(&tracked),
        WindowEvent::CloseRequested { .. } => save(tracked.app_handle()),
        _ => {}
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    const MAIN: Rect = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };

    #[test]
    fn fits_inside_one_monitor() {
        assert!(fits_on_monitors(&rect(100, 100, 800, 600), &[MAIN]));
    }

    #[test]
    fn fits_exactly_the_monitor() {
        assert!(fits_on_monitors(&MAIN, &[MAIN]));
    }

    #[test]
    fn rejects_window_on_removed_monitor() {
        // 右にあったディスプレイを外した後の位置
        assert!(!fits_on_monitors(&rect(2000, 100, 800, 600), &[MAIN]));
    }

    #[test]
    fn rejects_window_partly_off_screen() {
        assert!(!fits_on_monitors(&rect(1500, 100, 800, 600), &[MAIN]));
        assert!(!fits_on_monitors(&rect(-10, 100, 800, 600), &[MAIN]));
        assert!(!fits_on_monitors(&rect(100, 1000, 800, 600), &[MAIN]));
    }

    #[test]
    fn rejects_window_larger_than_monitor() {
        assert!(!fits_on_monitors(&rect(0, 0, 2560, 1440), &[MAIN]));
    }

    #[test]
    fn fits_across_adjacent_monitors() {
        let right = rect(1920, 0, 1920, 1080);
        assert!(fits_on_monitors(&rect(1500, 100, 800, 600), &[MAIN, right]));
    }

    #[test]
    fn rejects_window_over_gap_between_monitors() {
        // 右のディスプレイが下にずれていて、ウィンドウの右上がどちらにも載らない
        let right = rect(1920, 500, 1920, 1080);
        assert!(!fits_on_monitors(
            &rect(1500, 100, 800, 600),
            &[MAIN, right]
        ));
    }

    #[test]
    fn fits_on_monitor_with_negative_origin() {
        let left = rect(-2560, -200, 2560, 1440);
        assert!(fits_on_monitors(
            &rect(-2000, -100, 800, 600),
            &[left, MAIN]
        ));
    }

    #[test]
    fn fits_on_mirrored_monitors() {
        assert!(fits_on_monitors(&rect(100, 100, 800, 600), &[MAIN, MAIN]));
        assert!(!fits_on_monitors(&rect(1500, 100, 800, 600), &[MAIN, MAIN]));
    }

    #[test]
    fn rejects_without_monitors() {
        assert!(!fits_on_monitors(&rect(100, 100, 800, 600), &[]));
    }

    #[test]
    fn rejects_empty_window() {
        assert!(!fits_on_monitors(&rect(100, 100, 0, 600), &[MAIN]));
    }

    #[test]
    fn state_round_trips_through_json() {
        let state = WindowState {
            outer: rect(-300, 40, 1024, 768),
            inner_width: 1024,
            inner_height: 740,
        };
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(serde_json::from_str::<WindowState>(&json).unwrap(), state);
    }
}
