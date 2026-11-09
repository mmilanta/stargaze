import test from 'node:test';
import assert from 'node:assert/strict';
import { createState, freshTheory, freshObservation, addNode, remember, history, deleteBranch, clearTheory, toggleTrait, progress, orbitRank, validTheory, changeSpeed, togglePlayback, SPEEDS, STOP, clock, previewCheck, DEFAULT_SETTINGS } from '../src/model.js';

test('signed playback crosses stop and resumes the previous reverse speed', () => {
  const o = freshObservation();
  changeSpeed(o, -1); assert.equal(SPEEDS[o.speed], -1);
  togglePlayback(o); assert.equal(o.speed, STOP);
  togglePlayback(o); assert.equal(SPEEDS[o.speed], -1);
  for (let i = 0; i < 20; i++) changeSpeed(o, 1);
  assert.equal(o.speed, SPEEDS.length - 1);
});
test('clock wraps correctly before day zero', () => {
  assert.equal(clock(-1), 'day -1 · 23:59');
  assert.equal(clock(1440), 'day 1 · 00:00');
});
test('dragging changes sibling order, and deleting a branch remaps the viewer safely', () => {
  const t = freshTheory(); addNode(t, 700, 330); const outer = t.selected;
  t.selected = 0; addNode(t, 600, 330); const inner = t.selected;
  assert.equal(orbitRank(t, t.nodes.find(n => n.id === outer)), 2);
  assert.equal(orbitRank(t, t.nodes.find(n => n.id === inner)), 1);
  addNode(t, 620, 400); t.viewer = t.selected;
  t.selected = inner; deleteBranch(t);
  assert.equal(t.viewer, null); assert.equal(t.nodes.length, 2);
  history(t); assert.equal(t.nodes.length, 4); assert.notEqual(t.viewer, null);
  history(t, true); assert.equal(t.nodes.length, 2); assert.equal(t.viewer, null);
});
test('clear can be undone, including traits and viewer', () => {
  const t = freshTheory(); addNode(t, 650, 330); toggleTrait(t, 'rings'); t.viewer = t.selected;
  clearTheory(t); assert.equal(t.nodes.length, 1); assert.equal(t.viewer, null);
  history(t); assert.equal(t.nodes.length, 2); assert.equal(t.nodes[1].rings, true); assert.equal(t.viewer, 1);
});
test('only-root edits are still saved progress', () => {
  const t = freshTheory(); assert.equal(progress(t).started, false);
  toggleTrait(t, 'star'); assert.equal(progress(t).started, true);
});
test('invalid saved diagrams are discarded without losing valid settings', () => {
  const t = freshTheory(); t.nodes.push({ id: 1, parent: 1, x: 0, y: 0 });
  assert.equal(validTheory(t), false);
  const s = createState({ theories: { '1-halo-volcano': t }, settings: { hud: 150, bounces: -100, fps: -1 } });
  assert.deepEqual(s.theories, {}); assert.equal(s.settings.hud, 150);
  assert.equal(s.settings.bounces, 1); assert.equal(s.settings.fps, DEFAULT_SETTINGS.fps);
});
test('the editor bounds history and node count', () => {
  const t = freshTheory();
  for (let i = 0; i < 70; i++) addNode(t, 100 + i, 100);
  assert.equal(t.nodes.length, 64);
  for (let i = 0; i < 150; i++) remember(t);
  assert.equal(t.undo.length, 100);
});
test('result preview requires a viewer and retains attempt history', () => {
  const t = freshTheory(); assert.equal(previewCheck(t), null);
  t.viewer = 0; assert.equal(previewCheck(t).total, 72.4);
  previewCheck(t); assert.equal(t.attempts.length, 2); assert.equal(t.best, 72.4);
});
