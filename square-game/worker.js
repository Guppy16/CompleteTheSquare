// Runs the engine off the main thread so the page never freezes while the
// AI thinks. It keeps its own copy of the module: the page sends the moves
// played so far, this replays them and answers with the square the engine
// would play. The page then plays that square on its own copy.

let ai = null;
const ready = (async () => {
  const bytes = await (await fetch('ai.wasm')).arrayBuffer();   // next to this script
  const { instance } = await WebAssembly.instantiate(bytes, {});
  ai = instance.exports;
})();

onmessage = async (event) => {
  const { id, kind, moves, boardSize, maxDepth, nodeBudget } = event.data;
  await ready;
  ai.reset();
  for (const index of moves) ai.play(Math.floor(index / boardSize), index % boardSize);
  const started = performance.now();
  if (kind === 'analyse') {
    // Every legal move scored for the side to move, best first.
    const count = ai.analyse(maxDepth, nodeBudget);
    const lines = [];
    for (let i = 0; i < count; i++) lines.push({ index: ai.analysis_move(i), score: ai.analysis_score(i) });
    postMessage({ id, lines, depth: ai.analysis_depth(), ms: Math.round(performance.now() - started) });
    return;
  }
  const index = ai.ai_suggest(maxDepth, nodeBudget);
  postMessage({ id, index, ms: Math.round(performance.now() - started) });
};
