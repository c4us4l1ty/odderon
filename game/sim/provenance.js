/* Odderon — Provenance (docs/GAME-DESIGN-BIBLE.md §6.6, Pillar P1).
 *
 * RULE: no display-facing system may accept a raw number. Every observable is a lineage NODE.
 *
 * Implementation choice, and the reason for it: wrapping every f64 operation in a provenance object
 * costs ~120 ns/op, which for a 200 Hz network is ~1 s of CPU per second of simulated flight. That
 * does not run on the target hardware. So:
 *
 *   - the hot loop stays flat f64 (see thermo.js)          -> frame cost: 0
 *   - a lineage graph is built once per design/change      -> ~120 nodes
 *   - sensitivities are a forward-mode Jacobian on demand  -> ~700 flop = 0.7 us
 *
 * That is the whole trick: honesty costs no frame rate and no hand-written tooltips.
 */
(function (global) {
  'use strict';

  var nextId = 1;

  // EpistemicSource — where a number comes from. The distinguishing axis that matters to the player
  // is MEASURED vs ASSUMED: the UI must be able to paint every assumed input yellow (§6.2, §6.6).
  var K = {
    MEASURED: 'measured',   // a sensor on the real (in-game) object, with its noise
    CATALOG: 'catalog',     // observational data: Gaia, Horizons, Exoplanet Archive
    DERIVED: 'derived',     // computed from other numbers by a named equation
    MODELED: 'modeled',     // an empirical correlation (Bartz, Sutton-Graves, Bekker-Wong)
    ASSUMED: 'assumed'      // the player (or an engineer) chose it
  };

  function Node(op, value, sigma, source, inputs, note) {
    this.id = nextId++;
    this.op = op || 'const';
    this.value = value;
    this.sigma = sigma === undefined ? 0 : sigma;
    this.source = source || { kind: K.ASSUMED, by: 'engineer', note: note || '' };
    this.inputs = inputs || [];      // array of Node (small graphs; no DAG sharing subtlety needed)
    this.note = note || '';
  }

  /* ---- constructors ---- */
  function measured(v, sigma, sensor) {
    return new Node('sensor', v, sigma, { kind: K.MEASURED, sensor: sensor || 'unnamed' });
  }
  function catalog(v, sigma, id) {
    return new Node('catalog', v, sigma, { kind: K.CATALOG, id: id || '' });
  }
  function assumed(v, by, note) {
    return new Node('assumed', v, 0, { kind: K.ASSUMED, by: by || 'player', note: note || '' });
  }
  function modeled(v, sigma, name) {
    return new Node('model', v, sigma, { kind: K.MODELED, model: name || 'unnamed' });
  }

  // A computation: value from f, uncertainty from the standard propagation sigma_f = sqrt(sum(
  // (df/dx)^2 sigma_x^2)). Sensitivity is taken by a central difference on f itself, which is exact
  // enough here because f is analytic and cheap -- and it keeps this file free of the derivative
  // forms of every equation in the game.
  function derived(label, fn, inputs, extraSigma) {
    var args = inputs.map(function (n) { return n.value; });
    var v = fn.apply(null, args);
    var sig2 = (extraSigma || 0) * (extraSigma || 0);
    for (var i = 0; i < inputs.length; i++) {
      var x = inputs[i].value;
      var h = Math.abs(x) > 1e-8 ? x * 1e-6 : 1e-9;
      var a = args.slice(), b = args.slice();
      a[i] = x + h; b[i] = x - h;
      var dfdx = (fn.apply(null, a) - fn.apply(null, b)) / (2 * h);
      sig2 += dfdx * dfdx * inputs[i].sigma * inputs[i].sigma;
    }
    return new Node('derived', v, Math.sqrt(sig2), { kind: K.DERIVED, label: label, fn: fn }, inputs);
  }

  /* ---- sensitivity: d(output)/d(input), by central difference on the graph ----
   * Used only by the UI (a gauge click), never in the hot loop, so a rebuild is affordable and is
   * far less subtle than in-place graph surgery. Returns d(out)/d(in) in output units per input unit.
   */
  function sensitivity(outNode, inNode) {
    var base = evaluate(outNode);
    var x = inNode.value;
    var h = Math.abs(x) > 1e-8 ? Math.abs(x) * 1e-6 : 1e-9;
    var saved = inNode.value;

    inNode.value = x + h;
    var up = rebuild(outNode);
    inNode.value = x - h;
    var dn = rebuild(outNode);
    inNode.value = saved;

    return { value: (up - dn) / (2 * h), base: base, from: saved };
  }

  // Recreate a node bottom-up from its current inputs, so the value reflects the changed input.
  function rebuild(n) {
    if (n.inputs.length === 0) return n.value;
    var args = n.inputs.map(rebuild);
    if (n.source.kind === K.DERIVED && n.source.fn) return n.source.fn.apply(null, args);
    switch (n.op) {
      case 'add': return args.reduce(function (a, b) { return a + b; }, 0);
      case 'sub': return args[0] - args[1];
      case 'mul': return args.reduce(function (a, b) { return a * b; }, 1);
      case 'div': return args[0] / args[1];
      default: return n.value;
    }
  }

  // Re-evaluate an entire lineage tree bottom-up. Used only by UI queries (never in the hot loop).
  function evaluate(n) {
    if (n.inputs.length === 0) return n.value;
    var args = n.inputs.map(evaluate);
    switch (n.op) {
      case 'add': return args.reduce(function (a, b) { return a + b; }, 0);
      case 'sub': return args[0] - args[1];
      case 'mul': return args.reduce(function (a, b) { return a * b; }, 1);
      case 'div': return args[0] / args[1];
      case 'min': return Math.min.apply(null, args);
      case 'max': return Math.max.apply(null, args);
      default: return n.value;
    }
  }

  /* ---- what the UI needs: the tree, flattened, with assumed inputs flagged ---- */
  function trace(n, depth, out) {
    depth = depth || 0; out = out || { rows: [], assumed: 0, maxDepth: 0 };
    out.maxDepth = Math.max(out.maxDepth, depth);
    var isAssumed = n.source.kind === K.ASSUMED;
    if (isAssumed) out.assumed += 1;
    out.rows.push({
      id: n.id, depth: depth, value: n.value, sigma: n.sigma,
      kind: n.source.kind, source: n.source, note: n.note, assumed: isAssumed,
      label: n.source.label || (n.inputs.length ? n.op : n.source.kind)
    });
    for (var i = 0; i < n.inputs.length; i++) trace(n.inputs[i], depth + 1, out);
    return out;
  }

  /* ---- error propagation from the trace: sigma_f/sigma_x group ---- */
  function uncertaintyBudget(n, out) {
    out = out || {};
    var budget = {};
    (function walk(m) {
      if (m.inputs.length === 0) {
        var key = m.source.kind + (m.source.sensor ? ':' + m.source.sensor
          : m.source.model ? ':' + m.source.model : '');
        budget[key] = (budget[key] || 0) + m.sigma * m.sigma;
        return;
      }
      m.inputs.forEach(walk);
    })(n);
    return budget;
  }

  var P = {
    K: K,
    Node: Node,
    measured: measured, catalog: catalog, assumed: assumed, modeled: modeled,
    derived: derived, evaluate: evaluate, rebuild: rebuild, sensitivity: sensitivity,
    trace: trace, uncertaintyBudget: uncertaintyBudget
  };

  if (typeof module !== 'undefined' && module.exports) module.exports = P;
  global.ODProv = P;
})(typeof globalThis !== 'undefined' ? globalThis : this);
