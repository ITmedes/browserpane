use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, trace};

use crate::capture::ffmpeg::CaptureRegion;

const DEFAULT_DEBUG_PORT: u16 = 9222;
const DEFAULT_POLL_MS: u64 = 80;
const EVAL_TIMEOUT_MS: u64 = 450;
const RETRY_BACKOFF_MS: u64 = 800;
const DEFAULT_WHEEL_CMD_TIMEOUT_MS: u64 = 90;
const DEFAULT_WHEEL_STEP_INTERVAL_MS: u64 = 8;
const MIN_TEXT_CMD_TIMEOUT_MS: u64 = EVAL_TIMEOUT_MS + DEFAULT_POLL_MS + 40;
const DEFAULT_TEXT_CMD_TIMEOUT_MS: u64 = MIN_TEXT_CMD_TIMEOUT_MS;
const DEFAULT_SCROLL_PAUSE_WINDOW_MS: u64 = 900;
const WINDOW_STATE_SETTLE_MS: u64 = 120;
const RESIZE_METRICS_SETTLE_MS: u64 = 90;
const RESIZE_METRICS_POLL_MS: u64 = 90;
const RESIZE_METRICS_ATTEMPTS: u32 = 4;
const EDITABLE_INPUT_TYPES: &[&str] = &[
    "", "text", "password", "email", "search", "tel", "url", "number",
];
const RESPONSIVE_OVERFLOW_BREAKPOINT_CSS_PX: u32 = 1000;
const RESPONSIVE_OVERFLOW_THRESHOLD_CSS_PX: u32 = 120;
const MIN_VIDEO_HINT_WIDTH: u32 = 120;
const MIN_VIDEO_HINT_HEIGHT: u32 = 90;
const MIN_EDITABLE_HINT_WIDTH: u32 = 2;
const MIN_EDITABLE_HINT_HEIGHT: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HintRegionKind {
    Video,
    Editable,
}

