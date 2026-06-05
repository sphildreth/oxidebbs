const terminalElement = document.getElementById('terminal');
const terminal = new Terminal({
  convertEol: false,
  cursorBlink: true,
  fontFamily: 'monospace',
  fontSize: 16,
});

let fitAddon;
if (window.FitAddon && typeof window.FitAddon.FitAddon === 'function') {
  fitAddon = new window.FitAddon.FitAddon();
  terminal.loadAddon(fitAddon);
}
terminal.open(terminalElement);
terminal.focus();
if (fitAddon) {
  fitAddon.fit();
}

const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
const socketUrl = `${protocol}//${window.location.host}/terminal/ws`;
const socket = new WebSocket(socketUrl);
socket.binaryType = 'arraybuffer';

const decoder = new TextDecoder('latin1');
const encoder = new TextEncoder();

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
    return new TextEncoder().encode(bytes);
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

  terminal.write(decoder.decode(terminalBytes));
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
      terminal.writeln('\\r\\nZMODEM session ended');
    });

    if (session.type === 'receive') {
      session.on('offer', (offer) => {
        const details = offer.get_details();
        const filename = details?.name || '<unknown>';
        terminal.writeln(`\\r\\nAccepting ZMODEM receive: ${filename}`);
        offer.on('input', () => {});
        offer.accept({ on_input: 'spool_array' })
          .then((payloads) => {
            const chunks = Array.isArray(payloads)
              ? payloads
              : [
                  payloads instanceof Uint8Array
                    ? payloads
                    : new TextEncoder().encode(String(payloads || '')),
                ];
            if (chunks.length === 0) {
              return;
            }
            Zmodem.Browser.save_to_disk(chunks, details.name);
          })
          .catch(() => {
            // If the transfer fails, keep terminal readable.
          });
      });

      session.start().catch(() => {
        terminal.writeln('\\r\\nZMODEM receive failed');
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

    terminal.writeln('\\r\\nZMODEM upload requested, waiting for file picker...');
    pickFiles()
      .then((files) => {
        if (files.length === 0) {
          terminal.writeln('\\r\\nZMODEM upload cancelled');
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
  zmodemSentry.consume(bytes);
});

socket.addEventListener('close', () => {
  terminal.writeln('\\r\\nDisconnected');
});

socket.addEventListener('error', () => {
  terminal.writeln('\\r\\nConnection error');
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

window.addEventListener('resize', () => {
  try {
    if (fitAddon) {
      fitAddon.fit();
    }
  } catch (_) {
    // no-op
  }
});
