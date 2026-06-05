const terminalElement = document.getElementById('terminal');
const BBS_COLUMNS = 80;
const DEFAULT_ROWS = 25;
const MIN_ROWS = 24;

const terminal = new Terminal({
  cols: BBS_COLUMNS,
  convertEol: false,
  cursorBlink: true,
  fontFamily: 'monospace',
  fontSize: 16,
  rows: DEFAULT_ROWS,
});

let fitAddon;
if (window.FitAddon && typeof window.FitAddon.FitAddon === 'function') {
  fitAddon = new window.FitAddon.FitAddon();
  terminal.loadAddon(fitAddon);
}
terminal.open(terminalElement);
terminal.focus();

const resizeTerminalRows = () => {
  if (!fitAddon || typeof fitAddon.proposeDimensions !== 'function') {
    return;
  }

  const dimensions = fitAddon.proposeDimensions();
  if (!dimensions) {
    return;
  }

  terminal.resize(BBS_COLUMNS, Math.max(MIN_ROWS, dimensions.rows));
};

resizeTerminalRows();

const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
const socketUrl = `${protocol}//${window.location.host}/terminal/ws`;
const socket = new WebSocket(socketUrl);
socket.binaryType = 'arraybuffer';

const encoder = new TextEncoder();

const CP437_HIGH_CODE_POINTS = [
  0x00c7, 0x00fc, 0x00e9, 0x00e2, 0x00e4, 0x00e0, 0x00e5, 0x00e7,
  0x00ea, 0x00eb, 0x00e8, 0x00ef, 0x00ee, 0x00ec, 0x00c4, 0x00c5,
  0x00c9, 0x00e6, 0x00c6, 0x00f4, 0x00f6, 0x00f2, 0x00fb, 0x00f9,
  0x00ff, 0x00d6, 0x00dc, 0x00a2, 0x00a3, 0x00a5, 0x20a7, 0x0192,
  0x00e1, 0x00ed, 0x00f3, 0x00fa, 0x00f1, 0x00d1, 0x00aa, 0x00ba,
  0x00bf, 0x2310, 0x00ac, 0x00bd, 0x00bc, 0x00a1, 0x00ab, 0x00bb,
  0x2591, 0x2592, 0x2593, 0x2502, 0x2524, 0x2561, 0x2562, 0x2556,
  0x2555, 0x2563, 0x2551, 0x2557, 0x255d, 0x255c, 0x255b, 0x2510,
  0x2514, 0x2534, 0x252c, 0x251c, 0x2500, 0x253c, 0x255e, 0x255f,
  0x255a, 0x2554, 0x2569, 0x2566, 0x2560, 0x2550, 0x256c, 0x2567,
  0x2568, 0x2564, 0x2565, 0x2559, 0x2558, 0x2552, 0x2553, 0x256b,
  0x256a, 0x2518, 0x250c, 0x2588, 0x2584, 0x258c, 0x2590, 0x2580,
  0x03b1, 0x00df, 0x0393, 0x03c0, 0x03a3, 0x03c3, 0x00b5, 0x03c4,
  0x03a6, 0x0398, 0x03a9, 0x03b4, 0x221e, 0x03c6, 0x03b5, 0x2229,
  0x2261, 0x00b1, 0x2265, 0x2264, 0x2320, 0x2321, 0x00f7, 0x2248,
  0x00b0, 0x2219, 0x00b7, 0x221a, 0x207f, 0x00b2, 0x25a0, 0x00a0,
];

const decodeCp437 = (bytes) => {
  const chars = new Array(bytes.length);
  for (let index = 0; index < bytes.length; index += 1) {
    const byte = bytes[index];
    const codePoint = byte < 0x80 ? byte : CP437_HIGH_CODE_POINTS[byte - 0x80];
    chars[index] = String.fromCodePoint(codePoint);
  }
  return chars.join('');
};

const normalizeBytesForDecode = (bytes) => {
  if (bytes === null || bytes === undefined) {
    return null;
  }

  if (bytes instanceof ArrayBuffer) {
    return new Uint8Array(bytes);
  }

  if (ArrayBuffer.isView(bytes)) {
    return new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  }

  if (Array.isArray(bytes)) {
    return Uint8Array.from(bytes);
  }

  if (typeof bytes === 'string') {
    return bytes;
  }

  return bytes;
};