impl Default for HintRegionKind {
    fn default() -> Self {
        Self::Video
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageHintState {
    pub video_region: Option<CaptureRegion>,
    pub region_kind: HintRegionKind,
    /// Browser content viewport in screen pixels (excludes toolbar/scrollbar).
    pub viewport: Option<CaptureRegion>,
    /// `window.devicePixelRatio * 1000`, rounded.
    pub device_scale_factor_milli: u16,
    pub scroll_y: Option<i64>,
    pub scroll_delta_y: i32,
    pub visible: bool,
    pub focused: bool,
    pub update_seq: u64,
}

impl Default for PageHintState {
    fn default() -> Self {
        Self {
            video_region: None,
            region_kind: HintRegionKind::Video,
            viewport: None,
            device_scale_factor_milli: 1000,
            scroll_y: None,
            scroll_delta_y: 0,
            visible: false,
            focused: false,
            update_seq: 0,
        }
    }
}

#[derive(Debug)]
struct WheelCommand {
    screen_pos: Option<(u16, u16)>,
    dx: i16,
    dy: i16,
    step_px: u16,
    step_interval: Duration,
    deadline: tokio::time::Instant,
    response: oneshot::Sender<bool>,
}

#[derive(Debug)]
struct TextCommand {
    text: String,
    deadline: tokio::time::Instant,
    response: oneshot::Sender<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct WheelDispatch {
    x: f64,
    y: f64,
    delta_x: i32,
    delta_y: i32,
}

type CdpWsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

struct CdpWindowInfo {
    window_id: u32,
    window_state: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResponsiveOverflowState {
    client_width: u32,
    scroll_width: u32,
    body_scroll_width: u32,
    narrow_breakpoint: bool,
    url: String,
}

pub struct BrowserCdpHandle {
    pub task: JoinHandle<()>,
    wheel_tx: mpsc::Sender<WheelCommand>,
    text_tx: mpsc::Sender<TextCommand>,
    wheel_timeout: Duration,
    wheel_step_interval: Duration,
    text_timeout: Duration,
}

impl BrowserCdpHandle {
    pub async fn dispatch_quantized_wheel(
        &self,
        screen_pos: Option<(u16, u16)>,
        dx: i16,
        dy: i16,
        step_px: u16,
    ) -> bool {
        if step_px == 0 || (dx == 0 && dy == 0) {
            return false;
        }
        let command_timeout =
            wheel_command_timeout(self.wheel_timeout, self.wheel_step_interval, dx, dy);
        let (response_tx, response_rx) = oneshot::channel();
        if self
            .wheel_tx
            .send(WheelCommand {
                screen_pos,
                dx,
                dy,
                step_px,
                step_interval: self.wheel_step_interval,
                deadline: tokio::time::Instant::now() + command_timeout,
                response: response_tx,
            })
            .await
            .is_err()
        {
            return false;
        }
        match tokio::time::timeout(command_timeout + Duration::from_millis(20), response_rx).await {
            Ok(Ok(sent)) => sent,
            _ => false,
        }
    }

    pub async fn dispatch_text(&self, text: String) -> bool {
        if text.is_empty() {
            return false;
        }
        let (response_tx, response_rx) = oneshot::channel();
        if self
            .text_tx
            .send(TextCommand {
                text,
                deadline: tokio::time::Instant::now() + self.text_timeout,
                response: response_tx,
            })
            .await
            .is_err()
        {
            return false;
        }
        match tokio::time::timeout(self.text_timeout + Duration::from_millis(20), response_rx).await
        {
            Ok(Ok(sent)) => sent,
            _ => false,
        }
    }
}

pub async fn resize_visible_target_window(width: u16, height: u16) -> bool {
    if width < 2 || height < 2 || !env_bool("BPANE_CHROMIUM_DEBUG_ENABLE", true) {
        return false;
    }

    let http = match reqwest::Client::builder()
        .timeout(Duration::from_millis(1200))
        .build()
    {
        Ok(client) => client,
        Err(err) => {
            debug!(?err, "cdp window resize client init failed");
            return false;
        }
    };

    let debug_port = env_u16("BPANE_CHROMIUM_DEBUG_PORT", DEFAULT_DEBUG_PORT);
    let probe_js = build_video_probe_js(false, DEFAULT_SCROLL_PAUSE_WINDOW_MS, 0);
    let mut request_id = 1;
    let Some((ws_url, mut stream, hint)) =
        connect_visible_target_stream(&http, debug_port, None, &probe_js, &mut request_id).await
    else {
        debug!(
            width,
            height,
            port = debug_port,
            "cdp window resize: no visible target"
        );
        return false;
    };

    let resized = resize_window_via_stream(
        &mut stream,
        u32::from(width),
        u32::from(height),
        hint.device_scale_factor_milli,
        &mut request_id,
    )
    .await;
    if resized {
        let _ = maybe_apply_resize_device_metrics(
            &mut stream,
            u32::from(width),
            u32::from(height),
            hint.device_scale_factor_milli,
            &probe_js,
            &mut request_id,
        )
        .await;
    }
    if !resized {
        debug!(width, height, target = %ws_url, "cdp window resize failed");
    }
    resized
}

const VIDEO_PROBE_JS_TEMPLATE: &str = r#"(() => {
  try {
    const ctlKey = "__bpaneVideoCtl";
    const readScrollY = () => Math.round(window.scrollY || window.pageYOffset || 0);
    const visible = document.visibilityState === "visible";
    const focused = typeof document.hasFocus === "function" ? !!document.hasFocus() : false;
    let ctl = globalThis[ctlKey];
    if (!ctl) {
      ctl = { lastScrollTs: 0, lastScrollY: readScrollY(), pendingScrollDy: 0 };
      try {
        Object.defineProperty(globalThis, ctlKey, {
          value: ctl,
          writable: false,
          configurable: true,
          enumerable: false,
        });
      } catch (_e) {
        globalThis[ctlKey] = ctl;
      }
      const markScroll = () => {
        const y = readScrollY();
        ctl.pendingScrollDy += (y - (ctl.lastScrollY || y));
        ctl.lastScrollY = y;
        ctl.lastScrollTs = Date.now();
      };
      window.addEventListener("scroll", markScroll, { capture: true, passive: true });
      window.addEventListener("wheel", markScroll, { capture: true, passive: true });
      window.addEventListener("touchmove", markScroll, { capture: true, passive: true });
    }

    if (typeof ctl.lastScrollY !== "number") ctl.lastScrollY = readScrollY();
    if (typeof ctl.pendingScrollDy !== "number") ctl.pendingScrollDy = 0;
    const currentScrollY = readScrollY();
    const drift = currentScrollY - ctl.lastScrollY;
    if (drift !== 0) {
      ctl.pendingScrollDy += drift;
      ctl.lastScrollY = currentScrollY;
    }
    const scrollDeltaY = Math.round(ctl.pendingScrollDy || 0);
    ctl.pendingScrollDy = 0;

    const now = Date.now();
    const scrollQuietMs = now - (ctl.lastScrollTs || 0);
    const recentlyScrolled = scrollQuietMs <= __SCROLL_PAUSE_WINDOW_MS__;

    // Scroll-snap: after scroll settles, nudge scrollY to the nearest
    // tile-aligned position.  __SCROLL_SNAP_CSS_PX__ is the tile size in
    // CSS pixels (e.g. 32 for 64 device px at 2x scale).  A value of 0
    // disables snapping.
    const snapStep = __SCROLL_SNAP_CSS_PX__;
    if (snapStep > 0 && !recentlyScrolled && scrollQuietMs < 5000) {
      const remainder = currentScrollY % snapStep;
      if (remainder !== 0) {
        const nearest = (remainder <= snapStep / 2)
          ? currentScrollY - remainder
          : currentScrollY + (snapStep - remainder);
        window.scrollTo({ top: nearest, behavior: "instant" });
      }
    }
    if (__PAUSE_ON_SCROLL__ && recentlyScrolled) {
      for (const v of document.querySelectorAll("video")) {
        if (!v || !v.isConnected || v.paused || v.ended) continue;
        try {
          const p = v.pause();
          if (p && typeof p.catch === "function") p.catch(() => {});
        } catch (_e) {}
      }
      return { visible, focused, scrollY: currentScrollY, scrollDeltaY, region: null, viewport: null };
    }

    const dpr = window.devicePixelRatio || 1;
    const insetX = Math.max(0, (window.outerWidth - window.innerWidth) / 2);
    const insetY = Math.max(0, window.outerHeight - window.innerHeight);
    const isElementVisible = (el) => {
      const style = window.getComputedStyle(el);
      if (!style) return false;
      if (style.display === 'none' || style.visibility === 'hidden') return false;
      if (Number(style.opacity || '1') < 0.05) return false;
      return true;
    };
    const toScreenRegion = (rect) => ({
      x: Math.round((window.screenX + insetX + rect.left) * dpr),
      y: Math.round((window.screenY + insetY + rect.top) * dpr),
      w: Math.round(rect.width * dpr),
      h: Math.round(rect.height * dpr),
    });
    const editableInputTypes = new Set([__EDITABLE_INPUT_TYPES__]);
    const editableRoles = new Set(['textbox', 'searchbox', 'combobox']);
    const nextComposedParent = (el) => {
      if (!el) return null;
      if (el.parentElement) return el.parentElement;
      const root = typeof el.getRootNode === 'function' ? el.getRootNode() : null;
      return root && root.host ? root.host : null;
    };
    const resolveDeepActiveElement = () => {
      let active = document.activeElement;
      while (active && active.shadowRoot && active.shadowRoot.activeElement) {
        active = active.shadowRoot.activeElement;
      }
      return active;
    };
    const isRoleEditable = (el) => {
      if (!el || typeof el.getAttribute !== 'function') return false;
      const role = String(el.getAttribute('role') || '').toLowerCase();
      if (!editableRoles.has(role)) return false;
      if (String(el.getAttribute('aria-disabled') || '').toLowerCase() === 'true') return false;
      if (String(el.getAttribute('aria-readonly') || '').toLowerCase() === 'true') return false;
      return true;
    };
    const resolveFocusedEditable = (el) => {
      let current = el;
      while (current && current.isConnected) {
        const tag = String(current.tagName || '').toLowerCase();
        if (current.isContentEditable) return current;
        if (tag === 'textarea') {
          return !current.disabled && !current.readOnly ? current : null;
        }
        if (tag === 'input') {
          const type = String(current.getAttribute('type') || current.type || 'text').toLowerCase();
          if (!editableInputTypes.has(type)) return null;
          return !current.disabled && !current.readOnly ? current : null;
        }
        if (isRoleEditable(current)) {
          return current;
        }
        current = nextComposedParent(current);
      }
      return null;
    };
    const expandEditableRect = (rect) => {
      const padX = Math.max(12, rect.width * 0.08);
      const padY = Math.max(8, rect.height * 0.35);
      return {
        left: rect.left - padX,
        top: rect.top - padY,
        width: rect.width + (padX * 2),
        height: rect.height + (padY * 2),
      };
    };

    const activeEditable = resolveFocusedEditable(resolveDeepActiveElement());
    if (activeEditable && isElementVisible(activeEditable)) {
      const rect = activeEditable.getBoundingClientRect();
      if (rect.width >= 2 && rect.height >= 2) {
        const expanded = expandEditableRect(rect);
        const contentW = document.documentElement.clientWidth || window.innerWidth;
        const vpX = Math.round((window.screenX + insetX) * dpr);
        const vpY = Math.round((window.screenY + insetY) * dpr);
        const vpW = Math.round(contentW * dpr);
        const vpH = Math.round(window.innerHeight * dpr);
        return {
          visible,
          focused,
          deviceScaleFactorMilli: Math.max(1, Math.round(dpr * 1000)),
          scrollY: currentScrollY,
          scrollDeltaY,
          regionKind: 'editable',
          region: toScreenRegion(expanded),
          viewport: { x: vpX, y: vpY, w: vpW, h: vpH },
        };
      }
    }

    let best = null;
    for (const v of document.querySelectorAll('video')) {
      if (!isElementVisible(v)) continue;
      if (!v.isConnected) continue;
      const rect = v.getBoundingClientRect();
      if (rect.width < 140 || rect.height < 100) continue;
      const area = rect.width * rect.height;
      const ready = (v.readyState || 0) >= 3;
      const playing = !v.paused && !v.ended && !v.seeking;
      const hasProgress = (v.currentTime || 0) > 0 || (v.playbackRate || 1) !== 0;
      if (!(ready && playing && hasProgress)) continue;
      const score = 10 + (area / 15000);
      if (!best || score > best.score) {
        best = {
          ...toScreenRegion(rect),
          score,
        };
      }
    }

    const contentW = document.documentElement.clientWidth || window.innerWidth;
    const vpX = Math.round((window.screenX + insetX) * dpr);
    const vpY = Math.round((window.screenY + insetY) * dpr);
    const vpW = Math.round(contentW * dpr);
    const vpH = Math.round(window.innerHeight * dpr);

    return {
      visible,
      focused,
      deviceScaleFactorMilli: Math.max(1, Math.round(dpr * 1000)),
      scrollY: currentScrollY,
      scrollDeltaY,
      regionKind: best ? 'video' : null,
      region: best ? { x: best.x, y: best.y, w: best.w, h: best.h } : null,
      viewport: { x: vpX, y: vpY, w: vpW, h: vpH },
    };
  } catch (_e) {
    return {
      visible: false,
      focused: false,
      deviceScaleFactorMilli: 1000,
      scrollY: null,
      scrollDeltaY: 0,
      region: null,
      viewport: null
    };
  }
})()"#;

fn build_video_probe_js(
    pause_on_scroll: bool,
    scroll_pause_window_ms: u64,
    scroll_snap_css_px: u32,
) -> String {
    VIDEO_PROBE_JS_TEMPLATE
        .replace(
            "__PAUSE_ON_SCROLL__",
            if pause_on_scroll { "true" } else { "false" },
        )
        .replace(
            "__SCROLL_PAUSE_WINDOW_MS__",
            &scroll_pause_window_ms.to_string(),
        )
        .replace(
            "__EDITABLE_INPUT_TYPES__",
            &editable_input_types_js_literal(),
        )
        .replace("__SCROLL_SNAP_CSS_PX__", &scroll_snap_css_px.to_string())
}

fn editable_input_types_js_literal() -> String {
    EDITABLE_INPUT_TYPES
        .iter()
        .map(|value| format!("'{}'", value))
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Debug, Deserialize)]
struct DevToolsTarget {
    #[serde(rename = "type")]
    target_type: String,
    url: Option<String>,
    title: Option<String>,
    #[serde(rename = "webSocketDebuggerUrl")]
    web_socket_debugger_url: Option<String>,
}

pub fn spawn_video_hint_task(shared: Arc<Mutex<PageHintState>>) -> BrowserCdpHandle {
    let wheel_timeout = Duration::from_millis(env_u64(
        "BPANE_CDP_WHEEL_TIMEOUT_MS",
        DEFAULT_WHEEL_CMD_TIMEOUT_MS,
    ));
    let wheel_step_interval = Duration::from_millis(
        env_u64(
            "BPANE_CDP_WHEEL_STEP_INTERVAL_MS",
            DEFAULT_WHEEL_STEP_INTERVAL_MS,
        )
        .min(24),
    );
    let text_timeout = Duration::from_millis(
        env_u64("BPANE_CDP_TEXT_TIMEOUT_MS", DEFAULT_TEXT_CMD_TIMEOUT_MS)
            .clamp(MIN_TEXT_CMD_TIMEOUT_MS, 1500),
    );
    let (wheel_tx, mut wheel_rx) = mpsc::channel::<WheelCommand>(64);
    let (text_tx, mut text_rx) = mpsc::channel::<TextCommand>(64);
    let task = tokio::spawn(async move {
        if !env_bool("BPANE_CDP_VIDEO_HINT", true) {
            trace!("cdp video hint disabled");
            set_shared_state(&shared, PageHintState::default());
            return;
        }

        let debug_port = env_u16("BPANE_CHROMIUM_DEBUG_PORT", DEFAULT_DEBUG_PORT);
        let poll_ms = env_u64("BPANE_CDP_POLL_MS", DEFAULT_POLL_MS).max(33);
        let poll_interval = Duration::from_millis(poll_ms);
        let retry_backoff = Duration::from_millis(RETRY_BACKOFF_MS);
        let pause_videos_on_scroll = env_bool("BPANE_CDP_PAUSE_VIDEOS_ON_SCROLL", true);
        let scroll_pause_window_ms = env_u64(
            "BPANE_CDP_SCROLL_PAUSE_WINDOW_MS",
            DEFAULT_SCROLL_PAUSE_WINDOW_MS,
        )
        .clamp(100, 10_000);
        // Scroll-snap step in CSS pixels.  Default: tile_size / device_scale = 64/2 = 32.
        // Set to 0 to disable scroll snapping.
        let scroll_snap_css_px = env_u64("BPANE_CDP_SCROLL_SNAP_CSS_PX", 32).min(256) as u32;
        let video_probe_js = build_video_probe_js(
            pause_videos_on_scroll,
            scroll_pause_window_ms,
            scroll_snap_css_px,
        );

        let http = match reqwest::Client::builder()
            .timeout(Duration::from_millis(1200))
            .build()
        {
            Ok(client) => client,
            Err(err) => {
                debug!(?err, "cdp video hint client init failed");
                set_shared_state(&shared, PageHintState::default());
                return;
            }
        };

        let mut ws_url: Option<String> = None;
        let mut ws_stream: Option<CdpWsStream> = None;
        let mut request_id: u64 = 1;
        let mut update_seq: u64 = 0;
        let mut warned_no_targets = false;
        let mut force_target_refresh = true;
        let mut prefetched_hint: Option<PageHintState> = None;

        loop {
            if ws_stream.is_none() || force_target_refresh {
                force_target_refresh = false;
                match connect_visible_target_stream(
                    &http,
                    debug_port,
                    ws_url.as_deref(),
                    &video_probe_js,
                    &mut request_id,
                )
                .await
                {
                    Some((next_url, stream, hint)) => {
                        warned_no_targets = false;
                        ws_url = Some(next_url);
                        ws_stream = Some(stream);
                        prefetched_hint = Some(hint);
                    }
                    None => {
                        if !warned_no_targets {
                            debug!(port = debug_port, "cdp: no visible browser target found");
                            warned_no_targets = true;
                        }
                        set_shared_state(&shared, PageHintState::default());
                        ws_url = None;
                        ws_stream = None;
                        prefetched_hint = None;
                        reject_pending_wheel_commands(&mut wheel_rx);
                        reject_pending_text_commands(&mut text_rx);
                        tokio::time::sleep(retry_backoff).await;
                        continue;
                    }
                }
            }

            let mut socket_failed = false;
            let Some(stream) = ws_stream.as_mut() else {
                set_shared_state(&shared, PageHintState::default());
                reject_pending_wheel_commands(&mut wheel_rx);
                tokio::time::sleep(retry_backoff).await;
                continue;
            };

            while let Ok(cmd) = wheel_rx.try_recv() {
                if !handle_wheel_command(stream, &shared, cmd, &mut request_id).await {
                    socket_failed = true;
                    break;
                }
            }
            while !socket_failed {
                match text_rx.try_recv() {
                    Ok(cmd) => {
                        if !handle_text_command(stream, &shared, cmd, &mut request_id).await {
                            socket_failed = true;
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }

            if socket_failed {
                ws_stream = None;
                prefetched_hint = None;
                set_shared_state(&shared, PageHintState::default());
                reject_pending_wheel_commands(&mut wheel_rx);
                reject_pending_text_commands(&mut text_rx);
                tokio::time::sleep(retry_backoff).await;
                continue;
            }

            let Some(mut hint) = (match prefetched_hint.take() {
                Some(hint) => Some(hint),
                None => evaluate_page_hint(stream, &video_probe_js, &mut request_id).await,
            }) else {
                ws_stream = None;
                prefetched_hint = None;
                set_shared_state(&shared, PageHintState::default());
                reject_pending_wheel_commands(&mut wheel_rx);
                reject_pending_text_commands(&mut text_rx);
                tokio::time::sleep(retry_backoff).await;
                continue;
            };

            if !hint.visible {
                debug!(
                    target = ws_url.as_deref().unwrap_or("<unknown>"),
                    focused = hint.focused,
                    "cdp target hidden; refreshing"
                );
                ws_stream = None;
                prefetched_hint = None;
                force_target_refresh = true;
                set_shared_state(&shared, PageHintState::default());
                continue;
            }

            update_seq = update_seq.wrapping_add(1).max(1);
            hint.update_seq = update_seq;
            set_shared_state(&shared, hint);
            let sleep = tokio::time::sleep(poll_interval);
            tokio::pin!(sleep);
            loop {
                tokio::select! {
                    _ = &mut sleep => break,
                    maybe_cmd = wheel_rx.recv() => {
                        let Some(cmd) = maybe_cmd else {
                            break;
                        };
                        let Some(stream) = ws_stream.as_mut() else {
                            let _ = cmd.response.send(false);
                            continue;
                        };
                        if !handle_wheel_command(stream, &shared, cmd, &mut request_id).await {
                            ws_stream = None;
                            prefetched_hint = None;
                            force_target_refresh = true;
                            set_shared_state(&shared, PageHintState::default());
                            break;
                        }
                    }
                    maybe_cmd = text_rx.recv() => {
                        let Some(cmd) = maybe_cmd else {
                            break;
                        };
                        let Some(stream) = ws_stream.as_mut() else {
                            let _ = cmd.response.send(false);
                            continue;
                        };
                        if !handle_text_command(stream, &shared, cmd, &mut request_id).await {
                            ws_stream = None;
                            prefetched_hint = None;
                            force_target_refresh = true;
                            set_shared_state(&shared, PageHintState::default());
                            break;
                        }
                    }
                }
            }
        }
    });
    BrowserCdpHandle {
        task,
        wheel_tx,
        text_tx,
        wheel_timeout,
        wheel_step_interval,
        text_timeout,
    }
}

async fn fetch_targets(http: &reqwest::Client, port: u16) -> Option<Vec<DevToolsTarget>> {
    let endpoint = format!("http://127.0.0.1:{port}/json/list");
    http.get(endpoint)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json::<Vec<DevToolsTarget>>()
        .await
        .ok()
}

fn score_target(target: &DevToolsTarget) -> i32 {
    let mut score = 0i32;
    match target.target_type.as_str() {
        "page" => score += 50,
        "tab" => score += 40,
        _ => score -= 20,
    }

    let url = target
        .url
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if url.starts_with("http://") || url.starts_with("https://") {
        score += 25;
    }
    if url.starts_with("chrome://") || url.starts_with("chrome-extension://") {
        score -= 100;
    }
    if url.contains("youtube")
        || url.contains("vimeo")
        || url.contains("twitch")
        || url.contains("netflix")
        || url.contains("primevideo")
    {
        score += 35;
    }

    let title = target
        .title
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if title.contains("youtube")
        || title.contains("video")
        || title.contains("player")
        || title.contains("stream")
    {
        score += 10;
    }

    score
}

fn ordered_target_candidates<'a>(
    targets: &'a [DevToolsTarget],
    current_ws_url: Option<&str>,
) -> Vec<&'a DevToolsTarget> {
    let mut candidates: Vec<(usize, &DevToolsTarget)> = targets
        .iter()
        .enumerate()
        .filter(|(_, target)| target.web_socket_debugger_url.is_some())
        .collect();

    candidates.sort_by(|(left_idx, left), (right_idx, right)| {
        let left_current = current_ws_url
            .map(|url| left.web_socket_debugger_url.as_deref() == Some(url))
            .unwrap_or(false);
        let right_current = current_ws_url
            .map(|url| right.web_socket_debugger_url.as_deref() == Some(url))
            .unwrap_or(false);
        right_current
            .cmp(&left_current)
            .then_with(|| score_target(right).cmp(&score_target(left)))
            .then_with(|| left_idx.cmp(right_idx))
    });

    candidates.into_iter().map(|(_, target)| target).collect()
}

async fn connect_visible_target_stream(
    http: &reqwest::Client,
    port: u16,
    current_ws_url: Option<&str>,
    video_probe_js: &str,
    request_id: &mut u64,
) -> Option<(String, CdpWsStream, PageHintState)> {
    let targets = fetch_targets(http, port).await?;
    let candidates = ordered_target_candidates(&targets, current_ws_url);
    let mut visible_fallback: Option<(String, CdpWsStream, PageHintState, String, String)> = None;

    for target in candidates {
        let Some(url) = target.web_socket_debugger_url.as_deref() else {
            continue;
        };
        let title = target.title.clone().unwrap_or_default();
        let url_hint = target.url.clone().unwrap_or_default();
        let Ok((mut stream, _)) = connect_async(url).await else {
            continue;
        };
        let Some(hint) = evaluate_page_hint(&mut stream, video_probe_js, request_id).await else {
            continue;
        };
        if !hint.visible {
            debug!(
                target = %url,
                focused = hint.focused,
                title = title,
                url_hint = url_hint,
                "cdp skipping hidden target"
            );
            continue;
        }
        if hint.focused {
            debug!(
                target = %url,
                focused = hint.focused,
                title = title,
                url_hint = url_hint,
                "cdp selected focused target"
            );
            return Some((url.to_string(), stream, hint));
        }
        if visible_fallback.is_none() {
            visible_fallback = Some((url.to_string(), stream, hint, title, url_hint));
        }
    }

    if let Some((url, stream, hint, title, url_hint)) = visible_fallback {
        debug!(
            target = %url,
            focused = hint.focused,
            title = title,
            url_hint = url_hint,
            "cdp selected visible fallback target"
        );
        return Some((url, stream, hint));
    }

    None
}

async fn evaluate_page_hint(
    stream: &mut CdpWsStream,
    video_probe_js: &str,
    request_id: &mut u64,
) -> Option<PageHintState> {
    let result = send_cdp_command(
        stream,
        request_id,
        "Runtime.evaluate",
        json!({
            "expression": video_probe_js,
            "returnByValue": true,
            "awaitPromise": false
        }),
    )
    .await?;
    let raw_value = result.pointer("/result/value")?;
    Some(parse_page_hint(raw_value))
}

fn reject_pending_wheel_commands(wheel_rx: &mut mpsc::Receiver<WheelCommand>) {
    while let Ok(cmd) = wheel_rx.try_recv() {
        let _ = cmd.response.send(false);
    }
}

fn reject_pending_text_commands(text_rx: &mut mpsc::Receiver<TextCommand>) {
    while let Ok(cmd) = text_rx.try_recv() {
        let _ = cmd.response.send(false);
    }
}

async fn resize_window_via_stream(
    stream: &mut CdpWsStream,
    width: u32,
    height: u32,
    scale_milli: u16,
    request_id: &mut u64,
) -> bool {
    let Some(window) = cdp_window_info(stream, request_id).await else {
        return false;
    };

    let dip_width = screen_px_to_window_dips(width, scale_milli);
    let dip_height = screen_px_to_window_dips(height, scale_milli);
    let restore_state = window
        .window_state
        .clone()
        .filter(|state| state != "normal" && state != "minimized");

    if restore_state.is_some() {
        let _ = cdp_set_window_bounds(
            stream,
            request_id,
            window.window_id,
            json!({ "windowState": "normal" }),
        )
        .await;
        tokio::time::sleep(Duration::from_millis(WINDOW_STATE_SETTLE_MS)).await;
    }

    if !cdp_set_window_bounds(
        stream,
        request_id,
        window.window_id,
        json!({
            "left": 0,
            "top": 0,
            "width": dip_width,
            "height": dip_height
        }),
    )
    .await
    {
        return false;
    }

    if let Some(window_state) = restore_state {
        if !cdp_set_window_bounds(
            stream,
            request_id,
            window.window_id,
            json!({ "windowState": window_state }),
        )
        .await
        {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(WINDOW_STATE_SETTLE_MS)).await;
    }

    true
}

fn screen_px_to_window_dips(px: u32, scale_milli: u16) -> u32 {
    let scale = u32::from(scale_milli.max(1));
    ((px.saturating_mul(1000) + (scale / 2)) / scale).max(1)
}

async fn maybe_apply_resize_device_metrics(
    stream: &mut CdpWsStream,
    width: u32,
    height: u32,
    fallback_scale_milli: u16,
    video_probe_js: &str,
    request_id: &mut u64,
) -> bool {
    let settled_hint = settle_page_hint_after_resize(stream, video_probe_js, request_id).await;
    let scale_milli = settled_hint
        .as_ref()
        .map(|hint| hint.device_scale_factor_milli)
        .unwrap_or(fallback_scale_milli)
        .max(1);
    let params = build_device_metrics_override(width, height, scale_milli);
    let applied = send_cdp_command(
        stream,
        request_id,
        "Emulation.setDeviceMetricsOverride",
        params,
    )
    .await
    .is_some();
    if applied {
        debug!(
            width,
            height,
            scale_milli,
            css_width = screen_px_to_window_dips(width, scale_milli),
            css_height = screen_px_to_window_dips(height, scale_milli),
            "cdp resize metrics override applied"
        );
        let _ = maybe_apply_horizontal_overflow_fix(stream, request_id).await;
    }
    applied
}

async fn settle_page_hint_after_resize(
    stream: &mut CdpWsStream,
    video_probe_js: &str,
    request_id: &mut u64,
) -> Option<PageHintState> {
    tokio::time::sleep(Duration::from_millis(RESIZE_METRICS_SETTLE_MS)).await;
    let mut last_hint: Option<PageHintState> = None;
    for attempt in 0..RESIZE_METRICS_ATTEMPTS {
        let hint = evaluate_page_hint(stream, video_probe_js, request_id).await?;
        if let Some(previous) = last_hint {
            if page_hint_is_stable(&previous, &hint) {
                return Some(hint);
            }
        }
        last_hint = Some(hint);
        if attempt + 1 < RESIZE_METRICS_ATTEMPTS {
            tokio::time::sleep(Duration::from_millis(RESIZE_METRICS_POLL_MS)).await;
        }
    }
    last_hint
}

fn page_hint_is_stable(previous: &PageHintState, current: &PageHintState) -> bool {
    previous.visible
        && current.visible
        && previous.viewport.is_some()
        && current.viewport.is_some()
        && previous.device_scale_factor_milli == current.device_scale_factor_milli
        && previous.viewport == current.viewport
}

fn build_device_metrics_override(width: u32, height: u32, scale_milli: u16) -> Value {
    let css_width = screen_px_to_window_dips(width, scale_milli);
    let css_height = screen_px_to_window_dips(height, scale_milli);
    json!({
        "width": css_width,
        "height": css_height,
        "deviceScaleFactor": f64::from(scale_milli.max(1)) / 1000.0,
        "mobile": false,
        "screenWidth": css_width,
        "screenHeight": css_height,
        "positionX": 0,
        "positionY": 0,
        "scale": 1
    })
}

async fn maybe_apply_horizontal_overflow_fix(
    stream: &mut CdpWsStream,
    request_id: &mut u64,
) -> bool {
    let Some(state) = evaluate_responsive_overflow_state(stream, request_id).await else {
        return false;
    };
    let enable = should_hide_horizontal_overflow(&state);
    if enable {
        debug!(
            url = state.url,
            client_width = state.client_width,
            scroll_width = state.scroll_width,
            body_scroll_width = state.body_scroll_width,
            "cdp applying horizontal overflow fix"
        );
    }
    set_horizontal_overflow_fix(stream, request_id, enable).await
}

async fn evaluate_responsive_overflow_state(
    stream: &mut CdpWsStream,
    request_id: &mut u64,
) -> Option<ResponsiveOverflowState> {
    let result = send_cdp_command(
        stream,
        request_id,
        "Runtime.evaluate",
        json!({
            "expression": "(() => ({ clientWidth: document.documentElement ? (document.documentElement.clientWidth || 0) : (window.innerWidth || 0), scrollWidth: document.documentElement ? (document.documentElement.scrollWidth || 0) : 0, bodyScrollWidth: document.body ? (document.body.scrollWidth || 0) : 0, narrowBreakpoint: matchMedia('(max-width: 1000px)').matches, url: location.href || '' }))()",
            "returnByValue": true,
            "awaitPromise": false
        }),
    )
    .await?;
    parse_responsive_overflow_state(result.pointer("/result/value")?)
}

fn parse_responsive_overflow_state(raw: &Value) -> Option<ResponsiveOverflowState> {
    Some(ResponsiveOverflowState {
        client_width: parse_i64(raw.get("clientWidth")?)
            .and_then(|n| u32::try_from(n.max(0)).ok())?,
        scroll_width: parse_i64(raw.get("scrollWidth")?)
            .and_then(|n| u32::try_from(n.max(0)).ok())?,
        body_scroll_width: parse_i64(raw.get("bodyScrollWidth")?)
            .and_then(|n| u32::try_from(n.max(0)).ok())?,
        narrow_breakpoint: parse_bool(raw.get("narrowBreakpoint")?).unwrap_or(false),
        url: raw.get("url")?.as_str()?.to_string(),
    })
}

fn should_hide_horizontal_overflow(state: &ResponsiveOverflowState) -> bool {
    if !state.narrow_breakpoint {
        return false;
    }
    if !state.url.starts_with("http://") && !state.url.starts_with("https://") {
        return false;
    }
    let overflow_width = state
        .scroll_width
        .max(state.body_scroll_width)
        .saturating_sub(state.client_width);
    overflow_width >= RESPONSIVE_OVERFLOW_THRESHOLD_CSS_PX
        && state.client_width <= RESPONSIVE_OVERFLOW_BREAKPOINT_CSS_PX
}

async fn set_horizontal_overflow_fix(
    stream: &mut CdpWsStream,
    request_id: &mut u64,
    enable: bool,
) -> bool {
    let expression = if enable {
        "(() => { const id = '__bpaneOverflowFix'; let style = document.getElementById(id); if (!style) { style = document.createElement('style'); style.id = id; (document.head || document.documentElement || document.body).appendChild(style); } style.textContent = 'html, body { overflow-x: hidden !important; }'; return true; })()"
    } else {
        "(() => { const style = document.getElementById('__bpaneOverflowFix'); if (style) style.remove(); return true; })()"
    };
    send_cdp_command(
        stream,
        request_id,
        "Runtime.evaluate",
        json!({
            "expression": expression,
            "returnByValue": true,
            "awaitPromise": false
        }),
    )
    .await
    .is_some()
}

async fn cdp_window_info(stream: &mut CdpWsStream, request_id: &mut u64) -> Option<CdpWindowInfo> {
    let result =
        send_cdp_command(stream, request_id, "Browser.getWindowForTarget", json!({})).await?;
    let window_id = result
        .get("windowId")
        .and_then(parse_i64)
        .and_then(|id| u32::try_from(id).ok())?;
    let window_state = result
        .pointer("/bounds/windowState")
        .and_then(Value::as_str)
        .map(str::to_owned);
    Some(CdpWindowInfo {
        window_id,
        window_state,
    })
}

async fn cdp_set_window_bounds(
    stream: &mut CdpWsStream,
    request_id: &mut u64,
    window_id: u32,
    bounds: Value,
) -> bool {
    send_cdp_command(
        stream,
        request_id,
        "Browser.setWindowBounds",
        json!({
            "windowId": window_id,
            "bounds": bounds
        }),
    )
    .await
    .is_some()
}

async fn send_cdp_command(
    stream: &mut CdpWsStream,
    request_id: &mut u64,
    method: &str,
    params: Value,
) -> Option<Value> {
    let current_request_id = *request_id;
    *request_id = request_id.wrapping_add(1).max(1);
    let cmd = json!({
        "id": current_request_id,
        "method": method,
        "params": params
    })
    .to_string();

    if stream.send(Message::Text(cmd.into())).await.is_err() {
        return None;
    }

    let deadline = tokio::time::Instant::now() + Duration::from_millis(EVAL_TIMEOUT_MS);
    while tokio::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, stream.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                if let Some(result) = parse_cdp_command_reply(text.as_ref(), current_request_id) {
                    return result;
                }
            }
            Ok(Some(Ok(Message::Binary(bytes)))) => {
                if let Ok(text) = std::str::from_utf8(bytes.as_ref()) {
                    if let Some(result) = parse_cdp_command_reply(text, current_request_id) {
                        return result;
                    }
                }
            }
            Ok(Some(Ok(Message::Ping(payload)))) => {
                if stream.send(Message::Pong(payload)).await.is_err() {
                    return None;
                }
            }
            Ok(Some(Ok(Message::Close(_)))) | Ok(Some(Err(_))) | Ok(None) => return None,
            Ok(Some(Ok(_))) => {}
            Err(_) => break,
        }
    }

    None
}

