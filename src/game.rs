//! Orbital-tree puzzle. Relative diagram radii determine the scored sibling order.
use serde::{Deserialize, Serialize};
use winit::keyboard::KeyCode;

use crate::sim::{BodyKind, Scene};
use crate::ui::{Rect, UiVertex, push_rect, push_text};

const MAX_NODES: usize = 64;
const INK: [f32; 4] = crate::ui::INK;
const MUTED: [f32; 4] = crate::ui::MUTED;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Traits {
    star: bool,
    rings: bool,
}

#[derive(Clone, Serialize, Deserialize)]
struct Node {
    parent: Option<usize>,
    pos: [f32; 2],
    /// Zero-based rank among siblings, from inner to outer orbit.
    orbit_rank: usize,
    traits: Traits,
}

impl Node {
    fn label(&self) -> String {
        if self.parent.is_none() {
            "C".into()
        } else {
            (self.orbit_rank + 1).to_string()
        }
    }
    fn reference(&self) -> String {
        if self.parent.is_none() {
            "C".into()
        } else {
            format!("#{}", self.orbit_rank + 1)
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Score {
    pub topology: f32,
    pub traits: f32,
    pub viewer: bool,
}

impl Score {
    pub fn total(self) -> f32 {
        self.topology * 0.6 + self.traits * 0.2 + if self.viewer { 20.0 } else { 0.0 }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Snapshot {
    nodes: Vec<Node>,
    selected: usize,
    viewer: Option<usize>,
    attempts: Vec<Score>,
    best: Option<f32>,
}

pub struct Game {
    pub open: bool,
    pub title: String,
    trait_pop: Option<(usize, usize, std::time::Instant)>,
    nodes: Vec<Node>,
    selected: usize,
    viewer: Option<usize>,
    dragging: Option<usize>,
    drag_saved: bool,
    undo: Vec<(Vec<Node>, Option<usize>, usize)>,
    redo: Vec<(Vec<Node>, Option<usize>, usize)>,
    pub confirm_clear: bool,
    pub result_open: bool,
    attempts: Vec<Score>,
    attempt_scroll: usize,
    best: Option<f32>,
    score: Option<Score>,
    message: String,
}

impl Default for Game {
    fn default() -> Self {
        Self {
            open: false,
            title: "First field study".into(),
            trait_pop: None,
            nodes: vec![Node {
                parent: None,
                pos: [0.5, 0.45],
                orbit_rank: 0,
                traits: Traits::default(),
            }],
            selected: 0,
            viewer: None,
            dragging: None,
            drag_saved: false,
            undo: Vec::new(),
            redo: Vec::new(),
            confirm_clear: false,
            result_open: false,
            attempts: Vec::new(),
            attempt_scroll: 0,
            best: None,
            score: None,
            message: "Select a parent, then right-click empty space to add.".into(),
        }
    }
}

impl Game {
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            nodes: self.nodes.clone(),
            selected: self.selected,
            viewer: self.viewer,
            attempts: self.attempts.clone(),
            best: self.best,
        }
    }
    pub fn restore(snapshot: &Snapshot) -> Option<Self> {
        let n = snapshot.nodes.len();
        if n == 0
            || n > MAX_NODES
            || snapshot.selected >= n
            || snapshot.viewer.is_some_and(|v| v >= n)
            || snapshot.nodes[0].parent.is_some()
            || snapshot
                .nodes
                .iter()
                .skip(1)
                .any(|node| node.parent.is_none())
        {
            return None;
        }
        for (i, node) in snapshot.nodes.iter().enumerate() {
            if node.pos.iter().any(|p| !p.is_finite() || p.abs() > 10000.0) {
                return None;
            }
            let mut next = Some(i);
            for step in 0..=n {
                let Some(j) = next else { break };
                if j >= n || step == n {
                    return None;
                }
                next = snapshot.nodes[j].parent;
            }
        }
        if snapshot
            .nodes
            .iter()
            .enumerate()
            .any(|(i, n)| n.parent.is_some_and(|p| p >= i))
        {
            return None;
        }
        if snapshot.attempts.len() > 1000
            || snapshot.attempts.iter().any(|s| {
                ![s.topology, s.traits]
                    .iter()
                    .all(|v| v.is_finite() && (0.0..=100.0).contains(v))
            })
            || snapshot
                .best
                .is_some_and(|b| !b.is_finite() || !(0.0..=100.0).contains(&b))
        {
            return None;
        }
        let mut game = Self {
            nodes: snapshot.nodes.clone(),
            selected: snapshot.selected,
            viewer: snapshot.viewer,
            attempts: snapshot.attempts.clone(),
            best: snapshot.best,
            ..Self::default()
        };
        game.reorder_by_distance();
        Some(game)
    }
    pub fn progress(&self) -> (usize, usize, Option<f32>) {
        (self.nodes.len() - 1, self.attempts.len(), self.best)
    }
    pub fn has_progress(&self) -> bool {
        self.nodes.len() > 1
            || self.viewer.is_some()
            || self.nodes[0].traits != Traits::default()
            || !self.attempts.is_empty()
    }
    pub fn scroll_attempts(&mut self, forward: bool) {
        self.attempt_scroll = if forward {
            (self.attempt_scroll + 1).min(self.attempts.len().saturating_sub(3))
        } else {
            self.attempt_scroll.saturating_sub(1)
        };
    }
    pub fn animating(&self) -> bool {
        self.open
            && self
                .trait_pop
                .is_some_and(|(_, _, at)| at.elapsed().as_secs_f32() < 0.45)
    }
    pub fn dismiss_result(&mut self) {
        self.result_open = false;
    }

    pub fn layout(&self, w: f32, h: f32, scale: f32) -> Layout {
        let mut l = Layout::new(w, h, scale);
        let margin = 38.0 * l.scale;
        // Keep the full diagram visible after an aspect-ratio change, without
        // stretching distances or changing the theory's coordinates/history.
        let fits = self.nodes.iter().all(|node| {
            let p = l.point(node.pos);
            p[0] >= l.canvas.x + margin
                && p[0] <= l.canvas.x + l.canvas.w - margin
                && p[1] >= l.canvas.y + margin
                && p[1] <= l.canvas.y + l.canvas.h - margin
        });
        if !fits {
            let mut low = self.nodes[0].pos;
            let mut high = low;
            for node in &self.nodes {
                for axis in 0..2 {
                    low[axis] = low[axis].min(node.pos[axis]);
                    high[axis] = high[axis].max(node.pos[axis]);
                }
            }
            l.unit = l
                .unit
                .min((l.canvas.w - 2.0 * margin) / (high[0] - low[0]).max(0.01))
                .min((l.canvas.h - 2.0 * margin) / (high[1] - low[1]).max(0.01));
            l.center = [(low[0] + high[0]) * 0.5, (low[1] + high[1]) * 0.5];
        }
        l
    }

    fn save(&mut self) {
        self.redo.clear();
        if self.undo.len() == 100 {
            self.undo.remove(0);
        }
        self.undo
            .push((self.nodes.clone(), self.viewer, self.selected));
        self.score = None;
        self.result_open = false;
    }

    pub fn toggle(&mut self) {
        self.confirm_clear = false;
        self.open = !self.open;
        self.release();
    }

    pub fn release(&mut self) {
        self.dragging = None;
        self.drag_saved = false;
    }

    fn add(&mut self, pos: [f32; 2]) {
        if self.nodes.len() >= MAX_NODES {
            return;
        }
        self.save();
        let orbit_rank = self.siblings(self.selected).len();
        self.nodes.push(Node {
            parent: Some(self.selected),
            pos,
            orbit_rank,
            traits: Traits::default(),
        });
        self.selected = self.nodes.len() - 1;
        self.reorder_by_distance();
        self.message = "Added. Distance from its parent sets the orbit order.".into();
    }

    fn delete(&mut self) {
        if self.selected == 0 {
            self.message =
                "The centre stays. Select an orbiting object to delete its branch.".into();
            return;
        }
        self.save();
        let mut removed = vec![false; self.nodes.len()];
        for (i, node) in self.nodes.iter().enumerate() {
            removed[i] = i == self.selected || node.parent.is_some_and(|p| removed[p]);
        }
        let mut indices = vec![0; self.nodes.len()];
        let mut kept = Vec::new();
        for (i, node) in self.nodes.iter().enumerate() {
            if !removed[i] {
                indices[i] = kept.len();
                kept.push(Node {
                    parent: node.parent.map(|p| indices[p]),
                    pos: node.pos,
                    orbit_rank: node.orbit_rank,
                    traits: node.traits,
                });
            }
        }
        self.viewer = self.viewer.filter(|&i| !removed[i]).map(|i| indices[i]);
        self.nodes = kept;
        for parent in 0..self.nodes.len() {
            for (rank, child) in self.siblings(parent).into_iter().enumerate() {
                self.nodes[child].orbit_rank = rank;
            }
        }
        self.selected = 0;
        self.message = "Branch deleted. Undo restores it.".into();
    }

    fn siblings(&self, parent: usize) -> Vec<usize> {
        let mut siblings: Vec<_> = self
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| (n.parent == Some(parent)).then_some(i))
            .collect();
        siblings.sort_by_key(|&i| self.nodes[i].orbit_rank);
        siblings
    }

    fn orbit_distance(&self, child: usize, parent: usize) -> f32 {
        let a = self.nodes[child].pos;
        let b = self.nodes[parent].pos;
        (a[0] - b[0]).hypot(a[1] - b[1])
    }

    fn reorder_by_distance(&mut self) {
        for parent in 0..self.nodes.len() {
            let mut siblings = self.siblings(parent);
            // Keep the previous ordering when two radii are exactly equal.
            siblings.sort_by(|&a, &b| {
                self.orbit_distance(a, parent)
                    .total_cmp(&self.orbit_distance(b, parent))
            });
            for (rank, child) in siblings.into_iter().enumerate() {
                self.nodes[child].orbit_rank = rank;
            }
        }
    }

    fn topology(&self) -> Topology {
        Topology::new(
            &self.nodes.iter().map(|n| n.parent).collect::<Vec<_>>(),
            &self
                .nodes
                .iter()
                .map(|n| n.orbit_rank as f64)
                .collect::<Vec<_>>(),
            self.nodes.iter().map(|n| n.traits).collect(),
        )
    }

    fn history(&mut self, redo: bool) {
        self.release();
        let snapshot = if redo {
            self.redo.pop()
        } else {
            self.undo.pop()
        };
        if let Some((nodes, viewer, selected)) = snapshot {
            let current = (
                std::mem::replace(&mut self.nodes, nodes),
                self.viewer,
                self.selected,
            );
            if redo {
                self.undo.push(current);
            } else {
                self.redo.push(current);
            }
            self.viewer = viewer;
            self.selected = selected;
            self.score = None;
            self.message = if redo { "Edit redone." } else { "Edit undone." }.into();
        }
    }

    fn clear(&mut self) {
        self.save();
        self.release();
        let fresh = Self::default();
        self.nodes = fresh.nodes;
        self.selected = 0;
        self.viewer = None;
        self.confirm_clear = false;
        self.message = "Theory cleared. Undo [Z] restores everything.".into();
    }

    fn action(&mut self, action: usize, scene: &Scene) {
        self.release();
        match action {
            0 => self.history(true),
            1 => {
                self.save();
                self.viewer = Some(self.selected);
                self.message = "Viewer placed on the selected object.".into();
            }
            2 => self.delete(),
            3 => self.history(false),
            4 => {
                if let Some(viewer) = self.viewer {
                    self.score = Some(score(
                        &self.topology(),
                        viewer,
                        &Topology::from_scene(scene),
                        scene.host,
                    ));
                    let score = self.score.unwrap();
                    self.best = Some(self.best.unwrap_or(0.0).max(score.total()));
                    if self.attempts.len() == 1000 {
                        self.attempts.remove(0);
                    }
                    self.attempts.push(score);
                    self.attempt_scroll = self.attempts.len().saturating_sub(3);
                    self.result_open = true;
                    self.message =
                        "Edit your theory and check again, or return to observing.".into();
                } else {
                    self.message =
                        "Select an object and choose Viewer here [V] before checking.".into();
                }
            }
            5 => self.confirm_clear = true,
            _ => unreachable!(),
        }
    }

    fn toggle_trait(&mut self, property: usize) {
        self.release();
        self.save();
        let traits = &mut self.nodes[self.selected].traits;
        if property == 0 {
            traits.star = !traits.star;
        } else {
            traits.rings = !traits.rings;
        }
        self.trait_pop = Some((self.selected, property, std::time::Instant::now()));
        self.message = "Object updated. Gold rays mark stars; an oval marks rings.".into();
    }

    fn hit_node(&self, l: &Layout, cursor: (f64, f64)) -> Option<usize> {
        self.nodes.iter().enumerate().rev().find_map(|(i, n)| {
            let p = l.point(n.pos);
            ((cursor.0 - p[0] as f64).hypot(cursor.1 - p[1] as f64) <= 23.0 * l.scale as f64)
                .then_some(i)
        })
    }

    pub fn right_click(&mut self, l: &Layout, cursor: (f64, f64)) {
        if !self.open
            || self.confirm_clear
            || self.result_open
            || !l.canvas.contains(cursor.0, cursor.1)
            || l.inspector.contains(cursor.0, cursor.1)
            || l.legend.contains(cursor.0, cursor.1)
        {
            return;
        }
        self.release();
        if let Some(i) = self.hit_node(l, cursor) {
            self.selected = i;
        } else if self.nodes.len() < MAX_NODES {
            self.add(l.normalized(cursor));
        } else {
            self.message = "Diagram limit: 64 objects. Delete a branch to make room.".into();
        }
    }

    pub fn key(&mut self, key: KeyCode, _l: &Layout, _cursor: (f64, f64), scene: &Scene) {
        if self.result_open {
            if key == KeyCode::PageUp {
                self.scroll_attempts(false);
            }
            if key == KeyCode::PageDown {
                self.scroll_attempts(true);
            }
            if key == KeyCode::Escape {
                self.result_open = false;
            }
            return;
        }
        if self.confirm_clear {
            match key {
                KeyCode::Enter => self.clear(),
                KeyCode::Escape => self.confirm_clear = false,
                _ => {}
            }
            return;
        }
        match key {
            KeyCode::KeyV => self.action(1, scene),
            KeyCode::Delete | KeyCode::Backspace => self.action(2, scene),
            KeyCode::KeyZ => self.action(3, scene),
            KeyCode::KeyY => self.action(0, scene),
            KeyCode::Enter => self.action(4, scene),
            KeyCode::KeyX => self.action(5, scene),
            KeyCode::KeyS => self.toggle_trait(0),
            KeyCode::KeyR => self.toggle_trait(1),
            _ => {}
        }
    }

    /// Modal editor clicks never leak into the observing view.
    pub fn click(&mut self, l: &Layout, cursor: (f64, f64), scene: &Scene) -> bool {
        if self.result_open {
            if l.result_close.contains(cursor.0, cursor.1) {
                self.result_open = false;
            } else if l.result_observe.contains(cursor.0, cursor.1) {
                self.result_open = false;
                self.open = false;
            }
            return true;
        }
        if self.confirm_clear {
            if l.confirm.contains(cursor.0, cursor.1) {
                self.clear();
            } else if l.cancel.contains(cursor.0, cursor.1) {
                self.confirm_clear = false;
            }
            return true;
        }
        if l.toggle.contains(cursor.0, cursor.1) {
            self.toggle();
            return true;
        }
        if !self.open {
            return false;
        }
        if let Some(property) = l
            .inspector_traits
            .iter()
            .position(|r| r.contains(cursor.0, cursor.1))
        {
            self.toggle_trait(property);
            return true;
        }
        if l.inspector.contains(cursor.0, cursor.1) || l.legend.contains(cursor.0, cursor.1) {
            return true;
        }
        if let Some(property) = l.traits.iter().position(|r| r.contains(cursor.0, cursor.1)) {
            self.toggle_trait(property);
        } else if let Some(action) = l
            .buttons
            .iter()
            .position(|r| r.contains(cursor.0, cursor.1))
        {
            self.action(action, scene);
        } else if l.canvas.contains(cursor.0, cursor.1)
            && let Some(i) = self.hit_node(l, cursor)
        {
            self.selected = i;
            self.dragging = Some(i);
            self.drag_saved = false;
        }
        true
    }

    pub fn motion(&mut self, l: &Layout, cursor: (f64, f64)) {
        if self.confirm_clear || self.result_open {
            return;
        }
        if let Some(i) = self.dragging {
            let pos = l.normalized(cursor);
            if self.nodes[i].pos == pos {
                return;
            }
            if !self.drag_saved {
                self.save();
                self.drag_saved = true;
            }
            self.nodes[i].pos = pos;
            self.reorder_by_distance();
            self.message =
                "Orbit order follows distance from each parent. Undo restores this drag.".into();
        }
    }
}

/// Children are explicitly ordered by orbital size, independent of storage order.
struct Topology {
    root: usize,
    children: Vec<Vec<usize>>,
    traits: Vec<Traits>,
}

impl Topology {
    fn new(parents: &[Option<usize>], orbital_order: &[f64], traits: Vec<Traits>) -> Self {
        let mut children = vec![Vec::new(); parents.len()];
        for (i, parent) in parents.iter().enumerate() {
            if let Some(parent) = parent {
                children[*parent].push(i);
            }
        }
        for family in &mut children {
            family.sort_by(|&a, &b| orbital_order[a].total_cmp(&orbital_order[b]));
        }
        Self {
            root: parents.iter().position(Option::is_none).expect("tree root"),
            children,
            traits,
        }
    }

    fn from_scene(scene: &Scene) -> Self {
        Self::new(
            &scene.bodies.iter().map(|b| b.parent).collect::<Vec<_>>(),
            &scene
                .bodies
                .iter()
                .map(|b| b.elements.a)
                .collect::<Vec<_>>(),
            scene
                .bodies
                .iter()
                .map(|b| Traits {
                    star: b.kind == BodyKind::Star,
                    rings: b.rings.is_some(),
                })
                .collect(),
        )
    }
}

/// Match the root, then the first orbit to the first orbit at every parent, etc.
/// Missing inner objects do not shift a viewer into a different orbital rank.
/// The complete sequence of ranks from the root identifies the viewer's location.
fn score(guess: &Topology, viewer: usize, truth: &Topology, host: usize) -> Score {
    fn matched(
        a: &Topology,
        i: usize,
        b: &Topology,
        j: usize,
        viewer: usize,
        host: usize,
    ) -> (usize, usize, bool) {
        let mut count = 1;
        let mut traits = usize::from(a.traits[i].star == b.traits[j].star)
            + usize::from(a.traits[i].rings == b.traits[j].rings);
        let mut viewer_matches = i == viewer && j == host;
        for (&ai, &bi) in a.children[i].iter().zip(&b.children[j]) {
            let (subtree, matched_traits, found_viewer) = matched(a, ai, b, bi, viewer, host);
            count += subtree;
            traits += matched_traits;
            viewer_matches |= found_viewer;
        }
        (count, traits, viewer_matches)
    }
    let (count, traits, viewer) = matched(guess, guess.root, truth, truth.root, viewer, host);
    Score {
        topology: 100.0 * count as f32 / guess.children.len().max(truth.children.len()) as f32,
        traits: 100.0 * traits as f32 / (2 * guess.children.len().max(truth.children.len())) as f32,
        viewer,
    }
}

pub struct Layout {
    pub toggle: Rect,
    pub maps: Rect,
    pub result_close: Rect,
    pub result_observe: Rect,
    pub result_maps: Rect,
    bar: Rect,
    inspector: Rect,
    inspector_traits: [Rect; 2],
    legend: Rect,
    canvas: Rect,
    buttons: [Rect; 6],
    traits: [Rect; 2],
    confirm: Rect,
    cancel: Rect,
    dialog: Rect,
    center: [f32; 2],
    unit: f32,
    scale: f32,
    toolbar: crate::ui::ToolbarStyle,
    dividers: [f32; 3],
}

impl Layout {
    pub fn new(w: f32, h: f32, scale: f32) -> Self {
        let compact = w / scale < 1300.0;
        let s = scale
            .min(w / if compact { 1280.0 } else { 1380.0 })
            .min(h / 620.0);
        let toolbar = crate::ui::ToolbarStyle::new(w, h, scale);
        let bs = toolbar.scale;
        let bar = toolbar.bar(w, h);
        let [maps, toggle] = toolbar.nav(bar);
        let nav_end = 217.0 * bs;
        let mut x = nav_end + 8.0 * bs;
        let mut control = |label: &str, extra: f32| {
            let width = crate::ui::toolbar_width(label, toolbar.keys) + extra;
            let r = toolbar.control(bar, x, width);
            x += (width + 4.0) * bs;
            r
        };
        let star = control("Is star [S]", 58.0);
        let rings = control("Has rings [R]", 58.0);
        let viewer = control("Viewer here [V]", 0.0);
        let traits_end = viewer.right() + 9.0 * bs;
        x = traits_end + 8.0 * bs;
        let mut control = |label: &str| {
            let width = crate::ui::toolbar_width(label, toolbar.keys);
            let r = toolbar.control(bar, x, width);
            x += (width + 4.0) * bs;
            r
        };
        let undo = control("Undo [Z]");
        let redo = control("Redo [Y]");
        let delete = control("Delete [Del]");
        let actions_end = delete.right() + 9.0 * bs;
        let check_width = crate::ui::toolbar_width("Check theory [Enter]", toolbar.keys) + 40.0;
        let check = toolbar.control(bar, w - (8.0 + check_width) * bs, check_width);
        let clear_width = crate::ui::toolbar_width("Clear all [X]", toolbar.keys);
        let clear = toolbar.control(bar, check.x - (4.0 + clear_width) * bs, clear_width);
        let dialog = Rect {
            x: (w - 640.0 * s) * 0.5,
            y: (h - 238.0 * s) * 0.5,
            w: 640.0 * s,
            h: 238.0 * s,
        };
        let result_x = (w - 760.0 * s) * 0.5;
        let result_y = (h - 496.0 * s) * 0.5;
        let inspector = Rect {
            x: w - 268.0 * s,
            y: 78.0 * s,
            w: 250.0 * s,
            h: 261.0 * s,
        };
        Self {
            toggle,
            maps,
            bar,
            result_close: Rect {
                x: result_x + 203.0 * s,
                y: result_y + 424.0 * s,
                w: 151.0 * s,
                h: 46.0 * s,
            },
            result_observe: Rect {
                x: result_x + 364.0 * s,
                y: result_y + 424.0 * s,
                w: 158.0 * s,
                h: 46.0 * s,
            },
            result_maps: Rect {
                x: result_x + 532.0 * s,
                y: result_y + 424.0 * s,
                w: 200.0 * s,
                h: 46.0 * s,
            },
            inspector,
            inspector_traits: std::array::from_fn(|i| Rect {
                x: inspector.x + 14.0 * s,
                y: inspector.y + (133.0 + i as f32 * 49.0) * s,
                w: 222.0 * s,
                h: 42.0 * s,
            }),
            legend: Rect {
                x: 18.0 * s,
                y: 78.0 * s,
                w: 241.0 * s,
                h: 78.0 * s,
            },
            buttons: [redo, viewer, delete, undo, check, clear],
            traits: [star, rings],
            dialog,
            confirm: Rect {
                x: dialog.x + 452.0 * s,
                y: dialog.y + 166.0 * s,
                w: 160.0 * s,
                h: 46.0 * s,
            },
            cancel: Rect {
                x: dialog.x + 282.0 * s,
                y: dialog.y + 166.0 * s,
                w: 160.0 * s,
                h: 46.0 * s,
            },
            center: [0.5, 0.5],
            unit: w.min(bar.y - 60.0 * s),
            canvas: Rect {
                x: 0.0,
                y: 60.0 * s,
                w,
                h: bar.y - 60.0 * s,
            },
            scale: s,
            toolbar,
            dividers: [nav_end, traits_end, actions_end],
        }
    }
    fn point(&self, p: [f32; 2]) -> [f32; 2] {
        let unit = self.unit;
        [
            self.canvas.x + self.canvas.w * 0.5 + (p[0] - self.center[0]) * unit,
            self.canvas.y + self.canvas.h * 0.5 + (p[1] - self.center[1]) * unit,
        ]
    }
    fn normalized(&self, p: (f64, f64)) -> [f32; 2] {
        let unit = self.unit;
        let margin = 38.0 * self.scale;
        [
            self.center[0]
                + ((p.0 as f32 - self.canvas.x).clamp(margin, self.canvas.w - margin)
                    - self.canvas.w * 0.5)
                    / unit,
            self.center[1]
                + ((p.1 as f32 - self.canvas.y).clamp(margin, self.canvas.h - margin)
                    - self.canvas.h * 0.5)
                    / unit,
        ]
    }
}

fn line(
    v: &mut Vec<UiVertex>,
    a: [f32; 2],
    b: [f32; 2],
    thickness: f32,
    color: [f32; 4],
    vp: [f32; 2],
) {
    crate::ui::line(v, a, b, thickness, color, vp);
}

/// Dashed orbit paths stay inside the diagram even when the full orbit extends
/// beyond it. The connecting line identifies the parent for nested systems.
fn dashed_orbit(
    verts: &mut Vec<UiVertex>,
    center: [f32; 2],
    radius: f32,
    clip: Rect,
    scale: f32,
    viewport: [f32; 2],
    selected: bool,
) {
    let count = (std::f32::consts::TAU * radius / scale)
        .ceil()
        .clamp(48.0, 2048.0) as usize;
    let color = if selected {
        [crate::ui::OK[0], crate::ui::OK[1], crate::ui::OK[2], 0.50]
    } else {
        [
            crate::ui::MUTED[0],
            crate::ui::MUTED[1],
            crate::ui::MUTED[2],
            0.18,
        ]
    };
    let inner = Rect {
        x: clip.x + scale,
        y: clip.y + scale,
        w: clip.w - 2.0 * scale,
        h: clip.h - 2.0 * scale,
    };
    let point = |i: usize| {
        let angle = i as f32 * std::f32::consts::TAU / count as f32;
        [
            center[0] + radius * angle.cos(),
            center[1] + radius * angle.sin(),
        ]
    };
    for i in 0..count {
        if i % 11 >= 5 {
            continue;
        }
        let a = point(i);
        let b = point(i + 1);
        if inner.contains(a[0] as f64, a[1] as f64) && inner.contains(b[0] as f64, b[1] as f64) {
            line(verts, a, b, scale, color, viewport);
        }
    }
}

fn trait_glyph(
    v: &mut Vec<UiVertex>,
    p: [f32; 2],
    star: bool,
    s: f32,
    color: [f32; 4],
    vp: [f32; 2],
) {
    if star {
        crate::ui::ellipse(v, p, [3.0 * s, 3.0 * s], s, color, vp);
        for i in 0..8 {
            let a = i as f32 * std::f32::consts::TAU / 8.0;
            line(
                v,
                [p[0] + 5.0 * s * a.cos(), p[1] + 5.0 * s * a.sin()],
                [p[0] + 8.0 * s * a.cos(), p[1] + 8.0 * s * a.sin()],
                s,
                color,
                vp,
            );
        }
    } else {
        crate::ui::ellipse(v, p, [3.0 * s, 3.0 * s], s, color, vp);
        crate::ui::ellipse(v, p, [8.0 * s, 2.5 * s], s, color, vp);
    }
}

fn trait_button(
    v: &mut Vec<UiVertex>,
    r: Rect,
    star: bool,
    on: bool,
    s: f32,
    appearance: (bool, bool, bool),
    vp: [f32; 2],
) {
    let (hover, inspector, keys) = appearance;
    let key = if star { "Is star [S]" } else { "Has rings [R]" };
    let mut r = r;
    if crate::motion::pressed(key, r) {
        r.y += 3.0 * s;
    }
    let ink = if star && on { crate::ui::ACCENT } else { INK };
    push_rect(
        v,
        r.x,
        r.y,
        r.w,
        r.h,
        [
            ink[0],
            ink[1],
            ink[2],
            if on {
                0.10
            } else if hover {
                0.08
            } else {
                0.0
            },
        ],
        vp,
    );
    if inspector || on || hover {
        crate::ui::outline(
            v,
            r,
            s,
            [ink[0], ink[1], ink[2], if on { 0.4 } else { 0.17 }],
            vp,
        );
    }
    trait_glyph(v, [r.x + 17.0 * s, r.y + r.h * 0.5], star, s, ink, vp);
    push_text(
        v,
        r.x + 37.0 * s,
        if inspector {
            r.y + 8.0 * s
        } else {
            r.center_y() - 5.25 * s
        },
        0.7875 * s,
        if star { "Is star" } else { "Has rings" },
        ink,
        vp,
    );
    if inspector {
        push_text(
            v,
            r.x + 37.0 * s,
            r.y + 23.0 * s,
            0.675 * s,
            if star {
                if on {
                    "Shines on its own"
                } else {
                    "Reflects light"
                }
            } else if on {
                "Ring system"
            } else {
                "No rings"
            },
            MUTED,
            vp,
        );
    }
    let show_key = !inspector && keys;
    let key_space = if show_key { 22.0 * s } else { 0.0 };
    crate::ui::switch(
        v,
        Rect {
            x: r.x + r.w - 32.0 * s - key_space,
            y: r.y + r.h * 0.5 - 6.0 * s,
            w: 22.0 * s,
            h: 12.0 * s,
        },
        on,
        [ink[0], ink[1], ink[2], if on { 1.0 } else { 0.55 }],
        vp,
    );
    if show_key {
        crate::ui::toolbar_keycap(
            v,
            [r.right() - 23.0 * s, r.center_y()],
            if star { "S" } else { "R" },
            s,
            ink,
            vp,
        );
    }
}

pub fn build(
    v: &mut Vec<UiVertex>,
    vp: [f32; 2],
    scale: f32,
    g: &Game,
    cursor: (f64, f64),
    _status: &str,
) {
    if !g.open {
        return;
    }
    let [w, h] = vp;
    let l = g.layout(w, h, scale);
    let s = l.scale;
    let c = l.canvas;
    let ui = crate::ui::BG;
    let text = |v: &mut Vec<UiVertex>, x: f32, y: f32, size: f32, value: &str, color| {
        push_text(v, x, y, size * s, value, color, vp)
    };
    let button = |v: &mut Vec<UiVertex>, r: Rect, label: &str, on: bool| {
        crate::ui::toolbar_button(
            v,
            r,
            label,
            l.toolbar,
            on,
            r.contains(cursor.0, cursor.1),
            vp,
        )
    };
    push_rect(v, 0.0, 0.0, w, h, [0.004, 0.023, 0.23, 1.0], vp);
    push_rect(v, 0.0, 0.0, w, 60.0 * s, ui, vp);
    push_rect(v, 0.0, 59.0 * s, w, s, crate::ui::LINE, vp);
    crate::ui::heading(v, 22.0 * s, 18.0 * s, 22.0 * s, "Your theory", INK, vp);
    text(
        v,
        157.0 * s,
        26.0 * s,
        0.675,
        &g.title.to_uppercase(),
        crate::ui::ACCENT,
    );
    text(
        v,
        330.0 * s,
        26.0 * s,
        0.75,
        "Select a parent, right-click to add. Drag to order orbits. 1 = innermost.",
        MUTED,
    );
    let meta = format!(
        "{} / 64 objects · {} undo steps",
        g.nodes.len(),
        g.undo.len()
    );
    text(
        v,
        w - ui_width(&meta, 0.75 * s) - 22.0 * s,
        26.0 * s,
        0.75,
        &meta,
        MUTED,
    );
    let step = 32.0 * s;
    let mut x = 16.0 * s;
    while x < w {
        let mut y = c.y + 32.0 * s;
        while y < c.y + c.h {
            push_rect(
                v,
                x.round(),
                y.round(),
                1.0,
                1.0,
                [MUTED[0], MUTED[1], MUTED[2], 0.24],
                vp,
            );
            y += step;
        }
        x += step;
    }
    let graph_start = v.len();
    for (i, node) in g.nodes.iter().enumerate() {
        if let Some(parent) = node.parent {
            let a = l.point(g.nodes[parent].pos);
            let b = l.point(node.pos);
            dashed_orbit(
                v,
                a,
                (b[0] - a[0]).hypot(b[1] - a[1]),
                c,
                s,
                vp,
                i == g.selected,
            );
            line(v, a, b, s, [INK[0], INK[1], INK[2], 0.35], vp);
        }
    }
    let preview = g.dragging.is_none()
        && l.canvas.contains(cursor.0, cursor.1)
        && !l.inspector.contains(cursor.0, cursor.1)
        && !l.legend.contains(cursor.0, cursor.1)
        && g.hit_node(&l, cursor).is_none()
        && !g.result_open
        && !g.confirm_clear;
    if preview {
        let p = l.point(g.nodes[g.selected].pos);
        let q = [cursor.0 as f32, cursor.1 as f32];
        let len = (p[0] - q[0]).hypot(p[1] - q[1]);
        let n = (len / (8.0 * s)).ceil() as usize;
        for i in (0..n).step_by(2) {
            let pt = |j: usize| {
                [
                    p[0] + (q[0] - p[0]) * j as f32 / n as f32,
                    p[1] + (q[1] - p[1]) * j as f32 / n as f32,
                ]
            };
            line(
                v,
                pt(i),
                pt((i + 1).min(n)),
                s,
                [INK[0], INK[1], INK[2], 0.30],
                vp,
            );
        }
        crate::ui::dashed_arc(
            v,
            q,
            [14.0 * s, 14.0 * s],
            [s, 3.0 * s],
            [0.0, std::f32::consts::TAU],
            [
                crate::ui::ACCENT[0],
                crate::ui::ACCENT[1],
                crate::ui::ACCENT[2],
                0.5,
            ],
            vp,
        );
        text(
            v,
            q[0] + 20.0 * s,
            q[1] - 4.0 * s,
            0.675,
            "right-click to add",
            MUTED,
        );
    }
    for (i, node) in g.nodes.iter().enumerate() {
        let p = l.point(node.pos);
        let r = 18.0 * s;
        let mut ring_front = None;
        for property in 0..2 {
            let on = if property == 0 {
                node.traits.star
            } else {
                node.traits.rings
            };
            let hover = i == g.selected
                && (l.traits[property].contains(cursor.0, cursor.1)
                    || l.inspector_traits[property].contains(cursor.0, cursor.1));
            let mut color = if property == 0 {
                crate::ui::ACCENT
            } else {
                INK
            };
            let mut pop = 1.0;
            if let Some((which, kind, at)) = g.trait_pop
                && i == which
                && property == kind
                && on
            {
                let f = if cfg!(test) {
                    1.0
                } else {
                    (at.elapsed().as_secs_f32() / 0.45).min(1.0)
                };
                pop = 1.0 - 0.45 * (1.0 - f).powi(3);
                color[3] = 0.3 + 0.7 * f;
            }
            if hover {
                color[3] = if on { 0.25 } else { 0.75 };
            }
            if on || hover {
                if property == 0 {
                    for ray in 0..8 {
                        if hover && !on && ray % 2 == 1 {
                            continue;
                        }
                        let a = ray as f32 * std::f32::consts::TAU / 8.0;
                        line(
                            v,
                            [
                                p[0] + 24.0 * s * pop * a.cos(),
                                p[1] + 24.0 * s * pop * a.sin(),
                            ],
                            [
                                p[0] + 31.0 * s * pop * a.cos(),
                                p[1] + 31.0 * s * pop * a.sin(),
                            ],
                            1.7 * s,
                            color,
                            vp,
                        );
                    }
                } else {
                    let radii = [31.0 * s * pop, 10.0 * s * pop];
                    let dashed = hover && !on;
                    if dashed {
                        crate::ui::dashed_arc(
                            v,
                            p,
                            radii,
                            [1.5 * s, 3.0 * s],
                            [0.0, std::f32::consts::TAU],
                            color,
                            vp,
                        );
                    } else {
                        crate::ui::ellipse(v, p, radii, 1.5 * s, color, vp);
                    }
                    ring_front = Some((radii, color, dashed));
                }
            }
        }
        crate::ui::disk(v, p, r, [0.004, 0.023, 0.23, 1.0], vp);
        crate::ui::ellipse(
            v,
            p,
            [r, r],
            1.5 * s,
            if node.traits.star {
                crate::ui::ACCENT
            } else {
                MUTED
            },
            vp,
        );
        if let Some((radii, color, dashed)) = ring_front {
            if dashed {
                crate::ui::dashed_arc(
                    v,
                    p,
                    radii,
                    [1.5 * s, 3.0 * s],
                    [0.0, std::f32::consts::PI],
                    color,
                    vp,
                );
            } else {
                crate::ui::arc_from(v, p, radii, 1.5 * s, [0.0, std::f32::consts::PI], color, vp);
            }
        }
        if i == g.selected {
            crate::ui::ellipse(v, p, [23.0 * s, 23.0 * s], 1.5 * s, crate::ui::OK, vp);
        }
        let label = node.label();
        text(
            v,
            p[0] - ui_width(&label, 0.9 * s) * 0.5,
            p[1] - 5.0 * s,
            0.9,
            &label,
            INK,
        );
        if g.viewer == Some(i) {
            let r = Rect {
                x: p[0] - 17.0 * s,
                y: p[1] + 33.0 * s,
                w: 34.0 * s,
                h: 15.0 * s,
            };
            push_rect(v, r.x, r.y, r.w, r.h, ui, vp);
            crate::ui::outline(v, r, s, crate::ui::OK, vp);
            text(v, r.x + 6.0 * s, r.y + 2.0 * s, 0.675, "YOU", crate::ui::OK);
        }
    }
    crate::ui::clip(v, graph_start, c, vp);
    let r = l.legend;
    push_rect(v, r.x, r.y, r.w, r.h, [ui[0], ui[1], ui[2], 0.65], vp);
    crate::ui::outline(v, r, s, [MUTED[0], MUTED[1], MUTED[2], 0.16], vp);
    for (i, line) in [
        "C centre of the system",
        "1, 2, 3… orbit order, innermost first",
        "* star · o rings · YOU viewer",
    ]
    .iter()
    .enumerate()
    {
        text(
            v,
            r.x + 14.0 * s,
            r.y + (17.0 + i as f32 * 19.0) * s,
            0.675,
            line,
            MUTED,
        );
    }
    let r = l.inspector;
    push_rect(v, r.x, r.y, r.w, r.h, [ui[0], ui[1], ui[2], 0.94], vp);
    crate::ui::outline(v, r, s, crate::ui::LINE, vp);
    text(
        v,
        r.x + 14.0 * s,
        r.y + 15.0 * s,
        0.675,
        "SELECTED OBJECT",
        crate::ui::ACCENT,
    );
    text(
        v,
        r.x + r.w - 36.0 * s,
        r.y + 15.0 * s,
        0.675,
        &g.nodes[g.selected].reference(),
        INK,
    );
    push_rect(v, r.x, r.y + 35.0 * s, r.w, s, crate::ui::LINE, vp);
    let n = &g.nodes[g.selected];
    let vals = [
        (
            "Role",
            if n.parent.is_none() {
                "Centre".into()
            } else if n.traits.star {
                "Star".into()
            } else {
                "Orbiting body".into()
            },
        ),
        (
            "Orbits",
            n.parent.map_or("—".into(), |p| g.nodes[p].reference()),
        ),
        (
            "Orbit order",
            n.parent.map_or("—".into(), |p| {
                format!("{} of {}", n.orbit_rank + 1, g.siblings(p).len())
            }),
        ),
        ("Satellites", g.siblings(g.selected).len().to_string()),
    ];
    for (i, (name, value)) in vals.iter().enumerate() {
        let y = r.y + (54.0 + i as f32 * 20.0) * s;
        text(v, r.x + 14.0 * s, y, 0.79, name, MUTED);
        text(
            v,
            r.x + r.w - 14.0 * s - ui_width(value, 0.79 * s),
            y,
            0.79,
            value,
            INK,
        );
    }
    for (i, r) in l.inspector_traits.iter().enumerate() {
        trait_button(
            v,
            *r,
            i == 0,
            if i == 0 {
                n.traits.star
            } else {
                n.traits.rings
            },
            s,
            (r.contains(cursor.0, cursor.1), true, false),
            vp,
        );
    }
    text(v, r.x + 14.0 * s, r.y + 235.0 * s, 0.79, "Viewer", MUTED);
    let viewer = if g.viewer == Some(g.selected) {
        "you are here"
    } else {
        "no"
    };
    text(
        v,
        r.x + r.w - 14.0 * s - ui_width(viewer, 0.79 * s),
        r.y + 235.0 * s,
        0.79,
        viewer,
        if g.viewer == Some(g.selected) {
            crate::ui::OK
        } else {
            INK
        },
    );
    text(v, 18.0 * s, l.bar.y - 50.0 * s, 0.79, &g.message, INK);
    text(
        v,
        18.0 * s,
        l.bar.y - 28.0 * s,
        0.72,
        "60% ordered structure + 20% star/ring traits + 20% viewer. Orbit 1 = innermost.",
        MUTED,
    );
    crate::ui::toolbar_panel(v, l.bar, l.toolbar.scale, &l.dividers, vp);
    button(v, l.maps, "Maps [M]", false);
    button(v, l.toggle, "Observe [Tab]", false);
    for (i, r) in l.traits.iter().enumerate() {
        trait_button(
            v,
            *r,
            i == 0,
            if i == 0 {
                n.traits.star
            } else {
                n.traits.rings
            },
            l.toolbar.scale,
            (r.contains(cursor.0, cursor.1), false, l.toolbar.keys),
            vp,
        );
    }
    for (i, label) in [
        "Redo [Y]",
        "Viewer here [V]",
        "Delete [Del]",
        "Undo [Z]",
        "Check theory [Enter]",
        "Clear all [X]",
    ]
    .iter()
    .enumerate()
    {
        let start = v.len();
        button(
            v,
            l.buttons[i],
            label,
            i == 1 && g.viewer == Some(g.selected),
        );
        let disabled = (i == 0 && g.redo.is_empty())
            || (i == 3 && g.undo.is_empty())
            || (i == 2 && g.selected == 0);
        if disabled {
            for p in &mut v[start..] {
                p.color[3] *= 0.35;
            }
        }
    }
    if g.result_open
        && let Some(score) = g.score
    {
        build_result(v, vp, &l, g, score, cursor);
    }
    if g.confirm_clear {
        push_rect(v, 0.0, 0.0, w, h, [0.0, 0.001, 0.016, 0.75], vp);
        let d = l.dialog;
        push_rect(v, d.x, d.y, d.w, d.h, ui, vp);
        crate::ui::outline(v, d, s, crate::ui::LINE, vp);
        crate::ui::heading(
            v,
            d.x + 28.0 * s,
            d.y + 25.0 * s,
            26.0 * s,
            "Clear entire theory?",
            INK,
            vp,
        );
        text(
            v,
            d.x + 28.0 * s,
            d.y + 79.0 * s,
            0.90,
            "This removes every object and unsets the viewer.",
            MUTED,
        );
        text(
            v,
            d.x + 28.0 * s,
            d.y + 99.0 * s,
            0.90,
            "Undo [Z] can restore everything afterwards.",
            MUTED,
        );
        text(
            v,
            d.x + 28.0 * s,
            d.y + 132.0 * s,
            0.825,
            &format!("{} orbiting objects will be removed.", g.nodes.len() - 1),
            crate::ui::WARN,
        );
        crate::ui::button(
            v,
            l.cancel,
            "Cancel [Esc]",
            s,
            false,
            l.cancel.contains(cursor.0, cursor.1),
            vp,
        );
        let start = v.len();
        crate::ui::button(
            v,
            l.confirm,
            "Clear all [Enter]",
            s,
            true,
            l.confirm.contains(cursor.0, cursor.1),
            vp,
        );
        for p in &mut v[start..] {
            if p.color == crate::ui::ACCENT {
                p.color = crate::ui::WARN;
            }
        }
    }
}
fn ui_width(value: &str, s: f32) -> f32 {
    crate::ui::text_width(value, s)
}
fn build_result(
    v: &mut Vec<UiVertex>,
    vp: [f32; 2],
    l: &Layout,
    g: &Game,
    score: Score,
    cursor: (f64, f64),
) {
    let [w, h] = vp;
    let s = l.scale;
    let d = Rect {
        x: (w - 760.0 * s) * 0.5,
        y: (h - 496.0 * s) * 0.5,
        w: 760.0 * s,
        h: 496.0 * s,
    };
    let text = |v: &mut Vec<UiVertex>, x: f32, y: f32, size: f32, value: &str, color| {
        push_text(v, x, y, size * s, value, color, vp)
    };
    push_rect(v, 0.0, 0.0, w, h, [0.0, 0.001, 0.016, 0.75], vp);
    push_rect(v, d.x, d.y, d.w, d.h, crate::ui::BG, vp);
    crate::ui::outline(v, d, s, crate::ui::LINE, vp);
    text(
        v,
        d.x + 28.0 * s,
        d.y + 30.0 * s,
        0.675,
        &format!(
            "THEORY CHECK · {} · ATTEMPT {}",
            g.title.to_uppercase(),
            g.attempts.len()
        ),
        crate::ui::ACCENT,
    );
    crate::ui::tracked_heading(
        v,
        [d.x + 28.0 * s, d.y + 52.0 * s],
        26.0 * s,
        if score.total() >= 99.9 {
            "Solved. The sky makes sense."
        } else if score.total() >= 70.0 {
            "Close. Something is still out of place."
        } else {
            "Keep observing."
        },
        -0.5 * s,
        INK,
        vp,
    );
    let p = [d.x + 128.0 * s, d.y + 197.0 * s];
    crate::ui::ellipse(v, p, [88.0 * s, 88.0 * s], 8.0 * s, crate::ui::LINE, vp);
    crate::ui::arc(
        v,
        p,
        [88.0 * s, 88.0 * s],
        8.0 * s,
        std::f32::consts::TAU * score.total() / 100.0 * crate::motion::progress(0.6),
        crate::ui::ACCENT,
        vp,
    );
    let value = format!("{:.1}", score.total());
    let width = crate::typeface::width(crate::typeface::Face::Sans, &value, 46.0 * s);
    crate::ui::tracked_heading(
        v,
        [p[0] - width * 0.5, p[1] - 30.0 * s],
        46.0 * s,
        &value,
        -s,
        INK,
        vp,
    );
    text(v, p[0] - 21.0 * s, p[1] + 24.0 * s, 0.675, "/ 100", MUTED);
    for (i, (name, value, points)) in [
        (
            "Ordered structure · 60%",
            score.topology,
            score.topology * 0.6,
        ),
        ("Star / ring traits · 20%", score.traits, score.traits * 0.2),
        (
            "Viewer position · 20%",
            if score.viewer { 100.0 } else { 0.0 },
            if score.viewer { 20.0 } else { 0.0 },
        ),
    ]
    .iter()
    .enumerate()
    {
        let x = d.x + 258.0 * s;
        let y = d.y + (122.0 + i as f32 * 41.0) * s;
        text(v, x, y, 0.79, name, INK);
        let detail = if i == 2 {
            format!(
                "{} → {points:.1} pts",
                if score.viewer {
                    "matches"
                } else {
                    "does not match"
                }
            )
        } else {
            format!("{value:.1}% → {points:.1} pts")
        };
        text(
            v,
            d.x + d.w - 28.0 * s - ui_width(&detail, 0.79 * s),
            y,
            0.79,
            &detail,
            MUTED,
        );
        push_rect(v, x, y + 21.0 * s, 472.0 * s, 6.0 * s, crate::ui::LINE, vp);
        push_rect(
            v,
            x,
            y + 21.0 * s,
            472.0 * s * value / 100.0,
            6.0 * s,
            if *value < 50.0 {
                crate::ui::WARN
            } else {
                crate::ui::ACCENT
            },
            vp,
        );
    }
    text(
        v,
        d.x + 258.0 * s,
        d.y + 246.0 * s,
        0.675,
        "Orbits are compared innermost-first under each parent.",
        MUTED,
    );
    text(
        v,
        d.x + 258.0 * s,
        d.y + 262.0 * s,
        0.675,
        "The hidden solution is never shown.",
        MUTED,
    );
    for (x, title) in [(d.x + 28.0 * s, "ATTEMPTS"), (d.x + 385.0 * s, "HINT")] {
        let r = Rect {
            x,
            y: d.y + 317.0 * s,
            w: 345.0 * s,
            h: 86.0 * s,
        };
        crate::ui::outline(v, r, s, crate::ui::LINE, vp);
        text(v, x + 12.0 * s, r.y + 13.0 * s, 0.675, title, MUTED);
    }
    if g.attempts.len() > 3 {
        text(
            v,
            d.x + 210.0 * s,
            d.y + 330.0 * s,
            0.60,
            "[PgUp PgDn] scroll",
            MUTED,
        );
    }
    for (i, attempt) in g
        .attempts
        .iter()
        .enumerate()
        .skip(g.attempt_scroll)
        .take(3)
        .enumerate()
    {
        let y = d.y + (350.0 + i as f32 * 15.0) * s;
        text(
            v,
            d.x + 40.0 * s,
            y,
            0.675,
            &format!("#{}", attempt.0 + 1),
            INK,
        );
        text(
            v,
            d.x + 333.0 * s,
            y,
            0.675,
            &format!("{:.1}", attempt.1.total()),
            INK,
        );
    }
    let hint = if score.total() >= 99.9 {
        [
            "Your reconstruction matches this sky.",
            "Try another map, or explore this one.",
        ]
    } else if score.topology < 99.9 {
        [
            "Check which objects orbit which parents.",
            "Orbit 1 is always the innermost sibling.",
        ]
    } else if score.traits < 99.9 {
        [
            "The shape is right. Check which objects shine",
            "on their own, and which have rings.",
        ]
    } else {
        [
            "The objects match. Check where you are",
            "standing in this system.",
        ]
    };
    for (i, line) in hint.iter().enumerate() {
        text(
            v,
            d.x + 397.0 * s,
            d.y + (351.0 + i as f32 * 16.0) * s,
            0.72,
            line,
            INK,
        );
    }
    for (r, label, on) in [
        (l.result_close, "Keep editing [Esc]", false),
        (l.result_observe, "Observe again [Tab]", false),
        (l.result_maps, "Choose another map [M]", true),
    ] {
        crate::ui::button(v, r, label, s, on, r.contains(cursor.0, cursor.1), vp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn plain(parents: &[Option<usize>]) -> Topology {
        Topology::new(
            parents,
            &(0..parents.len()).map(|i| i as f64).collect::<Vec<_>>(),
            vec![Traits::default(); parents.len()],
        )
    }

    fn puzzle() -> Scene {
        crate::config::load(concat!(env!("CARGO_MANIFEST_DIR"), "/configs/puzzle.yaml")).unwrap()
    }

    fn correct_puzzle() -> Game {
        let mut g = Game::default();
        g.nodes[0].traits.star = true;
        g.add([0.35, 0.3]);
        for x in [0.45, 0.6, 0.75] {
            g.selected = 1;
            g.add([x, 0.7]);
        }
        g.viewer = Some(4);
        g
    }

    #[test]
    fn inspector_is_interactive_and_never_creates_hidden_objects() {
        let scene = puzzle();
        let mut g = Game {
            open: true,
            ..Game::default()
        };
        let l = g.layout(1280.0, 720.0, 1.0);
        let r = l.inspector_traits[0];
        let cursor = ((r.x + r.w * 0.5) as f64, (r.y + r.h * 0.5) as f64);
        g.right_click(&l, cursor);
        assert_eq!(g.nodes.len(), 1);
        g.click(&l, cursor, &scene);
        assert!(g.nodes[0].traits.star);
        let position = l.point([0.7, 0.7]);
        g.right_click(&l, (position[0] as f64, position[1] as f64));
        assert_eq!(g.nodes.len(), 2);
        g.key(KeyCode::KeyV, &l, (0.0, 0.0), &scene);
        for _ in 0..5 {
            g.key(KeyCode::Enter, &l, (0.0, 0.0), &scene);
            g.dismiss_result();
        }
        assert_eq!(g.attempts.len(), 5);
        assert_eq!(g.attempt_scroll, 2);
        g.scroll_attempts(false);
        g.scroll_attempts(false);
        assert_eq!(g.attempt_scroll, 0);
        g.scroll_attempts(true);
        assert_eq!(g.attempt_scroll, 1);
    }
    #[test]
    fn map_menu_preserves_current_puzzle_until_another_map_loads() {
        let mut game = correct_puzzle();
        game.open = true;
        game.score = Some(Score {
            topology: 100.0,
            traits: 100.0,
            viewer: true,
        });
        let mut app = crate::App {
            state: crate::State::with_scene(puzzle()),
            menu: crate::menu::Menu::default(),
            game: Some(game),
            window: None,
            renderer: None,
            stars: vec![],
            last_title: String::new(),
            scale: 1.0,
            cursor: (0.0, 0.0),
            motion: crate::motion::Motion::default(),
            pressed_key: None,
            mouse_down: false,
            show_labels: true,
            show_hud: true,
            auto_exposure: true,
            ev_bias: 0.0,
            exposure_dragging: false,
            occluded: false,
            graphics: crate::graphics::Menu::default(),
            pacer: crate::graphics::FramePacer::default(),
            progress: crate::progress::Book::default(),
            active_path: None,
            study_started: false,
            leave: None,
            discard: None,
            controls_back: crate::menu::Page::Title,
            targets_open: false,
            toast: None,
        };
        app.state.sim_time = 42.0;
        app.state.view_lock = Some(crate::ViewLock::Background(glam::DVec3::X));
        app.toggle_menu();
        assert!(app.menu.open);
        assert_eq!(app.menu.entries[0].path.file_stem().unwrap(), "puzzle");
        app.toggle_menu();
        assert!(!app.menu.open);
        let game = app.game.as_ref().unwrap();
        assert!(game.open);
        assert_eq!(game.nodes.len(), 5);
        assert_eq!(game.score.unwrap().total(), 100.0);
        assert_eq!(app.state.sim_time, 42.0);
        assert!(app.state.view_lock.is_some());

        app.toggle_menu();
        // A directory cannot be loaded as a YAML map.
        app.select_map(std::path::Path::new(env!("CARGO_MANIFEST_DIR")));
        assert!(app.menu.open && app.menu.error.is_some());
        assert_eq!(app.game.as_ref().unwrap().nodes.len(), 5);
        assert_eq!(app.state.sim_time, 42.0);

        let path = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/configs/halo.yaml"));
        app.select_map(path);
        assert!(!app.menu.open && app.menu.error.is_none());
        assert_eq!(app.state.scene.body(app.state.scene.host).name, "Halo");
        assert!(app.state.scene.atmosphere.is_some());
        assert!(crate::WARPS[app.state.warp] == 0.0 && app.state.view_lock.is_none());
        let game = app.game.as_ref().unwrap();
        assert!(!game.open);
        assert_eq!(game.nodes.len(), 1);
        assert!(game.viewer.is_none() && game.score.is_none() && game.undo.is_empty());
    }

    #[test]
    fn outermost_of_three_moons_is_distinct_from_inner_and_middle() {
        let tree = plain(&[None, Some(0), Some(1), Some(1), Some(1)]);
        assert_eq!(score(&tree, 4, &tree, 4).total(), 100.0);
        for viewer in [2, 3] {
            let result = score(&tree, viewer, &tree, 4);
            assert_eq!(result.topology, 100.0);
            assert!(!result.viewer);
            assert_eq!(result.total(), 80.0);
        }
        // Omitting the innermost moon must not shift the viewer to rank three.
        let missing = plain(&[None, Some(0), Some(1), Some(1)]);
        assert!(!score(&missing, 3, &tree, 4).viewer);
    }

    #[test]
    fn swapping_planets_with_different_satellite_families_loses_credit() {
        let a = plain(&[None, Some(0), Some(0), Some(1)]);
        let b = plain(&[None, Some(0), Some(0), Some(2)]);
        let result = score(&a, 3, &b, 3);
        assert!(result.topology < 100.0);
        assert!(!result.viewer);
    }

    #[test]
    fn scoring_penalizes_missing_extra_and_wrong_parent_links() {
        let tree = plain(&[None, Some(0), Some(1)]);
        let flat = plain(&[None, Some(0), Some(0)]);
        let short = plain(&[None, Some(0)]);
        assert!(score(&flat, 2, &tree, 2).topology < 100.0);
        assert!(score(&short, 1, &tree, 2).topology < 100.0);
        assert!(score(&tree, 2, &short, 1).topology < 100.0);
    }

    #[test]
    fn root_need_not_be_first_and_anchor_is_part_of_topology() {
        let a = plain(&[None, Some(0), Some(0)]);
        let b = plain(&[Some(1), None, Some(1)]);
        assert_eq!(score(&a, 1, &b, 0).total(), 100.0);
    }

    #[test]
    fn scene_order_comes_from_orbital_size_not_file_order() {
        let mut scene = puzzle();
        let guess = correct_puzzle();
        assert_eq!(
            score(
                &guess.topology(),
                4,
                &Topology::from_scene(&scene),
                scene.host
            )
            .total(),
            100.0
        );
        // All three swapped bodies share the same parent and have no children.
        scene.bodies.swap(2, 4);
        scene.host = 2;
        assert_eq!(
            score(
                &guess.topology(),
                4,
                &Topology::from_scene(&scene),
                scene.host
            )
            .total(),
            100.0
        );
        let giant = scene.bodies.iter().position(|b| b.name == "giant").unwrap();
        let ordered = Topology::from_scene(&scene);
        assert_eq!(ordered.children[giant], vec![4, 3, 2]);
    }

    #[test]
    fn star_and_ring_traits_are_scored_at_the_ordered_location() {
        let mut truth = plain(&[None, Some(0), Some(0)]);
        truth.traits[0].star = true;
        truth.traits[2].rings = true;
        let mut guess = plain(&[None, Some(0), Some(0)]);
        let missing = score(&guess, 2, &truth, 2);
        assert_eq!(missing.topology, 100.0);
        assert!(missing.traits < 100.0);
        guess.traits[0].star = true;
        guess.traits[2].rings = true;
        assert_eq!(score(&guess, 2, &truth, 2).total(), 100.0);
        guess.traits[2].rings = false;
        guess.traits[1].rings = true;
        assert!(score(&guess, 2, &truth, 2).traits < 100.0);
    }

    #[test]
    fn deleting_branch_remaps_viewer_and_compacts_orbit_ranks() {
        let mut g = correct_puzzle();
        g.nodes[4].traits.rings = true;
        g.selected = 2;
        g.delete();
        assert_eq!(g.nodes.len(), 4);
        assert_eq!(g.viewer, Some(3));
        assert_eq!(g.nodes[2].orbit_rank, 0);
        assert_eq!(g.nodes[3].orbit_rank, 1);
        assert!(g.nodes[3].traits.rings);
        let (nodes, viewer, _) = g.undo.pop().unwrap();
        assert_eq!(nodes.len(), 5);
        assert_eq!(viewer, Some(4));
        g.selected = 1;
        g.delete();
        assert_eq!(g.nodes.len(), 1);
        assert_eq!(g.viewer, None);
    }

    #[test]
    fn notebook_interaction_creates_marks_scores_and_undoes_without_sky_clicks() {
        let scene =
            crate::config::load(concat!(env!("CARGO_MANIFEST_DIR"), "/configs/puzzle.yaml"))
                .unwrap();
        let mut g = Game::default();
        let l = Layout::new(1280.0, 720.0, 1.0);
        let center = |r: Rect| ((r.x + r.w * 0.5) as f64, (r.y + r.h * 0.5) as f64);
        assert!(!g.click(&l, (400.0, 400.0), &scene));
        assert!(g.click(&l, center(l.toggle), &scene));
        assert!(g.open);
        g.click(&l, center(l.traits[0]), &scene);
        assert!(g.nodes[0].traits.star);
        g.click(&l, center(l.traits[1]), &scene);
        assert!(g.nodes[0].traits.rings);
        g.click(&l, center(l.buttons[3]), &scene);
        assert!(!g.nodes[0].traits.rings);
        assert!(g.nodes[0].traits.star);
        g.click(&l, center(l.buttons[4]), &scene);
        assert!(g.score.is_none()); // A viewer is required before submission.
        let pos = l.point([0.8, 0.7]);
        g.right_click(&l, (pos[0] as f64, pos[1] as f64));
        assert_eq!(g.nodes.len(), 2);
        assert_eq!(g.nodes[1].parent, Some(0));
        g.click(&l, center(l.buttons[1]), &scene);
        assert_eq!(g.viewer, Some(1));
        g.click(&l, center(l.buttons[4]), &scene);
        assert!(g.score.is_some());
        assert!(g.result_open);
        g.key(KeyCode::Escape, &l, (0.0, 0.0), &scene);
        assert!(!g.result_open);
        g.click(&l, center(l.buttons[3]), &scene);
        assert_eq!(g.viewer, None);
        assert!(g.score.is_none());
        g.click(&l, center(l.toggle), &scene);
        assert!(!g.open);
    }

    #[test]
    fn dragging_changes_orbit_order_viewer_score_and_undoes_as_one_edit() {
        let scene = puzzle();
        let truth = Topology::from_scene(&scene);
        let mut g = correct_puzzle();
        let layout = Layout::new(1280.0, 720.0, 1.0);
        let original = g.nodes[4].pos;
        let history = g.undo.len();
        g.score = Some(score(&g.topology(), 4, &truth, scene.host));
        assert_eq!(g.score.unwrap().total(), 100.0);
        g.dragging = Some(4);
        for pos in [[0.4, 0.45], [0.4, 0.4], [0.4, 0.35]] {
            let point = layout.point(pos);
            g.motion(&layout, (point[0] as f64, point[1] as f64));
        }
        g.release();
        assert_eq!(g.nodes[4].orbit_rank, 0);
        assert_eq!(g.siblings(1), vec![4, 2, 3]);
        assert_eq!(g.viewer, Some(4));
        assert!(g.score.is_none());
        assert!(!score(&g.topology(), 4, &truth, scene.host).viewer);
        assert_eq!(g.undo.len(), history + 1);
        let (nodes, viewer, selected) = g.undo.pop().unwrap();
        g.nodes = nodes;
        g.viewer = viewer;
        g.selected = selected;
        assert_eq!(g.nodes[4].pos, original);
        assert_eq!(score(&g.topology(), 4, &truth, scene.host).total(), 100.0);
    }

    #[test]
    fn placement_and_buttons_agree_with_visible_distance_at_any_window_shape() {
        let mut g = correct_puzzle();
        g.selected = 1;
        g.add([0.4, 0.35]); // newest object is nearer, rather than always outermost
        assert_eq!(g.nodes[5].orbit_rank, 0);
        for (w, h) in [(1280.0, 720.0), (640.0, 1200.0), (320.0, 240.0)] {
            let l = Layout::new(w, h, 1.0);
            for parent in 0..g.nodes.len() {
                let center = l.point(g.nodes[parent].pos);
                let radii: Vec<_> = g
                    .siblings(parent)
                    .iter()
                    .map(|&i| {
                        let p = l.point(g.nodes[i].pos);
                        (p[0] - center[0]).hypot(p[1] - center[1])
                    })
                    .collect();
                assert!(radii.windows(2).all(|r| r[0] <= r[1]));
            }
        }
    }

    #[test]
    fn clear_confirmation_blocks_edits_and_supports_cancel_undo_and_redo() {
        let mut g = correct_puzzle();
        g.open = true;
        let l = Layout::new(1280.0, 720.0, 1.0);
        let scene = puzzle();
        let cursor = (l.canvas.x as f64 + 50.0, l.canvas.y as f64 + 50.0);
        g.key(KeyCode::KeyX, &l, cursor, &scene);
        assert!(g.confirm_clear);
        g.right_click(&l, cursor);
        g.key(KeyCode::Delete, &l, cursor, &scene);
        g.key(KeyCode::KeyS, &l, cursor, &scene);
        assert_eq!(g.nodes.len(), 5);
        g.click(
            &l,
            (l.toggle.x as f64 + 1.0, l.toggle.y as f64 + 1.0),
            &scene,
        );
        assert!(g.open && g.confirm_clear);
        g.key(KeyCode::Escape, &l, cursor, &scene);
        assert!(!g.confirm_clear && g.nodes.len() == 5);
        g.key(KeyCode::KeyX, &l, cursor, &scene);
        g.key(KeyCode::Enter, &l, cursor, &scene);
        assert_eq!(g.nodes.len(), 1);
        assert!(g.viewer.is_none() && !g.confirm_clear);
        g.key(KeyCode::KeyZ, &l, cursor, &scene);
        assert_eq!(g.nodes.len(), 5);
        assert_eq!(g.viewer, Some(4));
        g.key(KeyCode::KeyY, &l, cursor, &scene);
        assert_eq!(g.nodes.len(), 1);
        g.key(KeyCode::KeyZ, &l, cursor, &scene);
        g.key(KeyCode::KeyS, &l, cursor, &scene);
        assert!(g.redo.is_empty(), "new edits must invalidate redo");
    }

    #[test]
    fn orbit_labels_are_local_to_parent_and_update_after_drag() {
        let mut g = correct_puzzle();
        assert_eq!(g.nodes[0].label(), "C");
        assert_eq!(g.nodes[1].label(), "1");
        assert_eq!(g.nodes[2].label(), "1");
        assert_eq!(g.nodes[4].label(), "3");
        let l = Layout::new(1280.0, 720.0, 1.0);
        g.dragging = Some(4);
        let point = l.point([0.4, 0.35]);
        g.motion(&l, (point[0] as f64, point[1] as f64));
        g.release();
        assert_eq!(g.nodes[4].label(), "1");
        assert_eq!(g.nodes[2].label(), "2");
        g.history(false);
        assert_eq!(g.nodes[4].label(), "3");
        g.history(true);
        assert_eq!(g.nodes[4].label(), "1");
    }

    #[test]
    fn right_click_add_uses_selected_parent_and_delete_redo_restores_branch_edit() {
        let mut g = Game {
            open: true,
            ..Game::default()
        };
        let l = Layout::new(1280.0, 720.0, 1.0);
        let scene = puzzle();
        for pos in [[0.8, 0.5], [0.9, 0.8]] {
            let p = l.point(pos);
            g.right_click(&l, (p[0] as f64, p[1] as f64));
        }
        assert_eq!(g.nodes[1].parent, Some(0));
        assert_eq!(g.nodes[2].parent, Some(1));
        g.key(KeyCode::KeyV, &l, (0.0, 0.0), &scene);
        g.selected = 1;
        g.key(KeyCode::Delete, &l, (0.0, 0.0), &scene);
        assert_eq!(g.nodes.len(), 1);
        assert_eq!(g.viewer, None);
        g.history(false);
        assert_eq!(g.nodes.len(), 3);
        assert_eq!(g.viewer, Some(2));
        g.history(true);
        assert_eq!(g.nodes.len(), 1);
    }

    #[test]
    fn wide_diagram_fits_after_resize_without_changing_orbit_order() {
        let mut g = correct_puzzle();
        let l = g.layout(1600.0, 720.0, 1.0);
        g.open = true;
        g.selected = 0;
        g.right_click(&l, (l.canvas.x as f64 + 40.0, l.canvas.y as f64 + 50.0));
        let saved_positions: Vec<_> = g.nodes.iter().map(|n| n.pos).collect();
        let saved_order = g.siblings(0);
        for (w, h, scale) in [
            (1280.0, 720.0, 1.0),
            (640.0, 1200.0, 1.6),
            (320.0, 240.0, 1.0),
        ] {
            let l = g.layout(w, h, scale);
            for node in &g.nodes {
                let p = l.point(node.pos);
                assert!(l.canvas.contains(p[0] as f64, p[1] as f64));
                let pos = l.normalized((p[0] as f64, p[1] as f64));
                assert!((pos[0] - node.pos[0]).abs() < 1e-5 && (pos[1] - node.pos[1]).abs() < 1e-5);
            }
            for confirmation in [false, true] {
                g.confirm_clear = confirmation;
                let mut verts = Vec::new();
                build(&mut verts, [w, h], scale, &g, (0.0, 0.0), "Stopped");
                assert!(verts.iter().all(|v| {
                    v.pos
                        .iter()
                        .all(|p| p.is_finite() && (-1.001..=1.001).contains(p))
                }));
            }
            assert_eq!(g.siblings(0), saved_order);
            assert_eq!(
                g.nodes.iter().map(|n| n.pos).collect::<Vec<_>>(),
                saved_positions
            );
        }
    }

    #[test]
    fn toolbar_navigation_matches_observation_and_clicks_use_the_new_slots() {
        let scene = puzzle();
        for (w, h, scale) in [
            (1920.0, 1080.0, 1.0),
            (1600.0, 900.0, 1.0),
            (1440.0, 900.0, 1.0),
            (1301.0, 720.0, 1.0),
            (1300.0, 720.0, 1.0),
            (1280.0, 480.0, 1.0),
            (640.0, 480.0, 2.0),
        ] {
            let mut g = Game {
                open: true,
                ..Game::default()
            };
            let l = g.layout(w, h, scale);
            let observe = crate::ui::layout(w, h, scale).with_puzzle(true);
            for (a, b) in [
                (l.maps, observe.home),
                (l.toggle, observe.toggle),
                (l.bar, observe.bar),
            ] {
                assert_eq!([a.x, a.y, a.w, a.h], [b.x, b.y, b.w, b.h]);
            }
            let controls = [
                l.maps,
                l.toggle,
                l.traits[0],
                l.traits[1],
                l.buttons[1],
                l.buttons[3],
                l.buttons[0],
                l.buttons[2],
                l.buttons[5],
                l.buttons[4],
            ];
            assert!(controls.windows(2).all(|p| p[0].right() <= p[1].x));
            for r in l.traits {
                assert!(g.click(&l, ((r.x + r.w * 0.5) as f64, r.center_y() as f64), &scene));
            }
            assert!(g.nodes[0].traits.star && g.nodes[0].traits.rings);
            assert_eq!(
                g.nodes.len(),
                1,
                "toolbar clicks must not add canvas objects"
            );
            assert!(g.click(
                &l,
                (
                    (l.toggle.x + l.toggle.w * 0.5) as f64,
                    l.toggle.center_y() as f64
                ),
                &scene
            ));
            assert!(!g.open);
        }
    }

    #[test]
    fn controls_fit_small_and_hidpi_windows() {
        for (w, h, s) in [
            (1280.0, 720.0, 1.0),
            (640.0, 480.0, 2.0),
            (320.0, 240.0, 1.0),
        ] {
            let l = Layout::new(w, h, s);
            for r in l
                .buttons
                .iter()
                .chain(l.traits.iter())
                .chain([&l.toggle, &l.maps, &l.canvas])
            {
                assert!(r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= w && r.y + r.h <= h);
            }
        }
    }
}
