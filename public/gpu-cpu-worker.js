/*
 * CPU side of the GPU lab hybrid scheduler.
 *
 * Runs the same deterministic horde kernel as `gpu-sim.js` for a slice of
 * independent runs. One worker per CPU core is the "max CPU" half of Engine D.
 */
'use strict';

importScripts('gpu-sim.js');

self.onmessage = function (event) {
  var msg = event.data || {};
  try {
    var state = CrownGpuSim.createState(msg.cfg);
    CrownGpuSim.initState(state);
    var t0 = performance.now();
    CrownGpuSim.runCpu(state, msg.steps || 1);
    var ms = performance.now() - t0;
    self.postMessage({
      result: {
        metrics: CrownGpuSim.metrics(state),
        hash: CrownGpuSim.hashState(state),
        ms: ms,
        runs: state.cfg.runs
      }
    });
  } catch (e) {
    self.postMessage({ error: String(e && e.stack || e) });
  }
};