fn parse_cdp_command_reply(payload: &str, request_id: u64) -> Option<Option<Value>> {
    let value: Value = serde_json::from_str(payload).ok()?;
    if value.get("id").and_then(Value::as_u64) != Some(request_id) {
        return None;
    }
    if value.get("error").is_some() {
        return Some(None);
    }
    Some(Some(value.get("result").cloned().unwrap_or(Value::Null)))
}

#[cfg(test)]
fn parse_eval_reply(payload: &str, request_id: u64) -> Option<PageHintState> {
    let value: Value = serde_json::from_str(payload).ok()?;
    if value.get("id").and_then(Value::as_u64) != Some(request_id) {
        return None;
    }

    if value.get("error").is_some() {
        return Some(PageHintState::default());
    }

    let result = value.pointer("/result/result")?;
    let raw_value = result.get("value")?;
    Some(parse_page_hint(raw_value))
}

fn parse_page_hint(raw: &Value) -> PageHintState {
    if raw.is_null() {
        return PageHintState::default();
    }

    if let Some(obj) = raw.as_object() {
        let region_kind = obj
            .get("regionKind")
            .and_then(parse_region_kind)
            .unwrap_or(HintRegionKind::Video);
        let device_scale_factor_milli = obj
            .get("deviceScaleFactorMilli")
            .and_then(parse_i64)
            .map(|n| n.clamp(1, u16::MAX as i64) as u16)
            .unwrap_or(1000);
        let scroll_y = obj.get("scrollY").and_then(parse_i64);
        let scroll_delta_y = obj
            .get("scrollDeltaY")
            .and_then(parse_i64)
            .map(|n| n.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
            .unwrap_or(0);
        let visible = obj.get("visible").and_then(parse_bool).unwrap_or(false);
        let focused = obj.get("focused").and_then(parse_bool).unwrap_or(false);
        let video_region = obj
            .get("region")
            .and_then(|value| parse_region(value, region_kind))
            .or_else(|| parse_region(raw, region_kind));
        let viewport = obj.get("viewport").and_then(parse_viewport);
        return PageHintState {
            video_region,
            region_kind,
            viewport,
            device_scale_factor_milli,
            scroll_y,
            scroll_delta_y,
            visible,
            focused,
            update_seq: 0,
        };
    }

    PageHintState {
        video_region: parse_region(raw, HintRegionKind::Video),
        ..PageHintState::default()
    }
}

fn parse_region(raw: &Value, kind: HintRegionKind) -> Option<CaptureRegion> {
    let x = parse_i64(raw.get("x")?)?;
    let y = parse_i64(raw.get("y")?)?;
    let w = parse_i64(raw.get("w")?)?;
    let h = parse_i64(raw.get("h")?)?;

    if w <= 0 || h <= 0 {
        return None;
    }

    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = x.saturating_add(w).max(0);
    let y1 = y.saturating_add(h).max(0);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }

    let width = (x1 - x0) as u32;
    let height = (y1 - y0) as u32;
    let (min_width, min_height) = match kind {
        HintRegionKind::Video => (MIN_VIDEO_HINT_WIDTH, MIN_VIDEO_HINT_HEIGHT),
        HintRegionKind::Editable => (MIN_EDITABLE_HINT_WIDTH, MIN_EDITABLE_HINT_HEIGHT),
    };
    if width < min_width || height < min_height {
        return None;
    }

    let mut width = width;
    let mut height = height;
    if width & 1 == 1 {
        width = width.saturating_sub(1);
    }
    if height & 1 == 1 {
        height = height.saturating_sub(1);
    }
    if width < 2 || height < 2 {
        return None;
    }

    Some(CaptureRegion {
        x: x0 as u32,
        y: y0 as u32,
        w: width,
        h: height,
    })
}

