import { test, expect } from '@playwright/test';

test.beforeEach(async ({ page }) => {
  page.on('pageerror', error => { throw error; });
  await page.goto('/');
  await expect(page.getByRole('navigation', { name: 'Main menu' })).toBeVisible();
});

test('normal launch hides Explore; keyboard selection opens settings', async ({ page }) => {
  await expect(page.getByRole('button', { name: 'Explore', exact: false })).toHaveCount(0);
  await page.keyboard.press('ArrowDown'); await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { name: 'Settings', exact: true })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('navigation', { name: 'Main menu' })).toBeVisible();
});

test('settings are a draft, Apply persists them and Back cancels them', async ({ page }) => {
  await page.getByRole('button', { name: /Settings/ }).click();
  await page.getByRole('button', { name: 'High', exact: true }).click();
  await page.getByRole('button', { name: /Back/ }).click();
  await page.getByRole('button', { name: /Settings/ }).click();
  await expect(page.getByRole('button', { name: 'Balanced', exact: true })).toHaveAttribute('aria-pressed', 'true');
  await page.getByRole('button', { name: 'High', exact: true }).click();
  await page.getByRole('button', { name: /Apply/ }).click();
  await page.reload(); await page.getByRole('button', { name: /Settings/ }).click();
  await expect(page.getByRole('button', { name: 'High', exact: true })).toHaveAttribute('aria-pressed', 'true');
});

test('level observation preserves the current four-panel HUD and keyboard actions', async ({ page }) => {
  await page.getByRole('button', { name: /^Play/ }).click();
  await expect(page.getByRole('heading', { name: 'Levels' })).toBeVisible();
  await page.getByRole('button', { name: /Play Enter/ }).first().click();
  await expect(page.getByRole('region', { name: 'Time controls' })).toBeVisible();
  await page.keyboard.press('ArrowLeft'); await expect(page.locator('#speed-readout')).toHaveText('−1 min/s');
  await page.keyboard.press('Space'); await expect(page.locator('#speed-readout')).toHaveText('0 min/s');
  await page.keyboard.press('Period'); await expect(page.locator('#ev-readout')).toHaveText('+0.25 EV');
  await expect(page.getByRole('switch', { name: 'Auto exposure' })).toHaveAttribute('aria-checked', 'false');
  await page.keyboard.press('KeyH'); await expect(page.getByRole('region', { name: 'Lock status' })).toHaveCount(0);
  await page.keyboard.press('KeyH'); await expect(page.getByRole('region', { name: 'Lock status' })).toBeVisible();
});

test('theory edits, viewer, checks, undo, and clear confirmation work', async ({ page }) => {
  await page.goto('/#theory');
  const svg = page.locator('#diagram'); await expect(svg).toBeVisible();
  await svg.click({ button: 'right', position: { x: 300, y: 200 } });
  await expect(page.locator('[data-node]')).toHaveCount(2);
  await page.keyboard.press('KeyS'); await page.keyboard.press('KeyV');
  await expect(page.locator('.viewer-marker')).toHaveCount(1);
  await page.keyboard.press('Enter'); await expect(page.getByRole('dialog')).toBeVisible();
  await expect(page.getByText('72.4', { exact: true })).toBeVisible();
  await page.keyboard.press('Escape');
  await page.keyboard.press('KeyX'); await expect(page.getByRole('heading', { name: 'Clear the entire theory?' })).toBeVisible();
  await page.getByRole('button', { name: /Clear all Enter/ }).click();
  await expect(page.locator('[data-node]')).toHaveCount(1);
  await page.keyboard.press('KeyZ'); await expect(page.locator('[data-node]')).toHaveCount(2);
  await page.keyboard.press('KeyM'); await expect(page.getByRole('heading', { name: 'Keep your progress?' })).toBeVisible();
  await page.getByRole('button', { name: 'Keep & leave' }).click();
  await expect(page.getByRole('button', { name: /Resume Enter/ })).toBeVisible();
});

