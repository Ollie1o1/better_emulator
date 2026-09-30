// NES player for the browser: drives nes_web.wasm (the Rust emulator core
// compiled to WebAssembly) with a canvas, keyboard/touch input and audio.
//
//   import { createPlayer } from './nes-player.js';
//   const player = await createPlayer({ canvas, wasmUrl: 'nes_web.wasm', onError });
//   await player.loadRomUrl('roms/thwaite.nes');
//   player.start();          // must follow a user gesture (autoplay rules)

const W = 256, H = 240;
const NES_FPS = 60.0988;
const FRAME_MS = 1000 / NES_FPS;
// Largest board the supported mappers address is 512 KB PRG + 256 KB CHR;
// refusing anything far beyond that keeps a stray huge file from exhausting
// wasm memory.
const MAX_ROM_BYTES = 4 * 1024 * 1024;

export const BUTTONS = { A: 1, B: 2, SELECT: 4, START: 8, UP: 16, DOWN: 32, LEFT: 64, RIGHT: 128 };

// KeyboardEvent.code → NES button
const KEYMAP = {
  ArrowUp: 'UP', ArrowDown: 'DOWN', ArrowLeft: 'LEFT', ArrowRight: 'RIGHT',
  KeyW: 'UP', KeyS: 'DOWN', KeyA: 'LEFT', KeyD: 'RIGHT',
  KeyZ: 'A', KeyK: 'A', KeyX: 'B', KeyJ: 'B',
  Enter: 'START', Space: 'START', ShiftLeft: 'SELECT', ShiftRight: 'SELECT',
};

// AudioWorklet: a FIFO of sample chunks posted from the main thread.
const WORKLET_SRC = `
class NesAudio extends AudioWorkletProcessor {
  constructor() {
    super();
    this.queue = []; this.offset = 0; this.buffered = 0;
    this.port.onmessage = (e) => {
      if (e.data === 'flush') { this.queue = []; this.offset = 0; this.buffered = 0; return; }
      this.queue.push(e.data); this.buffered += e.data.length;
      // Cap latency at ~100 ms: drop the oldest audio if we fall behind
      while (this.buffered > sampleRate / 10 && this.queue.length > 1) {
        const old = this.queue.shift(); this.buffered -= old.length - this.offset; this.offset = 0;
      }
    };
  }
  process(_, outputs) {
    const out = outputs[0][0];
    for (let i = 0; i < out.length; i++) {
      const cur = this.queue[0];
      if (!cur) { out[i] = 0; continue; }
      out[i] = cur[this.offset++]; this.buffered--;
      if (this.offset >= cur.length) { this.queue.shift(); this.offset = 0; }
    }
    return true;
  }
}
registerProcessor('nes-audio', NesAudio);
`;