fn parse_region_kind(raw: &Value) -> Option<HintRegionKind> {
    let kind = raw.as_str()?.trim().to_ascii_lowercase();
    match kind.as_str() {
        "video" => Some(HintRegionKind::Video),
        "editable" => Some(HintRegionKind::Editable),
        _ => None,
    }
}

fn parse_viewport(raw: &Value) -> Option<CaptureRegion> {
    let x = parse_i64(raw.get("x")?)?;
    let y = parse_i64(raw.get("y")?)?;
    let w = parse_i64(raw.get("w")?)?;
    let h = parse_i64(raw.get("h")?)?;

    if w <= 0 || h <= 0 {
        return None;
    }

    let x0 = x.max(0) as u32;
    let y0 = y.max(0) as u32;
    let width = w.max(0) as u32;
    let height = h.max(0) as u32;

    if width < 10 || height < 10 {
        return None;
    }

    Some(CaptureRegion {
        x: x0,
        y: y0,
        w: width,
        h: height,
    })
}

fn parse_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|n| i64::try_from(n).ok()))
        .or_else(|| value.as_f64().map(|n| n.round() as i64))
}

fn parse_bool(value: &Value) -> Option<bool> {
    value.as_bool()
}

fn env_bool(name: &str, default: bool) -> bool {
    match std::env::var(name) {
        Ok(raw) => match raw.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "0" | "false" | "no" | "off" => false,
            _ => default,
        },
        Err(_) => default,
    }
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(default)
}