test('Explore is opt-in and search/favorites do not break empty states', async ({ page }) => {
  await page.goto('/?explore=1');
  await page.getByRole('button', { name: /^Explore/ }).click();
  await page.getByRole('searchbox').fill('no such system');
  await expect(page.getByRole('heading', { name: 'No systems found' })).toBeVisible();
  await page.getByRole('searchbox').fill('solar');
  await expect(page.getByRole('heading', { name: 'Solar System' })).toBeVisible();
  await page.getByRole('button', { name: 'Favorite Solar System' }).click();
  await page.getByRole('button', { name: /Favorites/ }).click();
  await expect(page.getByRole('heading', { name: 'Solar System' })).toBeVisible();
});

for (const [width, height] of [[640, 480], [800, 600], [1280, 720], [1920, 1080]]) {
  test(`HUD boxes fit and remain apart at ${width}x${height}`, async ({ page }) => {
    await page.setViewportSize({ width, height }); await page.goto('/#observe');
    await expect(page.locator('.time-panel')).toBeVisible();
    const rects = await page.locator('.hud-legend,.time-panel,.exposure-panel,.lock-panel').evaluateAll(nodes => nodes.map(n => { const r = n.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height, right: r.right, bottom: r.bottom }; }));
    for (const r of rects) { expect(r.x).toBeGreaterThanOrEqual(0); expect(r.y).toBeGreaterThanOrEqual(0); expect(r.right).toBeLessThanOrEqual(width + .1); expect(r.bottom).toBeLessThanOrEqual(height + .1); }
    for (let i = 0; i < rects.length; i++) for (let j = i + 1; j < rects.length; j++) {
      const a = rects[i], b = rects[j]; expect(a.x < b.right && b.x < a.right && a.y < b.bottom && b.y < a.bottom).toBeFalsy();
    }
  });
}

test('captures representative screens for visual inspection', async ({ page }) => {
  for (const screen of ['title', 'levels', 'settings', 'controls', 'observe', 'theory', 'texture']) {
    await page.goto(`/#${screen}`); await page.evaluate(() => document.fonts.ready);
    await page.screenshot({ path: `test-results/${screen}-1280x720.png` });
  }
});

test('reload keeps the active level and theory; browser Back restores the previous screen', async ({ page }) => {
  await page.getByRole('button', { name: /^Play/ }).click();
  await page.getByRole('button', { name: /Play Enter/ }).nth(1).click();
  await expect(page.getByRole('heading', { name: 'Shadow season' })).toBeVisible();
  await page.keyboard.press('Tab');
  await page.locator('#diagram').click({ button: 'right', position: { x: 300, y: 200 } });
  await page.reload();
  await expect(page.locator('[data-node]')).toHaveCount(2);
  await expect(page.locator('.theory-header .eyebrow')).toHaveText('Shadow season');
  await page.keyboard.press('Tab');
  await expect(page.getByRole('heading', { name: 'Shadow season' })).toBeVisible();
  await page.goBack();
  await expect(page.getByRole('heading', { name: 'Your theory' })).toBeVisible();
});

test('mobile menus, settings, and editor controls stay reachable', async ({ page }) => {
  await page.setViewportSize({ width: 640, height: 480 });
  for (const route of ['title', 'levels', 'settings', 'controls', 'theory']) {
    await page.goto(`/#${route}`);
    expect(await page.locator('body').evaluate(el => el.scrollWidth <= innerWidth)).toBeTruthy();
    if (route === 'settings') {
      await page.getByRole('slider', { name: 'HUD scale' }).scrollIntoViewIfNeeded();
      await expect(page.getByRole('slider', { name: 'HUD scale' })).toBeVisible();
      await expect(page.getByRole('button', { name: /Apply/ })).toBeVisible();
    }
    if (route === 'theory') await expect(page.getByRole('button', { name: /Check theory/ })).toBeVisible();
    await page.screenshot({ path: `test-results/${route}-640x480.png` });
  }
});