export async function createPlayer({ canvas, wasmUrl, volume = 0.6, onError = console.error }) {
  const { instance } = await WebAssembly.instantiateStreaming(fetch(wasmUrl), {});
  const wasm = instance.exports;

  const ctx2d = canvas.getContext('2d');
  canvas.width = W;
  canvas.height = H;
  const image = ctx2d.createImageData(W, H);

  let audioCtx = null, audioNode = null, gain = null;
  let running = false, rafId = 0, last = 0, acc = 0;
  let pad1 = 0, touchPad = 0;
  // Buttons pressed since the last frame. A tap shorter than one frame still
  // reaches the game, because it's held for at least the next frame.
  let tapped = 0;
  let romLoaded = false;
  let bootRate = 0;   // audio sample rate the current ROM was booted with
  let crashed = false; // a wasm trap leaves the instance unusable
  let loadSeq = 0;     // only the most recent ROM request may boot

  async function initAudio() {
    if (audioCtx) return;
    audioCtx = new AudioContext();
    const url = URL.createObjectURL(new Blob([WORKLET_SRC], { type: 'text/javascript' }));
    await audioCtx.audioWorklet.addModule(url);
    URL.revokeObjectURL(url);
    audioNode = new AudioWorkletNode(audioCtx, 'nes-audio', { outputChannelCount: [1] });
    gain = audioCtx.createGain();
    gain.gain.value = volume;
    audioNode.connect(gain).connect(audioCtx.destination);
  }

  function boot(bytes) {
    if (bytes.length > MAX_ROM_BYTES) throw new Error('File is too large to be an NES ROM');
    const ptr = wasm.rom_buffer(bytes.length);
    new Uint8Array(wasm.memory.buffer, ptr, bytes.length).set(bytes);
    const rate = audioCtx ? audioCtx.sampleRate : 44100;
    bootRate = rate;
    // On failure the previous game is left running
    if (!wasm.load_rom(rate)) throw new Error('Not a supported NES ROM (iNES file, mappers 0, 1, 2, 3, 4 or 7)');
    romLoaded = true;
    romBytes = bytes;
    audioNode?.port.postMessage('flush');
    acc = 0;
  }

  let romBytes = null;

  function frame() {
    wasm.set_buttons(pad1 | touchPad | tapped, 0);
    tapped = 0;
    wasm.run_frame();

    const n = wasm.audio_len();
    if (audioNode && n > 0) {
      // Copy out: wasm memory can move if it grows
      audioNode.port.postMessage(new Float32Array(wasm.memory.buffer, wasm.audio_ptr(), n).slice());
    }
  }

  // Run ~1.5 s silently so a paused player shows the game's title screen
  // rather than its blank first frame.
  function preview(frames = 90) {
    for (let i = 0; i < frames; i++) wasm.run_frame();
    draw();
  }

  function draw() {
    image.data.set(new Uint8Array(wasm.memory.buffer, wasm.frame_ptr(), W * H * 4));
    ctx2d.putImageData(image, 0, 0);
  }

  function stop() {
    running = false;
    cancelAnimationFrame(rafId);
    audioCtx?.suspend();
  }

  function loop(now) {
    rafId = requestAnimationFrame(loop);
    const dt = now - last;
    last = now;
    // Tab was hidden / long stall: resync instead of fast-forwarding
    acc = dt > 250 ? FRAME_MS : acc + dt;
    let ran = 0;
    try {
      // Paced to the NES's 60.1 Hz regardless of display refresh (120/144 Hz)
      while (acc >= FRAME_MS && ran < 3) {
        frame();
        acc -= FRAME_MS;
        ran++;
      }
      if (ran) draw();
    } catch (e) {
      crashed = true;
      stop();
      onError(new Error(`The emulator stopped unexpectedly; reload the page to play again. (${e.message})`));
    }
  }

  // Keyboard is captured only while the player element has focus, so the
  // arrow keys and space still scroll the page otherwise.
  const focusTarget = canvas.closest('[data-nes-player]') || canvas;
  focusTarget.addEventListener('keydown', (e) => {
    const b = KEYMAP[e.code];
    if (!b) return;
    e.preventDefault();
    pad1 |= BUTTONS[b];
    tapped |= BUTTONS[b];
  });
  focusTarget.addEventListener('keyup', (e) => {
    const b = KEYMAP[e.code];
    if (!b) return;
    e.preventDefault();
    pad1 &= ~BUTTONS[b];
  });
  focusTarget.addEventListener('blur', () => { pad1 = 0; });

  return {
    /** Fetch and boot a ROM by URL. */
    async loadRomUrl(url) {
      const seq = ++loadSeq;
      const res = await fetch(url);
      if (!res.ok) throw new Error(`Could not load ${url} (${res.status})`);
      const bytes = new Uint8Array(await res.arrayBuffer());
      if (seq !== loadSeq) return; // superseded by a newer load
      boot(bytes);
      if (!running) preview();
    },
    /** Boot a ROM from a File (file picker / drag and drop). */
    async loadRomFile(file) {
      const seq = ++loadSeq;
      if (file.size > MAX_ROM_BYTES) throw new Error('File is too large to be an NES ROM');
      const bytes = new Uint8Array(await file.arrayBuffer());
      if (seq !== loadSeq) return;
      boot(bytes);
    },
    /** Start emulation. Call from a click/keypress so audio may start. */
    async start() {
      if (crashed) throw new Error('The emulator stopped unexpectedly; reload the page to play again.');
      await initAudio();
      await audioCtx.resume();
      // First start: the ROM was booted before audio existed; re-boot at the
      // device's real sample rate so pitch is right (resume doesn't re-boot).
      if (romBytes && bootRate !== audioCtx.sampleRate) boot(romBytes);
      if (running || !romLoaded) return;
      running = true;
      last = performance.now();
      rafId = requestAnimationFrame(loop);
      focusTarget.focus({ preventScroll: true });
    },
    pause: stop,
    reset() { if (!crashed) wasm.reset(); },
    setVolume(v) { if (gain) gain.gain.value = v; volume = v; },
    /** On-screen controls: press/release a named button. */
    press(name, down) {
      if (down) { touchPad |= BUTTONS[name]; tapped |= BUTTONS[name]; }
      else touchPad &= ~BUTTONS[name];
    },
    get running() { return running; },
  };
}