fn env_u16(name: &str, default: u16) -> u16 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<u16>().ok())
        .unwrap_or(default)
}

fn set_shared_state(shared: &Arc<Mutex<PageHintState>>, next: PageHintState) {
    let mut guard = match shared.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    // Preserve viewport once established — it only changes on display resize,
    // and transient CDP errors should not wipe it out.
    let mut final_state = next;
    if final_state.viewport.is_none() && guard.viewport.is_some() {
        final_state.viewport = guard.viewport;
        final_state.device_scale_factor_milli = guard.device_scale_factor_milli;
    }
    if *guard != final_state {
        *guard = final_state;
    }
}

fn build_wheel_dispatch(
    hint: &PageHintState,
    screen_pos: Option<(u16, u16)>,
    dx: i16,
    dy: i16,
    step_px: u16,
) -> Option<WheelDispatch> {
    if step_px == 0 || (dx == 0 && dy == 0) {
        return None;
    }
    let viewport = hint.viewport?;
    let scale = (hint.device_scale_factor_milli.max(1) as f64) / 1000.0;
    let css_step = ((step_px as f64) / scale).round().max(1.0) as i32;
    let viewport_right = viewport.x.saturating_add(viewport.w);
    let viewport_bottom = viewport.y.saturating_add(viewport.h);
    let default_screen_x = viewport.x.saturating_add(viewport.w / 2) as u16;
    let default_screen_y = viewport.y.saturating_add(viewport.h / 2) as u16;
    let (screen_x, screen_y) = match screen_pos {
        Some((x, y))
            if (x as u32) >= viewport.x
                && (x as u32) < viewport_right
                && (y as u32) >= viewport.y
                && (y as u32) < viewport_bottom =>
        {
            (x, y)
        }
        _ => (default_screen_x, default_screen_y),
    };
    let css_w = (viewport.w as f64 / scale).max(1.0);
    let css_h = (viewport.h as f64 / scale).max(1.0);
    let css_x = (((screen_x as f64) - viewport.x as f64) / scale).clamp(0.0, css_w - 1.0);
    let css_y = (((screen_y as f64) - viewport.y as f64) / scale).clamp(0.0, css_h - 1.0);
    Some(WheelDispatch {
        x: css_x,
        y: css_y,
        delta_x: dx as i32 * css_step,
        delta_y: -(dy as i32) * css_step,
    })
}

