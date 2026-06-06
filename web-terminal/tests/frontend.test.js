import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const projectRoot = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const indexHtml = readFileSync(path.join(projectRoot, 'index.html'), 'utf8');
const mainJs = readFileSync(path.join(projectRoot, 'src', 'main.js'), 'utf8');
const stylesCss = readFileSync(path.join(projectRoot, 'styles.css'), 'utf8');

test('terminal fills viewport', () => {
  assert.match(indexHtml, /id="terminal"/);
  assert.match(indexHtml, /\/terminal\/styles.css/);
  assert.match(stylesCss, /position:\s*fixed/);
  assert.match(stylesCss, /inset:\s*0/);
  assert.match(stylesCss, /height:\s*100%/);
  assert.match(stylesCss, /html,\s*body,\s*#terminal/);
});

test('terminal connects_to_terminal_ws', () => {
  assert.match(mainJs, /\/terminal\/ws/);
  assert.match(mainJs, /new WebSocket/);
});

test('terminal_writes_incoming_bytes', () => {
  assert.match(mainJs, /socket\.addEventListener\('message'/);
  assert.match(mainJs, /terminal\.write\(/);
  assert.match(mainJs, /new Uint8Array/);
});

test('terminal_sends_keyboard_bytes', () => {
  assert.match(mainJs, /terminal\.onData\(/);
  assert.match(mainJs, /encoder\.encode\(data\)/);
  assert.match(mainJs, /socket\.send/);
});

test('zmodem_sentry_routes_bytes', () => {
  assert.match(mainJs, /new Zmodem\.Sentry/);
  assert.match(mainJs, /to_terminal:/);
  assert.match(mainJs, /sender:/);
  assert.match(mainJs, /Zmodem\.Browser\.save_to_disk/);
  assert.match(mainJs, /Zmodem\.Browser\.send_files/);
});
