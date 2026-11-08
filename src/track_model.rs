//! Retained page rows: cover/like updates visit only matching track identities.
use crate::{MainWindow, TrackRow};
use slint::{Model, ModelRc, VecModel};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

pub struct TrackModel {
    rows: VecModel<TrackRow>,
    paths: RefCell<HashMap<String, Vec<usize>>>,
}

impl TrackModel {
    pub fn new(rows: Vec<TrackRow>) -> Self {
        let result = Self {
            rows: VecModel::default(),
            paths: RefCell::default(),
        };
        result.replace(rows);
        result
    }

    /// Preserve delegates when only row values change. Structural edits retain
    /// the common prefix/suffix (especially removing a song from Liked).
    pub fn replace(&self, rows: Vec<TrackRow>) {
        let old_len = self.rows.row_count();
        let mut prefix = 0;
        while prefix < old_len.min(rows.len())
            && self.rows.row_data(prefix).unwrap().path == rows[prefix].path
        {
            prefix += 1;
        }
        let mut suffix = 0;
        while suffix < (old_len - prefix).min(rows.len() - prefix)
            && self.rows.row_data(old_len - suffix - 1).unwrap().path
                == rows[rows.len() - suffix - 1].path
        {
            suffix += 1;
        }
        if prefix != old_len || prefix != rows.len() {
            let mut paths = HashMap::<_, Vec<_>>::new();
            for (i, row) in rows.iter().enumerate() {
                paths.entry(row.path.to_string()).or_default().push(i);
            }
            *self.paths.borrow_mut() = paths;
        }
        // A wholesale reorder should not shift a large Vec once per row.
        if (old_len - prefix - suffix).max(rows.len() - prefix - suffix) > 32 {
            self.rows.set_vec(rows);
            return;
        }
        for _ in prefix..old_len - suffix {
            self.rows.remove(prefix);
        }
        for (i, row) in rows
            .iter()
            .enumerate()
            .take(rows.len() - suffix)
            .skip(prefix)
        {
            self.rows.insert(i, row.clone());
        }
        for (i, row) in rows.into_iter().enumerate() {
            if self.rows.row_data(i).as_ref() != Some(&row) {
                self.rows.set_row_data(i, row);
            }
        }
    }

    pub fn update_path(&self, path: &str, mut update: impl FnMut(&mut TrackRow)) {
        let indices = self.paths.borrow().get(path).cloned().unwrap_or_default();
        for i in indices {
            let Some(mut row) = self.rows.row_data(i) else {
                continue;
            };
            let previous = row.clone();
            update(&mut row);
            // Identity belongs to the model; callers edit presentation only.
            row.path = previous.path.clone();
            if row != previous {
                self.rows.set_row_data(i, row);
            }
        }
    }
}

