import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const css = readFileSync(new URL('../src/style.css', import.meta.url), 'utf8');
const mainCss = readFileSync(new URL('../src/main-interface.css', import.meta.url), 'utf8');
const mainSource = readFileSync(new URL('../src/main.tsx', import.meta.url), 'utf8');

test('processing sliders preserve the approved SVG geometry', () => {
  assert.match(css, /--control-track-width:\s*151px/);
  assert.match(css, /--control-track-height:\s*8px/);
  assert.match(css, /--control-thumb-width:\s*28px/);
  assert.match(css, /--control-thumb-height:\s*18px/);
  assert.match(css, /--control-value-width:\s*70px/);
  assert.match(css, /--control-value-height:\s*26px/);
});

test('processing sliders preserve the approved SVG materials', () => {
  assert.match(css, /--control-track-fill:\s*#181818/);
  assert.match(css, /--control-thumb-top:\s*#989898/);
  assert.match(css, /--control-thumb-bottom:\s*#3c3c3c/i);
  assert.match(css, /control-noise\.svg/);
});

test('header status pills preserve the final Figma materials', () => {
  assert.match(mainCss, /aspect-ratio:\s*56\s*\/\s*22/);
  assert.match(mainCss, /border:\s*1px solid #596570/i);
  assert.match(mainCss, /linear-gradient\(164deg, #313b45 0%, #29323a 38%, #1c2228 100%\)/i);
  assert.match(mainCss, /inset 0 1px 0 rgba\(255, 255, 255, 0\.22\)/);
  assert.match(mainCss, /radial-gradient\(circle at 38% 30%, #72ed91 21%, #34c759 83%, #198a36\)/i);
});

test('processing controls use SF Compact Medium', () => {
  assert.match(css, /@font-face\s*{[\s\S]*?font-family:"SF Compact Text"/);
  assert.match(css, /\.control>label,[\s\S]*?font-weight:500/);
  assert.match(css, /\.control-value[\s\S]*?font-weight:500/);
});

test('the final stylesheet is imported after the legacy application rules', () => {
  assert.ok(mainSource.indexOf('"./main-interface.css"') > mainSource.indexOf('"./style.css"'));
});

test('the approved instrument surface is continuous through header meters and tabs', () => {
  assert.match(mainCss, /\.instrument-header,\s*\.meterstrip,\s*\.tabs\s*{/);
  assert.match(mainCss, /instrument-grain\.svg/);
  assert.match(mainCss, /\.meterstrip\s*{[\s\S]*?padding:\s*0 1\.413428%/);
});

test('the meter panel has only the approved white inner lower highlight', () => {
  assert.match(mainCss, /\.meter-canvas-scroll::after\s*{[\s\S]*?border: 0\.5px solid rgba\(255, 255, 255, 0\.03\)/);
  assert.match(mainCss, /border-right-color: rgba\(255, 255, 255, 0\.055\)/);
  assert.match(mainCss, /border-bottom-color: rgba\(255, 255, 255, 0\.16\)/);
  assert.match(mainCss, /inset 0 -1\.5px 0 -1px rgba\(255, 255, 255, 0\.075\)/);
  assert.doesNotMatch(mainCss, /\.meter-canvas-scroll::after\s*{[\s\S]*?inset 0 -\d+px[^;]*rgba\(0, 0, 0/);
  assert.doesNotMatch(mainCss, /\.meter-canvas-scroll::before/);
});

test('the processing tabs have the approved size background and separator shadow', () => {
  assert.match(mainCss, /\.tabs\s*{[\s\S]*?height:\s*58px/);
  assert.match(mainCss, /\.tabs\s*{[\s\S]*?gap:\s*8px/);
  assert.match(mainCss, /border-bottom:\s*1px solid #0c0c0c/i);
  assert.match(mainCss, /0 1px 0 rgba\(151, 150, 146, 0\.68\)/);
  assert.match(mainCss, /\.tabs > button\s*{[\s\S]*?height:\s*32px/);
});

test('single-group processing pages match the three-column approved composition', () => {
  assert.match(mainCss, /\.single-group-page \.processing-group\s*{[\s\S]*?width:\s*82\.5%/);
  assert.match(mainCss, /\.single-group-page \.processing-group > h3\s*{\s*display:\s*none/);
  assert.match(mainCss, /grid-template-columns:\s*repeat\(3, 237px\)/);
  assert.match(mainCss, /grid-template-rows:\s*repeat\(6, auto\)/);
  assert.match(mainCss, /grid-auto-flow:\s*column/);
});

test('band mix reserves enough width for two controls without overflow', () => {
  assert.match(css, /\.band-mix-page \.processing-group\s*{[^}]*min-width:650px/);
});
