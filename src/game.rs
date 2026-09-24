//! Orbital-tree puzzle. Relative diagram radii determine the scored sibling order.
use winit::keyboard::KeyCode;

use crate::sim::{BodyKind, Scene};
use crate::ui::{Rect, UiVertex, push_rect, push_text};

const MAX_NODES: usize = 64;
const INK: [f32; 4] = [0.85, 0.91, 0.97, 1.0];
const MUTED: [f32; 4] = [0.48, 0.61, 0.73, 1.0];
const ACCENT: [f32; 4] = [0.34, 0.87, 0.75, 1.0];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Traits {
    star: bool,
    rings: bool,
}

#[derive(Clone)]
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
}

#[derive(Clone, Copy, Debug)]
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

pub struct Game {
    pub open: bool,
    nodes: Vec<Node>,
    selected: usize,
    viewer: Option<usize>,
    dragging: Option<usize>,
    drag_saved: bool,
    undo: Vec<(Vec<Node>, Option<usize>, usize)>,
    redo: Vec<(Vec<Node>, Option<usize>, usize)>,
    pub confirm_clear: bool,
    score: Option<Score>,
    message: String,
}

impl Default for Game {
    fn default() -> Self {
        Self {
            open: false,
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
            score: None,
            message:
                "Select a parent, then right-click empty space to add. [N] also adds at the cursor."
                    .into(),
        }
    }
}