const toTerminal = (bytes) => {
  const terminalBytes = normalizeBytesForDecode(bytes);

  if (!terminalBytes || terminalBytes.length === 0) {
    return;
  }

  if (typeof terminalBytes === 'string') {
    terminal.write(terminalBytes);
    return;
  }

  if (!(terminalBytes instanceof Uint8Array)) {
    return;
  }

  terminal.write(decodeCp437(terminalBytes));
};

const writeToSocket = (data) => {
  if (socket.readyState !== WebSocket.OPEN) {
    return;
  }

  if (Array.isArray(data)) {
    socket.send(new Uint8Array(data));
    return;
  }

  if (data instanceof Uint8Array) {
    socket.send(data);
    return;
  }

  socket.send(encoder.encode(data));
};

const zmodemSentry = new Zmodem.Sentry({
  to_terminal: toTerminal,
  sender: writeToSocket,
  on_retract: () => {},
  on_detect: (detection) => {
    let session;

    try {
      session = detection.confirm();
    } catch {
      return;
    }

    session.on('session_end', () => {
      terminal.writeln('\r\nZMODEM session ended');
    });

    if (session.type === 'receive') {
      session.on('offer', (offer) => {
        const details = offer.get_details();
        const filename = details?.name || '<unknown>';
        const chunks = [];
        terminal.writeln(`\r\nAccepting ZMODEM receive: ${filename}`);
        offer.accept({
          on_input: (payload) => {
            chunks.push(Uint8Array.from(payload));
          },
        })
          .then((payloads) => {
            const receivedChunks = chunks.length > 0
              ? chunks
              : Array.isArray(payloads)
              ? payloads
              : [
                  payloads instanceof Uint8Array
                    ? payloads
                    : new TextEncoder().encode(String(payloads || '')),
                ];
            if (receivedChunks.length === 0) {
              return;
            }
            Zmodem.Browser.save_to_disk(receivedChunks, details.name);
          })
          .catch((error) => {
            terminal.writeln(`\r\nZMODEM receive failed: ${error?.message || error || 'unknown error'}`);
          });
      });

      session.start().catch((error) => {
        terminal.writeln(`\r\nZMODEM receive failed: ${error?.message || error || 'unknown error'}`);
      });
      return;
    }

    const fileInput = document.querySelector('#zmodem-upload');
    if (!fileInput) {
      session.deny?.();
      return;
    }

    const pickFiles = () => {
      return new Promise((resolve) => {
        const onChange = () => {
          const files = Array.from(fileInput.files || []);
          fileInput.removeEventListener('change', onChange);
          resolve(files);
          fileInput.value = '';
        };

        fileInput.addEventListener('change', onChange);
        fileInput.click();
      });
    };

    terminal.writeln('\r\nZMODEM upload requested, waiting for file picker...');
    pickFiles()
      .then((files) => {
        if (files.length === 0) {
          terminal.writeln('\r\nZMODEM upload cancelled');
          return;
        }
        return Zmodem.Browser.send_files(session, files);
      })
      .catch(() => {
        // Ignore upload errors; transfer can still continue from terminal output.
      });
  },
});

const socketBytesToArray = async (payload) => {
  if (payload instanceof Blob) {
    return new Uint8Array(await payload.arrayBuffer());
  }

  if (payload instanceof ArrayBuffer) {
    return new Uint8Array(payload);
  }

  return encoder.encode(String(payload));
};

socket.addEventListener('open', () => {
  terminal.writeln('');
});

socket.addEventListener('message', async (event) => {
  const bytes = await socketBytesToArray(event.data);
  try {
    zmodemSentry.consume(bytes);
  } catch (error) {
    terminal.writeln(`\r\nZMODEM protocol error: ${error?.message || error || 'unknown error'}`);
  }
});

socket.addEventListener('close', () => {
  terminal.writeln('\r\nDisconnected');
});

socket.addEventListener('error', () => {
  terminal.writeln('\r\nConnection error');
});

terminal.onData((data) => {
  if (socket.readyState !== WebSocket.OPEN) {
    return;
  }

  writeToSocket(encoder.encode(data));
});

const uploadInput = document.createElement('input');
uploadInput.type = 'file';
uploadInput.id = 'zmodem-upload';
uploadInput.multiple = true;
uploadInput.style.display = 'none';
document.body.appendChild(uploadInput);

window.addEventListener('keydown', (event) => {
  if (event.ctrlKey && event.shiftKey && event.key.toLowerCase() === 'u') {
    uploadInput.click();
  }
});

window.addEventListener('resize', resizeTerminalRows);

window.addEventListener('resize', () => {
  try {
    if (fitAddon) {
      fitAddon.fit();
    }
  } catch (_) {
    // no-op
  }
});
