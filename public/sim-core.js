/*
 * Crown Survivors core loader (raw WASM ABI, no wasm-bindgen).
 *
 * Used by the browser product and by `sim-worker.js`. Each execution context
 * (main thread or worker) must own its own `Core` instance: the module keeps a
 * shared input/output buffer per instance, so it is not safe to interleave
 * calls across instances.
 *
 *   const core = await CrownSimCore.load();
 *   const handle = core.create({ seed: 7, auto: true });
 *   const state = core.step(handle, 600, { auto: true });
 *   core.destroy(handle);
 *
 * All methods exchange JSON. Errors from the core are thrown as `Error` with a
 * `payload` property holding the raw `{ error }` object.
 */
(function (root) {
  'use strict';

  var DEFAULT_URL = 'wasm/crown_sim_core.wasm';
  var encoder = new TextEncoder();
  var decoder = new TextDecoder();

  function readOut(instance, len) {
    var ptr = instance.exports.sim_out_ptr();
    var mem = new Uint8Array(instance.exports.memory.buffer, ptr, len);
    // `TextDecoder` copies; do it before any further core call can reallocate.
    return decoder.decode(mem);
  }

  function writeIn(instance, value) {
    var text = JSON.stringify(value === undefined ? null : value);
    var bytes = encoder.encode(text);
    var ptr = instance.exports.sim_input_reserve(bytes.length);
    new Uint8Array(instance.exports.memory.buffer, ptr, bytes.length).set(bytes);
  }

  function Core(instance) {
    this.instance = instance;
    this.version = 0;
    this.closed = false;
  }

  Core.prototype._decodeOut = function (len) {
    var text = readOut(this.instance, len);
    var value;
    try {
      value = JSON.parse(text);
    } catch (e) {
      throw new Error('sim-core: invalid JSON: ' + text.slice(0, 200));
    }
    if (value && typeof value === 'object' && value.error) {
      var err = new Error('sim-core: ' + value.error);
      err.payload = value;
      throw err;
    }
    return value;
  };

  Core.prototype._call = function (name, payload, args) {
    if (this.closed) throw new Error('sim-core: instance is closed');
    var exports = this.instance.exports;
    if (typeof exports[name] !== 'function') {
      throw new Error('sim-core: missing export ' + name);
    }
    if (payload !== undefined) writeIn(this.instance, payload);
    var len = args && args.length ? exports[name].apply(null, args) : exports[name]();
    return this._decodeOut(len);
  };

  /** Static content/balance data (superset of the legacy `meta` message). */
  Core.prototype.meta = function () {
    return this._call('sim_meta');
  };

  /** Create a run. Returns the integer handle. */
  Core.prototype.create = function (config) {
    if (this.closed) throw new Error('sim-core: instance is closed');
    writeIn(this.instance, config || {});
    // `sim_create` is the one export that returns the handle instead of the
    // output length; read the JSON payload through the shared buffer.
    var handle = this.instance.exports.sim_create() >>> 0;
    var out = this._decodeOut(this.instance.exports.sim_out_len());
    if (!handle || (out && out.error)) {
      var err = new Error('sim-core: ' + ((out && out.error) || 'sim_create failed'));
      err.payload = out;
      throw err;
    }
    this.version = out.version || 0;
    return handle;
  };

  Core.prototype.destroy = function (handle) {
    return this._call('sim_destroy', undefined, [handle]);
  };

  /** Patch live config: `time_limit`, `auto`, `god`, `curse`, `profile`, ... */
  Core.prototype.configure = function (handle, patch) {
    return this._call('sim_configure', patch || {}, [handle]);
  };

  /** Advance `steps` fixed 1/60s steps. Returns the compact status object. */
  Core.prototype.step = function (handle, steps, input) {
    return this._call('sim_step', input || {}, [handle, steps >>> 0]);
  };

  /** Run to death / win / time limit. `opts.max_steps` caps the loop. */
  Core.prototype.run = function (handle, opts) {
    return this._call('sim_run', opts || {}, [handle]);
  };

  /** State snapshot. `detail=true` includes entities for rendering. */
  Core.prototype.snapshot = function (handle, detail) {
    return this._call('sim_snapshot', undefined, [handle, detail ? 1 : 0]);
  };

  Core.prototype.pick = function (handle, index) {
    return this._call('sim_pick', undefined, [handle, index >>> 0]);
  };

  /** Self-contained batch (`{configs:[...]}`) inside one instance. */
  Core.prototype.batch = function (configs) {
    return this._call('sim_batch', { configs: configs || [] }, []);
  };

  Core.prototype.close = function () {
    this.closed = true;
    this.instance = null;
  };

  /**
   * Fetch the raw core bytes without compiling them. Callers that fan out to
   * many workers can fetch once and hand the same bytes to every worker.
   *
   * @param {string} [url]
   * @returns {Promise<ArrayBuffer>}
   */
  async function fetchBytes(url) {
    url = url || DEFAULT_URL;
    var res = await fetch(url, { cache: 'no-cache' });
    if (!res.ok) {
      throw new Error('sim-core: failed to fetch ' + url + ' (' + res.status + ')');
    }
    return res.arrayBuffer();
  }

  /**
   * Compile + instantiate the core.
   *
   * `module` (a `WebAssembly.Module`) skips both fetch and compile; that is what
   * `sim-pool.js` uses so a pool of N workers pays for one compile, not N.
   *
   * @param {{url?:string, buffer?:ArrayBuffer|Uint8Array, module?:WebAssembly.Module}} [options]
   */
  async function load(options) {
    options = options || {};
    var module = options.module;
    if (!module) {
      var bytes = options.buffer;
      if (!bytes) bytes = await fetchBytes(options.url);
      module = await WebAssembly.compile(bytes);
    }
    var instance = await WebAssembly.instantiate(module, {});
    var core = new Core(instance);
    var meta = core.meta();
    core.version = meta.version || 0;
    core.engine = meta.engine || 'crown-sim-core';
    return core;
  }

  var api = {
    load: load,
    fetchBytes: fetchBytes,
    Core: Core,
    DEFAULT_URL: DEFAULT_URL
  };
  root.CrownSimCore = api;
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