impl Game {
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
        if !self.open || self.confirm_clear || !l.canvas.contains(cursor.0, cursor.1) {
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

    pub fn key(&mut self, key: KeyCode, l: &Layout, cursor: (f64, f64), scene: &Scene) {
        if self.confirm_clear {
            match key {
                KeyCode::Enter => self.clear(),
                KeyCode::Escape => self.confirm_clear = false,
                _ => {}
            }
            return;
        }
        match key {
            KeyCode::KeyN => self.right_click(l, cursor),
            KeyCode::KeyV => self.action(1, scene),
            KeyCode::Delete => self.action(2, scene),
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
        if self.confirm_clear {
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
    bar: Rect,
    canvas: Rect,
    buttons: [Rect; 6],
    traits: [Rect; 2],
    confirm: Rect,
    cancel: Rect,
    dialog: Rect,
    center: [f32; 2],
    unit: f32,
    scale: f32,
}

impl Layout {
    pub fn new(w: f32, h: f32, scale: f32) -> Self {
        let s = scale.min(w / 1316.0).min(h / 400.0);
        let bar = Rect {
            x: 0.0,
            y: h - 52.0 * s,
            w,
            h: 52.0 * s,
        };
        let mut x = 10.0 * s;
        let mut control = |width: f32, gap: f32| {
            x += gap * s;
            let r = Rect {
                x,
                y: bar.y + 9.0 * s,
                w: width * s,
                h: 34.0 * s,
            };
            x += (width + 6.0) * s;
            r
        };
        let maps = control(84.0, 0.0);
        let toggle = control(120.0, 0.0);
        let viewer = control(142.0, 18.0);
        let star = control(90.0, 0.0);
        let rings = control(138.0, 0.0);
        let undo = control(90.0, 18.0);
        let redo = control(90.0, 0.0);
        let delete = control(128.0, 0.0);
        let clear = control(132.0, 0.0);
        let check = control(142.0, 18.0);
        let dialog = Rect {
            x: (w - 470.0 * s) * 0.5,
            y: (h - 160.0 * s) * 0.5,
            w: 470.0 * s,
            h: 160.0 * s,
        };
        Self {
            toggle,
            maps,
            bar,
            buttons: [redo, viewer, delete, undo, check, clear],
            traits: [star, rings],
            confirm: Rect {
                x: dialog.x + 238.0 * s,
                y: dialog.y + 108.0 * s,
                w: 212.0 * s,
                h: 32.0 * s,
            },
            cancel: Rect {
                x: dialog.x + 20.0 * s,
                y: dialog.y + 108.0 * s,
                w: 196.0 * s,
                h: 32.0 * s,
            },
            dialog,
            center: [0.5, 0.5],
            unit: (w - 24.0 * s).min(h - 152.0 * s),
            canvas: Rect {
                x: 12.0 * s,
                y: 58.0 * s,
                w: w - 24.0 * s,
                h: h - 152.0 * s,
            },
            scale: s,
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
    verts: &mut Vec<UiVertex>,
    a: [f32; 2],
    b: [f32; 2],
    thickness: f32,
    color: [f32; 4],
    viewport: [f32; 2],
) {
    let length = (b[0] - a[0]).hypot(b[1] - a[1]);
    let count = (length / thickness.max(1.0)).ceil().max(1.0) as usize;
    for i in 0..=count {
        let t = i as f32 / count as f32;
        push_rect(
            verts,
            a[0] + (b[0] - a[0]) * t - thickness * 0.5,
            a[1] + (b[1] - a[1]) * t - thickness * 0.5,
            thickness,
            thickness,
            color,
            viewport,
        );
    }
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
    let count = (std::f32::consts::TAU * radius / (4.0 * scale))
        .ceil()
        .clamp(48.0, 2048.0) as usize;
    let color = if selected {
        [0.22, 0.48, 0.46, 1.0]
    } else {
        [0.13, 0.27, 0.34, 1.0]
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
        if i % 6 >= 3 {
            continue;
        }
        let a = point(i);
        let b = point(i + 1);
        if inner.contains(a[0] as f64, a[1] as f64) && inner.contains(b[0] as f64, b[1] as f64) {
            line(verts, a, b, scale, color, viewport);
        }
    }
}

fn circle(
    verts: &mut Vec<UiVertex>,
    p: [f32; 2],
    radius: f32,
    color: [f32; 4],
    viewport: [f32; 2],
) {
    let mut last = [p[0] + radius, p[1]];
    for i in 1..=48 {
        let a = i as f32 * std::f32::consts::TAU / 48.0;
        let next = [p[0] + a.cos() * radius, p[1] + a.sin() * radius];
        line(verts, last, next, 1.5, color, viewport);
        last = next;
    }
}

pub fn build(
    verts: &mut Vec<UiVertex>,
    viewport: [f32; 2],
    scale: f32,
    game: &Game,
    cursor: (f64, f64),
    observation_status: &str,
) {
    let [w, h] = viewport;
    let l = game.layout(w, h, scale);
    let s = l.scale;
    let text = |verts: &mut Vec<UiVertex>, x: f32, y: f32, size: f32, value: &str, color| {
        push_text(verts, x, y, size * s, value, color, viewport);
    };
    let button = |verts: &mut Vec<UiVertex>, rect: Rect, label: &str, active: bool| {
        crate::ui::toolbar_button(
            verts,
            rect,
            label,
            s,
            active,
            rect.contains(cursor.0, cursor.1),
            viewport,
        );
    };
    if game.open {
        push_rect(verts, 0.0, 0.0, w, h, [0.012, 0.023, 0.04, 1.0], viewport);
        crate::ui::toolbar_panel(
            verts,
            l.bar,
            s,
            &[l.buttons[1], l.buttons[3], l.buttons[4]],
            viewport,
        );
        text(verts, 20.0 * s, 20.0 * s, 1.6, "YOUR THEORY", INK);
        text(
            verts,
            200.0 * s,
            23.0 * s,
            1.0,
            "Select a parent, right-click to add [N]. Drag to order orbits. 1 = innermost.",
            MUTED,
        );
        for (r, label, active) in [
            (l.traits[0], "[S]tar", game.nodes[game.selected].traits.star),
            (
                l.traits[1],
                "Has [R]ings",
                game.nodes[game.selected].traits.rings,
            ),
        ] {
            button(verts, r, label, active);
        }
        for (i, label) in [
            "Redo [Y]",
            "[V]iewer here",
            "Delete [Del]",
            "Undo [Z]",
            "Check [Enter]",
            "Clear all [X]",
        ]
        .iter()
        .enumerate()
        {
            button(
                verts,
                l.buttons[i],
                label,
                i == 1 && game.viewer == Some(game.selected),
            );
        }
        let c = l.canvas;
        push_rect(
            verts,
            c.x,
            c.y,
            c.w,
            c.h,
            [0.021, 0.039, 0.062, 1.0],
            viewport,
        );
        let step = 32.0 * s;
        let mut x = c.x + step;
        while x < c.x + c.w {
            let mut y = c.y + step;
            while y < c.y + c.h {
                push_rect(verts, x, y, 1.0, 1.0, [0.14, 0.21, 0.28, 1.0], viewport);
                y += step;
            }
            x += step;
        }
        for (i, node) in game.nodes.iter().enumerate() {
            if let Some(parent) = node.parent {
                let a = l.point(game.nodes[parent].pos);
                let b = l.point(node.pos);
                dashed_orbit(
                    verts,
                    a,
                    (b[0] - a[0]).hypot(b[1] - a[1]),
                    c,
                    s,
                    viewport,
                    i == game.selected,
                );
            }
        }
        for node in &game.nodes {
            if let Some(parent) = node.parent {
                let a = l.point(game.nodes[parent].pos);
                let b = l.point(node.pos);
                line(verts, a, b, 1.5 * s, [0.23, 0.40, 0.48, 1.0], viewport);
            }
        }
        for (i, node) in game.nodes.iter().enumerate() {
            let p = l.point(node.pos);
            let r = 18.0 * s;
            // Opaque plate keeps connections out of the node interior.
            push_rect(
                verts,
                p[0] - r,
                p[1] - r,
                r * 2.0,
                r * 2.0,
                [0.021, 0.039, 0.062, 1.0],
                viewport,
            );
            let color = if node.traits.star {
                [1.0, 0.77, 0.28, 1.0]
            } else {
                MUTED
            };
            circle(verts, p, r, color, viewport);
            if node.traits.star {
                for ray in 0..8 {
                    let angle = ray as f32 * std::f32::consts::TAU / 8.0;
                    let point = |radius: f32| {
                        [
                            p[0] + angle.cos() * radius * s,
                            p[1] + angle.sin() * radius * s,
                        ]
                    };
                    line(verts, point(26.0), point(32.0), 2.0 * s, color, viewport);
                }
            }
            if node.traits.rings {
                let mut last = [p[0] + 31.0 * s, p[1]];
                for segment in 1..=48 {
                    let angle = segment as f32 * std::f32::consts::TAU / 48.0;
                    let next = [p[0] + angle.cos() * 31.0 * s, p[1] + angle.sin() * 10.0 * s];
                    line(verts, last, next, 1.5 * s, INK, viewport);
                    last = next;
                }
            }
            if i == game.selected {
                circle(verts, p, r + 4.0 * s, ACCENT, viewport);
            }
            let label = node.label();
            text(
                verts,
                p[0] - label.len() as f32 * 4.8 * s,
                p[1] - 4.8 * s,
                1.2,
                &label,
                INK,
            );
            let label = if game.viewer == Some(i) { "YOU" } else { "" };
            if !label.is_empty() {
                let y = if p[1] + 48.0 * s < c.y + c.h {
                    p[1] + 37.0 * s
                } else {
                    p[1] - 46.0 * s
                };
                let width = label.len() as f32 * 8.0 * s;
                let x = (p[0] - width * 0.5).clamp(c.x + 4.0 * s, c.x + c.w - width - 4.0 * s);
                push_rect(
                    verts,
                    x - 2.0 * s,
                    y - 2.0 * s,
                    width + 4.0 * s,
                    12.0 * s,
                    [0.021, 0.039, 0.062, 1.0],
                    viewport,
                );
                text(
                    verts,
                    x,
                    y,
                    1.0,
                    label,
                    if game.viewer == Some(i) {
                        ACCENT
                    } else {
                        MUTED
                    },
                );
            }
        }
        text(verts, 20.0 * s, h - 83.0 * s, 1.1, &game.message, INK);
        if let Some(score) = game.score {
            text(
                verts,
                20.0 * s,
                h - 63.0 * s,
                1.35,
                &format!(
                    "Score: {:.1}/100  Orbits: {:.1}%  Traits: {:.1}%  Viewer: {}",
                    score.total(),
                    score.topology,
                    score.traits,
                    if score.viewer {
                        "matches"
                    } else {
                        "does not match"
                    }
                ),
                ACCENT,
            );
        } else {
            text(
                verts,
                20.0 * s,
                h - 63.0 * s,
                1.0,
                "60% ordered structure + 20% star/ring traits + 20% viewer. Orbit 1 = innermost.",
                MUTED,
            );
        }
        button(verts, l.maps, "[M]enu", false);
        button(verts, l.toggle, "Observe [Tab]", false);
        if game.confirm_clear {
            push_rect(verts, 0.0, 0.0, w, h, [0.0, 0.0, 0.0, 0.72], viewport);
            let d = l.dialog;
            push_rect(
                verts,
                d.x,
                d.y,
                d.w,
                d.h,
                [0.035, 0.065, 0.10, 1.0],
                viewport,
            );
            text(
                verts,
                d.x + 20.0 * s,
                d.y + 22.0 * s,
                1.6,
                "Clear the entire theory?",
                INK,
            );
            text(
                verts,
                d.x + 20.0 * s,
                d.y + 58.0 * s,
                1.1,
                "Removes all objects and the viewer.",
                MUTED,
            );
            text(
                verts,
                d.x + 20.0 * s,
                d.y + 77.0 * s,
                1.1,
                "Undo can restore everything.",
                MUTED,
            );
            button(verts, l.cancel, "Cancel [Esc]", false);
            button(verts, l.confirm, "Clear all [Enter]", false);
        }
    } else {
        text(verts, 16.0 * s, 18.0 * s, 1.1, observation_status, MUTED);
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
            show_labels: true,
            show_hud: true,
            auto_exposure: true,
            ev_bias: 0.0,
            exposure_dragging: false,
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
    fn keyboard_add_uses_selected_parent_and_delete_redo_restores_branch_edit() {
        let mut g = Game {
            open: true,
            ..Game::default()
        };
        let l = Layout::new(1280.0, 720.0, 1.0);
        let scene = puzzle();
        for pos in [[0.8, 0.5], [0.9, 0.8]] {
            let p = l.point(pos);
            g.key(KeyCode::KeyN, &l, (p[0] as f64, p[1] as f64), &scene);
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
