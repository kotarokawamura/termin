// Copyright (C) [2026/10/19] kotaro,termin
//
// This program is free software; you can redistribute it and/or
// modify it under the terms of the GNU General Public License
// as published by the Free Software Foundation; version 2
// of the License.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program; if not, write to the Free Software
// Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301, USA.
use std::time::Duration;
use eframe::egui;

/// 再描画・PTYポーリングの駆動モード
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PollMode {
    /// イベント駆動モード：PTYからの出力通知や入力があった時のみ再描画する（デフォルト）
    #[default]
    EventDriven,
    /// FPSポーリングモード：指定されたFPS間隔で常時ポーリング・再描画し続ける
    ContinuousFps,
}

impl PollMode {
    pub fn from_cli_str(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "event" | "event-driven" | "event_driven" => Some(Self::EventDriven),
            "fps" | "continuous" | "polling" | "poll" => Some(Self::ContinuousFps),
            _ => None,
        }
    }
}

/// フレーム再描画スケジューラ
pub struct RepaintScheduler {
    mode: PollMode,
}

impl RepaintScheduler {
    pub fn new(mode: PollMode) -> Self {
        Self { mode }
    }

    #[allow(dead_code)]
    pub fn mode(&self) -> PollMode {
        self.mode
    }

    /// イベント駆動モード時にPTYスレッドへ渡す Wakeup 用の Context を返す。
    /// ContinuousFps モード時は PTY スレッドからの強制 Wakeup を行わず一定周期でポーリングする。
    pub fn pty_wakeup_ctx(&self, ctx: &egui::Context) -> Option<egui::Context> {
        match self.mode {
            PollMode::EventDriven => Some(ctx.clone()),
            PollMode::ContinuousFps => None,
        }
    }

    /// Called at each frame end to schedule the next frame's redraw according to the mode.
    pub fn schedule_next_frame(&self, ctx: &egui::Context, pty_updated: bool, target_fps: u32) {
        let fps = target_fps.clamp(1, 240) as u64;
        let interval = Duration::from_millis((1000 / fps).max(1));

        match self.mode {
            PollMode::ContinuousFps => {
                ctx.request_repaint_after(interval);
            }
            PollMode::EventDriven => {
                if pty_updated {
                    ctx.request_repaint_after(interval);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "test/scheduler_tests.rs"]
mod tests;