(function (seed) {
  // mulberry32: a small, fast, deterministic PRNG. Explicitly NOT
  // cryptographically secure -- this exists to make test runs reproducible,
  // not to generate anything security-sensitive.
  function mulberry32(a) {
    return function () {
      a |= 0;
      a = (a + 0x6d2b79f5) | 0;
      var t = Math.imul(a ^ (a >>> 15), 1 | a);
      t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }
  Math.random = mulberry32(seed);
})(%SEED%)