fn wheel_dispatch_step_count(dx: i16, dy: i16) -> u16 {
    dx.unsigned_abs().max(dy.unsigned_abs())
}

fn wheel_command_timeout(base: Duration, step_interval: Duration, dx: i16, dy: i16) -> Duration {
    let extra_steps = wheel_dispatch_step_count(dx, dy).saturating_sub(1) as u32;
    let extra = step_interval
        .checked_mul(extra_steps)
        .unwrap_or(Duration::from_millis(DEFAULT_WHEEL_CMD_TIMEOUT_MS));
    base.saturating_add(extra)
}

fn paced_wheel_components(dx: i16, dy: i16) -> Vec<(i16, i16)> {
    let steps = wheel_dispatch_step_count(dx, dy);
    if steps == 0 {
        return Vec::new();
    }
    let dx_steps = dx.unsigned_abs();
    let dy_steps = dy.unsigned_abs();
    let dx_sign = dx.signum();
    let dy_sign = dy.signum();
    let mut out = Vec::with_capacity(steps as usize);
    for idx in 0..steps {
        let step_dx = if idx < dx_steps { dx_sign } else { 0 };
        let step_dy = if idx < dy_steps { dy_sign } else { 0 };
        out.push((step_dx, step_dy));
    }
    out
}

async fn handle_wheel_command(
    stream: &mut CdpWsStream,
    shared: &Arc<Mutex<PageHintState>>,
    cmd: WheelCommand,
    request_id: &mut u64,
) -> bool {
    if tokio::time::Instant::now() > cmd.deadline {
        let _ = cmd.response.send(false);
        return true;
    }
    let hint = match shared.lock() {
        Ok(g) => *g,
        Err(poisoned) => *poisoned.into_inner(),
    };
    let dispatches: Vec<WheelDispatch> = paced_wheel_components(cmd.dx, cmd.dy)
        .into_iter()
        .filter_map(|(step_dx, step_dy)| {
            build_wheel_dispatch(&hint, cmd.screen_pos, step_dx, step_dy, cmd.step_px)
        })
        .collect();
    if dispatches.is_empty() {
        let _ = cmd.response.send(false);
        return true;
    }
    let mut sent_any = false;
    for (idx, dispatch) in dispatches.into_iter().enumerate() {
        if idx > 0 && !cmd.step_interval.is_zero() {
            if tokio::time::Instant::now() > cmd.deadline {
                let _ = cmd.response.send(sent_any);
                return true;
            }
            tokio::time::sleep(cmd.step_interval).await;
        }
        if tokio::time::Instant::now() > cmd.deadline {
            let _ = cmd.response.send(sent_any);
            return true;
        }
        let current_request_id = *request_id;
        *request_id = request_id.wrapping_add(1).max(1);
        let payload = json!({
            "id": current_request_id,
            "method": "Input.dispatchMouseEvent",
            "params": {
                "type": "mouseWheel",
                "x": dispatch.x,
                "y": dispatch.y,
                "deltaX": dispatch.delta_x,
                "deltaY": dispatch.delta_y,
                "pointerType": "mouse"
            }
        })
        .to_string();
        if stream.send(Message::Text(payload.into())).await.is_err() {
            let _ = cmd.response.send(sent_any);
            return false;
        }
        sent_any = true;
    }
    let _ = cmd.response.send(sent_any);
    true
}

