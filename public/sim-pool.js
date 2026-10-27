/*
 * Worker pool for the headless WASM core.
 *
 * One worker per simulated run. The pool is deliberately thin: it owns the
 * worker lifecycle, forwards messages and lets the host address slots by index.
 *
 *   const pool = new CrownSimPool(8);
 *   pool.onMessage((slot, msg) => { ... });
 *   await pool.init();
 *   pool.send(0, { type: 'reset', config: { seed: 7, auto: true }, start: true });
 *   ...
 *   pool.terminate();
 */
(function (root) {
  'use strict';

  function CrownSimPool(size, options) {
    options = options || {};
    this.size = Math.max(0, size | 0);
    this.options = options;
    this.workers = [];
    this.ready = false;
    this.meta = null;
    this.module = null;
    this._prepared = null;
    this._listeners = [];
    this._closed = false;
  }

  CrownSimPool.prototype.onMessage = function (fn) {
    this._listeners.push(fn);
    return this;
  };

  CrownSimPool.prototype._emit = function (slot, msg) {
    for (var i = 0; i < this._listeners.length; i++) {
      try { this._listeners[i](slot, msg); } catch (e) { /* host error */ }
    }
  };

  /**
   * Load + compile the core once for the whole pool. Workers then instantiate
   * the same `WebAssembly.Module` (structured-clone supported by every engine
   * that supports workers). Falls back to letting each worker fetch the bytes
   * if the environment cannot compile or clone a module.
   */
  CrownSimPool.prototype._prepare = function () {
    if (this._prepared) return this._prepared;
    var self = this;
    if (self.options.module) {
      this._prepared = Promise.resolve({ module: self.options.module });
      return this._prepared;
    }
    if (typeof WebAssembly === 'undefined' || typeof WebAssembly.compile !== 'function') {
      this._prepared = Promise.resolve({ url: self.options.wasmUrl });
      return this._prepared;
    }
    var loadMeta = root.CrownSimCore || {};
    var url = self.options.wasmUrl ||
      (typeof loadMeta.DEFAULT_URL === 'string' ? loadMeta.DEFAULT_URL : 'wasm/crown_sim_core.wasm');
    var fetchBytes = loadMeta.fetchBytes || function (u) {
      return fetch(u, { cache: 'no-cache' }).then(function (res) {
        if (!res.ok) throw new Error('failed to fetch ' + u + ' (' + res.status + ')');
        return res.arrayBuffer();
      });
    };
    this._prepared = fetchBytes(url)
      .then(function (bytes) {
        return WebAssembly.compile(bytes);
      })
      .then(function (module) {
        self.module = module;
        return { module: module };
      })
      .catch(function (err) {
        // Fetch/compile failed on the main thread; let workers try on their own
        // and surface a real error there instead of failing the whole pool.
        if (typeof console !== 'undefined' && console.warn) {
          console.warn('sim-pool: shared module prepare failed, falling back to per-worker load:', err);
        }
        return { url: url };
      });
    return this._prepared;
  };

  CrownSimPool.prototype.init = function () {
    var self = this;
    var workerUrl = this.options.workerUrl || 'sim-worker.js';
    var config = this.options.config || null;

    return this._prepare().then(function (shared) {
      var jobs = [];

      for (var i = 0; i < self.size; i++) {
        jobs.push(new Promise(function (resolve, reject) {
          var slot = self.workers.length;
          var worker = new Worker(workerUrl);
          self.workers.push(worker);
          var settled = false;
          var timeout = setTimeout(function () {
            if (settled) return;
            settled = true;
            reject(new Error('headless worker ' + slot + ' failed to start'));
          }, self.options.timeout || 20000);

          worker.onmessage = function (event) {
            var msg = event.data;
            if (!msg || !msg.type) return;
            if (msg.type === 'meta' && !self.meta) self.meta = msg.data;
            if (msg.type === 'ready' && !settled) {
              settled = true;
              clearTimeout(timeout);
              resolve();
            }
            self._emit(slot, msg);
          };
          worker.onerror = function (event) {
            var err = new Error('headless worker ' + slot + ': ' + (event.message || 'runtime error'));
            self._emit(slot, { type: 'error', error: err.message });
            if (!settled) {
              settled = true;
              clearTimeout(timeout);
              reject(err);
            }
          };

          var initMsg = { type: 'init', config: config, url: self.options.wasmUrl };
          if (shared.module) initMsg.module = shared.module;
          else if (shared.url) initMsg.url = shared.url;
          try {
            worker.postMessage(initMsg);
          } catch (e) {
            // `WebAssembly.Module` is not cloneable in this engine; the worker
            // will fetch and compile the core itself.
            worker.postMessage({ type: 'init', config: config, url: initMsg.url });
          }
        }));
      }

      return Promise.all(jobs).then(function () {
        self.ready = true;
        return self;
      });
    });
  };

  CrownSimPool.prototype.send = function (slot, msg) {
    var worker = this.workers[slot];
    if (worker) worker.postMessage(msg);
  };

  CrownSimPool.prototype.broadcast = function (msg) {
    for (var i = 0; i < this.workers.length; i++) {
      this.workers[i].postMessage(msg);
    }
  };

  CrownSimPool.prototype.terminate = function () {
    this._closed = true;
    for (var i = 0; i < this.workers.length; i++) {
      try { this.workers[i].terminate(); } catch (e) {}
    }
    this.workers = [];
    this.ready = false;
  };

  root.CrownSimPool = CrownSimPool;
})(typeof globalThis !== 'undefined' ? globalThis : this);