impl Model for TrackModel {
    type Data = TrackRow;
    fn row_count(&self) -> usize {
        self.rows.row_count()
    }
    fn row_data(&self, row: usize) -> Option<TrackRow> {
        self.rows.row_data(row)
    }
    fn model_tracker(&self) -> &dyn slint::ModelTracker {
        self.rows.model_tracker()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub(crate) fn mounted(w: &MainWindow) -> Vec<ModelRc<TrackRow>> {
    let mut models = Vec::new();
    for model in [
        w.get_tracks(),
        w.get_home_page_tracks(),
        w.get_home_recommendation_tracks(),
        w.get_explore_page_tracks(),
        w.get_library_page_tracks(),
        w.get_liked_page_tracks(),
        w.get_playlist_page_tracks(),
        w.get_queue_page_tracks(),
        w.get_search_page_tracks(),
        w.get_settings_page_tracks(),
        w.get_queue_tracks(),
    ] {
        if !models.contains(&model) {
            models.push(model);
        }
    }
    models
}

pub(crate) fn set_tracks(w: &MainWindow, rows: Vec<TrackRow>) {
    let previous = match w.get_active_view().as_str() {
        "home" => w.get_home_page_tracks(),
        "explore" => w.get_explore_page_tracks(),
        "library" => w.get_library_page_tracks(),
        "liked" => w.get_liked_page_tracks(),
        "playlist" => w.get_playlist_page_tracks(),
        "queue" => w.get_queue_page_tracks(),
        "search" => w.get_search_page_tracks(),
        "settings" => w.get_settings_page_tracks(),
        // Player entry must not replace any retained list.
        _ => return,
    };
    if let Some(model) = previous.as_any().downcast_ref::<TrackModel>() {
        model.replace(rows);
        w.set_tracks(previous);
    } else {
        w.set_tracks(Rc::new(TrackModel::new(rows)).into());
    }
}

pub(crate) fn set_home_recommendations(w: &MainWindow, rows: Vec<TrackRow>) {
    w.set_home_revision(w.get_home_revision().wrapping_add(1));
    let previous = w.get_home_recommendation_tracks();
    if let Some(model) = previous.as_any().downcast_ref::<TrackModel>() {
        model.replace(rows);
        w.set_home_recommendation_tracks(previous);
    } else {
        w.set_home_recommendation_tracks(Rc::new(TrackModel::new(rows)).into());
    }
}

/// Rebind the playlist's retained model after a batch of artwork updates.
/// This also invalidates Slint's indexed bindings used by its cover collage.
pub(crate) fn rebind_playlist(w: &MainWindow) {
    let previous = w.get_playlist_page_tracks();
    let rows = (0..previous.row_count())
        .filter_map(|index| previous.row_data(index))
        .collect();
    let model: ModelRc<TrackRow> = Rc::new(TrackModel::new(rows)).into();
    w.set_playlist_page_tracks(model.clone());
    w.set_tracks(model);
}

pub(crate) fn update_path(w: &MainWindow, path: &str, mut update: impl FnMut(&mut TrackRow)) {
    for model in mounted(w) {
        if let Some(rows) = model.as_any().downcast_ref::<TrackModel>() {
            rows.update_path(path, &mut update);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rows(paths: &[&str]) -> Vec<TrackRow> {
        paths
            .iter()
            .map(|path| TrackRow {
                path: (*path).into(),
                ..Default::default()
            })
            .collect()
    }
    #[test]
    fn model_rc_can_update_delayed_artwork_without_reopening() {
        let model: ModelRc<TrackRow> = Rc::new(TrackModel::new(rows(&["first", "second", "first"]))).into();
        let mut pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(1, 1);
        pixels.make_mut_bytes().copy_from_slice(&[255, 0, 0, 255]);
        let art = slint::Image::from_rgba8(pixels);
        let retained = model.as_any().downcast_ref::<TrackModel>()
            .expect("mounted ModelRc must expose its TrackModel");
        retained.update_path("first", |row| row.art = art.clone());
        assert_eq!(model.row_data(0).unwrap().art, art);
        assert_eq!(model.row_data(2).unwrap().art, art);
        assert_eq!(model.row_data(1).unwrap().art.size().width, 0);
    }
    #[test]
    fn keyed_updates_follow_removal_reorder_and_duplicate_paths() {
        let model = TrackModel::new(rows(&["a", "b", "a", "c"]));
        model.update_path("a", |r| r.liked = true);
        assert!(model.row_data(0).unwrap().liked && model.row_data(2).unwrap().liked);
        model.replace(rows(&["c", "a", "b"]));
        model.update_path("a", |r| r.downloading = true);
        assert!(model.row_data(1).unwrap().downloading);
        assert!(!model.row_data(0).unwrap().downloading);
        model.replace(rows(&["c", "b"]));
        model.update_path("b", |r| r.liked = true);
        assert!(model.row_data(1).unwrap().liked);
        model.replace(vec![]);
        model.update_path("b", |r| r.liked = true);
        assert_eq!(model.row_count(), 0);
    }
}
