(function (startTimeMs) {
  var virtualNow = startTimeMs;
  var OriginalDate = Date;

  function FakeDate() {
    if (!(this instanceof FakeDate)) {
      // Called as a function (not `new Date()`) -- real Date returns a
      // string in this case; delegate to the real constructor's own
      // function-call behavior.
      return OriginalDate();
    }
    if (arguments.length === 0) {
      return new OriginalDate(virtualNow);
    }
    var args = Array.prototype.slice.call(arguments);
    return new (Function.prototype.bind.apply(OriginalDate, [null].concat(args)))();
  }
  FakeDate.prototype = OriginalDate.prototype;
  FakeDate.now = function () {
    return virtualNow;
  };
  FakeDate.parse = OriginalDate.parse;
  FakeDate.UTC = OriginalDate.UTC;
  window.Date = FakeDate;

  var perfOrigin = virtualNow;
  performance.now = function () {
    return virtualNow - perfOrigin;
  };

  // Timers/animation frames are queued against virtual time and only fire
  // when explicitly advanced -- see virtual-clock spec: "Time does not
  // pass on its own".
  var nextId = 1;
  var timers = new Map(); // id -> {callback, args, targetTime, intervalMs (or null)}

  window.setTimeout = function (callback, delay) {
    var id = nextId++;
    var args = Array.prototype.slice.call(arguments, 2);
    timers.set(id, {
      callback: callback,
      args: args,
      targetTime: virtualNow + (delay || 0),
      intervalMs: null,
    });
    return id;
  };
  window.clearTimeout = function (id) {
    timers.delete(id);
  };

  window.setInterval = function (callback, delay) {
    var id = nextId++;
    var args = Array.prototype.slice.call(arguments, 2);
    var intervalMs = delay || 0;
    timers.set(id, {
      callback: callback,
      args: args,
      targetTime: virtualNow + intervalMs,
      intervalMs: intervalMs,
    });
    return id;
  };
  window.clearInterval = function (id) {
    timers.delete(id);
  };

  // Modeled as a fixed ~60fps timer, not tied to real paint -- there's no
  // real paint to tie it to in a virtual-time world (design.md Decision #5).
  var FRAME_MS = 1000 / 60;
  window.requestAnimationFrame = function (callback) {
    var id = nextId++;
    timers.set(id, {
      callback: callback,
      args: [virtualNow],
      targetTime: virtualNow + FRAME_MS,
      intervalMs: null,
    });
    return id;
  };
  window.cancelAnimationFrame = function (id) {
    timers.delete(id);
  };

  // Repeatedly finds and fires the single earliest still-due timer, so a
  // callback that schedules another callback still due within the same
  // advance is picked up too (design.md Decision #3).
  window.__aibAdvanceClock = function (ms) {
    var target = virtualNow + ms;
    for (;;) {
      var earliestId = null;
      var earliestTime = Infinity;
      timers.forEach(function (t, id) {
        if (t.targetTime <= target && t.targetTime < earliestTime) {
          earliestTime = t.targetTime;
          earliestId = id;
        }
      });
      if (earliestId === null) {
        break;
      }
      var timer = timers.get(earliestId);
      virtualNow = timer.targetTime;
      if (timer.intervalMs !== null) {
        timer.targetTime = virtualNow + timer.intervalMs;
      } else {
        timers.delete(earliestId);
      }
      timer.callback.apply(null, timer.args);
    }
    virtualNow = target;
  };
})(%START_TIME_MS%)