async fn handle_text_command(
    stream: &mut CdpWsStream,
    shared: &Arc<Mutex<PageHintState>>,
    cmd: TextCommand,
    request_id: &mut u64,
) -> bool {
    if tokio::time::Instant::now() > cmd.deadline {
        let _ = cmd.response.send(false);
        return true;
    }

    let hint = match shared.lock() {
        Ok(g) => *g,
        Err(poisoned) => *poisoned.into_inner(),
    };
    if !hint.visible || !hint.focused {
        let _ = cmd.response.send(false);
        return true;
    }

    let sent = send_cdp_command(
        stream,
        request_id,
        "Input.insertText",
        json!({
            "text": cmd.text,
        }),
    )
    .await
    .is_some();
    let _ = cmd.response.send(sent);
    sent
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_region_clamps_negative_origin() {
        let raw = json!({
            "x": -40,
            "y": -20,
            "w": 320,
            "h": 180
        });
        let region = parse_region(&raw, HintRegionKind::Video).expect("region");
        assert_eq!(region.x, 0);
        assert_eq!(region.y, 0);
        assert_eq!(region.w, 280);
        assert_eq!(region.h, 160);
    }

    #[test]
    fn parse_region_accepts_small_editable_hint() {
        let raw = json!({
            "x": 20,
            "y": 30,
            "w": 140,
            "h": 44
        });
        let region = parse_region(&raw, HintRegionKind::Editable).expect("region");
        assert_eq!(region.x, 20);
        assert_eq!(region.y, 30);
        assert_eq!(region.w, 140);
        assert_eq!(region.h, 44);
        assert!(parse_region(&raw, HintRegionKind::Video).is_none());
    }

    #[test]
    fn parse_eval_reply_matches_request_id() {
        let payload = json!({
            "id": 7,
            "result": {
                "result": {
                    "value": {
                        "scrollY": 320,
                        "scrollDeltaY": 64,
                        "region": { "x": 100, "y": 120, "w": 640, "h": 360 }
                    }
                }
            }
        })
        .to_string();
        let parsed = parse_eval_reply(&payload, 7).expect("parsed");
        assert_eq!(parsed.scroll_y, Some(320));
        assert_eq!(parsed.scroll_delta_y, 64);
        assert_eq!(parsed.device_scale_factor_milli, 1000);
        let region = parsed.video_region.expect("region");
        assert_eq!(region.x, 100);
        assert_eq!(region.y, 120);
        assert_eq!(region.w, 640);
        assert_eq!(region.h, 360);
    }

    #[test]
    fn parse_eval_reply_ignores_other_request_id() {
        let payload = json!({
            "id": 9,
            "result": { "result": { "value": null } }
        })
        .to_string();
        assert!(parse_eval_reply(&payload, 1).is_none());
    }

    #[test]
    fn parse_eval_reply_accepts_legacy_region_shape() {
        let payload = json!({
            "id": 11,
            "result": {
                "result": {
                    "value": { "x": 20, "y": 30, "w": 320, "h": 180 }
                }
            }
        })
        .to_string();
        let parsed = parse_eval_reply(&payload, 11).expect("parsed");
        assert_eq!(parsed.scroll_y, None);
        assert_eq!(parsed.scroll_delta_y, 0);
        let region = parsed.video_region.expect("region");
        assert_eq!(region.x, 20);
        assert_eq!(region.y, 30);
        assert_eq!(region.w, 320);
        assert_eq!(region.h, 180);
    }

    #[test]
    fn parse_eval_reply_reads_editable_region_kind() {
        let payload = json!({
            "id": 13,
            "result": {
                "result": {
                    "value": {
                        "regionKind": "editable",
                        "region": { "x": 50, "y": 70, "w": 180, "h": 40 }
                    }
                }
            }
        })
        .to_string();
        let parsed = parse_eval_reply(&payload, 13).expect("parsed");
        assert_eq!(parsed.region_kind, HintRegionKind::Editable);
        let region = parsed.video_region.expect("region");
        assert_eq!(region.x, 50);
        assert_eq!(region.y, 70);
        assert_eq!(region.w, 180);
        assert_eq!(region.h, 40);
    }

    #[test]
    fn parse_eval_reply_reads_visibility_flags() {
        let payload = json!({
            "id": 12,
            "result": {
                "result": {
                    "value": {
                        "visible": true,
                        "focused": false,
                        "viewport": { "x": 40, "y": 80, "w": 640, "h": 320 }
                    }
                }
            }
        })
        .to_string();
        let parsed = parse_eval_reply(&payload, 12).expect("parsed");
        assert!(parsed.visible);
        assert!(!parsed.focused);
        assert_eq!(
            parsed.viewport,
            Some(CaptureRegion {
                x: 40,
                y: 80,
                w: 640,
                h: 320,
            })
        );
    }

    #[test]
    fn page_hint_is_stable_requires_matching_viewport_and_scale() {
        let previous = PageHintState {
            visible: true,
            device_scale_factor_milli: 2000,
            viewport: Some(CaptureRegion {
                x: 0,
                y: 0,
                w: 1416,
                h: 1036,
            }),
            ..PageHintState::default()
        };
        let current = PageHintState {
            visible: true,
            device_scale_factor_milli: 2000,
            viewport: Some(CaptureRegion {
                x: 0,
                y: 0,
                w: 1416,
                h: 1036,
            }),
            ..PageHintState::default()
        };
        assert!(page_hint_is_stable(&previous, &current));

        let resized = PageHintState {
            viewport: Some(CaptureRegion {
                x: 0,
                y: 0,
                w: 1280,
                h: 960,
            }),
            ..current
        };
        assert!(!page_hint_is_stable(&previous, &resized));
    }

    #[test]
    fn build_device_metrics_override_uses_css_screen_size() {
        let params = build_device_metrics_override(1416, 1210, 2000);
        assert_eq!(params.get("width").and_then(Value::as_u64), Some(708));
        assert_eq!(params.get("height").and_then(Value::as_u64), Some(605));
        assert_eq!(params.get("screenWidth").and_then(Value::as_u64), Some(708));
        assert_eq!(
            params.get("screenHeight").and_then(Value::as_u64),
            Some(605)
        );
        assert_eq!(
            params.get("deviceScaleFactor").and_then(Value::as_f64),
            Some(2.0)
        );
    }

    #[test]
    fn parse_responsive_overflow_state_reads_expected_fields() {
        let raw = json!({
            "clientWidth": 693,
            "scrollWidth": 708,
            "bodyScrollWidth": 1000,
            "narrowBreakpoint": true,
            "url": "https://news.google.com/home"
        });
        assert_eq!(
            parse_responsive_overflow_state(&raw),
            Some(ResponsiveOverflowState {
                client_width: 693,
                scroll_width: 708,
                body_scroll_width: 1000,
                narrow_breakpoint: true,
                url: "https://news.google.com/home".to_string(),
            })
        );
    }

    #[test]
    fn should_hide_horizontal_overflow_requires_narrow_http_overflow() {
        assert!(should_hide_horizontal_overflow(&ResponsiveOverflowState {
            client_width: 693,
            scroll_width: 708,
            body_scroll_width: 1000,
            narrow_breakpoint: true,
            url: "https://news.google.com/home".to_string(),
        }));

        assert!(!should_hide_horizontal_overflow(&ResponsiveOverflowState {
            client_width: 693,
            scroll_width: 740,
            body_scroll_width: 760,
            narrow_breakpoint: true,
            url: "https://news.google.com/home".to_string(),
        }));

        assert!(!should_hide_horizontal_overflow(&ResponsiveOverflowState {
            client_width: 1201,
            scroll_width: 1600,
            body_scroll_width: 1600,
            narrow_breakpoint: false,
            url: "https://news.google.com/home".to_string(),
        }));

        assert!(!should_hide_horizontal_overflow(&ResponsiveOverflowState {
            client_width: 693,
            scroll_width: 708,
            body_scroll_width: 1000,
            narrow_breakpoint: true,
            url: "chrome://newtab/".to_string(),
        }));
    }

    #[test]
    fn build_wheel_dispatch_uses_pointer_inside_viewport() {
        let hint = PageHintState {
            viewport: Some(CaptureRegion {
                x: 100,
                y: 200,
                w: 800,
                h: 600,
            }),
            device_scale_factor_milli: 2000,
            ..PageHintState::default()
        };
        let dispatch = build_wheel_dispatch(&hint, Some((500, 500)), 1, -1, 64).expect("dispatch");
        assert_eq!(dispatch.x, 200.0);
        assert_eq!(dispatch.y, 150.0);
        assert_eq!(dispatch.delta_x, 32);
        assert_eq!(dispatch.delta_y, 32);
    }

    #[test]
    fn build_wheel_dispatch_falls_back_to_viewport_center() {
        let hint = PageHintState {
            viewport: Some(CaptureRegion {
                x: 40,
                y: 80,
                w: 640,
                h: 320,
            }),
            device_scale_factor_milli: 1000,
            ..PageHintState::default()
        };
        let dispatch = build_wheel_dispatch(&hint, Some((5, 5)), 0, 1, 64).expect("dispatch");
        assert_eq!(dispatch.x, 320.0);
        assert_eq!(dispatch.y, 160.0);
        assert_eq!(dispatch.delta_x, 0);
        assert_eq!(dispatch.delta_y, -64);
    }

    #[test]
    fn paced_wheel_components_split_multi_notch_vertical_input() {
        assert_eq!(paced_wheel_components(0, 3), vec![(0, 1), (0, 1), (0, 1)]);
        assert_eq!(paced_wheel_components(0, -2), vec![(0, -1), (0, -1)]);
    }

    #[test]
    fn paced_wheel_components_preserve_diagonal_magnitude() {
        assert_eq!(
            paced_wheel_components(2, -3),
            vec![(1, -1), (1, -1), (0, -1)]
        );
        assert_eq!(
            paced_wheel_components(-3, 1),
            vec![(-1, 1), (-1, 0), (-1, 0)]
        );
    }

    #[test]
    fn wheel_command_timeout_grows_with_paced_steps() {
        let base = Duration::from_millis(90);
        let step = Duration::from_millis(8);
        assert_eq!(wheel_command_timeout(base, step, 0, 1), base);
        assert_eq!(
            wheel_command_timeout(base, step, 0, 4),
            base + Duration::from_millis(24)
        );
    }

    #[test]
    fn parse_cdp_command_reply_returns_result_for_matching_id() {
        let payload = json!({
            "id": 42,
            "result": { "windowId": 7 }
        })
        .to_string();
        let result = parse_cdp_command_reply(&payload, 42)
            .expect("matching reply")
            .expect("successful result");
        assert_eq!(result.get("windowId").and_then(Value::as_u64), Some(7));
    }

    #[test]
    fn parse_cdp_command_reply_ignores_errors_and_other_ids() {
        let wrong_id = json!({
            "id": 8,
            "result": { "ok": true }
        })
        .to_string();
        assert!(parse_cdp_command_reply(&wrong_id, 9).is_none());

        let error = json!({
            "id": 9,
            "error": { "message": "failed" }
        })
        .to_string();
        assert_eq!(parse_cdp_command_reply(&error, 9), Some(None));
    }

    #[test]
    fn screen_px_to_window_dips_respects_device_scale_factor() {
        assert_eq!(screen_px_to_window_dips(2924, 2000), 1462);
        assert_eq!(screen_px_to_window_dips(2008, 2000), 1004);
        assert_eq!(screen_px_to_window_dips(1272, 1000), 1272);
        assert_eq!(screen_px_to_window_dips(1, 3000), 1);
    }

    #[test]
    fn text_command_timeout_floor_covers_probe_latency() {
        assert!(DEFAULT_TEXT_CMD_TIMEOUT_MS >= EVAL_TIMEOUT_MS + DEFAULT_POLL_MS);
        assert_eq!(DEFAULT_TEXT_CMD_TIMEOUT_MS, MIN_TEXT_CMD_TIMEOUT_MS);
    }

    #[test]
    fn editable_input_types_cover_text_entry_controls() {
        assert_eq!(
            EDITABLE_INPUT_TYPES,
            &["", "text", "password", "email", "search", "tel", "url", "number"]
        );
    }

    #[test]
    fn build_video_probe_js_includes_editable_input_allowlist() {
        let js = build_video_probe_js(false, DEFAULT_SCROLL_PAUSE_WINDOW_MS, 0);
        assert!(js.contains("'email'"));
        assert!(js.contains("'search'"));
        assert!(js.contains("'tel'"));
        assert!(js.contains("'url'"));
        assert!(js.contains("'number'"));
        assert!(!js.contains("__EDITABLE_INPUT_TYPES__"));
    }

    #[test]
    fn build_video_probe_js_supports_shadow_and_role_editables() {
        let js = build_video_probe_js(false, DEFAULT_SCROLL_PAUSE_WINDOW_MS, 0);
        assert!(js.contains("shadowRoot.activeElement"));
        assert!(js.contains("'searchbox'"));
        assert!(js.contains("'textbox'"));
        assert!(js.contains("'combobox'"));
    }

    #[test]
    fn build_video_probe_js_treats_role_textbox_as_editable() {
        let js = build_video_probe_js(false, DEFAULT_SCROLL_PAUSE_WINDOW_MS, 0);
        assert!(js.contains("const editableRoles = new Set(['textbox', 'searchbox', 'combobox'])"));
        assert!(js.contains("if (isRoleEditable(current))"));
        assert!(js.contains("aria-readonly"));
        assert!(js.contains("aria-disabled"));
    }

    #[test]
    fn build_video_probe_js_allows_tiny_focused_editables() {
        let js = build_video_probe_js(false, DEFAULT_SCROLL_PAUSE_WINDOW_MS, 0);
        assert!(js.contains("rect.width >= 2 && rect.height >= 2"));
    }
}
