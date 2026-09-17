//! 目录表文件系统监听：非递归监视条目父目录，事件去抖后增量刷新 catalog
//! 并广播 SSE。周期性全量失效作为监听遗漏的兜底。

use crate::catalog::Catalog;
use crate::hub::Hub;
use notify::{RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

const DEBOUNCE: Duration = Duration::from_millis(200);
const WATCH_RESYNC: Duration = Duration::from_secs(5);
const FULL_REFRESH: Duration = Duration::from_secs(60);
const TICK: Duration = Duration::from_millis(100);
const FILES_CHANGED: &str = r#"{"type":"files_changed"}"#;

/// 启动目录表监听任务；token 取消后退出。
pub fn spawn_catalog_watcher(
    catalog: Arc<Catalog>,
    hub: Arc<Hub>,
    token: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut watcher = match notify::recommended_watcher(
            move |result: Result<notify::Event, notify::Error>| {
                let _ = tx.send(result);
            },
        ) {
            Ok(watcher) => watcher,
            Err(err) => {
                warn!("无法创建文件系统监听，回退为 TTL 刷新: {err}");
                while !token.is_cancelled() {
                    tokio::time::sleep(FULL_REFRESH).await;
                    catalog.mark_list_stale();
                    hub.publish(FILES_CHANGED.to_string());
                }
                return;
            }
        };

        let mut watched: HashSet<PathBuf> = HashSet::new();
        let mut pending: HashSet<PathBuf> = HashSet::new();
        let mut dirty = false;
        let mut last_event: Option<Instant> = None;
        let mut last_sync = Instant::now()
            .checked_sub(WATCH_RESYNC)
            .unwrap_or_else(Instant::now);
        let mut last_full = Instant::now();
        let mut tick = tokio::time::interval(TICK);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        // 初始：用当前缓存建立监听，并做一次列表预热。
        catalog.list();
        sync_watch_roots(&catalog, &mut watcher, &mut watched);

        while !token.is_cancelled() {
            tokio::select! {
                _ = token.cancelled() => break,
                maybe_event = rx.recv() => {
                    let Some(result) = maybe_event else { break };
                    match result {
                        Ok(event) => {
                            for path in event.paths {
                                pending.insert(path);
                            }
                            dirty = true;
                            last_event = Some(Instant::now());
                        }
                        Err(err) => debug!("文件系统监听错误: {err}"),
                    }
                }
                _ = tick.tick() => {
                    if dirty {
                        let ready = last_event
                            .map(|at| at.elapsed() >= DEBOUNCE)
                            .unwrap_or(true);
                        if ready {
                            let paths = std::mem::take(&mut pending);
                            let mut changed = false;
                            for path in &paths {
                                if catalog.apply_path_event(path) {
                                    changed = true;
                                }
                            }
                            // 父目录级事件（重命名旧路径等）也可能只命中目录本身，
                            // 若精确匹配无变化，保守地使列表缓存过期一次。
                            if !changed && !paths.is_empty() {
                                catalog.mark_list_stale();
                                changed = true;
                            }
                            dirty = false;
                            if changed {
                                hub.publish(FILES_CHANGED.to_string());
                            }
                        }
                    }

                    if last_sync.elapsed() >= WATCH_RESYNC {
                        sync_watch_roots(&catalog, &mut watcher, &mut watched);
                        last_sync = Instant::now();
                    }

                    if last_full.elapsed() >= FULL_REFRESH {
                        catalog.mark_list_stale();
                        last_full = Instant::now();
                    }
                }
            }
        }

        for root in &watched {
            let _ = watcher.unwatch(root);
        }
    })
}

fn sync_watch_roots(
    catalog: &Catalog,
    watcher: &mut notify::RecommendedWatcher,
    watched: &mut HashSet<PathBuf>,
) {
    let desired: HashSet<PathBuf> = catalog.watch_roots().into_iter().collect();
    let to_remove: Vec<PathBuf> = watched.difference(&desired).cloned().collect();
    for root in to_remove {
        let _ = watcher.unwatch(&root);
        watched.remove(&root);
    }
    let to_add: Vec<PathBuf> = desired.difference(watched).cloned().collect();
    for root in to_add {
        match watcher.watch(&root, RecursiveMode::NonRecursive) {
            Ok(()) => {
                watched.insert(root);
            }
            Err(err) => {
                debug!("跳过监听 {root:?}: {err}");
            }
        }
    }
}
